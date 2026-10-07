// Wi_WWAV.app: the window, menus and the bridge to the Rust core.
// The core itself (library, journal, engine supervisor, sync) is crates/wi-core.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("Wi_WWAV failed to start");
}
