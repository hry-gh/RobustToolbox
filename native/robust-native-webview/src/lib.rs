mod platform;

#[cfg(target_os = "windows")]
mod webview_win;
#[cfg(target_os = "macos")]
mod webview_mac;
#[cfg(target_os = "linux")]
mod webview_linux;

#[cfg(target_os = "windows")]
use webview_win as imp;
#[cfg(target_os = "macos")]
use webview_mac as imp;
#[cfg(target_os = "linux")]
use webview_linux as imp;

use std::ffi::{c_char, c_int, c_void};

// --- Public C API ---

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_init() -> c_int {
    imp::init()
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_shutdown() {
    imp::shutdown();
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_pump() {
    imp::pump();
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_create(parent: *mut c_void, url: *const c_char) -> *mut c_void {
    imp::create(parent, url)
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_destroy(handle: *mut c_void) {
    if !handle.is_null() {
        imp::destroy(handle);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_navigate(handle: *mut c_void, url: *const c_char) {
    if !handle.is_null() && !url.is_null() {
        imp::navigate(handle, url);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_reload(handle: *mut c_void) {
    if !handle.is_null() {
        imp::reload(handle);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_stop(handle: *mut c_void) {
    if !handle.is_null() {
        imp::stop(handle);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_go_back(handle: *mut c_void) {
    if !handle.is_null() {
        imp::go_back(handle);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_go_forward(handle: *mut c_void) {
    if !handle.is_null() {
        imp::go_forward(handle);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_can_go_back(handle: *mut c_void) -> bool {
    if handle.is_null() { return false; }
    imp::can_go_back(handle)
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_can_go_forward(handle: *mut c_void) -> bool {
    if handle.is_null() { return false; }
    imp::can_go_forward(handle)
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_is_loading(handle: *mut c_void) -> bool {
    if handle.is_null() { return false; }
    imp::is_loading(handle)
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_execute_js(handle: *mut c_void, code: *const c_char) {
    if !handle.is_null() && !code.is_null() {
        imp::execute_js(handle, code);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_set_size(handle: *mut c_void, width: c_int, height: c_int) {
    if !handle.is_null() {
        imp::set_size(handle, width, height);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_set_bounds(
    handle: *mut c_void,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
) {
    if !handle.is_null() {
        imp::set_bounds(handle, x, y, width, height);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_load_html(
    handle: *mut c_void,
    html: *const c_char,
    base_url: *const c_char,
) {
    if !handle.is_null() && !html.is_null() {
        imp::load_html(handle, html, base_url);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_set_scheme_handler(
    callback: Option<platform::SchemeCallbackFn>,
    user_data: *mut c_void,
) {
    imp::set_scheme_handler(callback, user_data);
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_respond_scheme(
    request_handle: *mut c_void,
    data: *const c_void,
    length: c_int,
    mime_type: *const c_char,
    status_code: c_int,
) {
    if !request_handle.is_null() {
        imp::respond_scheme(request_handle, data, length, mime_type, status_code);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_set_message_handler(
    handle: *mut c_void,
    callback: Option<platform::MessageCallbackFn>,
    user_data: *mut c_void,
) {
    if !handle.is_null() {
        imp::set_message_handler(handle, callback, user_data);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_set_before_browse_handler(
    callback: Option<platform::BeforeBrowseCallbackFn>,
    user_data: *mut c_void,
) {
    imp::set_before_browse_handler(callback, user_data);
}
