use std::collections::vec_deque;
use std::iter::Map;

use sdl2::video::Window;
use sdl2::VideoSubsystem;

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use winit::dpi::PhysicalSize;
pub use winit::monitor::MonitorHandle;
use winit::platform_impl::platform;

pub trait HasWinitWindow {
    fn inner_size(&self) -> PhysicalSize<u32>;
    fn scale_factor(&self) -> f64;
    fn current_monitor(&self) -> Option<MonitorHandle>;
}

impl HasWinitWindow for Window {
    fn inner_size(&self) -> PhysicalSize<u32> {
        let (w, h) = self.size();
        PhysicalSize::new(w, h)
    }

    fn scale_factor(&self) -> f64 {
        let id = self.display_index().unwrap();
        self.subsystem().display_dpi(id).unwrap().0 as f64 / 96.0
    }

    fn current_monitor(&self) -> Option<MonitorHandle> {
        match self.raw_window_handle() {
            RawWindowHandle::Win32(handle) => {
                let inner = platform::monitor::current_monitor(handle.hwnd as _);
                Some(MonitorHandle { inner })
            }
            _ => None,
        }
    }
}

pub trait HasMonitors {
    type Iter: Iterator<Item = MonitorHandle>;

    fn available_monitors(&self) -> Self::Iter;
}

impl HasMonitors for VideoSubsystem {
    type Iter = Map<
        vec_deque::IntoIter<platform::monitor::MonitorHandle>,
        fn(platform::monitor::MonitorHandle) -> MonitorHandle,
    >;

    fn available_monitors(&self) -> Self::Iter {
        platform::monitor::available_monitors()
            .into_iter()
            .map(|inner| MonitorHandle { inner })
    }
}
