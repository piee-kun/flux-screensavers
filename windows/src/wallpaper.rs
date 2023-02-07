use glutin::monitor::MonitorHandle;
use glutin::platform::windows::MonitorHandleExtWindows;
use std::path::PathBuf;
use windows::{core::*, Win32::System::Com::*, Win32::UI::Shell::*};

pub fn get(monitor: &MonitorHandle) -> Result<PathBuf> {
    unsafe {
        // Should already be initialized by winit
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)?;

        let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)?;

        let wallpaper: PWSTR = desktop.GetWallpaper(&HSTRING::from(monitor.native_id()))?;

        // TODO; check that the path is valid (file exists)

        // CoUninitialize();

        let path = wallpaper.to_string().unwrap();
        Ok(PathBuf::from(path))
    }
}
