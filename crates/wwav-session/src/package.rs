//! A `.wwavsession` package on disk: creating and opening it, journalled
//! changes, undo and redo, saving, recovery and snapshots.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use crate::autosave::Clock;
use crate::journal::{self, Journal, Line, Mark, Step, Txn};
use crate::model::Session;
use crate::{canonical, diff, fsx, sha256_hex, Error, Result};

/// The folders every package has (`docs/SPEC.md` 6.5).
const FOLDERS: [&str; 5] = ["media", "plugin-state", "renders", "journal", "cache"];

/// A copy of `session.json` is kept at most this often while you work.
pub const SNAPSHOT_EVERY_MS: u64 = 10 * 60 * 1000;
/// And kept this long.
pub const SNAPSHOT_KEEP_MS: u64 = 30 * 24 * 60 * 60 * 1000;

/// What opening found.
#[derive(Debug, Clone, PartialEq)]
pub struct Opened {
    /// Changes the journal had that `session.json` didn't, now replayed.
    pub recovered: Option<Recovered>,
    /// The journal didn't match `session.json` (it was changed outside the
    /// app, or the journal was damaged). The file is opened as it is, the
    /// old journal is kept beside the new one, and undo starts here.
    pub history_reset: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recovered {
    pub changes: usize,
    /// When the first of them was made.
    pub since_ms: u64,
}

impl Recovered {
    /// "Recovered 14 changes from 21:12.", in local time: the app passes the
    /// local offset from UTC in minutes.
    pub fn sentence(&self, utc_offset_minutes: i32) -> String {
        let minutes = (self.since_ms / 60_000) as i64 + utc_offset_minutes as i64;
        let of_day = minutes.rem_euclid(24 * 60);
        let noun = if self.changes == 1 {
            "change"
        } else {
            "changes"
        };
        format!(
            "Recovered {} {noun} from {:02}:{:02}.",
            self.changes,
            of_day / 60,
            of_day % 60
        )
    }
}

/// A copy of `session.json` for File → Revert to.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub at_ms: u64,
    pub path: PathBuf,
}

/// A save written and synced to `session.json.tmp`, not yet renamed.
#[derive(Debug)]
pub struct Staged {
    bytes: Vec<u8>,
    sha: String,
    changes: u64,
}

pub struct Package {
    dir: PathBuf,
    clock: Arc<dyn Clock>,
    session: Session,
    /// The session as JSON, the document the journal's pointers address.
    /// Always `session.to_value()`.
    doc: Value,
    journal: Journal,
    /// Changes that can be undone, the last one on top.
    done: Vec<Txn>,
    /// Changes undone that can be redone, the next one on top.
    undone: Vec<Txn>,
    /// The sha256 of what `session.json` holds.
    saved_sha: String,
    /// Edits, undos and redos since opening, and that count when
    /// `session.json` last caught up.
    changes: u64,
    saved_changes: u64,
    last_change_ms: u64,
    last_snapshot_ms: Option<u64>,
}

impl Package {
    /// Makes `<sessions_dir>/<ULID>.wwavsession` for `session`, whole or not
    /// at all: it is built under a temporary name and renamed into place.
    pub fn create(sessions_dir: &Path, session: Session, clock: Arc<dyn Clock>) -> Result<Package> {
        let dir = sessions_dir.join(format!("{}.wwavsession", session.id));
        if dir.exists() {
            return Err(Error::Exists(dir));
        }
        fs::create_dir_all(sessions_dir)?;
        let building = fsx::tmp_path(&dir);
        if building.exists() {
            fs::remove_dir_all(&building)?; // a create that crashed part way
        }
        fs::create_dir(&building)?;
        for folder in FOLDERS {
            fs::create_dir(building.join(folder))?;
        }
        let doc = as_it_reads_back(&session)?.to_value();
        let bytes = canonical::to_bytes(&doc);
        fsx::write_synced(&building.join("session.json"), &bytes)?;
        let now = clock.now_ms();
        let start = Mark {
            id: wwav_ids::ulid(),
            at: now,
            sha256: sha256_hex(&bytes),
        };
        Journal::create(&building.join("journal/undo.ndjson"), &start)?;
        // The session as it was made is the first thing Revert to offers.
        fs::create_dir(building.join("journal/snapshots"))?;
        fsx::write_synced(&snapshot_path(&building, now), &bytes)?;
        fsx::sync_dir(&building.join("journal/snapshots"))?;
        fsx::sync_dir(&building)?;
        fs::rename(&building, &dir)?;
        fsx::sync_dir(sessions_dir)?;
        Ok(Package::open(&dir, clock)?.0)
    }

