//! The other records `library.sqlite` holds (docs/SPEC.md 9.6): the views'
//! own records, Console sessions as sequences, and what the plugin scanner
//! found.

use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

use crate::journal::object;
use crate::{refused, Error, Result, Store, Txn};

/// One of a view's records, such as Heat's Task or TimeBlock (3.15), kept as
/// JSON under its kind and its own id. `text` is what search reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Doc {
    pub kind: String,
    pub key: String,
    pub json: Value,
    pub text: String,
}

fn doc_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<(String, String, String, String)> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
}

fn to_doc((kind, key, json, text): (String, String, String, String)) -> Result<Doc> {
    Ok(Doc {
        kind,
        key,
        json: serde_json::from_str(&json)?,
        text,
    })
}

/// A Console session: an edit list, never media (2.5). The edit list itself
/// is the package's `session.json` (6.5); the library names the package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sequence {
    pub id: String,
    pub title: String,
    pub package: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanStatus {
    Ok,
    Crashed,
    Hung,
}

impl ScanStatus {
    fn as_str(self) -> &'static str {
        match self {
            ScanStatus::Ok => "ok",
            ScanStatus::Crashed => "crashed",
            ScanStatus::Hung => "hung",
        }
    }

    fn parse(s: &str) -> Option<ScanStatus> {
        [ScanStatus::Ok, ScanStatus::Crashed, ScanStatus::Hung]
            .into_iter()
            .find(|v| v.as_str() == s)
    }
}

/// One plugin as `wwav-scan` found it. One that crashed or hung is kept out
/// and not checked again until its version or file date changes (9.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginScan {
    pub path: String,
    pub format: String,
    pub uid: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub modified_ms: i64,
    pub status: ScanStatus,
    /// Why it was kept out, said once.
    pub reason: String,
    pub checked_ms: i64,
}

impl Store {
    pub fn doc(&self, kind: &str, key: &str) -> Result<Option<Doc>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT kind, key, json, text FROM docs WHERE kind = ?1 AND key = ?2",
        )?;
        stmt.query_row([kind, key], doc_from)
            .optional()?
            .map(to_doc)
            .transpose()
    }

    pub fn docs(&self, kind: &str) -> Result<Vec<Doc>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT kind, key, json, text FROM docs WHERE kind = ?1 ORDER BY key",
        )?;
        let docs = stmt.query_map([kind], doc_from)?;
        docs.map(|d| to_doc(d?)).collect()
    }

    /// Records of every kind whose text has every word typed, newest first.
    pub fn search_docs(&self, text: &str, limit: usize) -> Result<Vec<Doc>> {
        let Some(query) = crate::organise::fts_query(text) else {
            return Ok(Vec::new());
        };
        let mut stmt = self.conn.prepare_cached(
            "SELECT d.kind, d.key, d.json, d.text FROM docs_fts f JOIN docs d ON d.n = f.rowid
             WHERE docs_fts MATCH ?1 ORDER BY f.rowid DESC LIMIT ?2",
        )?;
        let docs = stmt.query_map(params![query, limit as i64], doc_from)?;
        docs.map(|d| to_doc(d?)).collect()
    }

    pub fn sequences(&self) -> Result<Vec<Sequence>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT id, title, package FROM sequences ORDER BY id")?;
        let seqs = stmt.query_map([], |r| {
            Ok(Sequence {
                id: r.get(0)?,
                title: r.get(1)?,
                package: r.get(2)?,
            })
        })?;
        Ok(seqs.collect::<rusqlite::Result<_>>()?)
    }

    /// Replaces what is known about each plugin file in `scans` with this
    /// check's results. Not a journal entry: a scan is the machine's, not an
    /// edit.
    pub fn record_plugin_scan(&mut self, scans: &[PluginScan]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for scan in scans {
            tx.execute("DELETE FROM plugin_scans WHERE path = ?1", [&scan.path])?;
        }
        for s in scans {
            tx.execute(
                "INSERT INTO plugin_scans
                   (path, uid, format, name, vendor, version, modified_ms, status, reason, checked_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    s.path,
                    s.uid,
                    s.format,
                    s.name,
                    s.vendor,
                    s.version,
                    s.modified_ms,
                    s.status.as_str(),
                    s.reason,
                    s.checked_ms
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn plugin_scans(&self) -> Result<Vec<PluginScan>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT path, uid, format, name, vendor, version, modified_ms, status, reason, checked_ms
             FROM plugin_scans ORDER BY name COLLATE NOCASE, path, uid",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                PluginScan {
                    path: r.get(0)?,
                    uid: r.get(1)?,
                    format: r.get(2)?,
                    name: r.get(3)?,
                    vendor: r.get(4)?,
                    version: r.get(5)?,
                    modified_ms: r.get(6)?,
                    status: ScanStatus::Ok,
                    reason: r.get(8)?,
                    checked_ms: r.get(9)?,
                },
                r.get::<_, String>(7)?,
            ))
        })?;
        rows.map(|row| {
            let (scan, status) = row?;
            let status = ScanStatus::parse(&status)
                .ok_or_else(|| Error::Corrupt(format!("scan status {status}")))?;
            Ok(PluginScan { status, ..scan })
        })
        .collect()
    }
}

