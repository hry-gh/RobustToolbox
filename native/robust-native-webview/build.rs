use std::{env, path::PathBuf, process::Command};

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or("".into());

    if target_os == "macos" {
        let cef_path = env::var("DEP_CEF_DLL_WRAPPER_CEF_DIR")
            .or_else(|_| env::var("CEF_PATH"))
            .expect("Neither DEP_CEF_DLL_WRAPPER_CEF_DIR nor CEF_PATH is set");
        let cef_path = PathBuf::from(cef_path);

        let mut build = cc::Build::new();
        build
            .include(cef_path)
            .cpp(true)
            .flag("--std=c++20")
            .file("src/mac_application.mm")
            .link_lib_modifier("+whole-archive")
            .warnings(false);

        // Use the macOS SDK archiver to produce BSD-format archives.
        // GNU ar (e.g. from Homebrew binutils) produces archives
        // incompatible with macOS's -force_load linker flag.
        if let Some(ar) = find_macos_ar() {
            build.archiver(ar);
        }

        build.compile("mac_application");

        println!("cargo::rerun-if-changed=src/mac_application.mm");
        println!("cargo::rustc-link-lib=framework=Cocoa");
    }
}

fn find_macos_ar() -> Option<PathBuf> {
    // Try xcrun first (works in Xcode and Command Line Tools).
    if let Ok(output) = Command::new("xcrun").args(["--find", "ar"]).output()
        && output.status.success()
    {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    // Fall back to /usr/bin/ar which is the macOS system default.
    let fallback = PathBuf::from("/usr/bin/ar");
    if fallback.exists() {
        return Some(fallback);
    }
    None
}
