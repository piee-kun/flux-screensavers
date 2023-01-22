// Disable the console window that pops up when you launch the .exe
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use flux::{settings::*, *};
use glow::HasContext;
use std::fs::File;
use std::rc::Rc;
use takeable::Takeable;

#[cfg(windows)]
use glutin::platform::windows::WindowBuilderExtWindows;
use raw_window_handle::RawWindowHandle;

mod cli;
mod settings_window;
mod surface;
use cli::Mode;

const MINIMUM_MOUSE_MOTION_TO_EXIT_SCREENSAVER: f64 = 10.0;

struct Instance<W> {
    flux: Flux,
    context: Takeable<glutin::ContextWrapper<glutin::PossiblyCurrent, W>>,
}

impl<W> Instance<W>
where
    W: std::fmt::Debug,
{
    pub fn draw(&mut self, timestamp: f64) {
        let context = self.context.take();
        self.context =
            unsafe { Takeable::new(context.make_current().expect("make OpenGL context current")) };
        self.flux.animate(timestamp);
        self.context.swap_buffers().expect("swap OpenGL buffers");
    }
}

enum WindowMode {
    AllDisplays(Vec<Instance<glutin::window::Window>>),
    PreviewWindow {
        #[allow(dead_code)]
        window: glutin::window::Window,
        instance: Box<Instance<()>>,
    },
}

fn main() {
    use simplelog::*;

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Debug,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Debug,
            Config::default(),
            // TODO: move to cache dir
            File::create("flux_screensaver.log").unwrap(),
        ),
    ])
    .expect("set up logging");

    match cli::read_flags().and_then(run_flux) {
        Ok(_) => std::process::exit(0),
        Err(err) => {
            log::error!("{}", err);
            std::process::exit(1)
        }
    };
}

fn run_flux(mode: Mode) -> Result<(), String> {
    if mode == Mode::Settings {
        log::debug!("Running settings");
        return settings_window::run().map_err(|err| err.to_string());
    }

    let event_loop = glutin::event_loop::EventLoop::new();
    let mut window_mode = match mode {
        Mode::Preview(raw_window_handle) => {
            #[cfg(not(windows))]
            panic!("Preview window unsupported");

            #[cfg(windows)]
            new_preview_window(&event_loop, &raw_window_handle, &Default::default())?
        }
        Mode::Screensaver => {
            let monitors = event_loop.available_monitors().collect();
            log::debug!("Available monitors: {:?}", monitors);

            let surfaces = surface::combine_monitors(monitors);
            log::debug!("Creating windows: {:?}", surfaces);

            let instances = surfaces
                .iter()
                .map(|surface| new_instance(&event_loop, &Default::default(), surface))
                .collect::<Result<Vec<Instance<glutin::window::Window>>, String>>()?;
            WindowMode::AllDisplays(instances)
        }
        _ => unreachable!()
    };

    let start = std::time::Instant::now();
    event_loop.run(move |event, _, control_flow| {
        use glutin::event::{DeviceEvent, Event, WindowEvent};
        use glutin::event_loop::ControlFlow;

        *control_flow = ControlFlow::Poll;

        match mode {
            Mode::Preview(_) => match event {
                Event::WindowEvent { event, .. } => {
                    if event == WindowEvent::CloseRequested {
                        *control_flow = ControlFlow::Exit
                    }
                }

                Event::MainEventsCleared => {
                    let timestamp = start.elapsed().as_secs_f64() * 1000.0;
                    match window_mode {
                        WindowMode::PreviewWindow {
                            ref mut instance, ..
                        } => instance.draw(timestamp),
                        _ => panic!("Unexpected window mode"),
                    }
                }

                Event::LoopDestroyed => *control_flow = ControlFlow::Exit,

                _ => (),
            },

            Mode::Screensaver => match event {
                Event::MainEventsCleared => {
                    let timestamp = start.elapsed().as_secs_f64() * 1000.0;
                    match window_mode {
                        WindowMode::AllDisplays(ref mut instances) => {
                            for instance in instances.iter_mut() {
                                instance.draw(timestamp);
                            }
                        }
                        _ => panic!("Unexpected window mode"),
                    }
                }

                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested { .. }
                    | WindowEvent::KeyboardInput { .. }
                    | WindowEvent::MouseInput { .. } => {}
                    _ => (),
                },

                Event::DeviceEvent {
                    event:
                        DeviceEvent::MouseMotion {
                            delta: (xrel, yrel),
                        },
                    ..
                } if f64::max(xrel.abs(), yrel.abs())
                    > MINIMUM_MOUSE_MOTION_TO_EXIT_SCREENSAVER =>
                {
                    *control_flow = ControlFlow::Exit
                }

                _ => {}
            },

            _ => (),
        }
    });
}

