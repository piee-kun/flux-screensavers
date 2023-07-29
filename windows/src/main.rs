// Disable the console window that pops up when you launch the .exe
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod config;
mod gl_context;
mod platform;
mod settings_window;
mod surface;
mod wallpaper;
mod winit_compat;

use cli::Mode;
use config::Config;
use flux::Flux;
use winit_compat::{HasMonitors, HasWinitWindow, MonitorHandle};

use std::collections::HashMap;
use std::{fs, path, process, rc::Rc};

use glow as GL;
use glow::HasContext;
use glutin::context::PossiblyCurrentContextGlSurfaceAccessor;
use glutin::prelude::GlSurface;

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle, RawWindowHandle};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

use sdl2::video::Window;
use winit::dpi::PhysicalSize;

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

// Higher values will make the screensaver tolerate more mouse movement before exiting.
const MINIMUM_MOUSE_MOTION_TO_EXIT_SCREENSAVER: f64 = 10.0;
// In milliseconds. TODO: likely doesn't work on most platforms
const FADE_TO_BLACK_DURATION: f64 = 300.0;

type WindowId = u32;

#[allow(dead_code)]
struct Instance {
    flux: Flux,
    window: Window,
    gl_context: gl_context::GLContext,
    swapchain: Swapchain,
}

enum Swapchain {
    Gl,

    #[cfg(windows)]
    Dxgi(platform::windows::dxgi_swapchain::DXGIInterop),
}

impl Instance {
    pub fn draw(&mut self, timestamp: f64) {
        match self.swapchain {
            Swapchain::Gl => {
                self.gl_context
                    .context
                    .make_current(&self.gl_context.surface)
                    .expect("make OpenGL context current");

                self.flux.animate(timestamp);

                self.gl_context
                    .surface
                    .swap_buffers(&self.gl_context.context)
                    .expect("swap OpenGL buffers");
            }

            #[cfg(windows)]
            Swapchain::Dxgi(ref mut dxgi_interop) => unsafe {
                platform::windows::dxgi_swapchain::with_dxgi_swapchain(dxgi_interop, |fbo| {
                    self.gl_context
                        .context
                        .make_current(&self.gl_context.surface)
                        .expect("make OpenGL context current");

                    self.flux.compute(timestamp);

                    self.gl_context
                        .gl
                        .bind_framebuffer(GL::FRAMEBUFFER, Some(*fbo));

                    self.flux.render();

                    self.gl_context.gl.bind_framebuffer(GL::FRAMEBUFFER, None);
                    self.gl_context.gl.finish();
                });
            },
        }
    }

    pub fn fade_to_black(&mut self, timestamp: f64) {
        match self.swapchain {
            Swapchain::Gl => {
                self.gl_context
                    .context
                    .make_current(&self.gl_context.surface)
                    .expect("make OpenGL context current");

                let progress = (timestamp / FADE_TO_BLACK_DURATION).clamp(0.0, 1.0) as f32;
                unsafe {
                    self.gl_context.gl.clear_color(0.0, 0.0, 0.0, progress);
                    self.gl_context.gl.clear(GL::COLOR_BUFFER_BIT);
                }

                self.gl_context
                    .surface
                    .swap_buffers(&self.gl_context.context)
                    .expect("swap OpenGL buffers");
            }

            #[cfg(windows)]
            Swapchain::Dxgi(ref mut dxgi_interop) => unsafe {
                platform::windows::dxgi_swapchain::with_dxgi_swapchain(dxgi_interop, |fbo| {
                    self.gl_context
                        .context
                        .make_current(&self.gl_context.surface)
                        .expect("make OpenGL context current");

                    self.gl_context
                        .gl
                        .bind_framebuffer(GL::FRAMEBUFFER, Some(*fbo));

                    let progress = (timestamp / FADE_TO_BLACK_DURATION).clamp(0.0, 1.0) as f32;
                    self.gl_context.gl.clear_color(0.0, 0.0, 0.0, progress);
                    self.gl_context.gl.clear(GL::COLOR_BUFFER_BIT);

                    self.gl_context.gl.bind_framebuffer(GL::FRAMEBUFFER, None);
                    self.gl_context.gl.finish();
                });
            },
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
        LevelFilter::Warn,
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
    platform::windows::dpi_awareness::set_dpi_awareness()?;

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
            if unsafe {
                platform::windows::window::set_window_parent_win32(
                    HWND(event_window_handle.hwnd as _),
                    preview_hwnd,
                )
            } {
                log::debug!("Linked preview window");
            }
        }
        _ => (),
    }

    let gl_context = gl_context::new_gl_context(
        window.raw_display_handle(),
        inner_size,
        raw_window_handle,
        Some(window.raw_window_handle()),
    );

    let swapchain = create_swapchain(&raw_window_handle, &gl_context);

    let wallpaper = window
        .current_monitor()
        .and_then(|monitor| wallpaper::get(&monitor).ok());

    let physical_size = window.inner_size();
    let scale_factor = window.scale_factor();
    let logical_size = physical_size.to_logical(scale_factor);
    let settings = config.to_settings(wallpaper);
    let flux = Flux::new(
        &gl_context.gl,
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
        window,
        swapchain,
    })
}

fn new_instance(
    video_subsystem: &sdl2::VideoSubsystem,
    config: &Config,
    surface: &surface::Surface,
) -> Result<Instance, String> {
    // Create the SDL window
    let window = video_subsystem
        .window("Flux", surface.size.width, surface.size.height)
        .position(surface.position.x, surface.position.y)
        .input_grabbed()
        .borderless()
        .hidden()
        .allow_highdpi()
        .build()
        .map_err(|err| err.to_string())?;

    #[cfg(windows)]
    unsafe {
        platform::windows::window::enable_transparency(&window.raw_window_handle())
    };

    let gl_context = gl_context::new_gl_context(
        window.raw_display_handle(),
        window.size().into(),
        window.raw_window_handle(),
        None,
    );

    let swapchain = create_swapchain(&window.raw_window_handle(), &gl_context);

    let physical_size = surface.size;
    let logical_size = physical_size.to_logical(surface.scale_factor);
    let settings = config.to_settings(surface.wallpaper.clone());
    let flux = Flux::new(
        &Rc::clone(&gl_context.gl),
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
        window,
        swapchain,
    })
}

#[cfg(not(windows))]
fn create_swapchain(
    raw_window_handle: &RawWindowHandle,
    gl_context: &gl_context::GLContext,
) -> Swapchain {
    Swapchain::Gl
}

#[cfg(windows)]
fn create_swapchain(
    raw_window_handle: &RawWindowHandle,
    gl_context: &gl_context::GLContext,
) -> Swapchain {
    let dxgi_interop =
        platform::windows::dxgi_swapchain::create_dxgi_swapchain(raw_window_handle, &gl_context.gl);

    match dxgi_interop {
        Ok(dxgi_interop) => Swapchain::Dxgi(dxgi_interop),
        Err(err) => {
            use glutin::surface::SwapInterval;
            use std::num::NonZeroU32;

            log::warn!(
                "Failed to create DXGI swapchain: {}. Falling back to GL.",
                err
            );

            // Try setting vsync.
            if let Err(res) = gl_context.surface.set_swap_interval(
                &gl_context.context,
                SwapInterval::Wait(NonZeroU32::new(1).unwrap()),
            ) {
                log::error!("Failed to set vsync: {res:?}");
            }

            Swapchain::Gl
        }
    }
}
