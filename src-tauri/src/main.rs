#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        clean_studio_lib::run_mcp()
    } else {
        clean_studio_lib::run()
    }
}
