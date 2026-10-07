//! wi-store: the library folder, `library.sqlite` and the undo journal
//! (docs/SPEC.md 2.5, 2.7, 2.8, 9.6).
//!
//! ```text
//! ~/Music/Wi_WWAV/
//!   library.sqlite   clips, tags, sequences, records of the views, the undo journal
//!   media/           01JA2B7X9Q4M8K3T5V6W0YHZRC.wwav   (ULID-named)
//!   sessions/        Console sessions
//!   trash/           deleted media, kept until you empty it
//! ```
//!
//! Every change goes through a [`Txn`]: open one with a room and a label,
//! make the edits, commit. The store snapshots each row it touches, so ⌘Z in
//! that room can put it back (`journal.rs` has the rules). Work that left the
//! machine (an upload the server has, a sent message) is never a journal
//! entry. A room is a view (Heat, Space, Console), the library drawer, or
//! [`Room::Sync`], where changes from your other devices are kept.

mod clips;
mod journal;
mod media;
mod merge;
mod organise;
mod records;
mod schema;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::{Connection, ErrorCode};

pub use clips::{
    ByExtension, Clip, Colour, Inspection, Inspector, Kind, NewClip, Placement, UPLOAD_QUEUE,
};
pub use journal::{Actor, DocChange, EntryDocs, EntryInfo, History, Menu, Txn};
pub use media::Files;
pub use organise::{Pin, SmartFolder, SmartRule, Tag, TagKind};
pub use records::{Doc, PluginScan, ScanStatus, Sequence};
pub use schema::SCHEMA_VERSION;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A sentence for the person: why this edit, undo or import didn't happen.
    #[error("{0}")]
    Refused(String),
    #[error("library.sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("journal snapshot: {0}")]
    Json(#[from] serde_json::Error),
    /// The file holds something this version never writes.
    #[error("library.sqlite is damaged: {0}")]
    Corrupt(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn refused<T>(sentence: impl Into<String>) -> Result<T> {
    Err(Error::Refused(sentence.into()))
}

/// Where a change was made. ⌘Z acts on the room you are in: Heat, Space or
/// the Console (the three views), or the library drawer over any of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Room {
    Heat,
    Space,
    Console,
    Library,
    /// Changes that came from your other devices. No view's ⌘Z reaches
    /// them, so a sync never takes the redo of the person's own undo; but
    /// the rules treat them as any other room's, so an undo that would put
    /// back a value they changed since waits, and says why.
    Sync,
}

impl Room {
    pub fn as_str(self) -> &'static str {
        match self {
            Room::Heat => "heat",
            Room::Space => "space",
            Room::Console => "console",
            Room::Library => "library",
            Room::Sync => "sync",
        }
    }

    pub fn parse(s: &str) -> Option<Room> {
        [
            Room::Heat,
            Room::Space,
            Room::Console,
            Room::Library,
            Room::Sync,
        ]
        .into_iter()
        .find(|r| r.as_str() == s)
    }

    /// The room as a sentence names it: "Undo it in the Console first."
    pub fn name(self) -> &'static str {
        match self {
            Room::Heat => "Learn",
            Room::Space => "Space",
            Room::Console => "the Console",
            Room::Library => "the library",
            Room::Sync => "your other devices",
        }
    }
}

/// How long a handle waits for another to finish writing.
const BUSY: Duration = Duration::from_secs(5);

/// Puts `library.sqlite` in WAL mode and returns the mode it is in. When two
/// handles open a new library at once, both ask; SQLite answers one "busy"
/// without waiting (waiting there could deadlock), so it waits here and asks
/// again, by which time the other has made the file WAL.
fn wal(conn: &Connection) -> Result<String> {
    let deadline = Instant::now() + BUSY;
    loop {
        match conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0)) {
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == ErrorCode::DatabaseBusy && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            mode => return Ok(mode?),
        }
    }
}

/// The library: one folder, one database.
pub struct Store {
    conn: Connection,
    root: PathBuf,
    tables: journal::Tables,
}

