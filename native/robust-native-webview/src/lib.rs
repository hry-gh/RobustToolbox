#![allow(unsafe_op_in_unsafe_fn)]

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

use std::ffi::{CStr, c_char};
use std::ptr;
use std::sync::Arc;

use cef::string::CefStringUtf8;
use cef::*;

use ffi_types::{PendingResponse, RnwBrowserCallbacks, RnwKeyEvent, RnwSettings};
use render_handler::CallbackData;
use state::{GLOBAL, GlobalState, with_browser, with_state};

// ============================================================================
// Helpers
// ============================================================================

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

// ============================================================================
// Lifecycle
// ============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_initialize(settings: *const RnwSettings) -> i32 {
    if settings.is_null() {
        return -1;
    }
    let s = unsafe { &*settings };

    #[cfg(target_os = "macos")]
    {
        if !s.framework_path.is_null() {
            // Use the explicit path provided by C#.
            let path_str = cstr_to_string(s.framework_path)
                .expect("framework_path is not valid UTF-8");
            let path = std::path::PathBuf::from(path_str);
            use std::os::unix::ffi::OsStrExt;
            let cstr = std::ffi::CString::new(path.as_os_str().as_bytes())
                .expect("framework_path contains null bytes");
            let result = cef::load_library(Some(unsafe { &*cstr.as_ptr().cast() }));
            assert_eq!(result, 1, "Failed to load CEF framework from provided path");
        } else {
            let loader =
                cef::library_loader::LibraryLoader::new(&std::env::current_exe().unwrap(), false);
            assert!(loader.load());
            // Intentionally leak: the library must stay loaded for the process lifetime.
            std::mem::forget(loader);
        }
    }

    cef::api_hash(cef::sys::CEF_API_VERSION_14100, 0);

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

    let mut cef_app = app::create_app();

    let result = cef::initialize(
        Some(args.as_main_args()),
        Some(&cef_settings),
        Some(&mut cef_app),
        ptr::null_mut(),
    );

    // Initialize global state
    {
        let mut guard = GLOBAL.lock().unwrap();
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
    // Drop all browser entries first
    {
        let mut guard = GLOBAL.lock().unwrap();
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

// ============================================================================
// Browser Management
// ============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_browser_create(
    url: *const c_char,
    _width: i32,
    _height: i32,
    callbacks: *const RnwBrowserCallbacks,
) -> u64 {
    if callbacks.is_null() {
        return 0;
    }
    let cbs = *callbacks;
    let url_str = cstr_to_string(url).unwrap_or_else(|| "about:blank".to_string());

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
        Some(browser) => with_state(|state| state.insert_browser(browser, cbs)).unwrap_or(0),
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_close(handle: u64) {
    // Remove from state and close
    let entry = with_state(|state| state.browsers.remove(&handle));
    if let Some(Some(entry)) = entry {
        if let Some(host) = entry.browser.host() {
            host.close_browser(1);
        }
    }
}

// ============================================================================
// Navigation
// ============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_browser_get_url(handle: u64, buf: *mut c_char, buf_len: i32) -> i32 {
    let url = with_browser(handle, |entry| {
        entry.browser.main_frame().map(|frame| {
            let url_userfree = frame.url();
            let url_utf16 = CefStringUtf16::from(&url_userfree);
            let url_utf8 = CefStringUtf8::from(&url_utf16);
            url_utf8.as_str().unwrap_or("").to_string()
        })
    });

    match url {
        Some(Some(url)) => {
            let bytes = url.as_bytes();
            let copy_len = bytes.len().min((buf_len as usize).saturating_sub(1));
            if !buf.is_null() && buf_len > 0 {
                ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy_len);
                *buf.add(copy_len) = 0;
            }
            bytes.len() as i32
        }
        _ => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_browser_load_url(handle: u64, url: *const c_char) {
    let cef_url = cstr_to_cef_string(url);
    with_browser(handle, |entry| {
        if let Some(frame) = entry.browser.main_frame() {
            frame.load_url(Some(&cef_url));
        }
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_browser_execute_js(handle: u64, code: *const c_char) {
    let cef_code = cstr_to_cef_string(code);
    let empty = CefString::from("");
    with_browser(handle, |entry| {
        if let Some(frame) = entry.browser.main_frame() {
            frame.execute_java_script(Some(&cef_code), Some(&empty), 1);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_is_loading(handle: u64) -> i32 {
    with_browser(handle, |entry| entry.browser.is_loading()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_can_go_back(handle: u64) -> i32 {
    with_browser(handle, |entry| entry.browser.can_go_back()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_can_go_forward(handle: u64) -> i32 {
    with_browser(handle, |entry| entry.browser.can_go_forward()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_go_back(handle: u64) {
    with_browser(handle, |entry| entry.browser.go_back());
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_go_forward(handle: u64) {
    with_browser(handle, |entry| entry.browser.go_forward());
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_reload(handle: u64) {
    with_browser(handle, |entry| entry.browser.reload());
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_stop_load(handle: u64) {
    with_browser(handle, |entry| entry.browser.stop_load());
}

// ============================================================================
// Input
// ============================================================================

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_send_mouse_move(
    handle: u64,
    x: i32,
    y: i32,
    modifiers: u32,
    mouse_leave: i32,
) {
    with_browser(handle, |entry| {
        if let Some(host) = entry.browser.host() {
            let event = MouseEvent { x, y, modifiers };
            host.send_mouse_move_event(Some(&event), mouse_leave);
        }
    });
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
    with_browser(handle, |entry| {
        if let Some(host) = entry.browser.host() {
            let event = MouseEvent { x, y, modifiers };
            let button_type = match button {
                1 => MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_MIDDLE),
                2 => MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_RIGHT),
                _ => MouseButtonType::from(cef::sys::cef_mouse_button_type_t::MBT_LEFT),
            };
            host.send_mouse_click_event(Some(&event), button_type, mouse_up, click_count);
        }
    });
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
    with_browser(handle, |entry| {
        if let Some(host) = entry.browser.host() {
            let event = MouseEvent { x, y, modifiers };
            host.send_mouse_wheel_event(Some(&event), delta_x, delta_y);
        }
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_browser_send_key_event(handle: u64, event: *const RnwKeyEvent) {
    if event.is_null() {
        return;
    }
    let e = &*event;
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

    with_browser(handle, |entry| {
        if let Some(host) = entry.browser.host() {
            host.send_key_event(Some(&key_event));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_was_resized(handle: u64) {
    with_browser(handle, |entry| {
        if let Some(host) = entry.browser.host() {
            host.was_resized();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_invalidate(handle: u64) {
    with_browser(handle, |entry| {
        if let Some(host) = entry.browser.host() {
            host.invalidate(PaintElementType::from(
                cef::sys::cef_paint_element_type_t::PET_VIEW,
            ));
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_browser_notify_move_or_resize_started(handle: u64) {
    with_browser(handle, |entry| {
        if let Some(host) = entry.browser.host() {
            host.notify_move_or_resize_started();
        }
    });
}

// ============================================================================
// Resource Request Response
// ============================================================================

/// Called by C# from within an on_resource_request callback to set the response data.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_request_set_response(
    request_id: u64,
    status_code: i32,
    mime_type: *const c_char,
    data: *const u8,
    data_len: i32,
) {
    let mime = cstr_to_string(mime_type).unwrap_or_else(|| "application/octet-stream".to_string());
    let data_vec = if !data.is_null() && data_len > 0 {
        std::slice::from_raw_parts(data, data_len as usize).to_vec()
    } else {
        Vec::new()
    };

    with_state(|state| {
        state.pending_responses.insert(
            request_id,
            PendingResponse {
                status_code,
                mime_type: mime,
                data: data_vec,
            },
        );
    });
}

// ============================================================================
// Scheme Handler Registration
// ============================================================================

/// Register a scheme handler factory for the "res" scheme.
/// The callback is called synchronously for each request.
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

// ============================================================================
// Window Browser (popup)
// ============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rnw_window_create(
    url: *const c_char,
    width: i32,
    height: i32,
    callbacks: *const RnwBrowserCallbacks,
) -> u64 {
    if callbacks.is_null() {
        return 0;
    }
    let cbs = *callbacks;
    let url_str = cstr_to_string(url).unwrap_or_else(|| "about:blank".to_string());

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
        Some(browser) => with_state(|state| state.insert_browser(browser, cbs)).unwrap_or(0),
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rnw_window_close(handle: u64) {
    rnw_browser_close(handle);
}
