//! Opens the core (crates/wi-core) on the library, with the engine beside
//! the app and the keychain for its secrets, and passes its events and
//! meters to the main window.

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

use crate::bridge::{self, Core, CoreError};
use crate::paths;
use crate::secrets::KeyringStore;

impl Core for wi_core::Core {
    fn invoke(&self, cmd: &str, args: Value) -> Result<Value, CoreError> {
        wi_core::Core::invoke(self, cmd, args)
    }
}

impl wi_core::SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        KeyringStore::get(self, key).map_err(|e| e.message)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        KeyringStore::set(self, key, value).map_err(|e| e.message)
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        KeyringStore::delete(self, key).map_err(|e| e.message)
    }
}

pub fn open<R: Runtime>(app: &AppHandle<R>) -> Result<Arc<dyn Core>, CoreError> {
    let browser = app.clone();
    // Sign-in opens www.wi-wwav.com in the system browser (9.7).
    let opener: wi_core::Opener = Arc::new(move |url: &str| {
        browser
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|e| e.to_string())
    });
    let mut config = wi_core::Config::new(
        paths::engine(),
        &paths::server(),
        Arc::new(KeyringStore::default()),
        opener,
    );
    config.sign_in_url = paths::sign_in();
    // Tasks a sync brings that no type matches are estimated by Claude, once each.
    config.auto_score = true;
    // The app reads mail on its own while it is open (3.10); a test's core doesn't.
    config.background_mail = true;
    let library = paths::library(app.path().audio_dir().ok());
    // Notes are markdown files in a folder beside the library file, and a
    // photo put in a capture inbox becomes one (docs/NOTES.md). The app may
    // build the text reader, ask Claude about a page when the person lets
    // it, and say when it is time to leave for something.
    config.notes_dir = Some(library.join("Notes"));
    config.capture_inboxes = wi_core::icloud_capture_inboxes();
    config.capture_claude = true;
    config.ocr_build = true;
    config.leave_notices = true;
    let core = wi_core::Core::open(&library, config)?;

    let (events, meters) = (core.events(), core.meters());
    let to_window = app.clone();
    std::thread::spawn(move || {
        for e in events {
            bridge::forward_event(&to_window, &e.event, e.payload);
        }
    });
    let to_window = app.clone();
    std::thread::spawn(move || {
        for frame in meters {
            bridge::forward_meters(&to_window, frame);
        }
    });
    Ok(Arc::new(core))
}
