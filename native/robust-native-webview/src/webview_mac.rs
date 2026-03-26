use std::cell::Cell;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::ptr;
use std::sync::Mutex;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, MainThreadOnly, DeclaredClass};
use objc2_app_kit::{NSView, NSWindow};
use objc2_foundation::{
    NSData, NSError, NSObject, NSRect, NSPoint, NSSize, NSString, NSURL,
    NSURLRequest, NSURLResponse,
};
use objc2_web_kit::{
    WKNavigation, WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate,
    WKScriptMessage, WKScriptMessageHandler, WKURLSchemeHandler, WKURLSchemeTask,
    WKUserContentController, WKWebView, WKWebViewConfiguration,
};

use crate::platform::*;

// Global state
static SCHEME_CB: Mutex<Option<(SchemeCallbackFn, usize)>> = Mutex::new(None);
static BEFORE_BROWSE_CB: Mutex<Option<(BeforeBrowseCallbackFn, usize)>> = Mutex::new(None);
static INITIALIZED: Mutex<bool> = Mutex::new(false);

fn store_cb(cb: SchemeCallbackFn, ud: *mut c_void) {
    *SCHEME_CB.lock().unwrap() = Some((cb, ud as usize));
}
fn load_scheme_cb() -> Option<(SchemeCallbackFn, *mut c_void)> {
    SCHEME_CB.lock().unwrap().map(|(cb, ud)| (cb, ud as *mut c_void))
}
fn store_bb_cb(cb: BeforeBrowseCallbackFn, ud: *mut c_void) {
    *BEFORE_BROWSE_CB.lock().unwrap() = Some((cb, ud as usize));
}
fn load_bb_cb() -> Option<(BeforeBrowseCallbackFn, *mut c_void)> {
    BEFORE_BROWSE_CB.lock().unwrap().map(|(cb, ud)| (cb, ud as *mut c_void))
}

// --- ObjC delegate classes ---

struct SchemeHandlerIvars;

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RobustSchemeHandler"]
    #[ivars = SchemeHandlerIvars]
    struct SchemeHandler;

    unsafe impl NSObjectProtocol for SchemeHandler {}

    unsafe impl WKURLSchemeHandler for SchemeHandler {
        #[unsafe(method(webView:startURLSchemeTask:))]
        unsafe fn start_url_scheme_task(
            &self,
            _web_view: &WKWebView,
            task: &ProtocolObject<dyn WKURLSchemeTask>,
        ) {
            let Some((callback, user_data)) = load_scheme_cb() else { return };

            let request = task.request();
            let Some(url) = request.URL() else { return };
            let Some(url_str) = url.absoluteString() else { return };
            let url_string = url_str.to_string();

            // Prevent ARC from releasing the task while we wait for response.
            // We take an explicit retain here and balance it in respond_scheme
            // via Retained::from_raw().
            let task_ptr = task as *const _ as *mut c_void;
            let task_obj = task_ptr as *mut AnyObject;
            let _retained = Retained::retain(task_obj).unwrap();
            std::mem::forget(_retained); // leak the retain count; respond_scheme will reclaim it

            let c_url = std::ffi::CString::new(url_string).unwrap();
            unsafe {
                callback(c_url.as_ptr(), task_ptr, user_data);
            }
        }

        #[unsafe(method(webView:stopURLSchemeTask:))]
        unsafe fn stop_url_scheme_task(
            &self,
            _web_view: &WKWebView,
            _task: &ProtocolObject<dyn WKURLSchemeTask>,
        ) {}
    }
);

impl SchemeHandler {
    fn new(mtm: objc2::MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send![Self::alloc(mtm), init] }
    }
}

// --- Navigation Delegate ---

struct NavigationDelegateIvars {
    handle: Cell<*mut c_void>,
}

