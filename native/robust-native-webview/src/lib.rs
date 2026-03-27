mod app;
mod client;
mod ffi_types;
mod life_span_handler;
mod load_handler;
mod render_handler;
mod request_handler;
mod resource_handler;
mod scheme_handler;
mod state;

use std::ffi::{CStr, CString, c_char};
use std::ptr;
use std::sync::Arc;

use cef::string::CefStringUtf8;
use cef::*;

use ffi_types::{
    RNW_ERROR, RNW_HANDLE_INVALID, ResponseContext, RnwBrowserCallbacks, RnwKeyEvent, RnwSettings,
};
use render_handler::CallbackData;
use state::{GLOBAL, GlobalState, get_browser, with_state};

unsafe fn cstr_to_string(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .ok()
        .map(|s| s.to_owned())
}

unsafe fn cstr_to_cef_string(p: *const c_char) -> CefString {
    if p.is_null() {
        return CefString::default();
    }
    match unsafe { CStr::from_ptr(p) }.to_str() {
        Ok(s) => CefString::from(s),
        Err(_) => CefString::default(),
    }
}

pub(crate) fn cef_userfree_to_cstring(s: &CefStringUserfree, default: &str) -> Option<CString> {
    let utf16 = CefStringUtf16::from(s);
    let utf8 = CefStringUtf8::from(&utf16);
    CString::new(utf8.as_str().unwrap_or(default)).ok()
}

