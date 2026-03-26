use std::ptr;

use cef::{App, ImplApp, ImplSchemeRegistrar, SchemeRegistrar, WrapApp, rc::Rc, wrap_app};

fn main() {
    #[cfg(target_os = "macos")]
    {
        // Load the CEF framework. Try the standard app bundle helper layout first,
        // then fall back to a flat layout where the framework is at ../Frameworks/
        // relative to the executable.
        let exe = std::env::current_exe().unwrap();
        let exe_dir = exe.parent().unwrap();

        // Flat layout: exe is in bin/Content.Client/, framework at bin/Frameworks/
        let flat_path = exe_dir
            .join("../Frameworks/Chromium Embedded Framework.framework/Chromium Embedded Framework");

        let path = if flat_path.exists() {
            flat_path.canonicalize().unwrap()
        } else {
            // Fall back to standard app bundle helper layout (../../../../Frameworks/...)
            let loader = cef::library_loader::LibraryLoader::new(&exe, true);
            assert!(loader.load());
            std::mem::forget(loader);
            // LibraryLoader handled loading, skip the manual load below
            cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);

            let args = cef::args::Args::new();
            let main_args = args.as_main_args();
            let mut app = DemoApp::new();
            let ret = cef::execute_process(Some(main_args), Some(&mut app), ptr::null_mut());
            std::process::exit(ret)
        };

        use std::os::unix::ffi::OsStrExt;
        let cstr = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        let result = unsafe { cef::load_library(Some(&*cstr.as_ptr().cast())) };
        assert_eq!(result, 1, "Failed to load CEF framework");
    }

    cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);

    let args = cef::args::Args::new();
    let main_args = args.as_main_args();

    let mut app = DemoApp::new();

    let ret = cef::execute_process(Some(main_args), Some(&mut app), ptr::null_mut());
    std::process::exit(ret)
}

// CefSchemeOptions
const SCHEME_STANDARD: i32 = 1 << 0;
const SCHEME_SECURE: i32 = 1 << 3;

wrap_app! {
    struct DemoApp {
    }

    impl App {
        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            let registrar = registrar.unwrap();
            // NOTE: KEEP IN SYNC WITH C# CODE!
            registrar.add_custom_scheme(Some(&"usr".into()), SCHEME_SECURE | SCHEME_STANDARD);
            registrar.add_custom_scheme(Some(&"res".into()), SCHEME_SECURE | SCHEME_STANDARD);
        }
    }
}
