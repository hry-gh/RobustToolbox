//! Shared constants and utilities for Robust native crates.

/// CEF scheme flags (from cef_scheme_options_t).
pub mod cef_schemes {
    /// Standard scheme (allows relative URLs, etc.)
    pub const SCHEME_STANDARD: i32 = 1 << 0;
    /// Secure scheme (treated as HTTPS-equivalent)
    pub const SCHEME_SECURE: i32 = 1 << 3;

    /// Custom schemes registered with CEF for resource loading.
    /// Both the main process (robust-native-webview) and subprocess (cef-helper)
    /// must register identical schemes in on_register_custom_schemes.
    pub const CUSTOM_SCHEMES: &[(&str, i32)] = &[
        ("usr", SCHEME_SECURE | SCHEME_STANDARD),
        ("res", SCHEME_SECURE | SCHEME_STANDARD),
    ];
}