/// Initializes the native webview with the provided settings.
///
/// # Safety
/// The `settings` pointer must be valid and point to a properly initialized `RnwSettings` struct.
/// Passing a null or invalid pointer will result in undefined behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_initialize(settings: *const RnwSettings) -> i32 {
    if settings.is_null() {
        return RNW_ERROR;
    }
    let s = unsafe { &*settings };

    #[cfg(target_os = "macos")]
    {
        if !s.framework_path.is_null() {
            let Some(path_str) = (unsafe { cstr_to_string(s.framework_path) }) else {
                eprintln!("[rnw] framework_path is not valid UTF-8");
                return RNW_ERROR;
            };
            let path = std::path::PathBuf::from(path_str);
            use std::os::unix::ffi::OsStrExt;
            let Ok(cstr) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
                eprintln!("[rnw] framework_path contains null bytes");
                return RNW_ERROR;
            };
            let result = cef::load_library(Some(unsafe { &*cstr.as_ptr().cast() }));
            if result != 1 {
                eprintln!("[rnw] Failed to load CEF framework from provided path");
                return RNW_ERROR;
            }
        } else {
            let Ok(exe) = std::env::current_exe() else {
                eprintln!("[rnw] Failed to get current exe path");
                return RNW_ERROR;
            };
            let loader = cef::library_loader::LibraryLoader::new(&exe, false);
            if !loader.load() {
                eprintln!("[rnw] Failed to load CEF framework via LibraryLoader");
                return RNW_ERROR;
            }
            // SAFETY: The loader must outlive the CEF process. We intentionally leak it
            // rather than dropping, which would unload the framework while CEF is running.
            std::mem::forget(loader);
        }

        // Set DYLD_FALLBACK_LIBRARY_PATH so CEF subprocesses can find ANGLE libs
        // (libGLESv2.dylib, libEGL.dylib) inside the framework bundle.
        if !s.framework_dir_path.is_null()
            && let Some(fw_dir) = unsafe { cstr_to_string(s.framework_dir_path) }
        {
            let libs_dir = std::path::PathBuf::from(&fw_dir).join("Libraries");
            unsafe { std::env::set_var("DYLD_FALLBACK_LIBRARY_PATH", &libs_dir) };
        }
    }

    cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);

    let args = cef::args::Args::new();

    let mut cef_settings = Settings {
        size: std::mem::size_of::<cef::sys::_cef_settings_t>(),
        windowless_rendering_enabled: 1,
        external_message_pump: 1,
        no_sandbox: if s.no_sandbox != 0 { 1 } else { 0 },
        remote_debugging_port: s.remote_debugging_port,
        ..Default::default()
    };

    if !s.subprocess_path.is_null() {
        cef_settings.browser_subprocess_path = unsafe { cstr_to_cef_string(s.subprocess_path) };
    }
    if !s.resources_dir_path.is_null() {
        cef_settings.resources_dir_path = unsafe { cstr_to_cef_string(s.resources_dir_path) };
    }
    if !s.locales_dir_path.is_null() {
        cef_settings.locales_dir_path = unsafe { cstr_to_cef_string(s.locales_dir_path) };
    }
    if !s.cache_path.is_null() {
        cef_settings.cache_path = unsafe { cstr_to_cef_string(s.cache_path) };
    }
    if !s.user_agent.is_null() {
        cef_settings.user_agent = unsafe { cstr_to_cef_string(s.user_agent) };
    }
    if !s.cookieable_schemes.is_null() {
        cef_settings.cookieable_schemes_list = unsafe { cstr_to_cef_string(s.cookieable_schemes) };
    }
    if !s.framework_dir_path.is_null() {
        cef_settings.framework_dir_path = unsafe { cstr_to_cef_string(s.framework_dir_path) };
    }
    if !s.main_bundle_path.is_null() {
        cef_settings.main_bundle_path = unsafe { cstr_to_cef_string(s.main_bundle_path) };
    }

    let mut cef_app = app::create_app();

    let result = cef::initialize(
        Some(args.as_main_args()),
        Some(&cef_settings),
        Some(&mut cef_app),
        ptr::null_mut(),
    );

    if let Ok(mut guard) = GLOBAL.lock() {
        *guard = Some(GlobalState::new());
    }

    result
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_do_message_loop_work() {
    cef::do_message_loop_work();
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_shutdown() {
    if let Ok(mut guard) = GLOBAL.lock() {
        *guard = None;
    }
    cef::shutdown();
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_flush_cookies() {
    if let Some(manager) = cef::cookie_manager_get_global_manager(None) {
        manager.flush_store(None);
    }
}

/// Creates a new offscreen browser.
///
/// # Safety
/// - `url` must be null or a valid null-terminated C string.
/// - `callbacks` must be a valid pointer to an initialized `RnwBrowserCallbacks` struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_browser_create(
    url: *const c_char,
    callbacks: *const RnwBrowserCallbacks,
) -> u64 {
    if callbacks.is_null() {
        return RNW_HANDLE_INVALID;
    }
    let cbs = unsafe { *callbacks };
    let url_str = unsafe { cstr_to_string(url) }.unwrap_or_else(|| "about:blank".to_string());

    let data = Arc::new(CallbackData { callbacks: cbs });
    let mut cef_client = client::create_client(data);

    let window_info = WindowInfo {
        size: std::mem::size_of::<cef::sys::_cef_window_info_t>(),
        windowless_rendering_enabled: 1,
        ..Default::default()
    };

    let browser_settings = BrowserSettings {
        size: std::mem::size_of::<cef::sys::_cef_browser_settings_t>(),
        windowless_frame_rate: 60,
        ..Default::default()
    };

    let cef_url = CefString::from(url_str.as_str());

    let browser = cef::browser_host_create_browser_sync(
        Some(&window_info),
        Some(&mut cef_client),
        Some(&cef_url),
        Some(&browser_settings),
        None,
        None,
    );

    match browser {
        Some(browser) => {
            with_state(|state| state.insert_browser(browser)).unwrap_or(RNW_HANDLE_INVALID)
        }
        None => RNW_HANDLE_INVALID,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_close(handle: u64) {
    let browser = with_state(|state| state.browsers.remove(&handle));
    if let Some(Some(browser)) = browser
        && let Some(host) = browser.host()
    {
        host.close_browser(1);
    }
}

/// Gets the current URL of the browser.
///
/// # Safety
/// `buf` must be null or a valid pointer to a writable buffer of at least `buf_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_browser_get_url(handle: u64, buf: *mut c_char, buf_len: i32) -> i32 {
    let Some(browser) = get_browser(handle) else {
        return RNW_ERROR;
    };
    let Some(frame) = browser.main_frame() else {
        return RNW_ERROR;
    };
    let url_userfree = frame.url();
    let url_utf16 = CefStringUtf16::from(&url_userfree);
    let url_utf8 = CefStringUtf8::from(&url_utf16);
    let url = url_utf8.as_str().unwrap_or("");
    let bytes = url.as_bytes();
    let copy_len = bytes.len().min((buf_len as usize).saturating_sub(1));
    if !buf.is_null() && buf_len > 0 {
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy_len);
            *buf.add(copy_len) = 0
        };
    }
    bytes.len() as i32
}

/// Navigates the browser to the specified URL.
///
/// # Safety
/// `url` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_browser_load_url(handle: u64, url: *const c_char) {
    let Some(browser) = get_browser(handle) else {
        return;
    };
    let cef_url = unsafe { cstr_to_cef_string(url) };
    if let Some(frame) = browser.main_frame() {
        frame.load_url(Some(&cef_url));
    }
}

