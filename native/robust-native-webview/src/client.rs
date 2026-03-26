use std::sync::Arc;

use cef::*;
use cef::rc::*;

use crate::life_span_handler;
use crate::load_handler;
use crate::render_handler::{self, CallbackData};
use crate::request_handler;

wrap_client! {
    struct RobustClient {
        data: Arc<CallbackData>,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            eprintln!("[rnw] client: render_handler requested");
            Some(render_handler::create_render_handler(self.data.clone()))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(load_handler::create_load_handler(self.data.clone()))
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(request_handler::create_request_handler(self.data.clone()))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(life_span_handler::create_life_span_handler(self.data.clone()))
        }
    }
}

pub fn create_client(data: Arc<CallbackData>) -> Client {
    RobustClient::new(data)
}
