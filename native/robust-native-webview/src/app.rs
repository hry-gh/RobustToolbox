use cef::*;
use robust_native_shared::cef_schemes::CUSTOM_SCHEMES;

wrap_app! {
    struct RobustApp;

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(cmd) = command_line else { return };

            cmd.append_switch(Some(&"--no-zygote".into()));
            cmd.append_switch(Some(&"--off-screen-rendering-enabled".into()));
            cmd.append_switch_with_value(
                Some(&"disable-threaded-scrolling".into()),
                Some(&"1".into()),
            );
            cmd.append_switch_with_value(
                Some(&"disable-features".into()),
                Some(&"TouchpadAndWheelScrollLatching,AsyncWheelEvents".into()),
            );
            // Avoid macOS Keychain access prompts.
            cmd.append_switch(Some(&"--use-mock-keychain".into()));
            cmd.append_switch(Some(&"--disable-background-networking".into()));
        }

        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            let Some(registrar) = registrar else { return };
            for (scheme, flags) in CUSTOM_SCHEMES {
                registrar.add_custom_scheme(Some(&(*scheme).into()), *flags);
            }
        }
    }
}

pub fn create_app() -> App {
    RobustApp::new()
}
