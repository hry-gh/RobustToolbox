use std::os::raw::c_void;
use std::sync::Arc;

use cef::*;

use crate::{cef_userfree_to_cstring, resource_handler, state};

/// Called synchronously; C# must call rnw_request_set_response before returning.
pub type ResSchemeCallback = unsafe extern "C" fn(
    user_data: *mut c_void,
    request_id: u64,
    url: *const std::ffi::c_char,
    method: *const std::ffi::c_char,
) -> i32;

pub struct SchemeCallbackData {
    pub callback: ResSchemeCallback,
    pub user_data: *mut c_void,
}

unsafe impl Send for SchemeCallbackData {}
unsafe impl Sync for SchemeCallbackData {}

wrap_scheme_handler_factory! {
    struct ResSchemeHandlerFactory {
        data: Arc<SchemeCallbackData>,
    }

    impl SchemeHandlerFactory {
        fn create(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _scheme_name: Option<&CefString>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let request = request?;

            let url_cstr = cef_userfree_to_cstring(&request.url(), "")?;
            let method_cstr = cef_userfree_to_cstring(&request.method(), "GET")?;

            let handled = unsafe {
                (self.data.callback)(
                    self.data.user_data,
                    0,
                    url_cstr.as_ptr(),
                    method_cstr.as_ptr(),
                )
            };

            if handled != 0 {
                let response = state::PENDING_RESPONSE.with(|cell| cell.borrow_mut().take());
                if let Some(response) = response {
                    return Some(resource_handler::create_buffered_resource_handler(response));
                }
            }

            None
        }
    }
}

pub fn create_res_scheme_handler_factory(data: Arc<SchemeCallbackData>) -> SchemeHandlerFactory {
    ResSchemeHandlerFactory::new(data)
}

