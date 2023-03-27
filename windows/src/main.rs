// Disable the console window that pops up when you launch the .exe
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod config;
mod settings_window;
mod surface;
mod wallpaper;
mod winit_compat;

use cli::Mode;
use config::Config;
use flux::Flux;
use winit_compat::{HasMonitors, HasWinitWindow, MonitorHandle};

use std::collections::HashMap;
use std::ffi::CString;
use std::num::NonZeroU32;
use std::{fs, path, process, rc::Rc};

use glow as GL;
use glow::HasContext;

use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

use sdl2::video::Window;
use winit::dpi::PhysicalSize;

use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version};
use glutin::display::{Display, DisplayApiPreference, GetGlDisplay};
use glutin::prelude::*;
use glutin::surface::{Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};

const MINIMUM_MOUSE_MOTION_TO_EXIT_SCREENSAVER: f64 = 10.0;

// In milliseconds
const FADE_TO_BLACK_DURATION: f64 = 300.0;

type WindowId = u32;

struct Instance {
    flux: Flux,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    gl: Rc<glow::Context>,
    window: Window,
}

impl Instance {
    pub fn draw(&mut self, timestamp: f64) {
        self.gl_context
            .make_current(&self.gl_surface)
            .expect("make OpenGL context current");

        self.flux.animate(timestamp);

        self.gl_surface
            .swap_buffers(&self.gl_context)
            .expect("swap OpenGL buffers");
    }

    pub fn fade_to_black(&mut self, timestamp: f64) {
        self.gl_context
            .make_current(&self.gl_surface)
            .expect("make OpenGL context current");

        let progress = (timestamp / FADE_TO_BLACK_DURATION).clamp(0.0, 1.0) as f32;
        unsafe {
            self.gl.clear_color(0.0, 0.0, 0.0, progress);
            self.gl.clear(GL::COLOR_BUFFER_BIT);
        }

        self.gl_surface
            .swap_buffers(&self.gl_context)
            .expect("swap OpenGL buffers");
    }
}

fn main() {
    let project_dirs = directories::ProjectDirs::from("me", "sandydoo", "Flux");
    let log_dir = project_dirs.as_ref().map(|dirs| dirs.data_local_dir());
    let config_dir = project_dirs.as_ref().map(|dirs| dirs.preference_dir());

    init_logging(log_dir);

    let config = Config::load(config_dir);

    match cli::read_flags().and_then(|mode| {
        if mode == Mode::Settings {
            settings_window::run(config)
                .map_err(|err| log::error!("{}", err))
                .unwrap();
            return Ok(());
        }

        run_flux(mode, config)
    }) {
        Ok(_) => process::exit(0),
        Err(err) => {
            log::error!("{}", err);
            process::exit(1)
        }
    };
}

fn init_logging(optional_log_dir: Option<&path::Path>) {
    use simplelog::*;

    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )];

    if let Some(log_dir) = optional_log_dir {
        let maybe_log_file = {
            fs::create_dir_all(log_dir).unwrap();
            let log_path = log_dir.join("flux_screensaver.log");
            fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(log_path)
        };

        if let Ok(log_file) = maybe_log_file {
            loggers.push(WriteLogger::new(
                LevelFilter::Warn,
                Config::default(),
                log_file,
            ));
        }
    }

    let _ = CombinedLogger::init(loggers);
    log_panics::init();
}

