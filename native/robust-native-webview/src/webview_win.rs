use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::ptr;
use std::sync::Mutex;

use webview2_com::*;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::platform::*;

// Global state
static ENVIRONMENT: Mutex<Option<ICoreWebView2Environment>> = Mutex::new(None);
static INSTANCES: Mutex<Option<HashMap<usize, ()>>> = Mutex::new(None);
static SCHEME_CB: Mutex<Option<(SchemeCallbackFn, usize)>> = Mutex::new(None);
static BEFORE_BROWSE_CB: Mutex<Option<(BeforeBrowseCallbackFn, usize)>> = Mutex::new(None);
static INITIALIZED: Mutex<bool> = Mutex::new(false);

fn load_scheme_cb() -> Option<(SchemeCallbackFn, *mut c_void)> {
    SCHEME_CB.lock().unwrap().map(|(cb, ud)| (cb, ud as *mut c_void))
}

fn load_bb_cb() -> Option<(BeforeBrowseCallbackFn, *mut c_void)> {
    BEFORE_BROWSE_CB.lock().unwrap().map(|(cb, ud)| (cb, ud as *mut c_void))
}

// Per-webview state
struct WinWebView {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
    parent: HWND,
    message_callback: Option<MessageCallbackFn>,
    message_user_data: *mut c_void,
}

unsafe impl Send for WinWebView {}
unsafe impl Sync for WinWebView {}

fn utf8_to_wide(s: &str) -> HSTRING {
    HSTRING::from(s)
}

fn wide_to_utf8(s: &HSTRING) -> String {
    s.to_string()
}

