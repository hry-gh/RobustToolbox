use std::sync::Arc;

use cef::*;

use crate::cef_userfree_to_cstring;
use crate::ffi_types::ResponseContext;
use crate::render_handler::CallbackData;
use crate::resource_handler;

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

            let Some(url_cstr) = cef_userfree_to_cstring(&request.url(), "") else {
                return 0;
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
            let cb = self.data.callbacks.on_resource_request?;
            let request = request?;

            let url_cstr = cef_userfree_to_cstring(&request.url(), "")?;

            // Deny file:// access.
            if url_cstr.to_bytes().starts_with(b"file://") {
                if let Some(ddh) = disable_default_handling {
                    *ddh = 1;
                }
                return None;
            }
            let method_cstr = cef_userfree_to_cstring(&request.method(), "GET")?;

            let mut response_ctx = ResponseContext::new();

            let handled = unsafe {
                cb(
                    self.data.callbacks.user_data,
                    url_cstr.as_ptr(),
                    method_cstr.as_ptr(),
                    &mut response_ctx,
                )
            };

            if handled != 0 {
                if response_ctx.was_set {
                    if let Some(ddh) = disable_default_handling {
                        *ddh = 1;
                    }
                    return Some(resource_handler::create_resource_request_handler(response_ctx));
                }
                // C# returned handled=1 but didn't write a response.
                eprintln!(
                    "[rnw] WARNING: on_resource_request returned handled=1 but no response was set for: {}",
                    url_cstr.to_string_lossy()
                );
            }

            None
        }
    }
}

pub fn create_request_handler(data: Arc<CallbackData>) -> RequestHandler {
    RobustRequestHandler::new(data)
}
