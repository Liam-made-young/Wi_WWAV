//! Opens the core (crates/wi-core) on the library, with the engine beside
//! the app, and passes its events and meters to the window.

use std::sync::Arc;

use tauri::{AppHandle, Runtime};

use crate::bridge::{Core, CoreError};

pub fn open<R: Runtime>(_app: &AppHandle<R>) -> Result<Arc<dyn Core>, CoreError> {
    Err(CoreError::new(
        "no_core",
        "This build of Wi_WWAV has no core in it.",
    ))
}
