use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use webview2_com::*;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::platform::*;

struct SendCom<T>(T);
unsafe impl<T> Send for SendCom<T> {}
unsafe impl<T> Sync for SendCom<T> {}

static ENVIRONMENT: Mutex<Option<SendCom<ICoreWebView2Environment>>> = Mutex::new(None);
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

struct WinWebView {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
    #[allow(dead_code)]
    parent: HWND,
    message_callback: Option<MessageCallbackFn>,
    message_user_data: *mut c_void,
}

unsafe impl Send for WinWebView {}
unsafe impl Sync for WinWebView {}

fn to_hstring(s: &str) -> HSTRING {
    HSTRING::from(s)
}

fn pwstr_to_string(p: PWSTR) -> String {
    if p.is_null() { return String::new(); }
    unsafe { p.to_string().unwrap_or_default() }
}

fn pump_until_ready(ready: &AtomicBool, timeout_iters: u32) {
    unsafe {
        let mut iters = 0u32;
        while !ready.load(Ordering::Acquire) && iters < timeout_iters {
            let mut msg = MSG::default();
            if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                std::thread::sleep(std::time::Duration::from_millis(1));
                iters += 1;
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

    let ready = Arc::new(AtomicBool::new(false));
    let ready2 = ready.clone();
    let env_result: Arc<Mutex<Option<ICoreWebView2Environment>>> = Arc::new(Mutex::new(None));
    let env_ref = env_result.clone();

    let handler = CreateCoreWebView2EnvironmentCompletedHandler::create(
        Box::new(move |_result, env| {
            if let Some(env) = env {
                *env_ref.lock().unwrap() = Some(env.clone());
            }
            ready2.store(true, Ordering::Release);
            Ok(())
        }),
    );

    // Register "res" as a custom scheme
    let options = CoreWebView2EnvironmentOptions::default();
    let scheme: ICoreWebView2CustomSchemeRegistration =
        CoreWebView2CustomSchemeRegistration::new("res".to_string()).into();
    unsafe {
        let _ = scheme.SetHasAuthorityComponent(true);
        options.set_scheme_registrations(vec![Some(scheme)]);
    }
    let options: ICoreWebView2EnvironmentOptions = options.into();

    let hr = unsafe {
        CreateCoreWebView2EnvironmentWithOptions(None, None, Some(&options), &handler)
    };

    if hr.is_err() {
        return -1;
    }

    pump_until_ready(&ready, 5000);

    let env = env_result.lock().unwrap().take();
    if let Some(env) = env {
        *ENVIRONMENT.lock().unwrap() = Some(SendCom(env));
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
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

pub fn create(parent_handle: *mut c_void, url: *const c_char) -> *mut c_void {
    if parent_handle.is_null() || !*INITIALIZED.lock().unwrap() {
        return ptr::null_mut();
    }

    let env = {
        let guard = ENVIRONMENT.lock().unwrap();
        match guard.as_ref() {
            Some(SendCom(env)) => env.clone(),
            None => return ptr::null_mut(),
        }
    };

    let parent = HWND(parent_handle as *mut _);

    let ready = Arc::new(AtomicBool::new(false));
    let ready2 = ready.clone();
    let controller_result: Arc<Mutex<Option<ICoreWebView2Controller>>> = Arc::new(Mutex::new(None));
    let ctrl_ref = controller_result.clone();

    let handler = CreateCoreWebView2ControllerCompletedHandler::create(
        Box::new(move |_result, controller| {
            if let Some(controller) = controller {
                *ctrl_ref.lock().unwrap() = Some(controller.clone());
            }
            ready2.store(true, Ordering::Release);
            Ok(())
        }),
    );

    let hr = unsafe { env.CreateCoreWebView2Controller(parent, &handler) };
    if hr.is_err() {
        return ptr::null_mut();
    }

    pump_until_ready(&ready, 5000);

    let controller = controller_result.lock().unwrap().take();
    let Some(controller) = controller else {
        return ptr::null_mut();
    };

    let webview: ICoreWebView2 = unsafe {
        match controller.CoreWebView2() {
            Ok(wv) => wv,
            Err(_) => return ptr::null_mut(),
        }
    };

    // Start with zero bounds — C# will call set_bounds with correct position/size
    unsafe {
        let _ = controller.SetBounds(RECT::default());
        let _ = controller.SetIsVisible(true);
    }

    // Enable devtools (F12)
    unsafe {
        if let Ok(settings) = webview.Settings() {
            let _ = settings.SetAreDevToolsEnabled(true);
            let _ = settings.SetAreDefaultContextMenusEnabled(true);
        }
    }

    // Shared handle pointer — set after boxing, read by event closures
    let shared_handle: Arc<Mutex<*mut c_void>> = Arc::new(Mutex::new(ptr::null_mut()));

    // Set up scheme handler for res://*
    if load_scheme_cb().is_some() {
        unsafe {
            let _ = webview.AddWebResourceRequestedFilter(
                w!("res://*"),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            );

            let mut token = 0i64;
            let _ = webview.add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(
                    Box::new(move |_sender, args| {
                        let Some(args) = args else { return Ok(()) };
                        let Some((callback, user_data)) = load_scheme_cb() else {
                            return Ok(());
                        };

                        let request = args.Request()?;
                        let mut uri = PWSTR::null();
                        request.Uri(&mut uri)?;
                        let uri_string = pwstr_to_string(uri);
                        CoTaskMemFree(Some(uri.0 as *const c_void));

                        let c_uri = std::ffi::CString::new(uri_string).unwrap();

                        let args_raw = std::mem::ManuallyDrop::new(args.clone());
                        let args_ptr = &*args_raw as *const _ as *mut c_void;

                        callback(c_uri.as_ptr(), args_ptr, user_data);
                        Ok(())
                    }),
                ),
                &mut token,
            );
        }
    }

    // Before-browse handler
    if load_bb_cb().is_some() {
        let handle_ref = shared_handle.clone();
        unsafe {
            let mut token = 0i64;
            let _ = webview.add_NavigationStarting(
                &NavigationStartingEventHandler::create(
                    Box::new(move |_sender, args| {
                        let Some(args) = args else { return Ok(()) };
                        let Some((callback, user_data)) = load_bb_cb() else {
                            return Ok(());
                        };

                        let mut uri = PWSTR::null();
                        args.Uri(&mut uri)?;
                        let uri_string = pwstr_to_string(uri);
                        CoTaskMemFree(Some(uri.0 as *const c_void));

                        let c_uri = std::ffi::CString::new(uri_string).unwrap();

                        let mut is_redirected = BOOL(0);
                        let _ = args.IsRedirected(&mut is_redirected);

                        let handle = *handle_ref.lock().unwrap();
                        let cancel = callback(
                            handle,
                            c_uri.as_ptr(),
                            is_redirected.0 as c_int,
                            user_data,
                        );
                        if cancel != 0 {
                            let _ = args.SetCancel(true);
                        }
                        Ok(())
                    }),
                ),
                &mut token,
            );
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
    *shared_handle.lock().unwrap() = handle;

    // Navigate to initial URL
    if !url.is_null() {
        unsafe {
            if let Ok(url_str) = CStr::from_ptr(url).to_str() {
                if !url_str.is_empty() {
                    let inst = from_handle::<WinWebView>(handle);
                    let _ = inst.webview.Navigate(&to_hstring(url_str));
                }
            }
        }
    }

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
            let _ = instance.webview.Navigate(&to_hstring(url_str));
        }
    }
}

pub fn reload(handle: *mut c_void) {
    unsafe { let _ = from_handle::<WinWebView>(handle).webview.Reload(); }
}

pub fn stop(handle: *mut c_void) {
    unsafe { let _ = from_handle::<WinWebView>(handle).webview.Stop(); }
}

pub fn go_back(handle: *mut c_void) {
    unsafe { let _ = from_handle::<WinWebView>(handle).webview.GoBack(); }
}

pub fn go_forward(handle: *mut c_void) {
    unsafe { let _ = from_handle::<WinWebView>(handle).webview.GoForward(); }
}

pub fn can_go_back(handle: *mut c_void) -> bool {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        let mut result = BOOL(0);
        let _ = instance.webview.CanGoBack(&mut result);
        result.as_bool()
    }
}

pub fn can_go_forward(handle: *mut c_void) -> bool {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        let mut result = BOOL(0);
        let _ = instance.webview.CanGoForward(&mut result);
        result.as_bool()
    }
}

pub fn is_loading(_handle: *mut c_void) -> bool { false }

pub fn execute_js(handle: *mut c_void, code: *const c_char) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        if let Ok(code_str) = CStr::from_ptr(code).to_str() {
            let _ = instance.webview.ExecuteScript(&to_hstring(code_str), None);
        }
    }
}

