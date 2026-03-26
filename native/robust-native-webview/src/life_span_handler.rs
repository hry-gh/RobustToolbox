use std::sync::Arc;

use cef::*;

use crate::render_handler::CallbackData;

wrap_life_span_handler! {
    struct RobustLifeSpanHandler {
        data: Arc<CallbackData>,
    }

    impl LifeSpanHandler {
        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            let Some(cb) = self.data.callbacks.on_before_close else { return };
            unsafe { cb(self.data.callbacks.user_data) };
        }
    }
}

pub fn create_life_span_handler(data: Arc<CallbackData>) -> LifeSpanHandler {
    RobustLifeSpanHandler::new(data)
}
