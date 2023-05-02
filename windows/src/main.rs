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
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use winit_compat::{HasMonitors, HasWinitWindow, MonitorHandle};

use std::collections::HashMap;
use std::ffi::CString;
use std::num::NonZeroU32;
use std::{fmt, fs, mem, path, process, rc::Rc};

use glow as GL;
use glow::HasContext;

use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

use sdl2::video::Window;
use winit::dpi::PhysicalSize;

use glutin::config::{ColorBufferType, Config as GLConfig, ConfigTemplateBuilder};
use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version};
use glutin::display::{Display, DisplayApiPreference, GetGlDisplay};
use glutin::prelude::*;
use glutin::surface::{Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};

// http://developer.download.nvidia.com/devzone/devcenter/gamegraphics/files/OptimusRenderingPolicies.pdf
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
#[no_mangle]
pub static mut NvOptimusEnablement: i32 = 1;

// https://gpuopen.com/learn/amdpowerxpressrequesthighperformance/
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
#[no_mangle]
pub static mut AmdPowerXpressRequestHighPerformance: i32 = 1;

const MINIMUM_MOUSE_MOTION_TO_EXIT_SCREENSAVER: f64 = 10.0;

// In milliseconds
const FADE_TO_BLACK_DURATION: f64 = 300.0;

type WindowId = u32;

struct Instance {
    flux: Flux,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    gl: Rc<glow::Context>,
    gl_window: Option<Window>,
    window: Window,
    dxgi_interop: DXGIInterop,
}

impl Instance {
    pub fn draw(&mut self, timestamp: f64) {
        unsafe {
            use windows::Win32::System::Threading::{WaitForSingleObject, INFINITE};
            WaitForSingleObject(self.dxgi_interop.frame_latency_waitable_object, INFINITE);

            (self.dxgi_interop.dx_interop.DXLockObjectsNV)(
                self.dxgi_interop.gl_handle_d3d,
                1,
                &mut self.dxgi_interop.color_handle_gl as *mut _,
            );

            self.gl_context
                .make_current(&self.gl_surface)
                .expect("make OpenGL context current");

            self.flux.compute(timestamp);

            self.gl
                .bind_framebuffer(GL::FRAMEBUFFER, Some(self.dxgi_interop.fbo));

            self.flux.render();

            self.gl.bind_framebuffer(GL::FRAMEBUFFER, None);

            self.gl.finish();

            (self.dxgi_interop.dx_interop.DXUnlockObjectsNV)(
                self.dxgi_interop.gl_handle_d3d,
                1,
                &mut self.dxgi_interop.color_handle_gl as *mut _,
            );

            let _ = self.dxgi_interop.swap_chain.Present(1, 0);
        }

        // self.gl_surface
        //     .swap_buffers(&self.gl_context)
        //     .expect("swap OpenGL buffers");
    }

    pub fn fade_to_black(&mut self, timestamp: f64) {
        unsafe {
            use windows::Win32::System::Threading::{WaitForSingleObject, INFINITE};
            WaitForSingleObject(self.dxgi_interop.frame_latency_waitable_object, INFINITE);

            (self.dxgi_interop.dx_interop.DXLockObjectsNV)(
                self.dxgi_interop.gl_handle_d3d,
                1,
                &mut self.dxgi_interop.color_handle_gl as *mut _,
            );

            self.gl_context
                .make_current(&self.gl_surface)
                .expect("make OpenGL context current");

            let progress = (timestamp / FADE_TO_BLACK_DURATION).clamp(0.0, 1.0) as f32;
            self.gl.clear_color(0.0, 0.0, 0.0, progress);
            self.gl.clear(GL::COLOR_BUFFER_BIT);

            self.gl
                .bind_framebuffer(GL::FRAMEBUFFER, Some(self.dxgi_interop.fbo));

            self.flux.render();

            self.gl.bind_framebuffer(GL::FRAMEBUFFER, None);
            self.gl.finish();

            (self.dxgi_interop.dx_interop.DXUnlockObjectsNV)(
                self.dxgi_interop.gl_handle_d3d,
                1,
                &mut self.dxgi_interop.color_handle_gl as *mut _,
            );

            let _ = self.dxgi_interop.swap_chain.Present(1, 0);
        }
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

    // By default, SDL disables the screensaver and doesn’t allow the display to sleep. We want
    // both of these things to happen in both screensaver and preview modes.
    sdl2::hint::set("SDL_VIDEO_ALLOW_SCREENSAVER", "1");

    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

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
                | Event::MouseButtonDown { .. } => {
                    log::debug!("EVENT");
                    break 'main;
                }

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

    let (gl_context, gl_surface, glow_context, dxgi_interop) = new_gl_context(
        window.raw_display_handle(),
        raw_window_handle,
        inner_size,
        None,
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
        gl_window: None,
        window,
        dxgi_interop,
    })
}

