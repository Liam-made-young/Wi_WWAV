//! The `.wwavsession` package (`docs/SPEC.md` 6.5): the Console's edit
//! list plus everything it needs to open on another machine.
//!
//! ```text
//! <ULID>.wwavsession/
//!   session.json      the session (model, written by canonical)
//!   media/            every file a clip points at, ULID-named
//!   plugin-state/     plugin states over 256 KB, ULID-named
//!   renders/          exports and freezes made from this session
//!   journal/          undo.ndjson, snapshots/ for File → Revert to, and
//!                     lock, held while the package is open
//!   cache/            peaks, proxies, analysis; safe to delete
//! ```
//!
//! - [`model`] is what `session.json` holds, keeping what it doesn't know.
//! - [`canonical`] writes it: the same session always gives the same bytes.
//! - [`Package`] creates and opens a package, in one place at a time. Every
//!   change is a journalled transaction ([`Package::edit`]), undone and
//!   redone with its label; only the parts a change touches are compared
//!   and journalled, so an edit costs the same in a long session as in a
//!   short one. [`Package::save`] writes `session.json` through a temporary
//!   file and a rename, and opening replays what the journal has that
//!   `session.json` doesn't ("Recovered 14 changes from 21:12.").
//! - [`Autosaver`] is the "2 s after the last change, or at a transport
//!   stop" rule, on the package's clock.
//! - [`Package::import_media`], [`Package::store_plugin_state`] and
//!   [`Package::send`] fill `media/` and `plugin-state/`, and write
//!   `<Title>.wwavsession.zip`.

pub mod canonical;
pub mod model;

mod autosave;
mod diff;
mod fsx;
mod journal;
mod media;
mod package;
mod part;
mod plugin;
mod send;

use std::io;
use std::path::PathBuf;

pub use autosave::{Autosaver, Clock, SystemClock, AUTOSAVE_AFTER_MS};
pub use media::{copying_sentence, Imported, Link};
pub use model::Session;
pub use package::{
    Opened, Package, Recovered, Snapshot, Staged, SNAPSHOT_EVERY_MS, SNAPSHOT_KEEP_MS,
};
pub use plugin::{older_version_sentence, INLINE_LIMIT};
pub use send::{folder_name, NAME_CHARS};

/// What went wrong, in words fit to show where they can be.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("session.json can't be read: {0}")]
    Json(#[from] serde_json::Error),
    #[error("This session was saved by a newer Wi_WWAV (wwavsession {0}).")]
    Newer(String),
    #[error("{} isn't a session: it has no session.json.", .0.display())]
    NotASession(PathBuf),
    #[error("A session is already at {}.", .0.display())]
    Exists(PathBuf),
    /// Another window or process has the package open.
    #[error("This session is already open in another window.")]
    InUse(PathBuf),
    /// The undo journal didn't fit the session. The session was saved as
    /// it stands and its history starts again from there.
    #[error("The undo history didn't fit this session, so it starts again from here.")]
    History,
    /// A change whose result couldn't be opened again, such as a NaN that
    /// JSON would write as null. Nothing was changed or journalled.
    #[error("This change can't be saved: {0}")]
    Unsaveable(String),
    #[error("The plugin state {0} doesn't match its sha256.")]
    StateChanged(String),
    #[error("A session can't be sent into its own folder. Choose a folder outside it.")]
    SendInside,
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}