fn run_flux(mode: Mode, config: Config) -> Result<(), String> {
    #[cfg(windows)]
    set_dpi_awareness()?;

    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    // SDL, by default, disables the screensaver and doesn’t allow the display
    // to sleep. We want both of these things to happen in both screensaver and
    // preview modes.
    video_subsystem.enable_screen_saver();

    match mode {
        Mode::Preview(raw_window_handle) => {
            #[cfg(not(windows))]
            panic!("Preview window unsupported");

            let mut instance = new_preview_window(&video_subsystem, raw_window_handle, &config)?;
            let start = std::time::Instant::now();
            let mut event_pump = sdl_context.event_pump()?;

            run_preview_loop(&mut event_pump, &mut instance, start)
        }

        Mode::Screensaver => {
            let monitors = video_subsystem
                .available_monitors()
                .map(|monitor| (monitor.clone(), wallpaper::get(&monitor).ok()))
                .collect::<Vec<(MonitorHandle, Option<std::path::PathBuf>)>>();
            log::debug!("Available monitors: {:?}", monitors);

            let surfaces = surface::combine_monitors(&monitors);
            log::debug!("Creating windows: {:?}", surfaces);

            let mut instances = surfaces
                .iter()
                .map(|surface| {
                    new_instance(&video_subsystem, &config, surface)
                        .map(|instance| (instance.window.id(), instance))
                })
                .collect::<Result<HashMap<WindowId, Instance>, String>>()?;

            // Hide the cursor and report relative mouse movements.
            sdl_context.mouse().set_relative_mouse_mode(true);

            // Unhide windows after context setup
            for instance in instances.values_mut() {
                instance.window.show();
            }

            let mut event_pump = sdl_context.event_pump()?;
            let start = std::time::Instant::now();

            run_main_loop(&mut event_pump, &mut instances, start)
        }

        _ => unreachable!(),
    }
}

fn run_preview_loop(
    event_pump: &mut sdl2::EventPump,
    instance: &mut Instance,
    start: std::time::Instant,
) -> Result<(), String> {
    use sdl2::event::Event;

    'main: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::Window {
                    win_event: sdl2::event::WindowEvent::Close,
                    ..
                } => break 'main,

                _ => (),
            }
        }

        let timestamp = start.elapsed().as_secs_f64() * 1000.0;
        instance.draw(timestamp);
    }

    Ok(())
}

