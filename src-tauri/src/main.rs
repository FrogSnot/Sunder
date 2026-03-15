#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // AppImage compatibility
    if std::env::var("APPIMAGE").is_ok() {
        std::env::set_var("NO_AT_BRIDGE", "1");
    }
    sunder_lib::run()
}