unsafe impl Send for NavigationDelegateIvars {}
unsafe impl Sync for NavigationDelegateIvars {}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RobustNavigationDelegate"]
    #[ivars = NavigationDelegateIvars]
    struct NavigationDelegate;

    unsafe impl NSObjectProtocol for NavigationDelegate {}

    unsafe impl WKNavigationDelegate for NavigationDelegate {
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        unsafe fn decide_policy(
            &self,
            _web_view: &WKWebView,
            action: &WKNavigationAction,
            decision_handler: &block2::DynBlock<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            let Some((callback, user_data)) = load_bb_cb() else {
                decision_handler.call((WKNavigationActionPolicy::Allow,));
                return;
            };

            let request = action.request();
            let url_string = request
                .URL()
                .and_then(|u| u.absoluteString())
                .map(|s| s.to_string())
                .unwrap_or_default();

            let c_url = std::ffi::CString::new(url_string).unwrap_or_default();
            let handle = self.ivars().handle.get();
            let cancel = unsafe {
                callback(handle, c_url.as_ptr(), 0, user_data)
            };

            if cancel != 0 {
                decision_handler.call((WKNavigationActionPolicy::Cancel,));
            } else {
                decision_handler.call((WKNavigationActionPolicy::Allow,));
            }
        }

        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        unsafe fn did_fail_provisional(
            &self,
            _web_view: &WKWebView,
            _navigation: Option<&WKNavigation>,
            error: &NSError,
        ) {
            eprintln!("[webview] didFailProvisionalNavigation: {}", error);
        }
    }
);

impl NavigationDelegate {
    fn new(mtm: objc2::MainThreadMarker, handle: *mut c_void) -> Retained<Self> {
        let this: Retained<Self> = unsafe { msg_send![Self::alloc(mtm), init] };
        this.ivars().handle.set(handle);
        this
    }
}

// --- Script Message Handler ---

struct MessageHandlerIvars {
    handle: Cell<*mut c_void>,
    callback: Cell<Option<MessageCallbackFn>>,
    user_data: Cell<*mut c_void>,
}

unsafe impl Send for MessageHandlerIvars {}
unsafe impl Sync for MessageHandlerIvars {}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RobustScriptMessageHandler"]
    #[ivars = MessageHandlerIvars]
    struct ScriptMessageHandler;

    unsafe impl NSObjectProtocol for ScriptMessageHandler {}

    unsafe impl WKScriptMessageHandler for ScriptMessageHandler {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        unsafe fn did_receive_message(
            &self,
            _controller: &WKUserContentController,
            message: &WKScriptMessage,
        ) {
            let Some(callback) = self.ivars().callback.get() else { return };

            let body = message.body();
            let body_str: Option<Retained<NSString>> = unsafe { msg_send![&*body, description] };
            let Some(body_str) = body_str else { return };

            let s = body_str.to_string();
            let c_str = std::ffi::CString::new(s).unwrap_or_default();

            let handle = self.ivars().handle.get();
            let user_data = self.ivars().user_data.get();
            unsafe {
                callback(handle, c_str.as_ptr(), user_data);
            }
        }
    }
);

impl ScriptMessageHandler {
    fn new(mtm: objc2::MainThreadMarker, handle: *mut c_void) -> Retained<Self> {
        let this: Retained<Self> = unsafe { msg_send![Self::alloc(mtm), init] };
        this.ivars().handle.set(handle);
        this.ivars().callback.set(None);
        this.ivars().user_data.set(ptr::null_mut());
        this
    }
}

// --- Per-instance state ---

struct MacWebView {
    webview: Retained<WKWebView>,
    #[allow(dead_code)]
    parent: Retained<NSWindow>,
    #[allow(dead_code)]
    navigation_delegate: Retained<NavigationDelegate>,
    message_handler: Retained<ScriptMessageHandler>,
}

// Can't put MainThreadOnly Retained in a Mutex, so store raw pointer.
struct SendSchemePtr(*const SchemeHandler);
unsafe impl Send for SendSchemePtr {}
unsafe impl Sync for SendSchemePtr {}
static SCHEME_HANDLER_PTR: Mutex<Option<SendSchemePtr>> = Mutex::new(None);

// --- Public API ---

pub fn init() -> c_int {
    let mut initialized = INITIALIZED.lock().unwrap();
    if *initialized {
        return 0;
    }

    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return -1;
    };

    let handler = SchemeHandler::new(mtm);
    let ptr = Retained::into_raw(handler);
    *SCHEME_HANDLER_PTR.lock().unwrap() = Some(SendSchemePtr(ptr));

    *initialized = true;
    0
}

pub fn shutdown() {
    let wrapper = SCHEME_HANDLER_PTR.lock().unwrap().take();
    if let Some(SendSchemePtr(ptr)) = wrapper {
        unsafe { let _ = Retained::from_raw(ptr as *mut SchemeHandler); }
    }
    *SCHEME_CB.lock().unwrap() = None;
    *BEFORE_BROWSE_CB.lock().unwrap() = None;
    *INITIALIZED.lock().unwrap() = false;
}