fn run_main_loop(
    event_pump: &mut sdl2::EventPump,
    instances: &mut HashMap<WindowId, Instance>,
    start: std::time::Instant,
) -> Result<(), String> {
    use sdl2::event::Event;

    'main: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::Window {
                    win_event: sdl2::event::WindowEvent::Close,
                    ..
                }
                | Event::KeyDown { .. }
                | Event::MouseButtonDown { .. } => break 'main,

                Event::MouseMotion { xrel, yrel, .. } => {
                    if f64::max(xrel.abs() as f64, yrel.abs() as f64)
                        > MINIMUM_MOUSE_MOTION_TO_EXIT_SCREENSAVER
                    {
                        break 'main;
                    }
                }

                _ => (),
            }
        }

        for (_, instance) in instances.iter_mut() {
            let timestamp = start.elapsed().as_secs_f64() * 1000.0;

            if timestamp < FADE_TO_BLACK_DURATION {
                instance.fade_to_black(timestamp);
            } else {
                instance.draw(timestamp);
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
fn new_preview_window(
    video_subsystem: &sdl2::VideoSubsystem,
    raw_window_handle: RawWindowHandle,
    config: &Config,
) -> Result<Instance, String> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    let win32_handle = match raw_window_handle {
        RawWindowHandle::Win32(handle) => handle,
        _ => return Err("This platform is not supported yet".to_string()),
    };

    let preview_hwnd = HWND(win32_handle.hwnd as _);

    let mut rect = RECT::default();
    unsafe {
        GetClientRect(preview_hwnd, &mut rect);
    }

    let inner_size = PhysicalSize::new(rect.right as u32, rect.bottom as u32);

    // Tell SDL that the window we’re about to adopt will be used with
    // OpenGL.
    sdl2::hint::set("SDL_VIDEO_FOREIGN_WINDOW_OPENGL", "1");
    let sdl_preview_window: *mut sdl2_sys::SDL_Window =
        unsafe { sdl2_sys::SDL_CreateWindowFrom(win32_handle.hwnd as _) };

    if sdl_preview_window.is_null() {
        return Err(format!(
            "Can’t create the preview window with the handle {:?}",
            win32_handle.hwnd
        ));
    }

    let preview_window: Window = unsafe {
        Window::from_ll(
            video_subsystem.clone(),
            sdl_preview_window,
            std::ptr::null_mut(),
        )
    };

    // You need to create an actual window to listen to events. We’ll
    // then link this to the preview window as a child to cleanup when
    // the preview dialog is closed.
    let window = video_subsystem
        .window("Flux Preview", inner_size.width, inner_size.height)
        .position(0, 0)
        .borderless()
        .hidden()
        .build()
        .map_err(|err| err.to_string())?;

    match window.raw_window_handle() {
        #[cfg(target_os = "windows")]
        raw_window_handle::RawWindowHandle::Win32(event_window_handle) => {
            if unsafe { set_window_parent_win32(HWND(event_window_handle.hwnd as _), preview_hwnd) }
            {
                log::debug!("Linked preview window");
            }
        }
        _ => (),
    }

    let (gl_context, gl_surface, glow_context) = new_gl_context(
        window.raw_display_handle(),
        raw_window_handle,
        inner_size,
        Some(window.raw_window_handle()),
    );

    let wallpaper = window
        .current_monitor()
        .and_then(|monitor| wallpaper::get(&monitor).ok());

    let physical_size = window.inner_size();
    let scale_factor = window.scale_factor();
    let logical_size = physical_size.to_logical(scale_factor);
    let settings = config.to_settings(wallpaper);
    let flux = Flux::new(
        &glow_context,
        logical_size.width,
        logical_size.height,
        physical_size.width,
        physical_size.height,
        &Rc::new(settings),
    )
    .map_err(|err| err.to_string())?;

    Ok(Instance {
        flux,
        gl_context,
        gl_surface,
        gl: Rc::clone(&glow_context),
        window,
    })
}

fn new_instance(
    video_subsystem: &sdl2::VideoSubsystem,
    config: &Config,
    surface: &surface::Surface,
) -> Result<Instance, String> {
    // Create the SDL window
    let mut window = video_subsystem
        .window("Flux", surface.size.width, surface.size.height)
        .position(surface.position.x, surface.position.y)
        .input_grabbed()
        .borderless()
        .hidden()
        .allow_highdpi()
        .opengl()
        .build()
        .map_err(|err| err.to_string())?;

    if let Err(err) = window.set_opacity(0.0) {
        log::warn!("Transparent window not supported: {}", err);
    }

    let (gl_context, gl_surface, glow_context) = new_gl_context(
        window.raw_display_handle(),
        window.raw_window_handle(),
        window.size().into(),
        None,
    );

    let physical_size = surface.size;
    let logical_size = physical_size.to_logical(surface.scale_factor);
    let settings = config.to_settings(surface.wallpaper.clone());
    let flux = Flux::new(
        &glow_context,
        logical_size.width,
        logical_size.height,
        physical_size.width,
        physical_size.height,
        &Rc::new(settings),
    )
    .map_err(|err| err.to_string())?;

    Ok(Instance {
        flux,
        gl_context,
        gl_surface,
        gl: Rc::clone(&glow_context),
        window,
    })
}

/// Create an OpenGL context, surface, and initialize the glow API.
///
/// Hacks
///
/// The optional attr_window should be used when rendering to the preview window. Instead of just
/// using the handle to the preview window, pass the window handle for the invisible event window
/// to work around a bug where Windows complains that it can't find the window class.
///
/// This code has been modified from glutin-winit and only supports WGL (Windows).
fn new_gl_context(
    raw_display_handle: RawDisplayHandle,
    raw_window_handle: RawWindowHandle,
    inner_size: PhysicalSize<u32>,

    // A hack to create the gl_display using the invisible event window
    // we create for the preview.
    attr_window: Option<RawWindowHandle>,
) -> (
    PossiblyCurrentContext,
    Surface<WindowSurface>,
    Rc<glow::Context>,
) {
    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_transparency(true)
        .compatible_with_native_window(raw_window_handle)
        .build();

    // Only WGL requires a window to create a full-fledged OpenGL context
    let attr_window = attr_window.unwrap_or(raw_window_handle);
    let preference = DisplayApiPreference::WglThenEgl(Some(attr_window));
    let gl_display = unsafe { Display::new(raw_display_handle, preference).unwrap() };

    let gl_config = unsafe {
        gl_display
            .find_configs(template)
            .unwrap()
            .reduce(|accum, config| {
                let transparency_check = config.supports_transparency().unwrap_or(false)
                    & !accum.supports_transparency().unwrap_or(false);

                if transparency_check || config.num_samples() > accum.num_samples() {
                    config
                } else {
                    accum
                }
            })
            .unwrap()
    };

    log::debug!(
        "Picked a config with {} samples and {:?} transparency",
        gl_config.num_samples(),
        gl_config.supports_transparency()
    );

    // Request the minimum required OpenGL version for Flux
    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .build(Some(raw_window_handle));

    // Fallback to GLES 3.0 (aka WebGL 2.0)
    let fallback_context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(Some(Version::new(3, 0))))
        .build(Some(raw_window_handle));

    let not_current_gl_context = unsafe {
        gl_display
            .create_context(&gl_config, &context_attributes)
            .unwrap_or_else(|_| {
                gl_display
                    .create_context(&gl_config, &fallback_context_attributes)
                    .expect("failed to create OpenGL context")
            })
    };

    let (width, height) = inner_size.non_zero().expect("non-zero window size").into();
    let attrs =
        SurfaceAttributesBuilder::<WindowSurface>::new().build(raw_window_handle, width, height);

    let gl_surface = unsafe {
        gl_config
            .display()
            .create_window_surface(&gl_config, &attrs)
            .unwrap()
    };

    // Make it current.
    let gl_context = not_current_gl_context.make_current(&gl_surface).unwrap();

    // Try setting vsync.
    if let Err(res) =
        gl_surface.set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()))
    {
        log::error!("Failed to set vsync: {res:?}");
    }

    let glow_context = unsafe {
        glow::Context::from_loader_function(|s| {
            gl_display.get_proc_address(&CString::new(s).unwrap().as_c_str()) as *const _
        })
    };
    log::debug!("{:?}", glow_context.version());

    (gl_context, gl_surface, Rc::new(glow_context))
}

