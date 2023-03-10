use std::{path::PathBuf, ptr};
use windows::{core::*, Win32::System::Com::*, Win32::UI::Shell::*};
use winit::monitor::MonitorHandle;
use winit::platform::windows::MonitorHandleExtWindows;

pub fn get(monitor: &MonitorHandle) -> Result<PathBuf> {
    unsafe {
        com_initialized();

        let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)?;

        let wallpaper: PWSTR = desktop.GetWallpaper(&HSTRING::from(monitor.native_id()))?;

        // TODO; check that the path is valid (file exists)

        let path = wallpaper.to_string().unwrap();
        Ok(PathBuf::from(path))
    }
}

// If using winit, COM should already be initalized with COINIT_APRTMENTTHREADED.
struct ComInitialized(*mut ());

impl Drop for ComInitialized {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

thread_local! {
    static COM_INITIALIZED: ComInitialized = {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).expect("initialize COM");
            ComInitialized(ptr::null_mut())
        }
    };
}

pub fn com_initialized() {
    COM_INITIALIZED.with(|_| {});
}
