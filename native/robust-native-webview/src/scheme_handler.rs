use std::os::raw::c_void;
use std::sync::Arc;

use cef::*;

use crate::ffi_types::ResponseContext;
use crate::{cef_userfree_to_cstring, resource_handler};

/// Scheme handler callback. C# writes response via the provided ResponseContext pointer.
pub type ResSchemeCallback = unsafe extern "C" fn(
    user_data: *mut c_void,
    url: *const std::ffi::c_char,
    method: *const std::ffi::c_char,
    response_ctx: *mut ResponseContext,
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

            // Create response context on the stack; C# writes to it via rnw_response_write
            let mut response_ctx = ResponseContext::new();

            let handled = unsafe {
                (self.data.callback)(
                    self.data.user_data,
                    url_cstr.as_ptr(),
                    method_cstr.as_ptr(),
                    &mut response_ctx,
                )
            };

            if handled != 0 {
                if response_ctx.was_set {
                    return Some(resource_handler::create_buffered_resource_handler(response_ctx));
                }
                // C# returned handled=1 but didn't write a response.
                eprintln!(
                    "[rnw] WARNING: scheme handler returned handled=1 but no response was set for: {}",
                    url_cstr.to_string_lossy()
                );
            }

            None
        }
    }
}

pub fn create_res_scheme_handler_factory(data: Arc<SchemeCallbackData>) -> SchemeHandlerFactory {
    ResSchemeHandlerFactory::new(data)
}