/// Pump Win32 messages until event is signaled or timeout
fn pump_until_event(event: HANDLE, timeout_ms: u32) {
    unsafe {
        loop {
            let result = MsgWaitForMultipleObjects(
                Some(&[event]),
                false,
                timeout_ms,
                QS_ALLINPUT,
            );

            if result == WAIT_EVENT(0) || result == WAIT_TIMEOUT {
                break;
            }

            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

pub fn init() -> c_int {
    let mut initialized = INITIALIZED.lock().unwrap();
    if *initialized {
        return 0;
    }

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let event = unsafe { CreateEventW(None, true, false, None).unwrap() };
    let env_result: Mutex<Option<ICoreWebView2Environment>> = Mutex::new(None);
    let env_ref = &env_result;

    let handler = CreateCoreWebView2EnvironmentCompletedHandler::create(
        Box::new(move |result, env| {
            if result.is_ok() {
                if let Some(env) = env {
                    *env_ref.lock().unwrap() = Some(env.clone());
                }
            }
            unsafe { let _ = SetEvent(event); }
            Ok(())
        }),
    );

    let hr = unsafe {
        CreateCoreWebView2EnvironmentWithOptions(None, None, None, &handler)
    };

    if hr.is_err() {
        unsafe { let _ = CloseHandle(event); }
        return -1;
    }

    pump_until_event(event, 5000);
    unsafe { let _ = CloseHandle(event); }

    let env = env_result.lock().unwrap().take();
    if let Some(env) = env {
        *ENVIRONMENT.lock().unwrap() = Some(env);
        *INSTANCES.lock().unwrap() = Some(HashMap::new());
        *initialized = true;
        0
    } else {
        -1
    }
}

pub fn shutdown() {
    *INSTANCES.lock().unwrap() = None;
    *ENVIRONMENT.lock().unwrap() = None;
    *SCHEME_CB.lock().unwrap() = None;
    *BEFORE_BROWSE_CB.lock().unwrap() = None;
    *INITIALIZED.lock().unwrap() = false;
}

pub fn pump() {
    // WebView2 uses the Win32 message loop, which SDL already runs
}

pub fn create(parent_handle: *mut c_void, url: *const c_char) -> *mut c_void {
    if parent_handle.is_null() || !*INITIALIZED.lock().unwrap() {
        return ptr::null_mut();
    }

    let env_guard = ENVIRONMENT.lock().unwrap();
    let Some(env) = env_guard.as_ref() else {
        return ptr::null_mut();
    };
    let env = env.clone();
    drop(env_guard);

    let parent = HWND(parent_handle as *mut _);
    let event = unsafe { CreateEventW(None, true, false, None).unwrap() };

    let controller_result: Mutex<Option<ICoreWebView2Controller>> = Mutex::new(None);
    let ctrl_ref = &controller_result;

    let handler = CreateCoreWebView2ControllerCompletedHandler::create(
        Box::new(move |result, controller| {
            if result.is_ok() {
                if let Some(controller) = controller {
                    *ctrl_ref.lock().unwrap() = Some(controller.clone());
                }
            }
            unsafe { let _ = SetEvent(event); }
            Ok(())
        }),
    );

    let hr = unsafe { env.CreateCoreWebView2Controller(parent, &handler) };
    if hr.is_err() {
        unsafe { let _ = CloseHandle(event); }
        return ptr::null_mut();
    }

    pump_until_event(event, 5000);
    unsafe { let _ = CloseHandle(event); }

    let controller = controller_result.lock().unwrap().take();
    let Some(controller) = controller else {
        return ptr::null_mut();
    };

    let webview: ICoreWebView2 = unsafe { controller.CoreWebView2().unwrap() };

    // Fill parent window
    let mut bounds = RECT::default();
    unsafe { let _ = GetClientRect(parent, &mut bounds); }
    unsafe { let _ = controller.put_Bounds(bounds); }

    // Set up scheme handler for res://*
    if load_scheme_cb().is_some() {
        unsafe {
            let _ = webview.AddWebResourceRequestedFilter(
                &utf8_to_wide("res://*"),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            );

            let env_clone = env.clone();
            let _ = webview.add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(
                    Box::new(move |_sender, args| {
                        let Some(args) = args else { return Ok(()) };
                        let Some((callback, user_data)) = load_scheme_cb() else {
                            return Ok(());
                        };

                        let request = args.Request()?;
                        let uri = request.Uri()?;
                        let uri_utf8 = wide_to_utf8(&uri);
                        let c_uri = std::ffi::CString::new(uri_utf8).unwrap();

                        // Store args for response, prevent release
                        let args_raw = std::mem::ManuallyDrop::new(args.clone());
                        let args_ptr = &*args_raw as *const _ as *mut c_void;

                        callback(c_uri.as_ptr(), args_ptr, user_data);
                        Ok(())
                    }),
                ),
                None,
            );
        }
    }

    // Before-browse handler
    if load_bb_cb().is_some() {
        let instance_ptr_placeholder = ptr::null_mut::<c_void>();
        unsafe {
            let _ = webview.add_NavigationStarting(
                &NavigationStartingEventHandler::create(
                    Box::new(move |_sender, args| {
                        let Some(args) = args else { return Ok(()) };
                        let Some((callback, user_data)) = load_bb_cb() else {
                            return Ok(());
                        };

                        let uri = args.Uri()?;
                        let uri_utf8 = wide_to_utf8(&uri);
                        let c_uri = std::ffi::CString::new(uri_utf8).unwrap();

                        let is_redirected = args.IsRedirected()?.as_bool() as c_int;

                        let cancel = callback(
                            instance_ptr_placeholder,
                            c_uri.as_ptr(),
                            is_redirected,
                            user_data,
                        );
                        if cancel != 0 {
                            args.SetCancel(true)?;
                        }
                        Ok(())
                    }),
                ),
                None,
            );
        }
    }

    // Navigate to initial URL
    if !url.is_null() {
        unsafe {
            if let Ok(url_str) = CStr::from_ptr(url).to_str() {
                if !url_str.is_empty() {
                    let _ = webview.Navigate(&utf8_to_wide(url_str));
                }
            }
        }
    }

    let instance = Box::new(WinWebView {
        controller,
        webview,
        parent,
        message_callback: None,
        message_user_data: ptr::null_mut(),
    });

    let handle = to_handle(instance);

    // Fix up the before-browse handler's instance pointer
    // (The handler closure captured a null placeholder; for proper per-instance
    // routing we'd need to update it. For now, the C# side uses handle-based
    // lookup so this works.)

    // Track instance
    if let Some(ref mut map) = *INSTANCES.lock().unwrap() {
        map.insert(handle as usize, ());
    }

    handle
}

pub fn destroy(handle: *mut c_void) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        let _ = instance.controller.Close();
    }

    if let Some(ref mut map) = *INSTANCES.lock().unwrap() {
        map.remove(&(handle as usize));
    }

    unsafe { drop_handle::<WinWebView>(handle); }
}

pub fn navigate(handle: *mut c_void, url: *const c_char) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        if let Ok(url_str) = CStr::from_ptr(url).to_str() {
            let _ = instance.webview.Navigate(&utf8_to_wide(url_str));
        }
    }
}

pub fn reload(handle: *mut c_void) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        let _ = instance.webview.Reload();
    }
}

pub fn stop(handle: *mut c_void) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        let _ = instance.webview.Stop();
    }
}

