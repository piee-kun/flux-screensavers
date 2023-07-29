use raw_window_handle::RawWindowHandle;
use windows::Win32::Foundation::HWND;

pub unsafe fn set_window_parent_win32(handle: HWND, parent_handle: HWND) -> bool {
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

pub unsafe fn enable_transparency(handle: &RawWindowHandle) {
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