pub fn pump() {}

pub fn create(parent_handle: *mut c_void, url: *const c_char) -> *mut c_void {
    if parent_handle.is_null() {
        return ptr::null_mut();
    }
    if !*INITIALIZED.lock().unwrap() {
        return ptr::null_mut();
    }

    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return ptr::null_mut();
    };

    unsafe {
        let parent: *mut NSWindow = parent_handle as *mut NSWindow;
        let parent_retained = Retained::retain(parent).unwrap();

        let config = WKWebViewConfiguration::new(mtm);

        // Register res:// scheme handler
        {
            let handler_guard = SCHEME_HANDLER_PTR.lock().unwrap();
            if let Some(SendSchemePtr(ptr)) = *handler_guard {
                let handler: &SchemeHandler = &*ptr;
                let protocol_handler = ProtocolObject::from_ref(handler);
                config.setURLSchemeHandler_forURLScheme(
                    Some(protocol_handler),
                    &NSString::from_str("res"),
                );
            }
        }

        // Create webview with zero frame
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
        let webview = WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), frame, &config);

        // Enable Safari Web Inspector
        webview.setInspectable(true);

        // Create delegates with placeholder handle
        let nav_delegate = NavigationDelegate::new(mtm, ptr::null_mut());
        let nav_ref: &NavigationDelegate = &nav_delegate;
        let protocol_delegate: &ProtocolObject<dyn WKNavigationDelegate> =
            ProtocolObject::from_ref(nav_ref);
        webview.setNavigationDelegate(Some(protocol_delegate));

        let msg_handler = ScriptMessageHandler::new(mtm, ptr::null_mut());
        let msg_ref: &ScriptMessageHandler = &msg_handler;
        let protocol_msg: &ProtocolObject<dyn WKScriptMessageHandler> =
            ProtocolObject::from_ref(msg_ref);
        let content_controller = config.userContentController();
        content_controller.addScriptMessageHandler_name(protocol_msg, &NSString::from_str("robust"));

        // Add as subview
        let content_view = parent_retained.contentView().unwrap();
        content_view.addSubview(&webview);

        let instance = Box::new(MacWebView {
            webview,
            parent: parent_retained,
            navigation_delegate: nav_delegate,
            message_handler: msg_handler,
        });

        let handle = to_handle(instance);

        // Fix up delegate handle pointers
        let inst = from_handle::<MacWebView>(handle);
        inst.navigation_delegate.ivars().handle.set(handle);
        inst.message_handler.ivars().handle.set(handle);

        // Navigate to initial URL
        if !url.is_null() {
            if let Ok(url_cstr) = CStr::from_ptr(url).to_str() {
                if !url_cstr.is_empty() {
                    if let Some(ns_url) = NSURL::URLWithString(&NSString::from_str(url_cstr)) {
                        let request = NSURLRequest::requestWithURL(&ns_url);
                        inst.webview.loadRequest(&request);
                    }
                }
            }
        }

        handle
    }
}

pub fn destroy(handle: *mut c_void) {
    unsafe {
        let instance = from_handle::<MacWebView>(handle);
        instance.webview.removeFromSuperview();
        drop_handle::<MacWebView>(handle);
    }
}

pub fn navigate(handle: *mut c_void, url: *const c_char) {
    unsafe {
        let instance = from_handle::<MacWebView>(handle);
        if let Ok(url_str) = CStr::from_ptr(url).to_str() {
            if let Some(ns_url) = NSURL::URLWithString(&NSString::from_str(url_str)) {
                let request = NSURLRequest::requestWithURL(&ns_url);
                instance.webview.loadRequest(&request);
            }
        }
    }
}

pub fn reload(handle: *mut c_void) {
    unsafe { from_handle::<MacWebView>(handle).webview.reload(); }
}

pub fn stop(handle: *mut c_void) {
    unsafe { from_handle::<MacWebView>(handle).webview.stopLoading(); }
}

pub fn go_back(handle: *mut c_void) {
    unsafe { from_handle::<MacWebView>(handle).webview.goBack(); }
}

pub fn go_forward(handle: *mut c_void) {
    unsafe { from_handle::<MacWebView>(handle).webview.goForward(); }
}

pub fn can_go_back(handle: *mut c_void) -> bool {
    unsafe { from_handle::<MacWebView>(handle).webview.canGoBack() }
}

pub fn can_go_forward(handle: *mut c_void) -> bool {
    unsafe { from_handle::<MacWebView>(handle).webview.canGoForward() }
}

