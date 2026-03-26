use std::ffi::c_char;
use std::os::raw::c_void;

pub const RNW_HANDLE_INVALID: u64 = 0;
pub const RNW_ERROR: i32 = -1;

#[repr(C)]
pub struct RnwSettings {
    pub no_sandbox: i32,
    pub subprocess_path: *const c_char,     // null on macOS
    pub resources_dir_path: *const c_char,  // null on macOS
    pub locales_dir_path: *const c_char,    // null on macOS
    pub remote_debugging_port: i32,
    pub cache_path: *const c_char,
    pub user_agent: *const c_char,          // null if not overridden
    pub cookieable_schemes: *const c_char,  // e.g. "usr,res"
    /// macOS only: path to the Chromium Embedded Framework dylib.
    /// If null, uses LibraryLoader with current_exe-relative path.
    pub framework_path: *const c_char,
    /// macOS only: path to the framework directory (the .framework folder).
    pub framework_dir_path: *const c_char,
    /// macOS only: path to the main bundle (app .app directory).
    pub main_bundle_path: *const c_char,
}

#[repr(C)]
pub struct RnwKeyEvent {
    /// 0 = RawKeyDown, 1 = KeyUp, 2 = Char
    pub event_type: i32,
    pub windows_key_code: i32,
    pub native_key_code: i32,
    pub character: u16,
    pub unmodified_character: u16,
    pub modifiers: u32,
    pub is_system_key: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RnwBrowserCallbacks {
    pub user_data: *mut c_void,

    /// dirty_rects is a flat array of [x, y, w, h, ...] with dirty_count entries.
    pub on_paint: Option<
        unsafe extern "C" fn(
            user_data: *mut c_void,
            width: i32,
            height: i32,
            buffer: *const u8,
            dirty_count: i32,
            dirty_rects: *const i32,
        ),
    >,

    pub get_view_rect:
        Option<unsafe extern "C" fn(user_data: *mut c_void, out_width: *mut i32, out_height: *mut i32)>,

    pub get_screen_info:
        Option<unsafe extern "C" fn(user_data: *mut c_void, out_scale: *mut f32)>,

    pub on_virtual_keyboard_requested:
        Option<unsafe extern "C" fn(user_data: *mut c_void, input_mode: i32)>,

    /// Return 1 to cancel, 0 to allow.
    pub on_before_browse: Option<
        unsafe extern "C" fn(
            user_data: *mut c_void,
            url: *const c_char,
            user_gesture: i32,
            is_redirect: i32,
        ) -> i32,
    >,

    /// Return 1 if handled (caller must call rnw_request_set_response before returning),
    /// 0 to let CEF handle it.
    pub on_resource_request: Option<
        unsafe extern "C" fn(
            user_data: *mut c_void,
            request_id: u64,
            url: *const c_char,
            method: *const c_char,
        ) -> i32,
    >,

    pub on_load_start: Option<unsafe extern "C" fn(user_data: *mut c_void)>,
    pub on_load_end: Option<unsafe extern "C" fn(user_data: *mut c_void, http_status_code: i32)>,
    pub on_before_close: Option<unsafe extern "C" fn(user_data: *mut c_void)>,
}

// Safety: The callbacks are function pointers with a user_data context.
// The C# side is responsible for thread safety of the user_data.
unsafe impl Send for RnwBrowserCallbacks {}
unsafe impl Sync for RnwBrowserCallbacks {}

/// Set by C# before returning from on_resource_request.
pub struct PendingResponse {
    pub status_code: i32,
    pub mime_type: String,
    pub data: Vec<u8>,
}
