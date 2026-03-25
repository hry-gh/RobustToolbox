use std::ffi::{c_char, c_int, c_void};

pub type SchemeCallbackFn = unsafe extern "C" fn(*const c_char, *mut c_void, *mut c_void);
pub type BeforeBrowseCallbackFn =
    unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int;
pub type MessageCallbackFn = unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void);

/// Convert a Box'd instance into an opaque handle.
pub fn to_handle<T>(instance: Box<T>) -> *mut c_void {
    Box::into_raw(instance) as *mut c_void
}

/// Get a reference from an opaque handle. Caller must ensure handle is valid.
pub unsafe fn from_handle<'a, T>(handle: *mut c_void) -> &'a mut T {
    unsafe { &mut *(handle as *mut T) }
}

/// Drop an instance from an opaque handle.
pub unsafe fn drop_handle<T>(handle: *mut c_void) {
    unsafe {
        let _ = Box::from_raw(handle as *mut T);
    }
}
