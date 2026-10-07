//! Wi_WWAV.app: the window, the menus and the bridge to the Rust core
//! (docs/SPEC.md 9.1). The core itself (library, journal, engine supervisor,
//! sync, the account) is `crates/wi-core`; this crate hosts it.

mod bridge;
mod core_link;
mod menu;
mod open;
mod paths;
mod secrets;
mod update;

use std::sync::Mutex;

use tauri::{AppHandle, Builder, Manager, RunEvent, Runtime, WebviewWindowBuilder};

pub use bridge::{forward_event, forward_meters, Allow, Bridge, Core, CoreError};
pub use menu::{History, CREDITS, ROOMS};
pub use open::EXTENSIONS;
pub use paths::engine_beside;
pub use secrets::{KeyringStore, NO_STORE, SERVICE};
pub use update::READY;

/// The shell's state and its one command, without the plugins and menus that
/// need a real window system. The tests build this on Tauri's mock runtime.
pub fn shell<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder
        .manage(Bridge::new())
        .manage(Mutex::new(menu::EditState::default()))
        .invoke_handler(tauri::generate_handler![bridge::core])
}

pub fn run() {
    // Linux and Windows start a second copy to open a file; it hands the
    // file to this one and leaves, so one library has one engine.
    let builder =
        Builder::default().plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            open::route(app, open::from_args(args));
        }));
    let app = shell(builder)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .menu(menu::build)
        .on_menu_event(menu::on_event)
        .setup(|app| {
            let handle = app.handle().clone();
            // The window draws while the core opens (the 1.5 s budget, 9.13).
            std::thread::spawn(move || {
                let core = core_link::open(&handle);
                handle.state::<Bridge>().set_core(core);
            });
            open::route(app.handle(), open::from_args(std::env::args()));
            update::watch(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Wi_WWAV failed to start");

    app.run(|app, event| match event {
        // macOS hands over files opened from Finder or dropped on the Dock.
        #[cfg(target_os = "macos")]
        RunEvent::Opened { urls } => {
            open::route(app, urls.iter().filter_map(|u| u.to_file_path().ok()))
        }
        RunEvent::Exit => update::install_on_quit(app),
        _ => {}
    });
}

/// Settings… (2.13): the settings window tauri.conf.json describes, or the
/// one already open.
pub fn open_settings<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("settings") {
        window.unminimize()?;
        return window.set_focus();
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "settings");
    let config = config.expect("tauri.conf.json describes the settings window");
    WebviewWindowBuilder::from_config(app, config)?.build()?;
    Ok(())
}
