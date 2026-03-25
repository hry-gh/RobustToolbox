fn main() {
    // Only CEF needs cc compilation (mac_application.mm)
    #[cfg(feature = "cef")]
    {
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        if target_os == "macos" {
            if let Some(cef_path) = std::env::var_os("CEF_PATH") {
                let cef_path = std::path::PathBuf::from(cef_path);
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
    }
}
