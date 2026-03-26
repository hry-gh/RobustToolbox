use std::collections::HashMap;
use std::sync::Mutex;

use cef::Browser;

use crate::ffi_types::{PendingResponse, RnwBrowserCallbacks};

/// A single browser instance tracked by the FFI layer.
pub struct BrowserEntry {
    pub browser: Browser,
    #[allow(dead_code)]
    pub callbacks: RnwBrowserCallbacks,
}

/// Global state shared across all FFI calls.
pub struct GlobalState {
    pub browsers: HashMap<u64, BrowserEntry>,
    pub next_handle: u64,
    /// Pending resource request responses, keyed by request_id.
    pub pending_responses: HashMap<u64, PendingResponse>,
    pub next_request_id: u64,
}

impl GlobalState {
    pub fn new() -> Self {
        Self {
            browsers: HashMap::new(),
            next_handle: 1,
            pending_responses: HashMap::new(),
            next_request_id: 1,
        }
    }

    pub fn insert_browser(&mut self, browser: Browser, callbacks: RnwBrowserCallbacks) -> u64 {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.browsers.insert(
            handle,
            BrowserEntry {
                browser,
                callbacks,
            },
        );
        handle
    }

    pub fn alloc_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }
}

pub static GLOBAL: Mutex<Option<GlobalState>> = Mutex::new(None);

/// Helper to run a closure with the global state locked.
/// Returns None if CEF is not initialized.
pub fn with_state<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut GlobalState) -> R,
{
    let mut guard = GLOBAL.lock().ok()?;
    let state = guard.as_mut()?;
    Some(f(state))
}

/// Helper to run a closure with a specific browser entry.
/// Returns None if the browser handle is invalid.
pub fn with_browser<F, R>(handle: u64, f: F) -> Option<R>
where
    F: FnOnce(&mut BrowserEntry) -> R,
{
    with_state(|state| {
        state.browsers.get_mut(&handle).map(|entry| f(entry))
    })?
}
