use std::ffi::CString;
use std::os::raw::c_void;
use std::sync::Arc;
use std::sync::Mutex;

use cef::*;
use cef::rc::*;
use cef::string::CefStringUtf8;

use crate::ffi_types::PendingResponse;
use crate::state;

/// Callback type for resolving res:// scheme requests.
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

            let url_userfree = request.url();
            let url_utf16 = CefStringUtf16::from(&url_userfree);
            let url_str = CefStringUtf8::from(&url_utf16);
            let url_cstr = CString::new(url_str.as_str().unwrap_or("")).ok()?;

            let method_userfree = request.method();
            let method_utf16 = CefStringUtf16::from(&method_userfree);
            let method_str = CefStringUtf8::from(&method_utf16);
            let method_cstr = CString::new(method_str.as_str().unwrap_or("GET")).ok()?;

            let request_id = state::with_state(|s| s.alloc_request_id()).unwrap_or(0);
            if request_id == 0 {
                return None;
            }

            let handled = unsafe {
                (self.data.callback)(
                    self.data.user_data,
                    request_id,
                    url_cstr.as_ptr(),
                    method_cstr.as_ptr(),
                )
            };

            if handled != 0 {
                let response = state::with_state(|s| s.pending_responses.remove(&request_id));
                if let Some(Some(response)) = response {
                    return Some(create_buffered_resource_handler(response));
                }
            }

            None
        }
    }
}

pub fn create_res_scheme_handler_factory(data: Arc<SchemeCallbackData>) -> SchemeHandlerFactory {
    ResSchemeHandlerFactory::new(data)
}

// Inline buffered resource handler for scheme responses.
struct ResponseState {
    response: PendingResponse,
    offset: usize,
}

wrap_resource_handler! {
    struct SchemeResourceHandler {
        state: Arc<Mutex<ResponseState>>,
    }

    impl ResourceHandler {
        fn open(
            &self,
            _request: Option<&mut Request>,
            handle_request: Option<&mut ::std::os::raw::c_int>,
            _callback: Option<&mut Callback>,
        ) -> ::std::os::raw::c_int {
            if let Some(hr) = handle_request {
                *hr = 1;
            }
            1
        }

        fn response_headers(
            &self,
            response: Option<&mut Response>,
            response_length: Option<&mut i64>,
            _redirect_url: Option<&mut CefString>,
        ) {
            let lock = self.state.lock().unwrap();
            if let Some(response) = response {
                response.set_status(lock.response.status_code);
                response.set_mime_type(Some(&CefString::from(lock.response.mime_type.as_str())));
            }
            if let Some(len) = response_length {
                *len = lock.response.data.len() as i64;
            }
        }

        fn read(
            &self,
            data_out: *mut u8,
            bytes_to_read: ::std::os::raw::c_int,
            bytes_read: Option<&mut ::std::os::raw::c_int>,
            _callback: Option<&mut ResourceReadCallback>,
        ) -> ::std::os::raw::c_int {
            let mut lock = self.state.lock().unwrap();
            let remaining = lock.response.data.len() - lock.offset;
            if remaining == 0 {
                if let Some(br) = bytes_read {
                    *br = 0;
                }
                return 0;
            }

            let to_read = (bytes_to_read as usize).min(remaining);
            unsafe {
                std::ptr::copy_nonoverlapping(
                    lock.response.data.as_ptr().add(lock.offset),
                    data_out,
                    to_read,
                );
            }
            lock.offset += to_read;

            if let Some(br) = bytes_read {
                *br = to_read as i32;
            }
            1
        }
    }
}

fn create_buffered_resource_handler(response: PendingResponse) -> ResourceHandler {
    let state = Arc::new(Mutex::new(ResponseState {
        response,
        offset: 0,
    }));
    SchemeResourceHandler::new(state)
}