impl Store {
    /// Writes a record outside the journal, for state that is not a change
    /// anyone undoes: Heat's timer and plan drafts, Settings → Claude's
    /// switches, what a calendar feed held (docs/HEAT.md). Refused for a
    /// record the journal has ever touched, so undo never meets a value it
    /// didn't write.
    pub fn set_doc(&mut self, kind: &str, key: &str, json: &Value, text: &str) -> Result<()> {
        if kind.is_empty() || kind.contains('/') {
            return refused(format!("A record's kind can't be empty or hold '/': '{kind}'."));
        }
        let id = format!("{kind}/{key}");
        let tx = self.conn.transaction()?;
        let journaled: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM txn_row WHERE tbl = 'docs' AND row_id = ?1)",
            [&id],
            |r| r.get(0),
        )?;
        if journaled {
            return refused(format!("'{id}' is kept in the journal, so it changes only through a Txn."));
        }
        tx.execute(
            "INSERT INTO docs (id, kind, key, json, text) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET json = excluded.json, text = excluded.text",
            params![id, kind, key, json.to_string(), text],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Removes a record written with [`Store::set_doc`].
    pub fn remove_doc(&mut self, kind: &str, key: &str) -> Result<()> {
        let id = format!("{kind}/{key}");
        let tx = self.conn.transaction()?;
        let journaled: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM txn_row WHERE tbl = 'docs' AND row_id = ?1)",
            [&id],
            |r| r.get(0),
        )?;
        if journaled {
            return refused(format!("'{id}' is kept in the journal, so it changes only through a Txn."));
        }
        tx.execute("DELETE FROM docs WHERE id = ?1", [&id])?;
        tx.commit()?;
        Ok(())
    }
}

fn missing_sequence<T>() -> Result<T> {
    refused("That session isn't in the library any more.")
}

impl Txn<'_> {
    /// Adds or replaces a room's record.
    pub fn put_doc(&mut self, kind: &str, key: &str, json: &Value, text: &str) -> Result<()> {
        if kind.is_empty() || kind.contains('/') {
            return refused(format!(
                "A record's kind can't be empty or hold '/': '{kind}'."
            ));
        }
        let id = format!("{kind}/{key}");
        let json = json.to_string();
        if self.get("docs", &id)?.is_some() {
            self.update("docs", &id, &[("json", json.into()), ("text", text.into())])?;
        } else {
            self.insert(
                "docs",
                object(json!({"id": id, "kind": kind, "key": key, "json": json, "text": text})),
            )?;
        }
        Ok(())
    }

    pub fn delete_doc(&mut self, kind: &str, key: &str) -> Result<()> {
        self.delete("docs", &format!("{kind}/{key}"))?;
        Ok(())
    }

    /// A new session, named by a new ULID. The package itself is written by
    /// the session code at `sessions/<id>.wwavsession`.
    pub fn add_sequence(&mut self, title: &str) -> Result<Sequence> {
        let id = wwav_ids::ulid();
        let seq = Sequence {
            package: format!("sessions/{id}.wwavsession"),
            id,
            title: title.to_string(),
        };
        self.insert(
            "sequences",
            object(json!({"id": seq.id, "title": seq.title, "package": seq.package})),
        )?;
        Ok(seq)
    }

    pub fn rename_sequence(&mut self, id: &str, title: &str) -> Result<()> {
        if self.update("sequences", id, &[("title", title.into())])? {
            Ok(())
        } else {
            missing_sequence()
        }
    }

    /// Removes the session's row, never its package. Its renders stay, no
    /// longer pointing back, until undo restores the link.
    pub fn delete_sequence(&mut self, id: &str) -> Result<()> {
        if self.get("sequences", id)?.is_none() {
            return missing_sequence();
        }
        let renders: Vec<String> = {
            let mut stmt = self
                .conn()
                .prepare_cached("SELECT id FROM clips WHERE from_sequence = ?1 ORDER BY id")?;
            let ids = stmt.query_map([id], |r| r.get(0))?;
            ids.collect::<rusqlite::Result<_>>()?
        };
        for clip in renders {
            self.update("clips", &clip, &[("from_sequence", Value::Null)])?;
        }
        self.delete("sequences", id)?;
        Ok(())
    }
}
