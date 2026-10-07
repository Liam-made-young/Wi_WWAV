//! Updates (docs/SPEC.md 9.9): Tauri's updater reads an Ed25519-signed
//! manifest at mi-wwav.com/desktop/latest.json. A new version downloads in
//! the background and installs when you quit, never mid-playback: the status
//! bar says "Update ready · installs when you quit", and nothing else
//! happens until then. The web UI is given none of the updater's commands,
//! so only quitting can install.

use std::sync::Mutex;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::bridge;

pub const READY: &str = "Update ready · installs when you quit";

/// People leave the app open for days, so it looks again once a day.
const AGAIN: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Default)]
struct Pending(Mutex<Option<(Update, Vec<u8>)>>);

/// The updater's public key from tauri.conf.json; empty until the founder's
/// key is made (tools/release/README.md).
pub fn pubkey<R: Runtime>(app: &AppHandle<R>) -> String {
    let plugins = &app.config().plugins.0;
    let key = plugins
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str());
    key.unwrap_or_default().to_string()
}

/// Starts looking for updates. A development build, or one with no key to
/// check signatures against, never looks.
pub fn watch<R: Runtime>(app: &AppHandle<R>) {
    app.manage(Pending::default());
    if cfg!(debug_assertions) || pubkey(app).is_empty() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || loop {
        match tauri::async_runtime::block_on(fetch(&app)) {
            Ok(Some(ready)) => {
                *app.state::<Pending>().0.lock().unwrap() = Some(ready);
                bridge::emit(
                    &app,
                    "status",
                    json!({ "area": "update", "sentence": READY }),
                );
                return;
            }
            Ok(None) => {}
            // Offline, or the server is down: say nothing and look tomorrow.
            Err(e) => eprintln!("wi-wwav: couldn't look for an update: {e}"),
        }
        std::thread::sleep(AGAIN);
    });
}

async fn fetch<R: Runtime>(
    app: &AppHandle<R>,
) -> tauri_plugin_updater::Result<Option<(Update, Vec<u8>)>> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(None);
    };
    let bytes = update.download(|_, _| {}, || {}).await?;
    Ok(Some((update, bytes)))
}

/// Called as the app quits: installs a downloaded update, if there is one.
pub fn install_on_quit<R: Runtime>(app: &AppHandle<R>) {
    let Some(pending) = app.try_state::<Pending>() else {
        return;
    };
    let Some((update, bytes)) = pending.0.lock().unwrap().take() else {
        return;
    };
    if let Err(e) = update.install(bytes) {
        eprintln!(
            "wi-wwav: the update to {} didn't install: {e}",
            update.version
        );
    }
}
