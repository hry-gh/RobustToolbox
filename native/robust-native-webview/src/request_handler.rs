use std::ffi::CString;
use std::sync::Arc;

use cef::*;
use cef::rc::*;
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
            // Temporarily disabled to debug resource_request_handler issue
            0
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
                eprintln!("[rnw] resource_request_handler: callback is None");
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

            eprintln!("[rnw] resource_request_handler: url={} is_nav={_is_navigation} is_dl={_is_download}", url_str.as_str().unwrap_or("?"));

            let handled = unsafe {
                cb(
                    self.data.callbacks.user_data,
                    request_id,
                    url_cstr.as_ptr(),
                    method_cstr.as_ptr(),
                )
            };

            eprintln!("[rnw] resource_request_handler: handled={handled}");

            if handled != 0 {
                // C# has called rnw_request_set_response before returning.
                // Retrieve the response data and store it for this request.
                let response = state::with_state(|s| s.pending_responses.remove(&request_id));
                eprintln!("[rnw] resource_request_handler: response present={}", response.as_ref().map(|r| r.is_some()).unwrap_or(false));
                if let Some(Some(response)) = response {
                    // Store the response in the per-browser pending data so the
                    // ResourceRequestHandler can retrieve it.
                    let handler = resource_handler::create_resource_request_handler_with_logging(response);
                    let raw_ptr = ImplResourceRequestHandler::get_raw(&handler);
                    eprintln!("[rnw] resource_request_handler: returning handler ptr={raw_ptr:?}");
                    return Some(handler);
                }
            }

            None
        }
    }
}

pub fn create_request_handler(data: Arc<CallbackData>) -> RequestHandler {
    RobustRequestHandler::new(data)
}
