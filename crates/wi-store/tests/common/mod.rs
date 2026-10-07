//! Helpers the store's tests share. The dump reads `library.sqlite` through a
//! second connection, the way any outside reader would, so a test compares
//! what is on disk rather than what the store says about itself.

#![allow(dead_code)] // each test file uses a different subset

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OpenFlags};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use wi_store::{Room, Store};

/// The rooms ⌘Z is pressed in: the three views and the library drawer.
pub const ROOMS: [Room; 4] = [Room::Heat, Room::Space, Room::Console, Room::Library];

pub fn library() -> (TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("Wi_WWAV")).unwrap();
    (dir, store)
}

/// A clip that was in the library before the journal began: its file goes in
/// `media/` and its row straight into the table, so no journal entry made it
/// and no undo takes it away. For a first state that undo and redo may change
/// the values of but never remove.
pub fn seed_clip(root: &Path, title: &str, bytes: &[u8], bpm: f64) -> String {
    let id = wwav_ids::ulid();
    let file = format!("media/{id}.wwav");
    std::fs::write(root.join(&file), bytes).unwrap();
    let conn = Connection::open(root.join("library.sqlite")).unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    conn.execute(
        "INSERT INTO clips (id, kind, file, sha256, bytes, title, bpm) VALUES (?1, 'wwav', ?2, ?3, ?4, ?5, ?6)",
        params![id, file, hex::encode(Sha256::digest(bytes)), bytes.len() as i64, title, bpm],
    )
    .unwrap();
    id
}

/// A small file outside the library, to import.
pub fn source_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let src = dir.join("outside");
    std::fs::create_dir_all(&src).unwrap();
    let path = src.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn reader(root: &Path) -> Connection {
    Connection::open_with_flags(
        root.join("library.sqlite"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
}

/// Tables that hold records: everything but the journal itself, the search
/// indexes (derived) and SQLite's own.
fn record_tables(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table'
               AND name NOT IN ('txn', 'txn_row') AND name NOT LIKE 'sqlite_%'
               AND name NOT LIKE '%_fts%'
             ORDER BY name",
        )
        .unwrap();
    let names = stmt.query_map([], |r| r.get(0)).unwrap();
    names.map(Result::unwrap).collect()
}

/// Every record table, every row, every value with its SQLite type, in one
/// canonical order. Two dumps are equal only if the records are byte for byte.
pub fn dump(root: &Path) -> String {
    let conn = reader(root);
    let mut out = String::new();
    for table in record_tables(&conn) {
        let stmt = conn.prepare(&format!("SELECT * FROM {table}")).unwrap();
        let cols: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        let order = (1..=cols.len())
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))
            .unwrap();
        writeln!(out, "## {table} ({})", cols.join(", ")).unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for i in 0..cols.len() {
                let cell = match row.get_ref(i).unwrap() {
                    ValueRef::Null => "null".to_string(),
                    ValueRef::Integer(n) => format!("i:{n}"),
                    ValueRef::Real(f) => format!("r:{:?}", f.to_bits()),
                    ValueRef::Text(t) => format!("t:{}", String::from_utf8_lossy(t)),
                    ValueRef::Blob(b) => format!("b:{b:?}"),
                };
                write!(out, "{cell}\t").unwrap();
            }
            out.push('\n');
        }
    }
    out
}

/// The journal as (id -> (room, state)).
pub fn journal(root: &Path) -> BTreeMap<String, (String, String)> {
    let conn = reader(root);
    let mut stmt = conn.prepare("SELECT id, room, state FROM txn").unwrap();
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?))))
        .unwrap();
    rows.map(Result::unwrap).collect()
}

/// FTS5's own check that each search index matches its table.
pub fn search_indexes_agree(root: &Path) {
    let conn = Connection::open(root.join("library.sqlite")).unwrap();
    for fts in ["clips_fts", "docs_fts"] {
        conn.execute(
            &format!("INSERT INTO {fts}({fts}) VALUES ('integrity-check')"),
            [],
        )
        .unwrap_or_else(|e| panic!("{fts} disagrees with its table: {e}"));
    }
}

/// Undo in every room until nothing more will undo; returns how many steps.
pub fn undo_all(store: &mut Store) -> usize {
    step_all(store, |s, room| s.undo(room))
}

pub fn redo_all(store: &mut Store) -> usize {
    step_all(store, |s, room| s.redo(room))
}

fn step_all(
    store: &mut Store,
    step: impl Fn(&mut Store, Room) -> wi_store::Result<Option<String>>,
) -> usize {
    let mut steps = 0;
    loop {
        let mut moved = false;
        for room in ROOMS {
            // A step another room's later change is holding back is refused;
            // the loop comes back to it once that room has moved.
            loop {
                match step(store, room) {
                    Ok(Some(_)) => {
                        steps += 1;
                        moved = true;
                    }
                    Ok(None) | Err(wi_store::Error::Refused(_)) => break,
                    Err(e) => panic!("{room:?}: {e}"),
                }
            }
        }
        if !moved {
            return steps;
        }
    }
}