// Specifying DPI awareness in the app manifest does not apply when running in a
// preview window.
#[cfg(windows)]
pub fn set_dpi_awareness() -> Result<(), String> {
    use windows::Win32::Foundation::E_INVALIDARG;
    use windows::Win32::UI::HiDpi::{
        GetProcessDpiAwareness, SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE,
        PROCESS_SYSTEM_DPI_AWARE,
    };

    if let Err(err) = unsafe { SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) } {
        match err.code() {
            E_INVALIDARG => return Err("Can’t enable support for high-resolution screens.".to_string()),
            // The app manifest settings, if applied, trigger this path.
            _ => {
                return match unsafe { GetProcessDpiAwareness(None) } {
                    Ok(awareness)
                        if awareness == PROCESS_PER_MONITOR_DPI_AWARE
                        || awareness == PROCESS_SYSTEM_DPI_AWARE => Ok(()),
                    _ => Err("Can’t enable support for high-resolution screens. The setting has been modified and set to an unsupported value.".to_string()),
                }
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
unsafe fn set_window_parent_win32(handle: HWND, parent_handle: HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetParent, SetWindowLongPtrA, GWL_STYLE, WINDOW_STYLE, WS_CHILD, WS_POPUP,
    };

    // Attach our window to the parent window.
    // You can get more error information with `GetLastError`
    // https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setparent
    SetParent(handle, parent_handle);

    let style = WINDOW_STYLE(GetWindowLongW(handle, GWL_STYLE) as u32);
    let new_style = (style & !WS_POPUP) | WS_CHILD;

    // `SetParent` doesn’t actually set the window style flags. `WS_POPUP` and
    // `WS_CHILD` are mutually exclusive.
    SetWindowLongPtrA(handle, GWL_STYLE, new_style.0 as isize);

    true
}

/// [`winit::dpi::PhysicalSize<u32>`] non-zero extensions.
trait NonZeroU32PhysicalSize {
    /// Converts to non-zero `(width, height)`.
    fn non_zero(self) -> Option<(NonZeroU32, NonZeroU32)>;
}
impl NonZeroU32PhysicalSize for PhysicalSize<u32> {
    fn non_zero(self) -> Option<(NonZeroU32, NonZeroU32)> {
        let w = NonZeroU32::new(self.width)?;
        let h = NonZeroU32::new(self.height)?;
        Some((w, h))
    }
}
