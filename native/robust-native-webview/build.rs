use std::env;

#[cfg(feature = "cef")]
use std::path::PathBuf;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    #[cfg(feature = "cef")]
    if target_os == "macos" {
        if let Some(cef_path) = env::var_os("CEF_PATH") {
            let cef_path = PathBuf::from(cef_path);
            cc::Build::new()
                .include(&cef_path)
                .cpp(true)
                .flag("--std=c++17")
                .file("src/mac_application.mm")
                .link_lib_modifier("+whole-archive")
                .warnings(false)
                .compile("mac_application");

            println!("cargo::rerun-if-changed=src/mac_application.mm");
            println!("cargo::rustc-link-lib=framework=Cocoa");
        }
    }

    match target_os.as_str() {
        "windows" => build_windows(),
        "macos" => build_macos(),
        "linux" => build_linux(),
        _ => {}
    }
}

fn build_windows() {
    cc::Build::new()
        .cpp(true)
        .flag("/std:c++17")
        .file("src/webview_win.cpp")
        .compile("webview_win");

    println!("cargo::rerun-if-changed=src/webview_win.cpp");
    println!("cargo::rustc-link-lib=static=WebView2LoaderStatic");
}

fn build_macos() {
    cc::Build::new()
        .file("src/webview_mac.m")
        .flag("-fobjc-arc")
        .archiver("/usr/bin/ar")
        .compile("webview_mac");

    println!("cargo::rerun-if-changed=src/webview_mac.m");
    println!("cargo::rustc-link-lib=framework=WebKit");
    println!("cargo::rustc-link-lib=framework=Foundation");
    println!("cargo::rustc-link-lib=framework=CoreFoundation");
}

#[cfg(target_os = "linux")]
fn build_linux() {
    let webkit = pkg_config::probe_library("webkit2gtk-4.1")
        .or_else(|_| pkg_config::probe_library("webkit2gtk-4.0"))
        .expect("webkit2gtk-4.1 or webkit2gtk-4.0 required");

    let mut build = cc::Build::new();
    build.file("src/webview_linux.c");

    for path in &webkit.include_paths {
        build.include(path);
    }

    build.compile("webview_linux");

    println!("cargo::rerun-if-changed=src/webview_linux.c");
}

#[cfg(not(target_os = "linux"))]
fn build_linux() {
    // Only used when cross-compiling or running build.rs on non-Linux
}
