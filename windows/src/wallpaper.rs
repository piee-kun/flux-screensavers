use std::path::PathBuf;
use windows::{core::*, Win32::System::Com::*, Win32::UI::Shell::*};

pub fn get() -> Result<PathBuf> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)?;

        let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)?;

        let wallpaper: PWSTR = desktop.GetWallpaper(PCWSTR(0 as _))?;

        // TODO; check that the path is valid (file exists)

        CoUninitialize();

        let path = wallpaper.to_string().unwrap();
        Ok(PathBuf::from(path))
    }
}