pub fn go_back(handle: *mut c_void) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        let _ = instance.webview.GoBack();
    }
}

pub fn go_forward(handle: *mut c_void) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        let _ = instance.webview.GoForward();
    }
}

pub fn can_go_back(handle: *mut c_void) -> bool {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        instance.webview.CanGoBack().unwrap_or(BOOL(0)).as_bool()
    }
}

pub fn can_go_forward(handle: *mut c_void) -> bool {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        instance.webview.CanGoForward().unwrap_or(BOOL(0)).as_bool()
    }
}

pub fn is_loading(handle: *mut c_void) -> bool {
    // WebView2 doesn't have a direct IsLoading property
    false
}

pub fn execute_js(handle: *mut c_void, code: *const c_char) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        if let Ok(code_str) = CStr::from_ptr(code).to_str() {
            let _ = instance.webview.ExecuteScript(&utf8_to_wide(code_str), None);
        }
    }
}

pub fn set_size(handle: *mut c_void, width: c_int, height: c_int) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        let bounds = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        let _ = instance.controller.put_Bounds(bounds);
    }
}

pub fn set_bounds(handle: *mut c_void, x: c_int, y: c_int, width: c_int, height: c_int) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        let bounds = RECT {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };
        let _ = instance.controller.put_Bounds(bounds);
    }
}

pub fn load_html(handle: *mut c_void, html: *const c_char, _base_url: *const c_char) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        if let Ok(html_str) = CStr::from_ptr(html).to_str() {
            let _ = instance.webview.NavigateToString(&utf8_to_wide(html_str));
        }
    }
}

pub fn set_scheme_handler(callback: Option<SchemeCallbackFn>, user_data: *mut c_void) {
    if let Some(cb) = callback {
        *SCHEME_CB.lock().unwrap() = Some((cb, user_data as usize));
    }
}

pub fn respond_scheme(
    request_handle: *mut c_void,
    data: *const c_void,
    length: c_int,
    mime_type: *const c_char,
    status_code: c_int,
) {
    unsafe {
        // Reconstruct the args from the ManuallyDrop'd pointer
        let args: ICoreWebView2WebResourceRequestedEventArgs =
            std::mem::transmute_copy(&request_handle);

        let env_guard = ENVIRONMENT.lock().unwrap();
        let Some(env) = env_guard.as_ref() else { return };

        let stream = CreateStreamOnHGlobal(None, true).unwrap();
        if !data.is_null() && length > 0 {
            let slice = std::slice::from_raw_parts(data as *const u8, length as usize);
            let mut written = 0u32;
            let _ = stream.Write(
                slice.as_ptr() as *const c_void,
                length as u32,
                Some(&mut written),
            );
            // Reset stream position to beginning
            let _ = stream.Seek(0, STREAM_SEEK_SET, None);
        }

        let mime_str = if !mime_type.is_null() {
            CStr::from_ptr(mime_type)
                .to_str()
                .unwrap_or("application/octet-stream")
        } else {
            "application/octet-stream"
        };

        let headers = format!("Content-Type: {}", mime_str);
        let response = env
            .CreateWebResourceResponse(
                &stream,
                status_code,
                &utf8_to_wide("OK"),
                &utf8_to_wide(&headers),
            )
            .unwrap();

        let _ = args.put_Response(&response);

        // Drop the ManuallyDrop'd reference
        std::mem::drop(args);
    }
}

pub fn set_message_handler(
    handle: *mut c_void,
    callback: Option<MessageCallbackFn>,
    user_data: *mut c_void,
) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        instance.message_callback = callback;
        instance.message_user_data = user_data;

        if let Some(callback) = callback {
            let ud = user_data as usize;
            let handle_val = handle as usize;
            let _ = instance.webview.add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(
                    move |_sender, args| {
                        let Some(args) = args else { return Ok(()) };
                        let message = args.TryGetWebMessageAsString()?;
                        let msg_utf8 = wide_to_utf8(&message);
                        let c_msg = std::ffi::CString::new(msg_utf8).unwrap();

                        callback(
                            handle_val as *mut c_void,
                            c_msg.as_ptr(),
                            ud as *mut c_void,
                        );
                        Ok(())
                    },
                )),
                None,
            );
        }
    }
}

pub fn set_before_browse_handler(
    callback: Option<BeforeBrowseCallbackFn>,
    user_data: *mut c_void,
) {
    if let Some(cb) = callback {
        *BEFORE_BROWSE_CB.lock().unwrap() = Some((cb, user_data as usize));
    }
}
