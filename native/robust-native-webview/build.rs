use std::{env, path::PathBuf};

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or("".into());

    if target_os == "macos" {
        // Get the CEF directory from cef-dll-sys's cargo metadata, or fall back to CEF_PATH env var.
        let cef_path = env::var("DEP_CEF_DLL_WRAPPER_CEF_DIR")
            .or_else(|_| env::var("CEF_PATH"))
            .expect("Neither DEP_CEF_DLL_WRAPPER_CEF_DIR nor CEF_PATH is set");
        let cef_path = PathBuf::from(cef_path);

        cc::Build::new()
            .include(cef_path)
            .cpp(true)
            .flag("--std=c++17")
            .file("src/mac_application.mm")
            // Use macOS native ar to produce BSD-format archives.
            // GNU ar (e.g. from Homebrew binutils) produces archives
            // incompatible with macOS's -force_load linker flag.
            .archiver("/usr/bin/ar")
            .link_lib_modifier("+whole-archive")
            .warnings(false)
            .compile("mac_application");

        println!("cargo::rerun-if-changed=src/mac_application.mm");
        println!("cargo::rustc-link-lib=framework=Cocoa");
    }
}
