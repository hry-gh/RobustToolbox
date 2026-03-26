use cef::*;

const SCHEME_STANDARD: i32 = 1 << 0;
const SCHEME_SECURE: i32 = 1 << 3;

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
            // NOTE: KEEP IN SYNC WITH cef-helper CODE!
            registrar.add_custom_scheme(Some(&"usr".into()), SCHEME_SECURE | SCHEME_STANDARD);
            registrar.add_custom_scheme(Some(&"res".into()), SCHEME_SECURE | SCHEME_STANDARD);
        }
    }
}

pub fn create_app() -> App {
    RobustApp::new()
}