/// Executes JavaScript code in the browser.
///
/// # Safety
/// `code` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_browser_execute_js(handle: u64, code: *const c_char) {
    let Some(browser) = get_browser(handle) else {
        return;
    };
    let cef_code = unsafe { cstr_to_cef_string(code) };
    let empty = CefString::from("");
    if let Some(frame) = browser.main_frame() {
        frame.execute_java_script(Some(&cef_code), Some(&empty), 1);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_is_loading(handle: u64) -> i32 {
    get_browser(handle).map(|b| b.is_loading()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_can_go_back(handle: u64) -> i32 {
    get_browser(handle).map(|b| b.can_go_back()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_can_go_forward(handle: u64) -> i32 {
    get_browser(handle).map(|b| b.can_go_forward()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_go_back(handle: u64) {
    if let Some(b) = get_browser(handle) {
        b.go_back();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_go_forward(handle: u64) {
    if let Some(b) = get_browser(handle) {
        b.go_forward();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_reload(handle: u64) {
    if let Some(b) = get_browser(handle) {
        b.reload();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_stop_load(handle: u64) {
    if let Some(b) = get_browser(handle) {
        b.stop_load();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_send_mouse_move(
    handle: u64,
    x: i32,
    y: i32,
    modifiers: u32,
    mouse_leave: i32,
) {
    let Some(browser) = get_browser(handle) else {
        return;
    };
    if let Some(host) = browser.host() {
        host.send_mouse_move_event(Some(&MouseEvent { x, y, modifiers }), mouse_leave);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_send_mouse_click(
    handle: u64,
    x: i32,
    y: i32,
    modifiers: u32,
    button: i32,
    mouse_up: i32,
    click_count: i32,
) {
    let Some(browser) = get_browser(handle) else {
        return;
    };
    if let Some(host) = browser.host() {
        let button_type = match button {
            1 => MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_MIDDLE),
            2 => MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_RIGHT),
            _ => MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_LEFT),
        };
        host.send_mouse_click_event(
            Some(&MouseEvent { x, y, modifiers }),
            button_type,
            mouse_up,
            click_count,
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_send_mouse_wheel(
    handle: u64,
    x: i32,
    y: i32,
    modifiers: u32,
    delta_x: i32,
    delta_y: i32,
) {
    let Some(browser) = get_browser(handle) else {
        return;
    };
    if let Some(host) = browser.host() {
        host.send_mouse_wheel_event(Some(&MouseEvent { x, y, modifiers }), delta_x, delta_y);
    }
}

/// Sends a key event to the browser.
///
/// # Safety
/// `event` must be null or a valid pointer to an initialized `RnwKeyEvent` struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_browser_send_key_event(handle: u64, event: *const RnwKeyEvent) {
    if event.is_null() {
        return;
    }
    let Some(browser) = get_browser(handle) else {
        return;
    };
    let e = unsafe { &*event };
    let event_type = match e.event_type {
        1 => KeyEventType::from(cef::sys::cef_key_event_type_t::KEYEVENT_KEYUP),
        2 => KeyEventType::from(cef::sys::cef_key_event_type_t::KEYEVENT_CHAR),
        _ => KeyEventType::from(cef::sys::cef_key_event_type_t::KEYEVENT_RAWKEYDOWN),
    };
    let key_event = KeyEvent {
        size: std::mem::size_of::<cef::sys::_cef_key_event_t>(),
        type_: event_type,
        modifiers: e.modifiers,
        windows_key_code: e.windows_key_code,
        native_key_code: e.native_key_code,
        is_system_key: e.is_system_key,
        character: e.character,
        unmodified_character: e.unmodified_character,
        focus_on_editable_field: 0,
    };
    if let Some(host) = browser.host() {
        host.send_key_event(Some(&key_event));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_was_resized(handle: u64) {
    if let Some(b) = get_browser(handle)
        && let Some(h) = b.host()
    {
        h.was_resized();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_invalidate(handle: u64) {
    if let Some(b) = get_browser(handle)
        && let Some(h) = b.host()
    {
        h.invalidate(PaintElementType::from(
            cef::sys::cef_paint_element_type_t::PET_VIEW,
        ));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_notify_move_or_resize_started(handle: u64) {
    if let Some(b) = get_browser(handle)
        && let Some(h) = b.host()
    {
        h.notify_move_or_resize_started();
    }
}

/// Write response data to a response context.
/// Called by C# from within an on_resource_request callback.
/// The `ctx` pointer is provided by Rust and remains valid for the duration of the callback.
///
/// # Safety
/// - `ctx` must be a valid pointer to a `ResponseContext` provided by a callback.
/// - `mime_type` must be null or a valid null-terminated C string.
/// - `data` must be null or a valid pointer to `data_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_response_write(
    ctx: *mut ResponseContext,
    status_code: i32,
    mime_type: *const c_char,
    data: *const u8,
    data_len: i32,
) {
    if ctx.is_null() {
        eprintln!("[rnw] rnw_response_write called with null context");
        return;
    }

    let ctx = unsafe { &mut *ctx };
    ctx.status_code = status_code;
    ctx.mime_type = unsafe { cstr_to_string(mime_type) }
        .unwrap_or_else(|| "application/octet-stream".to_string());
    ctx.data = if !data.is_null() && data_len > 0 {
        unsafe { std::slice::from_raw_parts(data, data_len as usize).to_vec() }
    } else {
        Vec::new()
    };
    ctx.was_set = true;
}

/// Registers a handler for the "res" scheme.
/// The callback is called synchronously for each request.
///
/// # Safety
/// - `callback` must be a valid function pointer.
/// - `user_data` must remain valid for the lifetime of the handler.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_register_res_scheme_handler(
    callback: scheme_handler::ResSchemeCallback,
    user_data: *mut std::os::raw::c_void,
) {
    let data = Arc::new(scheme_handler::SchemeCallbackData {
        callback,
        user_data,
    });

    let mut factory = scheme_handler::create_res_scheme_handler_factory(data);
    cef::register_scheme_handler_factory(
        Some(&CefString::from("res")),
        Some(&CefString::from("")),
        Some(&mut factory),
    );
}

/// Registers a handler for a custom scheme and domain.
/// The callback is called synchronously for each request to that scheme+domain.
///
/// # Safety
/// - `scheme` and `domain` must be valid null-terminated C strings.
/// - `callback` must be a valid function pointer.
/// - `user_data` must remain valid for the lifetime of the handler.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_register_scheme_handler(
    scheme: *const c_char,
    domain: *const c_char,
    callback: scheme_handler::ResSchemeCallback,
    user_data: *mut std::os::raw::c_void,
) {
    let scheme_str = unsafe { cstr_to_string(scheme).unwrap_or_default() };
    let domain_str = unsafe { cstr_to_string(domain).unwrap_or_default() };

    let data = Arc::new(scheme_handler::SchemeCallbackData {
        callback,
        user_data,
    });

    let mut factory = scheme_handler::create_res_scheme_handler_factory(data);
    cef::register_scheme_handler_factory(
        Some(&CefString::from(scheme_str.as_str())),
        Some(&CefString::from(domain_str.as_str())),
        Some(&mut factory),
    );
}

/// Creates a new windowed browser (popup).
///
/// # Safety
/// - `url` must be null or a valid null-terminated C string.
/// - `callbacks` must be a valid pointer to an initialized `RnwBrowserCallbacks` struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_window_create(
    url: *const c_char,
    width: i32,
    height: i32,
    callbacks: *const RnwBrowserCallbacks,
) -> u64 {
    if callbacks.is_null() {
        return RNW_HANDLE_INVALID;
    }
    let cbs = unsafe { *callbacks };
    let url_str = unsafe { cstr_to_string(url).unwrap_or_else(|| "about:blank".to_string()) };

    let data = Arc::new(CallbackData { callbacks: cbs });
    let mut cef_client = client::create_client(data);

    let window_info = WindowInfo {
        size: std::mem::size_of::<cef::sys::_cef_window_info_t>(),
        bounds: Rect {
            x: 0,
            y: 0,
            width,
            height,
        },
        runtime_style: RuntimeStyle::from(cef::sys::cef_runtime_style_t::CEF_RUNTIME_STYLE_ALLOY),
        ..Default::default()
    };

    let browser_settings = BrowserSettings {
        size: std::mem::size_of::<cef::sys::_cef_browser_settings_t>(),
        ..Default::default()
    };

    let cef_url = CefString::from(url_str.as_str());

    let browser = cef::browser_host_create_browser_sync(
        Some(&window_info),
        Some(&mut cef_client),
        Some(&cef_url),
        Some(&browser_settings),
        None,
        None,
    );

    match browser {
        Some(browser) => {
            with_state(|state| state.insert_browser(browser)).unwrap_or(RNW_HANDLE_INVALID)
        }
        None => RNW_HANDLE_INVALID,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_window_close(handle: u64) {
    rnw_browser_close(handle);
}