fn new_instance(
    video_subsystem: &sdl2::VideoSubsystem,
    config: &Config,
    surface: &surface::Surface,
) -> Result<Instance, String> {
    // Create the SDL window
    let window = video_subsystem
        .window("Flux", surface.size.width, surface.size.height + 1)
        .position(surface.position.x, surface.position.y)
        .input_grabbed()
        .borderless()
        .hidden()
        .allow_highdpi()
        .build()
        .map_err(|err| err.to_string())?;

    unsafe { enable_transparency(&window.raw_window_handle()) };

    let hidden_window = video_subsystem
        .window("GL Context Window", 1, 1)
        .hidden()
        .opengl()
        .build()
        .map_err(|err| err.to_string())?;

    let (gl_context, gl_surface, glow_context, dxgi_interop) = new_gl_context(
        window.raw_display_handle(),
        window.raw_window_handle(),
        window.size().into(),
        Some(hidden_window.raw_window_handle()),
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
        gl_window: Some(hidden_window),
        window,
        dxgi_interop,
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
    actual_raw_window_handle: RawWindowHandle,
    inner_size: PhysicalSize<u32>,

    hidden_window: Option<RawWindowHandle>,
    // A hack to create the gl_display using the invisible event window
    // we create for the preview.
    attr_window: Option<RawWindowHandle>,
) -> (
    PossiblyCurrentContext,
    Surface<WindowSurface>,
    Rc<glow::Context>,
    DXGIInterop,
) {
    use glutin::config::ConfigSurfaceTypes;

    let raw_window_handle = hidden_window.unwrap();

    let template = ConfigTemplateBuilder::new()
        .with_buffer_type(glutin::config::ColorBufferType::Rgb {
            r_size: 8,
            g_size: 8,
            b_size: 8,
        })
        .with_alpha_size(8)
        .with_transparency(true)
        .compatible_with_native_window(raw_window_handle)
        // .with_surface_type(ConfigSurfaceTypes::empty())
        .build();

    // Only WGL requires a window to create a full-fledged OpenGL context
    let attr_window = attr_window.unwrap_or(raw_window_handle);
    let preference = DisplayApiPreference::WglThenEgl(Some(attr_window));
    let gl_display = unsafe { Display::new(raw_display_handle, preference).unwrap() };

    // Rank the configs by transparency and alpha size, while prefering the original order of the
    // configs.
    #[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
    struct Rank {
        supports_transparency: bool,
        alpha_size: u8,
        samples: i8,
        prefer_original_order: isize,
    }

    let (gl_config_index, gl_config) = unsafe {
        gl_display
            .find_configs(template)
            .unwrap()
            .enumerate()
            .map(|(index, config)| {
                log::debug!("Found config #{index}:\n{}", HumanConfig::new(&config));
                (index, config)
            })
            .max_by_key(|(index, config)| Rank {
                supports_transparency: config.supports_transparency().unwrap_or(false),
                alpha_size: config.alpha_size(),
                samples: -(config.num_samples() as i8),
                prefer_original_order: -(*index as isize),
            })
            .expect("cannot find a suitable GL config")
    };

    log::debug!(
        "Picked config #{gl_config_index}:\n{}",
        HumanConfig::new(&gl_config)
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
    // ContextAttributesBuilder::new().build(Some(raw_window_handle));

    let gl_surface = unsafe {
        gl_config
            .display()
            // .create_context(&gl_config, &attrs)
            .create_window_surface(&gl_config, &attrs)
            .unwrap()
    };

    // Make it current.
    let gl_context = not_current_gl_context.make_current(&gl_surface).unwrap();

    // Try setting vsync.
    // if let Err(res) =
    //     gl_surface.set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()))
    // {
    //     log::error!("Failed to set vsync: {res:?}");
    // }

    let glow_context = unsafe {
        glow::Context::from_loader_function(|s| {
            gl_display.get_proc_address(&CString::new(s).unwrap().as_c_str()) as *const _
        })
    };
    log::debug!("{:?}", glow_context.version());

    // use glutin::display::GetDisplayExtensions;
    // let extensions = match gl_display {
    //     Display::Wgl(ref wgl) => wgl.extensions(),
    //     _ => panic!("Unsupported display"),
    // };

    log::debug!("Creating DXGI swapchain");
    let dxgi_interop = create_dxgi_swapchain(
        &actual_raw_window_handle,
        &glow_context,
        width.into(),
        height.into(),
    );
    log::debug!("Created DXGI swapchain");

    // Set common GL state
    unsafe {
        glow_context.disable(GL::MULTISAMPLE);
    }

    (gl_context, gl_surface, Rc::new(glow_context), dxgi_interop)
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

#[cfg(windows)]
unsafe fn enable_transparency(handle: &RawWindowHandle) {
    use windows::Win32::Graphics::{
        Dwm::{DwmEnableBlurBehindWindow, DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND},
        Gdi::{CreateRectRgn, DeleteObject},
    };

    let hwnd = match handle {
        raw_window_handle::RawWindowHandle::Win32(event_window_handle) => {
            HWND(event_window_handle.hwnd as _)
        }
        _ => panic!("This platform is not supported yet"),
    };

    // Empty region for the blur effect, so the window is fully transparent
    let region = CreateRectRgn(0, 0, -1, -1);

    let bb = DWM_BLURBEHIND {
        dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
        fEnable: true.into(),
        hRgnBlur: region,
        fTransitionOnMaximized: false.into(),
    };
    if let Err(err) = DwmEnableBlurBehindWindow(hwnd, &bb) {
        log::warn!("Failed to set window transparency: {:?}", err);
    }
    DeleteObject(region);
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

#[derive(Debug)]
struct HumanConfig {
    color_buffer_type: Option<ColorBufferType>,
    alpha_size: u8,
    depth_size: u8,
    stencil_size: u8,
    float_pixels: bool,
    srgb_capable: bool,
    supports_transparency: bool,
    num_samples: u8,
    hardware_accelerated: bool,
}

impl HumanConfig {
    fn new(config: &GLConfig) -> Self {
        Self {
            color_buffer_type: config.color_buffer_type(),
            alpha_size: config.alpha_size(),
            depth_size: config.depth_size(),
            stencil_size: config.stencil_size(),
            float_pixels: config.float_pixels(),
            srgb_capable: config.srgb_capable(),
            supports_transparency: config.supports_transparency().unwrap_or(false),
            num_samples: config.num_samples(),
            hardware_accelerated: config.hardware_accelerated(),
        }
    }
}

impl fmt::Display for HumanConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let HumanConfig {
            color_buffer_type,
            alpha_size,
            depth_size,
            stencil_size,
            float_pixels,
            srgb_capable,
            supports_transparency,
            num_samples,
            hardware_accelerated,
        } = self;
        write!(
            f,
            "Color buffer type: {color_buffer_type:?}\n\
               Alpha size: {alpha_size}\n\
               Depth size: {depth_size}\n\
               Stencil size: {stencil_size}\n\
               Float pixels: {float_pixels}\n\
               sRGB capable: {srgb_capable}\n\
               Supports transparency: {supports_transparency}\n\
               Number of samples: {num_samples}\n\
               Hardware accelerated: {hardware_accelerated}"
        )
    }
}

// https://github.com/Osspial/render_to_dxgi/blob/master/src/main.rs
// https://github.com/nlguillemot/OpenGL-on-DXGI/blob/master/main.cpp
fn create_dxgi_swapchain(
    raw_window_handle: &RawWindowHandle,
    gl: &glow::Context,
    width: u32,
    height: u32,
) -> DXGIInterop {
    use windows::core::*;
    use windows::Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_11_0,
    };
    use windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, D3D11CreateDeviceAndSwapChain, ID3D11Device, ID3D11DeviceContext,
        ID3D11RenderTargetView, ID3D11Texture2D, D3D11_CREATE_DEVICE_FLAG,
        D3D11_CREATE_DEVICE_PREVENT_INTERNAL_THREADING_OPTIMIZATIONS,
        D3D11_CREATE_DEVICE_SINGLETHREADED, D3D11_SDK_VERSION,
    };
    use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, DXGI_MODE_DESC,
        DXGI_MODE_SCALING_UNSPECIFIED, DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED, DXGI_SAMPLE_DESC,
    };
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory2, IDXGIFactory2, IDXGISwapChain, IDXGISwapChain2, DXGI_SWAP_CHAIN_DESC,
        DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING,
        DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT, DXGI_SWAP_EFFECT_DISCARD,
        DXGI_SWAP_EFFECT_FLIP_DISCARD, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_BACK_BUFFER,
        DXGI_USAGE_RENDER_TARGET_OUTPUT, DXGI_USAGE_UNORDERED_ACCESS,
    };

    let win32_handle = match raw_window_handle {
        RawWindowHandle::Win32(handle) => handle,
        _ => panic!("This platform is not supported yet"),
    };

    let hwnd = HWND(win32_handle.hwnd as _);

    let mut p_device: Option<ID3D11Device> = None;
    let mut p_context: Option<ID3D11DeviceContext> = None;
    let mut p_swap_chain: Option<IDXGISwapChain> = None;

    unsafe {
        D3D11CreateDeviceAndSwapChain(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            D3D11_CREATE_DEVICE_PREVENT_INTERNAL_THREADING_OPTIMIZATIONS
                | D3D11_CREATE_DEVICE_FLAG(0)
                | D3D11_CREATE_DEVICE_SINGLETHREADED,
            // D3D11_CREATE_DEVICE_FLAG(0), // | D3D11_CREATE_DEVICE_DEBUG,
            // Some(&[D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_11_0]),
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            // None,
            D3D11_SDK_VERSION,
            Some(&DXGI_SWAP_CHAIN_DESC {
                BufferDesc: DXGI_MODE_DESC {
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    // Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    // Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    // ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
                    // Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
                    ..Default::default()
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                // | DXGI_USAGE_BACK_BUFFER
                // | DXGI_USAGE_UNORDERED_ACCESS,
                BufferCount: 2,
                OutputWindow: hwnd,
                Windowed: true.into(),
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                // SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                // Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
                Flags: (DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0
                    | DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0) as u32,
                ..Default::default()
            }),
            Some(&mut p_swap_chain),
            Some(&mut p_device),
            None,
            Some(&mut p_context),
        )
        .expect("Failed to create DXGI device and swapchain");
    }

    let swap_chain = p_swap_chain.expect("failed to created swapchain");
    let context = p_context.expect("failed to create immediate context");
    let device = p_device.clone().expect("failed to create device");

    log::debug!("Created device, context, and swapchain");

    let swap_chain_2: IDXGISwapChain2 = swap_chain.cast().expect("failed to cast the SwapChain");

    let frame_latency_waitable_object = unsafe { swap_chain_2.GetFrameLatencyWaitableObject() };

    use std::borrow::Cow;
    use std::ffi::CStr;
    use windows::core::PCSTR;
    use windows::Win32::Graphics::Gdi::HDC;
    use windows::Win32::Graphics::OpenGL::{wglGetCurrentDC, wglGetProcAddress};

    unsafe {
        let dc = wglGetCurrentDC();
        let get_extensions_string_arb: Option<unsafe extern "C" fn(hdc: HDC) -> *const c_char> =
            mem::transmute(wglGetProcAddress(PCSTR(
                &b"wglGetExtensionsStringARB\0"[0] as *const u8,
            )));

        let extensions = match get_extensions_string_arb {
            Some(wglGetExtensionsStringARB) => {
                CStr::from_ptr(wglGetExtensionsStringARB(dc)).to_string_lossy()
            }
            None => Cow::Borrowed(""),
        };

        // Load function pointers.
        for extension in extensions.split(' ') {
            if extension == "WGL_NV_DX_interop" {
                log::debug!("Found WGL_NV_DX_interop");
            }
            if extension == "WGL_NV_DX_interop2" {
                log::debug!("Found WGL_NV_DX_interop2");
            }
        }

        log::debug!("Extensions: {}", extensions);
    }

    let dx_interop = unsafe {
        WGLDXInteropExtensionFunctions {
            DXCloseDeviceNV: mem::transmute(wglGetProcAddress(PCSTR(
                &b"wglDXCloseDeviceNV\0"[0] as *const u8,
            ))),
            DXLockObjectsNV: mem::transmute(wglGetProcAddress(PCSTR(
                &b"wglDXLockObjectsNV\0"[0] as *const u8,
            ))),
            DXOpenDeviceNV: mem::transmute(wglGetProcAddress(PCSTR(
                &b"wglDXOpenDeviceNV\0"[0] as *const u8,
            ))),
            DXRegisterObjectNV: mem::transmute(wglGetProcAddress(PCSTR(
                &b"wglDXRegisterObjectNV\0"[0] as *const u8,
            ))),
            DXSetResourceShareHandleNV: mem::transmute(wglGetProcAddress(PCSTR(
                &b"wglDXSetResourceShareHandleNV\0"[0] as *const u8,
            ))),
            DXUnlockObjectsNV: mem::transmute(wglGetProcAddress(PCSTR(
                &b"wglDXUnlockObjectsNV\0"[0] as *const u8,
            ))),
            DXUnregisterObjectNV: mem::transmute(wglGetProcAddress(PCSTR(
                &b"wglDXUnregisterObjectNV\0"[0] as *const u8,
            ))),
        }
    };
    log::debug!("Fetched interop extension functions");

    unsafe {
        let gl_handle_d3d = (dx_interop.DXOpenDeviceNV)(device.as_raw());
        if gl_handle_d3d.is_invalid() {
            log::error!("Failed to open the GL DX interop device");
        } else {
            log::debug!("Opened interop device");
        }

        let mut color_buffer: ID3D11Texture2D = swap_chain_2.GetBuffer(0).unwrap();
        let mut color_buffer_view: Option<ID3D11RenderTargetView> = None;

        // let desc = D3D11_RENDER_TARGET_VIEW_DESC {
        //     Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        //     Usage: D3D11_USAGE_DEFAULT,
        //     ..Default::default()
        // };

        device
            .CreateRenderTargetView(&color_buffer, None, Some(&mut color_buffer_view))
            .unwrap();

        context.OMSetRenderTargets(Some(&[color_buffer_view.clone()]), None);
        let clear_color: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
        context.ClearRenderTargetView(
            color_buffer_view.as_ref().unwrap(),
            &clear_color as *const _,
        );
        log::debug!("Cleared render target view");

        let rbo = gl.create_renderbuffer().unwrap();
        let fbo = gl.create_framebuffer().unwrap();
        let texture = gl.create_texture().unwrap();

        // I don't think this is necessary, at least according to the khronos example
        // gl.bind_renderbuffer(GL::RENDERBUFFER, Some(rbo));
        // gl.renderbuffer_storage(GL::RENDERBUFFER, GL::RGBA8, width as i32, height as i32);
        // gl.bind_renderbuffer(GL::RENDERBUFFER, None);

        log::debug!("Created GL renderbuffer");

        const WGL_ACCESS_READ_WRITE_NV: u32 = 0x0001;
        const WGL_ACCESS_READ_WRITE_DISCARD_NV: u32 = 0x0002;
        // let color_handle_gl = (dx_interop.DXRegisterObjectNV)(
        //     gl_handle_d3d,
        //     color_buffer.as_raw(),
        //     rbo.0.into(),
        //     GL::RENDERBUFFER,
        //     WGL_ACCESS_READ_WRITE_DISCARD_NV,
        // );

        // According to my testing, AMD graphics cards don't support sharig renderbuffers.
        let color_handle_gl = (dx_interop.DXRegisterObjectNV)(
            gl_handle_d3d,
            color_buffer.as_raw(),
            texture.0.into(),
            GL::TEXTURE_2D,
            WGL_ACCESS_READ_WRITE_DISCARD_NV,
        );

        if color_handle_gl.is_invalid() {
            log::error!("Failed to register color buffer handle");
        } else {
            log::debug!("Registered color buffer");
        }

        // Bind the texture to the framebuffer
        gl.bind_framebuffer(GL::FRAMEBUFFER, Some(fbo));
        gl.framebuffer_texture_2d(
            GL::FRAMEBUFFER,
            GL::COLOR_ATTACHMENT0,
            GL::TEXTURE_2D,
            Some(texture),
            0,
        );

        // gl.viewport(0, 0, width as i32, height as i32);
        // gl.bind_renderbuffer(GL::RENDERBUFFER, Some(rbo));
        // gl.bind_framebuffer(GL::FRAMEBUFFER, Some(fbo));
        // gl.framebuffer_renderbuffer(
        //     GL::FRAMEBUFFER,
        //     GL::COLOR_ATTACHMENT0,
        //     GL::RENDERBUFFER,
        //     Some(rbo),
        // );
        log::debug!("Created GL framebuffer");

        match gl.check_framebuffer_status(GL::FRAMEBUFFER) {
            GL::FRAMEBUFFER_INCOMPLETE_ATTACHMENT => {
                log::error!("FRAMEBUFFER_INCOMPLETE_ATTACHMENT")
            }
            GL::FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT => {
                log::error!("FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT")
            }
            GL::FRAMEBUFFER_COMPLETE => {
                log::debug!("FRAMEBUFFER_COMPLETE");
            }
            other => log::debug!("Framebuffer status: {:#x}", other),
        }

        gl.bind_framebuffer(GL::FRAMEBUFFER, None);

        DXGIInterop {
            device,
            context,
            swap_chain: swap_chain_2,
            gl_handle_d3d,
            dx_interop,
            color_handle_gl,
            frame_latency_waitable_object,
            fbo,
            rbo,
        }
    }
}

