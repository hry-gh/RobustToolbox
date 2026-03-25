// TODO: Implement using webkit2gtk crate
use std::ffi::{c_char, c_int, c_void};
use crate::platform::*;

pub fn init() -> c_int { -1 }
pub fn shutdown() {}
pub fn pump() {}
pub fn create(_parent: *mut c_void, _url: *const c_char) -> *mut c_void { std::ptr::null_mut() }
pub fn destroy(_handle: *mut c_void) {}
pub fn navigate(_handle: *mut c_void, _url: *const c_char) {}
pub fn reload(_handle: *mut c_void) {}
pub fn stop(_handle: *mut c_void) {}
pub fn go_back(_handle: *mut c_void) {}
pub fn go_forward(_handle: *mut c_void) {}
pub fn can_go_back(_handle: *mut c_void) -> bool { false }
pub fn can_go_forward(_handle: *mut c_void) -> bool { false }
pub fn is_loading(_handle: *mut c_void) -> bool { false }
pub fn execute_js(_handle: *mut c_void, _code: *const c_char) {}
pub fn set_size(_handle: *mut c_void, _w: c_int, _h: c_int) {}
pub fn set_bounds(_handle: *mut c_void, _x: c_int, _y: c_int, _w: c_int, _h: c_int) {}
pub fn load_html(_handle: *mut c_void, _html: *const c_char, _base_url: *const c_char) {}
pub fn set_scheme_handler(_cb: Option<SchemeCallbackFn>, _ud: *mut c_void) {}
pub fn respond_scheme(_rh: *mut c_void, _data: *const c_void, _len: c_int, _mime: *const c_char, _status: c_int) {}
pub fn set_message_handler(_handle: *mut c_void, _cb: Option<MessageCallbackFn>, _ud: *mut c_void) {}
pub fn set_before_browse_handler(_cb: Option<BeforeBrowseCallbackFn>, _ud: *mut c_void) {}
