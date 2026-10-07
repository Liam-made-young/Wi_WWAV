// Wi_WWAV.app: the window, menus and the bridge to the Rust core
// (src/lib.rs). The core itself (library, journal, engine supervisor, sync)
// is crates/wi-core.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    wi_wwav_app::run();
}
