use std::ffi::{c_char, c_int, c_void};
use std::ptr;
use std::sync::Mutex;

struct SendPtr(*mut c_void);
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

#[cfg(target_os = "windows")]
mod ffi {
    use std::ffi::{c_char, c_int, c_void};

    unsafe extern "C" {
        pub fn webview_win_init() -> c_int;
        pub fn webview_win_shutdown();
        pub fn webview_win_pump();
        pub fn webview_win_create(parent: *mut c_void, url: *const c_char) -> *mut c_void;
        pub fn webview_win_destroy(handle: *mut c_void);
        pub fn webview_win_navigate(handle: *mut c_void, url: *const c_char);
        pub fn webview_win_reload(handle: *mut c_void);
        pub fn webview_win_stop(handle: *mut c_void);
        pub fn webview_win_go_back(handle: *mut c_void);
        pub fn webview_win_go_forward(handle: *mut c_void);
        pub fn webview_win_can_go_back(handle: *mut c_void) -> c_int;
        pub fn webview_win_can_go_forward(handle: *mut c_void) -> c_int;
        pub fn webview_win_is_loading(handle: *mut c_void) -> c_int;
        pub fn webview_win_execute_js(handle: *mut c_void, code: *const c_char);
        pub fn webview_win_set_size(handle: *mut c_void, width: c_int, height: c_int);
        pub fn webview_win_set_bounds(handle: *mut c_void, x: c_int, y: c_int, width: c_int, height: c_int);
        pub fn webview_win_load_html(handle: *mut c_void, html: *const c_char, base_url: *const c_char);
        pub fn webview_win_set_scheme_handler(
            callback: Option<unsafe extern "C" fn(*const c_char, *mut c_void, *mut c_void)>,
            user_data: *mut c_void,
        );
        pub fn webview_win_respond_scheme(
            request_handle: *mut c_void,
            data: *const c_void,
            length: c_int,
            mime_type: *const c_char,
            status_code: c_int,
        );
        pub fn webview_win_set_message_handler(
            handle: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void)>,
            user_data: *mut c_void,
        );
    }
}

#[cfg(target_os = "macos")]
mod ffi {
    use std::ffi::{c_char, c_int, c_void};

    unsafe extern "C" {
        pub fn webview_mac_init() -> c_int;
        pub fn webview_mac_shutdown();
        pub fn webview_mac_pump();
        pub fn webview_mac_create(parent: *mut c_void, url: *const c_char) -> *mut c_void;
        pub fn webview_mac_destroy(handle: *mut c_void);
        pub fn webview_mac_navigate(handle: *mut c_void, url: *const c_char);
        pub fn webview_mac_reload(handle: *mut c_void);
        pub fn webview_mac_stop(handle: *mut c_void);
        pub fn webview_mac_go_back(handle: *mut c_void);
        pub fn webview_mac_go_forward(handle: *mut c_void);
        pub fn webview_mac_can_go_back(handle: *mut c_void) -> c_int;
        pub fn webview_mac_can_go_forward(handle: *mut c_void) -> c_int;
        pub fn webview_mac_is_loading(handle: *mut c_void) -> c_int;
        pub fn webview_mac_execute_js(handle: *mut c_void, code: *const c_char);
        pub fn webview_mac_set_size(handle: *mut c_void, width: c_int, height: c_int);
        pub fn webview_mac_set_bounds(handle: *mut c_void, x: c_int, y: c_int, width: c_int, height: c_int);
        pub fn webview_mac_load_html(handle: *mut c_void, html: *const c_char, base_url: *const c_char);
        pub fn webview_mac_set_scheme_handler(
            callback: Option<unsafe extern "C" fn(*const c_char, *mut c_void, *mut c_void)>,
            user_data: *mut c_void,
        );
        pub fn webview_mac_respond_scheme(
            request_handle: *mut c_void,
            data: *const c_void,
            length: c_int,
            mime_type: *const c_char,
            status_code: c_int,
        );
        pub fn webview_mac_set_message_handler(
            handle: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void)>,
            user_data: *mut c_void,
        );
    }
}

#[cfg(target_os = "linux")]
mod ffi {
    use std::ffi::{c_char, c_int, c_void};

