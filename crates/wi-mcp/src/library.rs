//! The backend Claude's calls reach: the library the app uses, through
//! wi-heat-store, one transaction per call (docs/SPEC.md 8.8).

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use wi_heat_store::{feed, mcp, tool_switches, Clock, Error};
use wi_store::Store;

use crate::prompts::School;
use crate::{Backend, ToolError};

/// The sentence for a library that isn't there (8.8).
pub const NO_LIBRARY: &str = "No Wi_WWAV library yet. Open the app once.";

/// Where the app keeps its library: `WI_WWAV_LIBRARY`, or `Wi_WWAV` in the
/// Music folder, as the app itself decides (app/src-tauri/src/paths.rs).
pub fn default_root() -> PathBuf {
    if let Some(path) = std::env::var_os("WI_WWAV_LIBRARY") {
        return path.into();
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("Music").join("Wi_WWAV")
}

pub struct Library {
    root: PathBuf,
    store: Option<Store>,
    /// Why there's no store: no library yet, or one made by a newer app.
    missing: String,
}

impl Library {
    /// The library at `root`, opened now if it's there and again on each
    /// call until it is, since Claude may start the helper before the app has
    /// ever run. The helper never makes a library itself.
    pub fn open(root: &Path) -> Library {
        let mut library = Library { root: root.to_path_buf(), store: None, missing: NO_LIBRARY.into() };
        library.reach();
        library
    }

    fn reach(&mut self) {
        if self.store.is_some() {
            return;
        }
        if !self.root.join("library.sqlite").is_file() {
            self.missing = NO_LIBRARY.into();
            return;
        }
        match Store::open(&self.root) {
            Ok(store) => self.store = Some(store),
            Err(e) => self.missing = e.to_string(),
        }
    }
}

impl Backend for Library {
    fn enabled(&mut self, tool: &str) -> bool {
        self.reach();
        match &self.store {
            Some(store) => tool_switches(store).map_or(true, |s| s.get(tool).copied().unwrap_or(true)),
            None => true,
        }
    }

    fn call(&mut self, tool: &str, args: &Map<String, Value>) -> Result<Value, ToolError> {
        self.reach();
        let Some(store) = self.store.as_mut() else {
            return Err(ToolError::new(self.missing.clone()));
        };
        mcp::call(store, &Clock::system(), tool, args).map_err(|e| match e {
            Error::Refused(sentence) => ToolError::new(sentence),
            Error::Store(e) => ToolError::new(format!("Learn couldn't save that: {e}")),
        })
    }

    fn school(&mut self) -> School {
        self.reach();
        let sheet = self.store.as_ref().and_then(|store| feed::school_sheet(store).ok()).unwrap_or_default();
        let text = |k: &str| sheet[k].as_str().unwrap_or_default().to_string();
        School { name: text("name"), host: text("host") }
    }
}
