use raw_window_handle::RawWindowHandle;
use std::ffi::c_void;

#[derive(PartialEq)]
pub enum Mode {
    Preview(RawWindowHandle),
    Screensaver,
    Settings,
}

pub fn read_flags() -> Result<Mode, String> {
    match std::env::args().nth(1).as_mut().map(|s| {
        s.make_ascii_lowercase();
        s.as_str()
    }) {
        // Settings panel
        //
        // /c -> you’re supposed to support this, but AFAIK the only way to get
        // this is to manually send it from the command line.
        //
        // /c:HWND -> the screensaver configuration window gives a window
        // handle. I’m not sure what it’s for. Maybe you’re supposed to use it
        // to close your settings window if the parent windows closes?
        //
        // No flags -> <right click + configure> sends no flags whatsoever.
        Some("/c") => Ok(Mode::Settings),
        Some(s) if s.starts_with("/c:") => Ok(Mode::Settings),

        // Run screensaver
        //
        // /s -> run the screensaver.
        //
        // /S -> <right click + test> sends an uppercase /S, which doesn’t
        // seem to be documented anywhere.
        Some("/s") | None => Ok(Mode::Screensaver),

        // Run preview
        //
        // /p HWND -> draw the screensaver in the preview window.
        //
        // /p:HWND -> TODO: apparently, this is also an option you need to
        // support.
        Some("/p") => {
            let handle_ptr = std::env::args()
                .nth(2)
                .ok_or("I can’t find the window to show the screensaver preview.")?
                .parse::<usize>()
                .map_err(|e| e.to_string())?;

            let mut handle = raw_window_handle::Win32Handle::empty();
            handle.hwnd = handle_ptr as *mut c_void;
            Ok(Mode::Preview(RawWindowHandle::Win32(handle)))
        }

        Some(s) => {
            return Err(format!("I don’t know what the argument {} is.", s));
        }
    }
}