    unsafe extern "C" {
        pub fn webview_linux_init() -> c_int;
        pub fn webview_linux_shutdown();
        pub fn webview_linux_pump();
        pub fn webview_linux_create(parent: *mut c_void, url: *const c_char) -> *mut c_void;
        pub fn webview_linux_destroy(handle: *mut c_void);
        pub fn webview_linux_navigate(handle: *mut c_void, url: *const c_char);
        pub fn webview_linux_reload(handle: *mut c_void);
        pub fn webview_linux_stop(handle: *mut c_void);
        pub fn webview_linux_go_back(handle: *mut c_void);
        pub fn webview_linux_go_forward(handle: *mut c_void);
        pub fn webview_linux_can_go_back(handle: *mut c_void) -> c_int;
        pub fn webview_linux_can_go_forward(handle: *mut c_void) -> c_int;
        pub fn webview_linux_is_loading(handle: *mut c_void) -> c_int;
        pub fn webview_linux_execute_js(handle: *mut c_void, code: *const c_char);
        pub fn webview_linux_set_size(handle: *mut c_void, width: c_int, height: c_int);
        pub fn webview_linux_set_bounds(handle: *mut c_void, x: c_int, y: c_int, width: c_int, height: c_int);
        pub fn webview_linux_load_html(handle: *mut c_void, html: *const c_char, base_url: *const c_char);
        pub fn webview_linux_set_scheme_handler(
            callback: Option<unsafe extern "C" fn(*const c_char, *mut c_void, *mut c_void)>,
            user_data: *mut c_void,
        );
        pub fn webview_linux_respond_scheme(
            request_handle: *mut c_void,
            data: *const c_void,
            length: c_int,
            mime_type: *const c_char,
            status_code: c_int,
        );
        pub fn webview_linux_set_message_handler(
            handle: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void)>,
            user_data: *mut c_void,
        );
    }
}

type SchemeCallbackFn = unsafe extern "C" fn(*const c_char, *mut c_void, *mut c_void);

static SCHEME_CALLBACK: Mutex<Option<(SchemeCallbackFn, SendPtr)>> = Mutex::new(None);

// Callback trampoline to forward to C#
unsafe extern "C" fn scheme_trampoline(url: *const c_char, request_handle: *mut c_void, _user_data: *mut c_void) {
    if let Ok(guard) = SCHEME_CALLBACK.lock() {
        if let Some((callback, ref user_data)) = *guard {
            unsafe { callback(url, request_handle, user_data.0); }
        }
    }
}