    /// Opens a package. Journal entries newer than `session.json` are
    /// replayed and reported; a leftover temporary file from a save that
    /// never got renamed is removed; snapshots past 30 days are pruned.
    pub fn open(dir: &Path, clock: Arc<dyn Clock>) -> Result<(Package, Opened)> {
        let json_path = dir.join("session.json");
        let bytes = match fs::read(&json_path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(Error::NotASession(dir.to_path_buf()))
            }
            Err(e) => return Err(e.into()),
        };
        // A crash between writing the new file and renaming it: the journal
        // holds everything it held.
        match fs::remove_file(fsx::tmp_path(&json_path)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
        for folder in FOLDERS {
            fs::create_dir_all(dir.join(folder))?;
        }
        fs::create_dir_all(dir.join("journal/snapshots"))?;

        let on_disk = Session::from_json_bytes(&bytes)?.to_value();
        let sha = sha256_hex(&bytes);
        let now = clock.now_ms();
        let journal_path = dir.join("journal/undo.ndjson");
        let read = match Journal::open(&journal_path) {
            Ok(j) => Some(j),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let had_journal = read.is_some();
        let replayed = read.and_then(|(journal, lines)| {
            let mut doc = on_disk.clone();
            let history = replay(lines?, &sha, &mut doc).ok()?;
            let session = Session::from_value(doc.clone()).ok()?;
            Some((journal, history, doc, session))
        });

        let (journal, history, doc, session, history_reset) = match replayed {
            Some((journal, history, doc, session)) => (journal, history, doc, session, false),
            None => {
                if had_journal {
                    // Kept, never deleted: it may be all that is left of
                    // something.
                    let kept = dir.join(format!("journal/undo-{}.ndjson", wwav_ids::ulid()));
                    fs::rename(&journal_path, kept)?;
                }
                let start = Mark {
                    id: wwav_ids::ulid(),
                    at: now,
                    sha256: sha.clone(),
                };
                let journal = Journal::create(&journal_path, &start)?;
                let session = Session::from_value(on_disk.clone())?;
                (journal, History::default(), on_disk, session, had_journal)
            }
        };

        let recovered = (history.replayed > 0).then_some(Recovered {
            changes: history.replayed,
            since_ms: history.replayed_since,
        });
        let mut pkg = Package {
            dir: dir.to_path_buf(),
            clock,
            session,
            doc,
            journal,
            done: history.done,
            undone: history.undone,
            saved_sha: sha,
            changes: history.replayed as u64,
            saved_changes: 0,
            last_change_ms: now,
            last_snapshot_ms: None,
        };
        pkg.last_snapshot_ms = pkg.snapshots()?.first().map(|s| s.at_ms);
        pkg.prune_snapshots()?;
        Ok((
            pkg,
            Opened {
                recovered,
                history_reset,
            },
        ))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Makes one change and journals it, labelled for the Edit menu ("move
    /// clip" → "Undo move clip"). When this returns, the change survives the
    /// process being killed. A change that changes nothing journals nothing
    /// and returns false.
    pub fn edit(&mut self, label: &str, change: impl FnOnce(&mut Session)) -> Result<bool> {
        let mut next = self.session.clone();
        change(&mut next);
        let next = as_it_reads_back(&next)?;
        let after = next.to_value();
        let rows = diff::diff(&self.doc, &after);
        if rows.is_empty() {
            return Ok(false);
        }
        let txn = Txn {
            id: wwav_ids::ulid(),
            at: self.clock.now_ms(),
            label: label.to_string(),
            rows,
        };
        self.journal.append(journal::TXN, &txn)?;
        self.done.push(txn);
        self.undone.clear();
        self.changed(next, after);
        Ok(true)
    }

    /// Undoes the last change; returns its label, or None with nothing to
    /// undo.
    pub fn undo(&mut self) -> Result<Option<String>> {
        self.step(true)
    }

    /// Redoes the last undone change; returns its label, or None.
    pub fn redo(&mut self) -> Result<Option<String>> {
        self.step(false)
    }

    /// Undoes (back) or redoes the transaction on top of its stack. The step
    /// is journalled before anything in memory changes.
    fn step(&mut self, back: bool) -> Result<Option<String>> {
        let top = if back {
            self.done.last()
        } else {
            self.undone.last()
        };
        let Some(txn) = top else {
            return Ok(None);
        };
        let mut doc = self.doc.clone();
        let fits = if back {
            diff::undo(&mut doc, &txn.rows)
        } else {
            diff::redo(&mut doc, &txn.rows)
        };
        fits.expect("the journal's rows fit the document they were made from");
        let session = Session::from_value(doc.clone())?;
        let step = Step {
            id: wwav_ids::ulid(),
            at: self.clock.now_ms(),
            txn: txn.id.clone(),
        };
        self.journal
            .append(if back { journal::UNDO } else { journal::REDO }, &step)?;
        let (from, to) = if back {
            (&mut self.done, &mut self.undone)
        } else {
            (&mut self.undone, &mut self.done)
        };
        let txn = from.pop().expect("checked above");
        let label = txn.label.clone();
        to.push(txn);
        self.changed(session, doc);
        Ok(Some(label))
    }

    /// The Edit menu's undo item: "Undo move clip", or "Nothing to undo.".
    pub fn undo_label(&self) -> String {
        match self.done.last() {
            Some(t) => format!("Undo {}", t.label),
            None => "Nothing to undo.".into(),
        }
    }

    /// The Edit menu's redo item: "Redo move clip", or "Nothing to redo.".
    pub fn redo_label(&self) -> String {
        match self.undone.last() {
            Some(t) => format!("Redo {}", t.label),
            None => "Nothing to redo.".into(),
        }
    }

    fn changed(&mut self, session: Session, doc: Value) {
        self.session = session;
        self.doc = doc;
        self.changes += 1;
        self.last_change_ms = self.clock.now_ms();
    }

    /// Whether `session.json` is behind the session.
    pub fn is_dirty(&self) -> bool {
        self.changes != self.saved_changes
    }

    pub(crate) fn last_change_ms(&self) -> u64 {
        self.last_change_ms
    }

    pub(crate) fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    /// Writes `session.json` if it is behind: to `session.json.tmp`, synced,
    /// then renamed over it, so a crash leaves the old file or the new one.
    /// Takes a snapshot when the last is 10 minutes old.
    pub fn save(&mut self) -> Result<()> {
        if let Some(staged) = self.stage_save()? {
            self.commit_save(staged)?;
        }
        Ok(())
    }

    /// The first half of [`Package::save`]: the new file is written and
    /// synced beside the old one, and the journal records its sha256.
    /// Public so a test can stop the process between the halves.
    #[doc(hidden)]
    pub fn stage_save(&mut self) -> Result<Option<Staged>> {
        if !self.is_dirty() {
            return Ok(None);
        }
        let bytes = canonical::to_bytes(&self.doc);
        let sha = sha256_hex(&bytes);
        if sha == self.saved_sha {
            // Changed and changed back: the file already holds this.
            self.saved_changes = self.changes;
            return Ok(None);
        }
        fsx::write_synced(&fsx::tmp_path(&self.json_path()), &bytes)?;
        let mark = Mark {
            id: wwav_ids::ulid(),
            at: self.clock.now_ms(),
            sha256: sha.clone(),
        };
        self.journal.append(journal::SAVE, &mark)?;
        Ok(Some(Staged {
            bytes,
            sha,
            changes: self.changes,
        }))
    }

    /// The second half of [`Package::save`]: the rename.
    #[doc(hidden)]
    pub fn commit_save(&mut self, staged: Staged) -> Result<()> {
        let path = self.json_path();
        fs::rename(fsx::tmp_path(&path), &path)?;
        fsx::sync_dir(&self.dir)?;
        self.saved_sha = staged.sha;
        self.saved_changes = staged.changes;
        self.snapshot(&staged.bytes)
    }

    fn json_path(&self) -> PathBuf {
        self.dir.join("session.json")
    }

    fn snapshot(&mut self, bytes: &[u8]) -> Result<()> {
        let now = self.clock.now_ms();
        if self
            .last_snapshot_ms
            .is_some_and(|t| now < t + SNAPSHOT_EVERY_MS)
        {
            return Ok(());
        }
        fsx::write_atomic(&snapshot_path(&self.dir, now), bytes)?;
        self.last_snapshot_ms = Some(now);
        self.prune_snapshots()
    }

    /// Copies of `session.json` for File → Revert to, newest first.
    pub fn snapshots(&self) -> Result<Vec<Snapshot>> {
        let mut out = Vec::new();
        for entry in fs::read_dir(self.dir.join("journal/snapshots"))? {
            let path = entry?.path();
            let at = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".json"))
                .and_then(wwav_ids::ulid_ms);
            if let Some(at_ms) = at {
                out.push(Snapshot { at_ms, path });
            }
        }
        out.sort_by_key(|s| std::cmp::Reverse(s.at_ms));
        Ok(out)
    }

    fn prune_snapshots(&self) -> Result<()> {
        let now = self.clock.now_ms();
        for s in self.snapshots()? {
            if now.saturating_sub(s.at_ms) > SNAPSHOT_KEEP_MS {
                fs::remove_file(&s.path)?;
            }
        }
        Ok(())
    }

    /// File → Revert to: the snapshot becomes the session, as one change
    /// that ⌘Z takes back.
    pub fn revert_to(&mut self, snapshot: &Snapshot) -> Result<()> {
        let then = Session::from_json_bytes(&fs::read(&snapshot.path)?)?;
        self.edit("revert", |s| *s = then)?;
        Ok(())
    }
}

/// The session as `session.json` would give it back, so what is journalled
/// is what reopens (a NaN in an optional field is written as null and
/// reads back as absent). One that wouldn't read back at all, such as a NaN
/// gain, is refused.
fn as_it_reads_back(session: &Session) -> Result<Session> {
    Session::from_value(session.to_value()).map_err(|e| Error::Unsaveable(e.to_string()))
}

/// `journal/snapshots/<ULID>.json`, the ULID made at `at_ms` on the
/// package's clock so its age reads off its name.
fn snapshot_path(dir: &Path, at_ms: u64) -> PathBuf {
    let random = wwav_ids::decode_ulid(&wwav_ids::ulid()).expect("a fresh ULID");
    let name = wwav_ids::encode_ulid(at_ms, random);
    dir.join(format!("journal/snapshots/{name}.json"))
}

#[derive(Default)]
struct History {
    done: Vec<Txn>,
    undone: Vec<Txn>,
    replayed: usize,
    replayed_since: u64,
}

/// Rebuilds the undo history from the journal and replays onto `doc` (the
/// session as `session.json` holds it) the entries newer than it: those
/// after the last `start` or `save` line naming `session.json`'s sha256.
fn replay(
    lines: Vec<Line>,
    sha: &str,
    doc: &mut Value,
) -> std::result::Result<History, diff::Mismatch> {
    let mismatch = || diff::Mismatch(String::new());
    let names_file = |l: &Line| matches!(l, Line::Start(m) | Line::Save(m) if m.sha256 == sha);
    let anchor = lines.iter().rposition(names_file).ok_or_else(mismatch)?;
    let mut h = History::default();
    for (i, line) in lines.into_iter().enumerate() {
        let newer = i > anchor;
        let at = match line {
            Line::Start(_) | Line::Save(_) => continue,
            Line::Txn(txn) => {
                if newer {
                    diff::redo(doc, &txn.rows)?;
                }
                let at = txn.at;
                h.done.push(txn);
                h.undone.clear();
                at
            }
            Line::Undo(step) => {
                let txn = h
                    .done
                    .pop()
                    .filter(|t| t.id == step.txn)
                    .ok_or_else(mismatch)?;
                if newer {
                    diff::undo(doc, &txn.rows)?;
                }
                h.undone.push(txn);
                step.at
            }
            Line::Redo(step) => {
                let txn = h
                    .undone
                    .pop()
                    .filter(|t| t.id == step.txn)
                    .ok_or_else(mismatch)?;
                if newer {
                    diff::redo(doc, &txn.rows)?;
                }
                h.done.push(txn);
                step.at
            }
        };
        if newer {
            if h.replayed == 0 {
                h.replayed_since = at;
            }
            h.replayed += 1;
        }
    }
    Ok(h)
}