#[cfg(windows)]
fn new_preview_window(
    event_loop: &glutin::event_loop::EventLoop<()>,
    raw_window_handle: &RawWindowHandle,
    settings: &Rc<Settings>,
) -> Result<WindowMode, String> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    let preview_hwnd = match raw_window_handle {
        RawWindowHandle::Win32(handle) => HWND(handle.hwnd as _),
        _ => return Err("This platform is not supported yet".to_string()),
    };

    let mut rect = RECT::default();
    unsafe {
        GetClientRect(preview_hwnd, &mut rect);
    }

    let window_builder = glutin::window::WindowBuilder::new()
        .with_title("Flux Preview")
        .with_parent_window(preview_hwnd.0)
        .with_inner_size(glutin::dpi::Size::Physical(glutin::dpi::PhysicalSize::new(
            rect.right as u32,
            rect.bottom as u32,
        )))
        .with_decorations(false);

    let window = window_builder.build(event_loop).unwrap();

    let context = unsafe {
        use glutin::platform::windows::{RawContextExt, WindowExtWindows};

        let hwnd = window.hwnd();
        glutin::ContextBuilder::new()
            .with_vsync(true)
            .with_gl_profile(glutin::GlProfile::Core)
            .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGl, (3, 3)))
            .with_multisampling(0)
            .with_double_buffer(Some(true))
            .build_raw_context(hwnd)
            .unwrap()
    };

    let context = unsafe { context.make_current().expect("make OpenGL context current") };

    let glow_context =
        unsafe { glow::Context::from_loader_function(|s| context.get_proc_address(s) as *const _) };
    log::debug!("{:?}", glow_context.version());

    let physical_size = window.inner_size();
    let scale_factor = window.scale_factor();
    let logical_size = physical_size.to_logical(scale_factor);
    let flux = Flux::new(
        &Rc::new(glow_context),
        logical_size.width,
        logical_size.height,
        physical_size.width,
        physical_size.height,
        settings,
    )
    .map_err(|err| err.to_string())?;

    let instance = Instance {
        flux,
        context: Takeable::new(context),
    };

    Ok(WindowMode::PreviewWindow {
        window,
        instance: Box::new(instance),
    })
}

fn new_instance(
    event_loop: &glutin::event_loop::EventLoop<()>,
    settings: &Rc<Settings>,
    surface: &surface::Surface,
) -> Result<Instance<glutin::window::Window>, String> {
    let window_builder = glutin::window::WindowBuilder::new()
        .with_title("Flux")
        .with_inner_size(surface.size)
        .with_position(surface.position)
        .with_maximized(true)
        .with_decorations(false);
    let context = glutin::ContextBuilder::new()
        .with_vsync(true)
        .with_gl_profile(glutin::GlProfile::Core)
        .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGl, (3, 3)))
        .with_multisampling(0)
        .with_double_buffer(Some(true))
        .build_windowed(window_builder, event_loop)
        .map_err(|err| err.to_string())?;

    let context = unsafe { context.make_current().expect("make OpenGL context current") };

    context.window().set_cursor_visible(false);

    let glow_context =
        unsafe { glow::Context::from_loader_function(|s| context.get_proc_address(s) as *const _) };
    log::debug!("{:?}", glow_context.version());

    let physical_size = surface.size;
    let logical_size = physical_size.to_logical(surface.scale_factor);
    let flux = Flux::new(
        &Rc::new(glow_context),
        logical_size.width,
        logical_size.height,
        physical_size.width,
        physical_size.height,
        &Default::default(),
    )
    .map_err(|err| err.to_string())?;

    Ok(Instance {
        flux,
        context: Takeable::new(context),
    })
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
