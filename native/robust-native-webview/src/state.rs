use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;

use cef::Browser;

use crate::ffi_types::PendingResponse;

pub struct GlobalState {
    pub browsers: HashMap<u64, Browser>,
    pub next_handle: u64,
}

impl GlobalState {
    pub fn new() -> Self {
        Self {
            browsers: HashMap::new(),
            next_handle: 1,
        }
    }

    pub fn insert_browser(&mut self, browser: Browser) -> u64 {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.browsers.insert(handle, browser);
        handle
    }
}

pub static GLOBAL: Mutex<Option<GlobalState>> = Mutex::new(None);

// Thread-local slot for passing a response from C# back to the calling Rust code.
thread_local! {
    pub static PENDING_RESPONSE: RefCell<Option<PendingResponse>> = const { RefCell::new(None) };
}

pub fn with_state<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut GlobalState) -> R,
{
    let mut guard = GLOBAL.lock().ok()?;
    Some(f(guard.as_mut()?))
}

/// The lock is released before returning, so it's safe to call
/// CEF methods that might trigger callbacks back into Rust.
pub fn get_browser(handle: u64) -> Option<Browser> {
    let guard = GLOBAL.lock().ok()?;
    guard.as_ref()?.browsers.get(&handle).cloned()
}