// --- Public C API ---

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_init() -> c_int {
    #[cfg(target_os = "windows")]
    { return unsafe { ffi::webview_win_init() }; }

    #[cfg(target_os = "macos")]
    { return unsafe { ffi::webview_mac_init() }; }

    #[cfg(target_os = "linux")]
    { return unsafe { ffi::webview_linux_init() }; }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    { -1 }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_shutdown() {
    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_shutdown(); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_shutdown(); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_shutdown(); }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_pump() {
    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_pump(); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_pump(); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_pump(); }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_create(parent: *mut c_void, url: *const c_char) -> *mut c_void {
    #[cfg(target_os = "windows")]
    { return unsafe { ffi::webview_win_create(parent, url) }; }

    #[cfg(target_os = "macos")]
    { return unsafe { ffi::webview_mac_create(parent, url) }; }

    #[cfg(target_os = "linux")]
    { return unsafe { ffi::webview_linux_create(parent, url) }; }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    { ptr::null_mut() }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_destroy(handle: *mut c_void) {
    if handle.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_destroy(handle); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_destroy(handle); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_destroy(handle); }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_navigate(handle: *mut c_void, url: *const c_char) {
    if handle.is_null() || url.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_navigate(handle, url); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_navigate(handle, url); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_navigate(handle, url); }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_reload(handle: *mut c_void) {
    if handle.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_reload(handle); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_reload(handle); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_reload(handle); }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_stop(handle: *mut c_void) {
    if handle.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_stop(handle); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_stop(handle); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_stop(handle); }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_go_back(handle: *mut c_void) {
    if handle.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_go_back(handle); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_go_back(handle); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_go_back(handle); }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_go_forward(handle: *mut c_void) {
    if handle.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_go_forward(handle); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_go_forward(handle); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_go_forward(handle); }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_can_go_back(handle: *mut c_void) -> bool {
    if handle.is_null() { return false; }

    #[cfg(target_os = "windows")]
    { return unsafe { ffi::webview_win_can_go_back(handle) != 0 }; }

    #[cfg(target_os = "macos")]
    { return unsafe { ffi::webview_mac_can_go_back(handle) != 0 }; }

    #[cfg(target_os = "linux")]
    { return unsafe { ffi::webview_linux_can_go_back(handle) != 0 }; }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    { false }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_can_go_forward(handle: *mut c_void) -> bool {
    if handle.is_null() { return false; }

    #[cfg(target_os = "windows")]
    { return unsafe { ffi::webview_win_can_go_forward(handle) != 0 }; }

    #[cfg(target_os = "macos")]
    { return unsafe { ffi::webview_mac_can_go_forward(handle) != 0 }; }

    #[cfg(target_os = "linux")]
    { return unsafe { ffi::webview_linux_can_go_forward(handle) != 0 }; }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    { false }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_is_loading(handle: *mut c_void) -> bool {
    if handle.is_null() { return false; }

    #[cfg(target_os = "windows")]
    { return unsafe { ffi::webview_win_is_loading(handle) != 0 }; }

    #[cfg(target_os = "macos")]
    { return unsafe { ffi::webview_mac_is_loading(handle) != 0 }; }

    #[cfg(target_os = "linux")]
    { return unsafe { ffi::webview_linux_is_loading(handle) != 0 }; }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    { false }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_execute_js(handle: *mut c_void, code: *const c_char) {
    if handle.is_null() || code.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_execute_js(handle, code); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_execute_js(handle, code); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_execute_js(handle, code); }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_set_size(handle: *mut c_void, width: c_int, height: c_int) {
    if handle.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_set_size(handle, width, height); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_set_size(handle, width, height); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_set_size(handle, width, height); }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_load_html(handle: *mut c_void, html: *const c_char, base_url: *const c_char) {
    if handle.is_null() || html.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_load_html(handle, html, base_url); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_load_html(handle, html, base_url); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_load_html(handle, html, base_url); }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_set_bounds(handle: *mut c_void, x: c_int, y: c_int, width: c_int, height: c_int) {
    if handle.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_set_bounds(handle, x, y, width, height); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_set_bounds(handle, x, y, width, height); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_set_bounds(handle, x, y, width, height); }
}

pub type SchemeCallback = unsafe extern "C" fn(*const c_char, *mut c_void, *mut c_void);

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_set_scheme_handler(
    callback: Option<SchemeCallback>,
    user_data: *mut c_void,
) {
    if let Some(cb) = callback {
        if let Ok(mut guard) = SCHEME_CALLBACK.lock() {
            *guard = Some((cb, SendPtr(user_data)));
        }

        #[cfg(target_os = "windows")]
        unsafe { ffi::webview_win_set_scheme_handler(Some(scheme_trampoline), ptr::null_mut()); }

        #[cfg(target_os = "macos")]
        unsafe { ffi::webview_mac_set_scheme_handler(Some(scheme_trampoline), ptr::null_mut()); }

        #[cfg(target_os = "linux")]
        unsafe { ffi::webview_linux_set_scheme_handler(Some(scheme_trampoline), ptr::null_mut()); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_respond_scheme(
    request_handle: *mut c_void,
    data: *const c_void,
    length: c_int,
    mime_type: *const c_char,
    status_code: c_int,
) {
    if request_handle.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_respond_scheme(request_handle, data, length, mime_type, status_code); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_respond_scheme(request_handle, data, length, mime_type, status_code); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_respond_scheme(request_handle, data, length, mime_type, status_code); }
}

pub type MessageCallback = unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void);

#[unsafe(no_mangle)]
pub extern "C" fn robust_webview_set_message_handler(
    handle: *mut c_void,
    callback: Option<MessageCallback>,
    user_data: *mut c_void,
) {
    if handle.is_null() { return; }

    #[cfg(target_os = "windows")]
    unsafe { ffi::webview_win_set_message_handler(handle, callback, user_data); }

    #[cfg(target_os = "macos")]
    unsafe { ffi::webview_mac_set_message_handler(handle, callback, user_data); }

    #[cfg(target_os = "linux")]
    unsafe { ffi::webview_linux_set_message_handler(handle, callback, user_data); }
}
