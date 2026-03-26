use std::ffi::CString;
use std::sync::Arc;

use cef::*;
use cef::string::CefStringUtf8;

use crate::render_handler::CallbackData;
use crate::resource_handler;
use crate::state;

wrap_request_handler! {
    struct RobustRequestHandler {
        data: Arc<CallbackData>,
    }

    impl RequestHandler {
        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            user_gesture: ::std::os::raw::c_int,
            is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let Some(cb) = self.data.callbacks.on_before_browse else { return 0 };
            let Some(request) = request else { return 0 };

            let url_userfree = request.url();
            let url_utf16 = CefStringUtf16::from(&url_userfree);
            let url_str = CefStringUtf8::from(&url_utf16);
            let url_cstr = match CString::new(url_str.as_str().unwrap_or("")) {
                Ok(s) => s,
                Err(_) => return 0,
            };

            unsafe {
                cb(
                    self.data.callbacks.user_data,
                    url_cstr.as_ptr(),
                    user_gesture,
                    is_redirect,
                )
            }
        }

        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _is_navigation: ::std::os::raw::c_int,
            _is_download: ::std::os::raw::c_int,
            _request_initiator: Option<&CefString>,
            disable_default_handling: Option<&mut ::std::os::raw::c_int>,
        ) -> Option<ResourceRequestHandler> {
            let Some(cb) = self.data.callbacks.on_resource_request else {
                return None;
            };
            let Some(request) = request else { return None };

            let url_userfree = request.url();
            let url_str = {
                let s = CefStringUtf16::from(&url_userfree);
                CefStringUtf8::from(&s)
            };
            let url_cstr = match CString::new(url_str.as_str().unwrap_or("")) {
                Ok(s) => s,
                Err(_) => return None,
            };

            let method_userfree = request.method();
            let method_str = {
                let s = CefStringUtf16::from(&method_userfree);
                CefStringUtf8::from(&s)
            };
            let method_cstr = match CString::new(method_str.as_str().unwrap_or("GET")) {
                Ok(s) => s,
                Err(_) => return None,
            };

            // Allocate a request ID for this request
            let request_id = state::with_state(|s| s.alloc_request_id()).unwrap_or(0);
            if request_id == 0 {
                return None;
            }

            let handled = unsafe {
                cb(
                    self.data.callbacks.user_data,
                    request_id,
                    url_cstr.as_ptr(),
                    method_cstr.as_ptr(),
                )
            };

            if handled != 0 {
                let response = state::with_state(|s| s.pending_responses.remove(&request_id));
                if let Some(Some(response)) = response {
                    if let Some(ddh) = disable_default_handling {
                        *ddh = 1;
                    }
                    eprintln!("[rnw] resource_request_handler: returning handler, data_len={}", response.data.len());
                    return Some(resource_handler::create_resource_request_handler(response));
                }
            }

            None
        }
    }
}

pub fn create_request_handler(data: Arc<CallbackData>) -> RequestHandler {
    RobustRequestHandler::new(data)
}