impl Store {
    /// Opens the library at `root`, making the folder, its three subfolders
    /// and `library.sqlite` if they aren't there, and bringing the schema up
    /// to date.
    pub fn open(root: &Path) -> Result<Store> {
        for folder in ["media", "sessions", "trash"] {
            std::fs::create_dir_all(root.join(folder))?;
        }
        let mut conn = Connection::open(root.join("library.sqlite"))?;
        conn.busy_timeout(BUSY)?;
        let mode = wal(&conn)?;
        if mode != "wal" {
            return Err(Error::Corrupt(format!("journal_mode is {mode}, not wal")));
        }
        conn.pragma_update(None, "foreign_keys", true)?;
        schema::migrate(&mut conn)?;
        let tables = journal::Tables::read(&conn)?;
        Ok(Store {
            conn,
            root: root.to_path_buf(),
            tables,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Opens a transaction: every edit made through it undoes as one, under
    /// `label` ("move clip" reads "Undo move clip"), in `room`.
    pub fn begin(&mut self, room: Room, label: &str) -> Result<Txn<'_>> {
        Txn::begin(&mut self.conn, &self.tables, room, label, Actor::You)
    }

    /// As [`Store::begin`], for a change someone other than the person made:
    /// Claude, through an MCP tool, with its reason (docs/SPEC.md 8.8).
    pub fn begin_by(&mut self, room: Room, label: &str, actor: Actor) -> Result<Txn<'_>> {
        Txn::begin(&mut self.conn, &self.tables, room, label, actor)
    }

    /// The newest journal entries first, only Claude's when `claude_only`.
    pub fn entries(&self, claude_only: bool, limit: usize) -> Result<Vec<EntryInfo>> {
        journal::entries(&self.conn, claude_only, limit)
    }

    /// The journal entries newer than `after` (all of them for None), oldest
    /// first, each with the records it changed. The core reads the ones the
    /// MCP helper wrote while the app was open (docs/HEAT.md).
    pub fn entries_after(&self, after: Option<&str>, limit: usize) -> Result<Vec<EntryDocs>> {
        journal::entries_after(&self.conn, after, limit)
    }

    /// One journal entry with the records it changed.
    pub fn entry_docs(&self, id: &str) -> Result<Option<EntryDocs>> {
        journal::entry_docs(&self.conn, id)
    }

    /// The newest journal entry's id, or None for a library with no changes.
    pub fn last_entry(&self) -> Result<Option<String>> {
        journal::last_entry(&self.conn)
    }

    /// SQLite's `data_version` for this handle: it differs between two reads
    /// when another handle, in this process or another, committed in
    /// between. A commit made through this handle never changes it, so the
    /// core's own writes are not news to it (docs/SPEC.md 8.8).
    pub fn data_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("PRAGMA data_version", [], |r| r.get(0))?)
    }

    /// Undoes one entry out of order and returns its label (Settings →
    /// Claude's Undo). Refused, with a sentence, when a later change touched
    /// the same values.
    pub fn undo_entry(&mut self, id: &str) -> Result<String> {
        journal::undo_entry(&mut self.conn, id)
    }

    /// Undoes the room's newest change and returns its label, or None when
    /// there is nothing to undo. A change that can't be undone yet is refused
    /// with the sentence [`Store::history`] shows.
    pub fn undo(&mut self, room: Room) -> Result<Option<String>> {
        journal::undo(&mut self.conn, room)
    }

    pub fn redo(&mut self, room: Room) -> Result<Option<String>> {
        journal::redo(&mut self.conn, room)
    }

    /// What ⌘Z and ⌘⇧Z would do in `room`, for the Edit menu.
    pub fn history(&self, room: Room) -> Result<History> {
        journal::history(&self.conn, room)
    }

    /// Records work that left the machine, such as a sent message. It is
    /// not a journal entry: until the room's next change, its Edit menu
    /// reads "Can't undo `what`." ("a message").
    pub fn record_outward(&mut self, room: Room, what: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        journal::record_outward(&tx, room, what)?;
        tx.commit()?;
        Ok(())
    }
}
