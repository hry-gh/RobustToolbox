use std::ptr;

use cef::{App, ImplApp, ImplSchemeRegistrar, SchemeRegistrar, WrapApp, rc::Rc, wrap_app};
use robust_native_shared::cef_schemes::CUSTOM_SCHEMES;

fn main() {
    #[cfg(target_os = "macos")]
    {
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("cef-helper: failed to get current exe path: {e}");
                std::process::exit(1);
            }
        };
        let Some(exe_dir) = exe.parent() else {
            eprintln!("cef-helper: exe has no parent directory");
            std::process::exit(1);
        };

        // Flat layout: exe is in bin/Content.Client/, framework at bin/Frameworks/
        let flat_path = exe_dir.join(
            "../Frameworks/Chromium Embedded Framework.framework/Chromium Embedded Framework",
        );

        if flat_path.exists() {
            // Resolve and load the framework directly.
            let path = flat_path.canonicalize().unwrap_or(flat_path);
            use std::os::unix::ffi::OsStrExt;
            let Ok(cstr) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
                eprintln!("cef-helper: framework path contains null bytes");
                std::process::exit(1);
            };
            let result = unsafe { cef::load_library(Some(&*cstr.as_ptr().cast())) };
            if result != 1 {
                eprintln!(
                    "cef-helper: failed to load CEF framework from {}",
                    path.display()
                );
                std::process::exit(1);
            }
        } else {
            // Fall back to standard app bundle helper layout (../../../../Frameworks/...)
            let loader = cef::library_loader::LibraryLoader::new(&exe, true);
            if !loader.load() {
                eprintln!("cef-helper: LibraryLoader failed to load CEF framework");
                std::process::exit(1);
            }
            // SAFETY: The loader must outlive the CEF subprocess. We intentionally leak it
            // rather than dropping, which would unload the framework.
            std::mem::forget(loader);
        }
    }

    cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);

    let args = cef::args::Args::new();
    let main_args = args.as_main_args();

    let mut app = HelperApp::new();

    let ret = cef::execute_process(Some(main_args), Some(&mut app), ptr::null_mut());
    std::process::exit(ret)
}

wrap_app! {
    struct HelperApp;

    impl App {
        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            let Some(registrar) = registrar else { return };
            for (scheme, flags) in CUSTOM_SCHEMES {
                registrar.add_custom_scheme(Some(&(*scheme).into()), *flags);
            }
        }
    }
}
