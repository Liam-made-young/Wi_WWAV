//! wi-core's own rows in `library.sqlite`, beside wi-store's tables:
//! settings, the account's name (never its token), each upload's server ids,
//! Heat's sync state and the galaxy last seen. None of them is journaled:
//! they are the machine's or the server's, not edits, so ⌘Z never touches
//! them. They live in one table of their own, `core_kv`, through a second
//! connection, so wi-store's schema stays its own.

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::bus::lock;
use crate::CoreError;

pub(crate) struct Kv {
    conn: Mutex<Connection>,
}

impl Kv {
    pub fn open(root: &Path) -> Result<Kv, CoreError> {
        let conn = Connection::open(root.join("library.sqlite"))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS core_kv (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        )?;
        Ok(Kv {
            conn: Mutex::new(conn),
        })
    }

    pub fn get(&self, key: &str) -> Result<Option<Value>, CoreError> {
        let text: Option<String> = lock(&self.conn)
            .query_row("SELECT value FROM core_kv WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(text.and_then(|t| serde_json::from_str(&t).ok()))
    }

    pub fn set(&self, key: &str, value: &Value) -> Result<(), CoreError> {
        lock(&self.conn).execute(
            "INSERT OR REPLACE INTO core_kv (key, value) VALUES (?1, ?2)",
            params![key, value.to_string()],
        )?;
        Ok(())
    }

    pub fn delete(&self, key: &str) -> Result<(), CoreError> {
        lock(&self.conn).execute("DELETE FROM core_kv WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Reads the library's tables directly, for questions wi-store's API
    /// doesn't answer (which record kinds exist).
    pub fn query_strings(&self, sql: &str) -> Result<Vec<String>, CoreError> {
        let conn = lock(&self.conn);
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The sha256 of every clip of exactly `bytes` bytes: a file can only be
    /// one of them if its size matches, so most imports never hash twice.
    pub fn shas_of_size(&self, bytes: u64) -> Result<Vec<String>, CoreError> {
        let conn = lock(&self.conn);
        let mut stmt = conn.prepare_cached("SELECT sha256 FROM clips WHERE bytes = ?1")?;
        let rows = stmt.query_map([bytes as i64], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Forgets the parts recorded for a clip's upload when the server starts
    /// it afresh under a new upload id: their etags belong to the old one.
    /// wi-store records parts and clears them only once the clip is up, so
    /// this one statement on its non-journaled `upload_part` table is here.
    pub fn clear_upload_parts(&self, clip: &str) -> Result<(), CoreError> {
        lock(&self.conn).execute("DELETE FROM upload_part WHERE clip_id = ?1", [clip])?;
        Ok(())
    }
}
