use std::sync::Arc;
use std::sync::Mutex;

use cef::*;

use crate::ffi_types::PendingResponse;

struct ResponseState {
    response: PendingResponse,
    offset: usize,
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
                return 0; // done
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
            1 // continue
        }
    }
}

pub fn create_buffered_resource_handler(response: PendingResponse) -> ResourceHandler {
    let state = Arc::new(Mutex::new(ResponseState {
        response,
        offset: 0,
    }));
    BufferedResourceHandler::new(state)
}

wrap_resource_request_handler! {
    struct BufferedResourceRequestHandler {
        response: Arc<Mutex<Option<PendingResponse>>>,
    }

    impl ResourceRequestHandler {
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
            let response = self.response.lock().unwrap().take()?;
            Some(create_buffered_resource_handler(response))
        }
    }
}

pub fn create_resource_request_handler(response: PendingResponse) -> ResourceRequestHandler {
    let response = Arc::new(Mutex::new(Some(response)));
    BufferedResourceRequestHandler::new(response)
}
