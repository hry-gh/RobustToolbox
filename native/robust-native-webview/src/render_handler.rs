use std::sync::Arc;

use cef::*;

use crate::ffi_types::RnwBrowserCallbacks;

pub struct CallbackData {
    pub callbacks: RnwBrowserCallbacks,
}

// Safety: function pointers + opaque user_data, C# handles thread safety
unsafe impl Send for CallbackData {}
unsafe impl Sync for CallbackData {}

wrap_render_handler! {
    struct RobustRenderHandler {
        data: Arc<CallbackData>,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            let Some(rect) = rect else { return };
            let Some(get_view_rect) = self.data.callbacks.get_view_rect else { return };
            let mut w: i32 = 1;
            let mut h: i32 = 1;
            unsafe { get_view_rect(self.data.callbacks.user_data, &mut w, &mut h) };
            rect.x = 0;
            rect.y = 0;
            rect.width = w.max(1);
            rect.height = h.max(1);
        }

        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> ::std::os::raw::c_int {
            let Some(screen_info) = screen_info else { return 0 };
            let Some(get_screen_info) = self.data.callbacks.get_screen_info else { return 0 };
            let mut scale: f32 = 1.0;
            unsafe { get_screen_info(self.data.callbacks.user_data, &mut scale) };
            screen_info.device_scale_factor = scale;
            1
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            _type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            let Some(on_paint) = self.data.callbacks.on_paint else { return };
            let Some(rects) = dirty_rects else { return };

            // Pack dirty rects into flat i32 array: [x0, y0, w0, h0, x1, y1, w1, h1, ...]
            let mut flat_rects: Vec<i32> = Vec::with_capacity(rects.len() * 4);
            for r in rects {
                flat_rects.push(r.x);
                flat_rects.push(r.y);
                flat_rects.push(r.width);
                flat_rects.push(r.height);
            }

            unsafe {
                on_paint(
                    self.data.callbacks.user_data,
                    width,
                    height,
                    buffer,
                    rects.len() as i32,
                    flat_rects.as_ptr(),
                );
            }
        }

        fn on_virtual_keyboard_requested(
            &self,
            _browser: Option<&mut Browser>,
            input_mode: TextInputMode,
        ) {
            let Some(cb) = self.data.callbacks.on_virtual_keyboard_requested else { return };
            let mode_raw: &cef::sys::cef_text_input_mode_t = input_mode.as_ref();
            unsafe { cb(self.data.callbacks.user_data, *mode_raw as i32) };
        }
    }
}

pub fn create_render_handler(data: Arc<CallbackData>) -> RenderHandler {
    RobustRenderHandler::new(data)
}
