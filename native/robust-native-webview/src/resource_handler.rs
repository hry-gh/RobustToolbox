use std::sync::Arc;
use std::sync::Mutex;

use cef::*;

use crate::ffi_types::ResponseContext;

struct ResponseState {
    status_code: i32,
    mime_type: String,
    data: Vec<u8>,
    offset: usize,
}

impl From<ResponseContext> for ResponseState {
    fn from(ctx: ResponseContext) -> Self {
        Self {
            status_code: ctx.status_code,
            mime_type: ctx.mime_type,
            data: ctx.data,
            offset: 0,
        }
    }
}

wrap_resource_handler! {
    struct BufferedResourceHandler {
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
            let Ok(lock) = self.state.lock() else { return };
            if let Some(response) = response {
                response.set_status(lock.status_code);
                response.set_mime_type(Some(&CefString::from(lock.mime_type.as_str())));
            }
            if let Some(len) = response_length {
                *len = lock.data.len() as i64;
            }
        }

        fn read(
            &self,
            data_out: *mut u8,
            bytes_to_read: ::std::os::raw::c_int,
            bytes_read: Option<&mut ::std::os::raw::c_int>,
            _callback: Option<&mut ResourceReadCallback>,
        ) -> ::std::os::raw::c_int {
            let Ok(mut lock) = self.state.lock() else {
                if let Some(br) = bytes_read { *br = 0; }
                return 0;
            };
            let remaining = lock.data.len() - lock.offset;
            if remaining == 0 {
                if let Some(br) = bytes_read {
                    *br = 0;
                }
                return 0; // done
            }

            let to_read = (bytes_to_read as usize).min(remaining);
            unsafe {
                std::ptr::copy_nonoverlapping(
                    lock.data.as_ptr().add(lock.offset),
                    data_out,
                    to_read,
                );
            }
            lock.offset += to_read;

            if let Some(br) = bytes_read {
                *br = to_read as i32;
            }
            1 // continue
        }
    }
}

pub fn create_buffered_resource_handler(response: ResponseContext) -> ResourceHandler {
    let state = Arc::new(Mutex::new(ResponseState::from(response)));
    BufferedResourceHandler::new(state)
}

wrap_resource_request_handler! {
    struct BufferedResourceRequestHandler {
        response: Arc<Mutex<Option<ResponseContext>>>,
    }

    impl ResourceRequestHandler {
        fn cookie_access_filter(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
        ) -> Option<CookieAccessFilter> {
            Some(AllowAllCookies::new())
        }

        fn on_before_resource_load(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            ReturnValue::from(cef::sys::cef_return_value_t::RV_CONTINUE)
        }

        fn resource_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let response = self.response.lock().ok()?.take()?;
            Some(create_buffered_resource_handler(response))
        }
    }
}

wrap_cookie_access_filter! {
    struct AllowAllCookies;

    impl CookieAccessFilter {
        fn can_send_cookie(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _cookie: Option<&Cookie>,
        ) -> ::std::os::raw::c_int {
            1
        }

        fn can_save_cookie(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _response: Option<&mut Response>,
            _cookie: Option<&Cookie>,
        ) -> ::std::os::raw::c_int {
            1
        }
    }
}

pub fn create_resource_request_handler(response: ResponseContext) -> ResourceRequestHandler {
    let response = Arc::new(Mutex::new(Some(response)));
    BufferedResourceRequestHandler::new(response)
}
