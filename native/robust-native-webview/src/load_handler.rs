use std::sync::Arc;

use cef::*;
use cef::rc::*;

use crate::render_handler::CallbackData;

wrap_load_handler! {
    struct RobustLoadHandler {
        data: Arc<CallbackData>,
    }

    impl LoadHandler {
        fn on_load_start(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _transition_type: TransitionType,
        ) {
            eprintln!("[rnw] on_load_start");
            let Some(cb) = self.data.callbacks.on_load_start else { return };
            unsafe { cb(self.data.callbacks.user_data) };
        }

        fn on_load_end(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            http_status_code: ::std::os::raw::c_int,
        ) {
            eprintln!("[rnw] on_load_end: status={http_status_code}");
            let Some(cb) = self.data.callbacks.on_load_end else { return };
            unsafe { cb(self.data.callbacks.user_data, http_status_code) };
        }

        fn on_load_error(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            eprintln!("[rnw] on_load_error: url={failed_url:?} err={error_text:?}");
        }
    }
}

pub fn create_load_handler(data: Arc<CallbackData>) -> LoadHandler {
    RobustLoadHandler::new(data)
}