pub fn set_size(handle: *mut c_void, width: c_int, height: c_int) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        let _ = instance.controller.SetBounds(RECT { left: 0, top: 0, right: width, bottom: height });
    }
}

pub fn set_bounds(handle: *mut c_void, x: c_int, y: c_int, width: c_int, height: c_int) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        let _ = instance.controller.SetBounds(RECT { left: x, top: y, right: x + width, bottom: y + height });
    }
}

pub fn load_html(handle: *mut c_void, html: *const c_char, _base_url: *const c_char) {
    unsafe {
        let instance = from_handle::<WinWebView>(handle);
        if let Ok(html_str) = CStr::from_ptr(html).to_str() {
            let _ = instance.webview.NavigateToString(&to_hstring(html_str));
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
        let args_ref = &*(request_handle as *const ICoreWebView2WebResourceRequestedEventArgs);

        let env = {
            let guard = ENVIRONMENT.lock().unwrap();
            match guard.as_ref() {
                Some(SendCom(env)) => env.clone(),
                None => return,
            }
        };

        let mime_str = if !mime_type.is_null() {
            CStr::from_ptr(mime_type).to_str().unwrap_or("application/octet-stream")
        } else {
            "application/octet-stream"
        };

        let stream: IStream = CreateStreamOnHGlobal(HGLOBAL::default(), true).unwrap();
        if !data.is_null() && length > 0 {
            let slice = std::slice::from_raw_parts(data as *const u8, length as usize);
            let mut written = 0u32;
            let _ = stream.Write(
                slice.as_ptr() as *const c_void,
                length as u32,
                Some(&mut written),
            );
            let _ = stream.Seek(0, STREAM_SEEK_SET, None);
        }

        let headers = format!("Content-Type: {}", mime_str);

        if let Ok(response) = env.CreateWebResourceResponse(
            &stream, status_code, w!("OK"), &to_hstring(&headers),
        ) {
            let _ = args_ref.SetResponse(&response);
        }

        std::mem::drop(std::mem::ManuallyDrop::into_inner(
            std::mem::ManuallyDrop::new(args_ref.clone()),
        ));
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
            let mut token = 0i64;
            let _ = instance.webview.add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(
                    move |_sender, args| {
                        let Some(args) = args else { return Ok(()) };
                        let mut message = PWSTR::null();
                        args.TryGetWebMessageAsString(&mut message)?;
                        let msg_string = pwstr_to_string(message);
                        CoTaskMemFree(Some(message.0 as *const c_void));

                        let c_msg = std::ffi::CString::new(msg_string).unwrap();
                        callback(handle_val as *mut c_void, c_msg.as_ptr(), ud as *mut c_void);
                        Ok(())
                    },
                )),
                &mut token,
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