pub fn is_loading(handle: *mut c_void) -> bool {
    unsafe { from_handle::<MacWebView>(handle).webview.isLoading() }
}

pub fn execute_js(handle: *mut c_void, code: *const c_char) {
    unsafe {
        let instance = from_handle::<MacWebView>(handle);
        if let Ok(code_str) = CStr::from_ptr(code).to_str() {
            instance
                .webview
                .evaluateJavaScript_completionHandler(&NSString::from_str(code_str), None);
        }
    }
}

pub fn set_size(handle: *mut c_void, width: c_int, height: c_int) {
    unsafe {
        let instance = from_handle::<MacWebView>(handle);
        let wv: &NSView = &instance.webview;
        let old_frame = wv.frame();
        let frame = NSRect::new(old_frame.origin, NSSize::new(width as f64, height as f64));
        wv.setFrame(frame);
    }
}

pub fn set_bounds(handle: *mut c_void, x: c_int, y: c_int, width: c_int, height: c_int) {
    unsafe {
        let instance = from_handle::<MacWebView>(handle);
        let content_view = instance.parent.contentView().unwrap();
        let parent_height = content_view.bounds().size.height;

        let frame = NSRect::new(
            NSPoint::new(x as f64, parent_height - y as f64 - height as f64),
            NSSize::new(width as f64, height as f64),
        );
        let wv: &NSView = &instance.webview;
        wv.setFrame(frame);
    }
}

pub fn load_html(handle: *mut c_void, html: *const c_char, base_url: *const c_char) {
    unsafe {
        let instance = from_handle::<MacWebView>(handle);
        if let Ok(html_str) = CStr::from_ptr(html).to_str() {
            let ns_html = NSString::from_str(html_str);
            let ns_base_url = if !base_url.is_null() {
                CStr::from_ptr(base_url)
                    .to_str()
                    .ok()
                    .and_then(|s| NSURL::URLWithString(&NSString::from_str(s)))
            } else {
                None
            };
            instance
                .webview
                .loadHTMLString_baseURL(&ns_html, ns_base_url.as_deref());
        }
    }
}

pub fn set_scheme_handler(callback: Option<SchemeCallbackFn>, user_data: *mut c_void) {
    if let Some(cb) = callback {
        store_cb(cb, user_data);
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
        let task: &ProtocolObject<dyn WKURLSchemeTask> = &*(request_handle as *const _);

        let response_data = if !data.is_null() && length > 0 {
            NSData::with_bytes(std::slice::from_raw_parts(data as *const u8, length as usize))
        } else {
            NSData::new()
        };

        let mime_str = if !mime_type.is_null() {
            CStr::from_ptr(mime_type)
                .to_str()
                .unwrap_or("application/octet-stream")
        } else {
            "application/octet-stream"
        };

        let request = task.request();
        let url = request.URL().unwrap();

        // NSURLResponse is MainThreadOnly so we use msg_send for alloc/init
        let ns_mime = NSString::from_str(mime_str);
        let cls: &AnyObject = objc2::class!(NSURLResponse).as_ref();
        let alloc_obj: *mut AnyObject = msg_send![cls, alloc];
        let response_obj: *mut AnyObject = msg_send![
            alloc_obj,
            initWithURL: &*url,
            MIMEType: &*ns_mime,
            expectedContentLength: length as isize,
            textEncodingName: ptr::null::<AnyObject>()
        ];
        let response: &NSURLResponse = &*(response_obj as *const NSURLResponse);

        task.didReceiveResponse(response);
        task.didReceiveData(&response_data);
        task.didFinish();

        // Release the response (we own it from alloc/init)
        let _ = Retained::from_raw(response_obj);

        // Balance the retain from start_url_scheme_task
        let _ = Retained::from_raw(request_handle as *mut AnyObject);
    }
}

pub fn set_message_handler(
    handle: *mut c_void,
    callback: Option<MessageCallbackFn>,
    user_data: *mut c_void,
) {
    unsafe {
        let instance = from_handle::<MacWebView>(handle);
        instance.message_handler.ivars().callback.set(callback);
        instance.message_handler.ivars().user_data.set(user_data);
    }
}

pub fn set_before_browse_handler(
    callback: Option<BeforeBrowseCallbackFn>,
    user_data: *mut c_void,
) {
    if let Some(cb) = callback {
        store_bb_cb(cb, user_data);
    }
}
