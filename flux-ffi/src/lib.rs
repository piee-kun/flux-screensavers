use flux::{settings::Settings, Flux};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::rc::Rc;

fn init_flux(
    logical_width: f32,
    logical_height: f32,
    physical_width: f32,
    physical_height: f32,
    settings_json_ptr: *const c_char,
) -> Result<Flux, String> {
    let raw_context = unsafe { glow::Context::from_loader_function(|addr| get_proc_address(addr)) };
    let context = Box::new(Rc::new(raw_context));

    let settings_json = unsafe { CStr::from_ptr(settings_json_ptr) }
        .to_str()
        .map_err(|err| err.to_string())?;
    let settings: Settings = serde_json::from_str(&settings_json).map_err(|err| err.to_string())?;
    let settings = Box::new(Rc::new(settings));

    Flux::new(
        &context,
        logical_width as u32,
        logical_height as u32,
        physical_width as u32,
        physical_height as u32,
        &settings,
    )
    .map_err(|err| err.to_string())
}

#[no_mangle]
pub extern "C" fn flux_new(
    logical_width: f32,
    logical_height: f32,
    physical_width: f32,
    physical_height: f32,
    settings_json_ptr: *const c_char,
) -> *mut Flux {
    let flux = match init_flux(
        logical_width,
        logical_height,
        physical_width,
        physical_height,
        settings_json_ptr,
    ) {
        Err(_msg) => {
            // TODO: log stuff
            return std::ptr::null_mut();
        }
        Ok(flux) => flux,
    };

    Box::into_raw(Box::new(flux))
}

#[no_mangle]
pub unsafe extern "C" fn flux_animate(flux: *mut Flux, timestamp: f64) {
    (&mut *flux).animate(timestamp);
}

#[no_mangle]
pub unsafe extern "C" fn flux_resize(
    flux: *mut Flux,
    logical_width: f32,
    logical_height: f32,
    physical_width: f32,
    physical_height: f32,
) {
    (&mut *flux).resize(
        logical_width as u32,
        logical_height as u32,
        physical_width as u32,
        physical_height as u32,
    );
}

#[no_mangle]
pub unsafe extern "C" fn flux_destroy(flux: *mut Flux) {
    if !flux.is_null() {
        drop(Box::from_raw(flux));
    }
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
