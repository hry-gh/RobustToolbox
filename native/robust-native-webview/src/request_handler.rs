use std::sync::Arc;

use cef::*;

use crate::cef_userfree_to_cstring;
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

            let Some(url_cstr) = cef_userfree_to_cstring(&request.url(), "") else {
                return 0;
            };

            let result = unsafe {
                cb(
                    self.data.callbacks.user_data,
                    url_cstr.as_ptr(),
                    user_gesture,
                    is_redirect,
                )
            };
            result
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

            let url_cstr = cef_userfree_to_cstring(&request.url(), "")?;
            let method_cstr = cef_userfree_to_cstring(&request.method(), "GET")?;

            let handled = unsafe {
                cb(
                    self.data.callbacks.user_data,
                    0,
                    url_cstr.as_ptr(),
                    method_cstr.as_ptr(),
                )
            };

            if handled != 0 {
                let response = state::PENDING_RESPONSE.with(|cell| cell.borrow_mut().take());
                if let Some(response) = response {
                    if let Some(ddh) = disable_default_handling {
                        *ddh = 1;
                    }
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
