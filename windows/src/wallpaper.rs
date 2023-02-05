use glutin::monitor::MonitorHandle;
use glutin::platform::windows::MonitorHandleExtWindows;
use std::path::PathBuf;
use windows::{core::*, Win32::System::Com::*, Win32::UI::Shell::*};

pub fn get(monitor: &MonitorHandle) -> Result<PathBuf> {
    unsafe {
        // Should already be initialized by winit
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)?;

        let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)?;

        log::debug!(
            "Getting wallpaper {:?} {} {}",
            monitor.name(),
            monitor.hmonitor(),
            monitor.native_id()
        );
        let monitor_id = desktop.GetMonitorDevicePathAt(0)?;
        log::debug!("{:?}", *monitor_id.0);
        let wallpaper: PWSTR = desktop.GetWallpaper(PCWSTR(monitor.native_id().as_ptr()))?;

        // TODO; check that the path is valid (file exists)

        CoUninitialize();

        let path = wallpaper.to_string().unwrap();
        Ok(PathBuf::from(path))
    }
}
