//! `journal/undo.ndjson`: the session's undo journal, one JSON object per
//! line, appended and synced before a change counts as made.
//!
//! It keeps 9.6's shape (a transaction with a label and the rows it
//! touched, each before and after) in a file that only grows, so undo and
//! redo are lines of their own:
//!
//! ```text
//! {"op":"start","id":"01J…","at":1791321120000,"sha256":"…"}
//! {"op":"txn","id":"01J…","at":…,"label":"move clip","rows":[{"path":"/tracks/0/events/2/at_ms","before":1000,"after":2000}]}
//! {"op":"undo","id":"01J…","at":…,"txn":"01J…"}
//! {"op":"redo","id":"01J…","at":…,"txn":"01J…"}
//! {"op":"save","id":"01J…","at":…,"sha256":"…"}
//! {"op":"saved","id":"01J…","at":…,"sha256":"…"}
//! ```
//!
//! `start`, `save` and `saved` name the sha256 of `session.json`'s bytes
//! for the state at that point. A `save` line is written before the rename
//! that puts the new `session.json` in place, so whichever file survives a
//! crash, a line names it, and every line after that line is a change the
//! file doesn't hold yet. A `saved` line is written once the file holds
//! it: a `session.json` older than a `saved` line was put back from outside
//! the app (a backup, git), not left by a crash.
//!
//! A line this version can't read (an `op` a newer app added, or the part
//! of a line a failed write left) is skipped, never fatal: ops added later
//! must not change the document.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::diff::Row;
use crate::fsx;

/// The state the journal starts from, or the state `session.json` is about
/// to hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub id: String,
    pub at: u64,
    pub sha256: String,
}

/// One change: its label ("move clip", shown as "Undo move clip") and the
/// rows it touched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Txn {
    pub id: String,
    pub at: u64,
    pub label: String,
    pub rows: Vec<Row>,
}

/// An undo or a redo of the transaction `txn`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub at: u64,
    pub txn: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Line {
    Start(Mark),
    Txn(Txn),
    Undo(Step),
    Redo(Step),
    Save(Mark),
    Saved(Mark),
}

/// The `op` word each body is written under.
pub const START: &str = "start";
pub const TXN: &str = "txn";
pub const UNDO: &str = "undo";
pub const REDO: &str = "redo";
pub const SAVE: &str = "save";
pub const SAVED: &str = "saved";

/// A line as written: the `op` first, then the body's fields.
#[derive(Serialize)]
struct Tagged<'a, T: Serialize> {
    op: &'static str,
    #[serde(flatten)]
    body: &'a T,
}

pub struct Journal {
    file: File,
    /// The length of the lines written whole: anything past it is part of
    /// a line whose append failed.
    len: u64,
}

impl Journal {
    /// A new journal whose first line is `start`.
    pub fn create(path: &Path, start: &Mark) -> io::Result<Journal> {
        let file = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(path)?;
        let mut journal = Journal { file, len: 0 };
        journal.append(START, start)?;
        fsx::sync_dir(path.parent().unwrap_or(Path::new(".")))?;
        Ok(journal)
    }

    /// Opens a journal and reads its lines. A last line without its newline
    /// was cut off by a crash mid-append; the call that wrote it never
    /// returned, so it is cut from the file. Lines this version can't read
    /// are skipped (see the module's notes).
    pub fn open(path: &Path) -> io::Result<(Journal, Vec<Line>)> {
        let mut file = OpenOptions::new().read(true).append(true).open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let whole = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
        if whole < bytes.len() {
            file.set_len(whole as u64)?;
            file.sync_all()?;
        }
        let lines = bytes[..whole]
            .split(|&b| b == b'\n')
            .filter_map(|l| serde_json::from_slice(l).ok())
            .collect();
        Ok((
            Journal {
                file,
                len: whole as u64,
            },
            lines,
        ))
    }

    /// Appends one line and syncs it: when this returns, the line survives
    /// the process being killed, and a power cut. When it fails (a full
    /// disk), the part of the line written is taken back, so the next line
    /// starts on a line of its own.
    pub fn append<T: Serialize>(&mut self, op: &'static str, body: &T) -> io::Result<()> {
        let mut line = serde_json::to_vec(&Tagged { op, body }).map_err(io::Error::other)?;
        line.push(b'\n');
        // A failed append whose cut-off part couldn't be taken back then.
        if self.file.metadata()?.len() > self.len {
            self.file.set_len(self.len)?;
        }
        // One write, so a crash leaves at most one cut-off line at the end.
        let written = self
            .file
            .write_all(&line)
            .and_then(|()| self.file.sync_data());
        if let Err(e) = written {
            // Truncating is allowed on a full disk; if it fails too, the
            // next append tries again before writing.
            let _ = self.file.set_len(self.len);
            return Err(e);
        }
        self.len += line.len() as u64;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lines_are_written_as_documented_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("undo.ndjson");
        let start = Mark {
            id: "A".into(),
            at: 1,
            sha256: "ab".into(),
        };
        let mut j = Journal::create(&path, &start).unwrap();
        let txn = Txn {
            id: "B".into(),
            at: 2,
            label: "move clip".into(),
            rows: vec![Row {
                path: "/x".into(),
                before: Some(json!(1)),
                after: Some(json!(2)),
            }],
        };
        j.append(TXN, &txn).unwrap();
        let undo = Step {
            id: "C".into(),
            at: 3,
            txn: "B".into(),
        };
        j.append(UNDO, &undo).unwrap();
        drop(j);
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "{\"op\":\"start\",\"id\":\"A\",\"at\":1,\"sha256\":\"ab\"}\n\
             {\"op\":\"txn\",\"id\":\"B\",\"at\":2,\"label\":\"move clip\",\"rows\":[{\"path\":\"/x\",\"before\":1,\"after\":2}]}\n\
             {\"op\":\"undo\",\"id\":\"C\",\"at\":3,\"txn\":\"B\"}\n"
        );
        let (_, lines) = Journal::open(&path).unwrap();
        assert_eq!(
            lines,
            vec![Line::Start(start), Line::Txn(txn), Line::Undo(undo)]
        );
    }

    fn mark(id: &str) -> Mark {
        Mark {
            id: id.into(),
            at: 1,
            sha256: "ab".into(),
        }
    }

    #[test]
    fn lines_it_cannot_read_are_skipped() {
        // A newer app's op, and the part of a line a failed write left
        // with its newline, sit between lines this version reads.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("undo.ndjson");
        let mut j = Journal::create(&path, &mark("A")).unwrap();
        drop(j);
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{\"op\":\"bookmark\",\"id\":\"X\",\"at\":1}\n{\"op\":\"txn\",\"id\n")
            .unwrap();
        drop(f);
        j = Journal::open(&path).unwrap().0;
        j.append(SAVED, &mark("B")).unwrap();
        let (_, lines) = Journal::open(&path).unwrap();
        assert_eq!(lines, vec![Line::Start(mark("A")), Line::Saved(mark("B"))]);
    }

    #[test]
    fn bytes_past_the_last_whole_line_are_taken_back_before_the_next() {
        // What a write that failed part way leaves when taking it back
        // failed too: the next append must not be joined onto it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("undo.ndjson");
        let mut j = Journal::create(&path, &mark("A")).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"op\":\"txn\",\"id\":\"01JC")
            .unwrap();
        j.append(SAVE, &mark("B")).unwrap();
        drop(j);
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2, "{text}");
        let (_, lines) = Journal::open(&path).unwrap();
        assert_eq!(lines, vec![Line::Start(mark("A")), Line::Save(mark("B"))]);
    }
}
