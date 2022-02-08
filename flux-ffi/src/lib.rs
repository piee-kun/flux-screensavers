use flux::{settings::Settings, Flux};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::rc::Rc;

#[no_mangle]
pub extern "C" fn flux_new(
    width: f32,
    height: f32,
    pixel_ratio: f64,
    settings_json_ptr: *const c_char,
) -> *mut Flux {
    let raw_context = unsafe { glow::Context::from_loader_function(|addr| get_proc_address(addr)) };
    let context = Box::new(Rc::new(raw_context));

    // TODO: can you do error handling with FFI?
    let settings_c_str = unsafe { CStr::from_ptr(settings_json_ptr) };
    let settings_slice = settings_c_str.to_str().unwrap();
    let settings: Settings = serde_json::from_str(&settings_slice).unwrap();
    let settings = Box::new(Rc::new(settings));

    let raw_ptr = Box::into_raw(Box::new(
        Flux::new(
            &context,
            width as u32,
            height as u32,
            pixel_ratio,
            &settings,
        )
        .unwrap(),
    ));
    raw_ptr
}

#[no_mangle]
pub unsafe extern "C" fn flux_animate(ptr: *mut Flux, timestamp: f32) {
    let flux = &mut *ptr;
    flux.animate(timestamp);

    // TODO: do we need to call forget?
    // std::mem::forget(flux);
}

#[no_mangle]
pub unsafe extern "C" fn flux_resize(ptr: *mut Flux, logical_width: f32, logical_height: f32) {
    let flux = &mut *ptr;
    flux.resize(logical_width as u32, logical_height as u32);
}

#[no_mangle]
pub unsafe extern "C" fn flux_destroy(ptr: *mut Flux) {
    let _flux = Box::from_raw(ptr);
    // Drop
}

#[cfg(target_os = "macos")]
pub fn get_proc_address(addr: &str) -> *const core::ffi::c_void {
    use core_foundation::base::TCFType;
    use core_foundation::bundle::{
        CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName,
    };
    use core_foundation::string::CFString;
    use std::str::FromStr;

    let symbol_name: CFString = FromStr::from_str(addr).unwrap();
    let framework_name: CFString = FromStr::from_str("com.apple.opengl").unwrap();
    let framework =
        unsafe { CFBundleGetBundleWithIdentifier(framework_name.as_concrete_TypeRef()) };
    let symbol =
        unsafe { CFBundleGetFunctionPointerForName(framework, symbol_name.as_concrete_TypeRef()) };
    symbol as *const _
}