use windows::Win32::Graphics::Direct3D11::{
    ID3D11Device, ID3D11DeviceContext, D3D11_CREATE_DEVICE_DEBUG,
};
use windows::Win32::Graphics::Dxgi::{IDXGISwapChain, IDXGISwapChain2};

struct DXGIInterop {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    // swap_chain: IDXGISwapChain,
    swap_chain: IDXGISwapChain2,
    gl_handle_d3d: HANDLE,
    dx_interop: WGLDXInteropExtensionFunctions,
    color_handle_gl: HANDLE,
    frame_latency_waitable_object: HANDLE,
    fbo: GL::NativeFramebuffer,
    rbo: GL::NativeRenderbuffer,
}

use std::os::raw::{c_char, c_int, c_uint, c_void};
use windows::Win32::Foundation::{BOOL, HANDLE};
type GLint = c_int;
type GLenum = c_uint;
type GLuint = c_uint;
#[allow(non_snake_case)]
pub(crate) struct WGLDXInteropExtensionFunctions {
    pub(crate) DXCloseDeviceNV: unsafe extern "C" fn(hDevice: HANDLE) -> BOOL,
    pub(crate) DXLockObjectsNV:
        unsafe extern "C" fn(hDevice: HANDLE, count: GLint, hObjects: *mut HANDLE) -> BOOL,
    pub(crate) DXOpenDeviceNV: unsafe extern "C" fn(dxDevice: *mut c_void) -> HANDLE,
    pub(crate) DXRegisterObjectNV: unsafe extern "C" fn(
        hDevice: HANDLE,
        dxResource: *mut c_void,
        name: GLuint,
        object_type: GLenum,
        access: GLenum,
    ) -> HANDLE,
    pub(crate) DXSetResourceShareHandleNV:
        unsafe extern "C" fn(dxResource: *mut c_void, shareHandle: HANDLE) -> BOOL,
    pub(crate) DXUnlockObjectsNV:
        unsafe extern "C" fn(hDevice: HANDLE, count: GLint, hObjects: *mut HANDLE) -> BOOL,
    pub(crate) DXUnregisterObjectNV: unsafe extern "C" fn(hDevice: HANDLE, hObject: HANDLE) -> BOOL,
}
