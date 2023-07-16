use crate::winit_compat::NonZeroU32PhysicalSize;

use std::ffi::CString;
use std::fmt;
use std::rc::Rc;

use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use winit::dpi::PhysicalSize;

use glow as GL;
use glow::HasContext;
use glutin::config::{ColorBufferType, Config as GLConfig, ConfigTemplateBuilder};
use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version};
use glutin::display::{Display, DisplayApiPreference, GetGlDisplay};
use glutin::prelude::*;
use glutin::surface::{Surface, SurfaceAttributesBuilder, WindowSurface};

pub struct GLContext {
    pub context: PossiblyCurrentContext,
    pub surface: Surface<WindowSurface>,
    pub gl: Rc<glow::Context>,
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
pub(crate) fn new_gl_context(
    raw_display_handle: RawDisplayHandle,
    inner_size: PhysicalSize<u32>,

    raw_window_handle: RawWindowHandle,
    // A hack to create the gl_display using the invisible event window
    // we create for the preview.
    attr_window: Option<RawWindowHandle>,
) -> GLContext {
    let template = ConfigTemplateBuilder::new()
        .with_buffer_type(glutin::config::ColorBufferType::Rgb {
            r_size: 8,
            g_size: 8,
            b_size: 8,
        })
        .with_alpha_size(8)
        .with_transparency(true)
        .compatible_with_native_window(raw_window_handle)
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

    let (width, height) = inner_size.non_zero().expect("non-zero window size");
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

    let glow_context = unsafe {
        glow::Context::from_loader_function(|s| {
            gl_display.get_proc_address(CString::new(s).unwrap().as_c_str()) as *const _
        })
    };
    log::debug!("{:?}", glow_context.version());

    // Set common GL state
    unsafe {
        glow_context.disable(GL::MULTISAMPLE);
    }

    GLContext {
        context: gl_context,
        surface: gl_surface,
        gl: Rc::new(glow_context),
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
