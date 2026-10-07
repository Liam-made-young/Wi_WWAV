//! A `.wwavsession` package on disk: creating and opening it, journalled
//! changes, undo and redo, saving, recovery and snapshots.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use crate::autosave::Clock;
use crate::journal::{self, Journal, Line, Mark, Step, Txn};
use crate::model::Session;
use crate::{canonical, diff, fsx, part, sha256_hex, Error, Result};

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
    /// The journal didn't match `session.json` (it was changed or put back
    /// outside the app, or the journal was damaged). The file is opened as
    /// it is, the old journal is kept beside the new one, and undo starts
    /// here.
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

/// A save written and synced to a temporary file beside `session.json`,
/// not yet renamed.
#[derive(Debug)]
pub struct Staged {
    tmp: PathBuf,
    bytes: Vec<u8>,
    sha: String,
    changes: u64,
}

pub struct Package {
    dir: PathBuf,
    /// `journal/lock`, held while the package is open; None on a
    /// filesystem that can't lock.
    _lock: Option<File>,
    clock: Arc<dyn Clock>,
    session: Session,
    /// The session as JSON, the document the journal's pointers address.
    /// Always equal to `session.to_value()`; edits and undo change the two
    /// together, part by part.
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
    ///
    /// A package is open in one place at a time: while a `Package` holds
    /// it (another window, or a helper process), a second open is refused
    /// with [`Error::InUse`].
    pub fn open(dir: &Path, clock: Arc<dyn Clock>) -> Result<(Package, Opened)> {
        let json_path = dir.join("session.json");
        if !json_path.is_file() {
            return Err(Error::NotASession(dir.to_path_buf()));
        }
        fs::create_dir_all(dir.join("journal"))?;
        let lock = lock(dir)?;
        // No save is under way now: a temporary file is one a crash left
        // before its rename, and the journal holds everything it held.
        remove_unrenamed_saves(dir)?;
        for folder in FOLDERS {
            fs::create_dir_all(dir.join(folder))?;
        }
        fs::create_dir_all(dir.join("journal/snapshots"))?;

        let bytes = fs::read(&json_path)?;
        let on_disk = Session::from_json_bytes(&bytes)?;
        let on_disk_doc = on_disk.to_value();
        let sha = sha256_hex(&bytes);
        let now = clock.now_ms();
        let read = match Journal::open(&dir.join("journal/undo.ndjson")) {
            Ok(j) => Some(j),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let had_journal = read.is_some();
        let restored = read.and_then(|(journal, lines)| {
            restore(lines, &sha, &on_disk, &on_disk_doc).map(|r| (journal, r))
        });
        let (journal, (history, session, doc), history_reset) = match restored {
            Some((journal, r)) => (journal, r, false),
            None => (
                new_journal(dir, &sha, now)?,
                (History::default(), on_disk, on_disk_doc),
                had_journal,
            ),
        };

        let recovered = (history.replayed > 0).then_some(Recovered {
            changes: history.replayed,
            since_ms: history.replayed_since,
        });
        let mut pkg = Package {
            dir: dir.to_path_buf(),
            _lock: lock,
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
        let rows = part::rows(&self.session, &mut next, &self.doc)?;
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
        if diff::redo(&mut self.doc, &txn.rows).is_err() {
            // The rows were made from this document, so they fit it; were
            // that ever not so, the session says what the document is.
            self.doc = next.to_value();
        }
        self.done.push(txn);
        self.undone.clear();
        self.session = next;
        self.touched();
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

    /// Undoes (back) or redoes the transaction on top of its stack. Nothing
    /// is kept unless the step is journalled.
    fn step(&mut self, back: bool) -> Result<Option<String>> {
        let stack = if back {
            &mut self.done
        } else {
            &mut self.undone
        };
        let Some(txn) = stack.pop() else {
            return Ok(None);
        };
        if part::apply(&mut self.doc, &mut self.session, &txn.rows, !back).is_err() {
            return Err(self.restart_history());
        }
        let step = Step {
            id: wwav_ids::ulid(),
            at: self.clock.now_ms(),
            txn: txn.id.clone(),
        };
        let op = if back { journal::UNDO } else { journal::REDO };
        if let Err(e) = self.journal.append(op, &step) {
            // Rows just applied always fit the other way.
            let _ = part::apply(&mut self.doc, &mut self.session, &txn.rows, back);
            let stack = if back {
                &mut self.done
            } else {
                &mut self.undone
            };
            stack.push(txn);
            return Err(e.into());
        }
        let label = txn.label.clone();
        if back {
            self.undone.push(txn);
        } else {
            self.done.push(txn);
        }
        self.touched();
        Ok(Some(label))
    }

    /// The undo history doesn't fit the session (a damaged journal): the
    /// session is saved as it stands and a new history starts from it, the
    /// old journal kept beside it. Only undo is lost, never a change.
    fn restart_history(&mut self) -> Error {
        self.done.clear();
        self.undone.clear();
        if let Err(e) = self.save() {
            return e;
        }
        match new_journal(&self.dir, &self.saved_sha, self.clock.now_ms()) {
            Ok(journal) => {
                self.journal = journal;
                Error::History
            }
            Err(e) => e,
        }
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

    fn touched(&mut self) {
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

    /// Writes `session.json` if it is behind: to a temporary file, synced,
    /// then renamed over it, so a crash leaves the old file or the new one.
    /// Takes a snapshot when the last is 10 minutes old.
    pub fn save(&mut self) -> Result<()> {
        if let Some(staged) = self.stage_save()? {
            self.commit_save(staged)?;
        }
        Ok(())
    }

    /// The first half of [`Package::save`]: the new file is written and
    /// synced beside the old one, under a name of its own, and the journal
    /// records its sha256. Public so a test can stop the process between
    /// the halves.
    #[doc(hidden)]
    pub fn stage_save(&mut self) -> Result<Option<Staged>> {
        if !self.is_dirty() {
            return Ok(None);
        }
        let bytes = canonical::to_bytes(&self.doc);
        let sha = sha256_hex(&bytes);
        if sha == self.saved_sha {
            // Changed and changed back: the file already holds this. The
            // journal says so, or opening would replay what led here.
            let mark = self.mark(sha);
            self.journal.append(journal::SAVED, &mark)?;
            self.saved_changes = self.changes;
            return Ok(None);
        }
        let tmp = fsx::unique_tmp_path(&self.json_path());
        let mark = self.mark(sha.clone());
        let staged = fsx::write_synced(&tmp, &bytes)
            .and_then(|()| self.journal.append(journal::SAVE, &mark));
        if let Err(e) = staged {
            let _ = fs::remove_file(&tmp);
            return Err(e.into());
        }
        Ok(Some(Staged {
            tmp,
            bytes,
            sha,
            changes: self.changes,
        }))
    }

    /// The second half of [`Package::save`]: the rename, then a `saved`
    /// line, so a `session.json` put back to an older save later is known
    /// for what it is.
    #[doc(hidden)]
    pub fn commit_save(&mut self, staged: Staged) -> Result<()> {
        fs::rename(&staged.tmp, self.json_path())?;
        fsx::sync_dir(&self.dir)?;
        self.saved_sha = staged.sha;
        self.saved_changes = staged.changes;
        let mark = self.mark(self.saved_sha.clone());
        self.journal.append(journal::SAVED, &mark)?;
        self.snapshot(&staged.bytes)
    }

    /// A journal line naming the state `session.json` holds or will.
    fn mark(&self, sha256: String) -> Mark {
        Mark {
            id: wwav_ids::ulid(),
            at: self.clock.now_ms(),
            sha256,
        }
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

/// Takes the package for this `Package`: the lock goes with it when it is
/// dropped or its process ends, however it ends. A filesystem that can't
/// lock (some network volumes) opens it anyway; each save still writes a
/// temporary file of its own.
fn lock(dir: &Path) -> Result<Option<File>> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("journal/lock"))?;
    // A process being started holds a copy of every open file until it
    // runs its program, so a lock just let go (a window closed and opened
    // again while the app starts its engine) can look held for a moment.
    for _ in 0..LOCK_TRIES {
        match file.try_lock() {
            Ok(()) => return Ok(Some(file)),
            Err(TryLockError::WouldBlock) => std::thread::sleep(LOCK_WAIT),
            Err(TryLockError::Error(_)) => return Ok(None),
        }
    }
    Err(Error::InUse(dir.to_path_buf()))
}

/// 50 tries 5 ms apart: a quarter of a second before an open is refused.
const LOCK_TRIES: u32 = 50;
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_millis(5);

/// Removes the temporary files of saves that never got renamed.
fn remove_unrenamed_saves(dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("session.json.") && name.ends_with(".tmp") {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// Starts a new history at `sha`, what `session.json` holds. An old
/// journal is kept beside it, never deleted: it may be all that is left of
/// something.
fn new_journal(dir: &Path, sha: &str, now: u64) -> Result<Journal> {
    let path = dir.join("journal/undo.ndjson");
    if path.exists() {
        let kept = dir.join(format!("journal/undo-{}.ndjson", wwav_ids::ulid()));
        fs::rename(&path, kept)?;
    }
    let start = Mark {
        id: wwav_ids::ulid(),
        at: now,
        sha256: sha.to_string(),
    };
    Ok(Journal::create(&path, &start)?)
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

impl History {
    /// Whether every change on the stacks undoes or redoes from `doc`, as
    /// ⌘Z and ⌘⇧Z will take them.
    fn fits(&self, doc: &Value) -> bool {
        let mut d = doc.clone();
        self.done
            .iter()
            .rev()
            .all(|t| diff::undo(&mut d, &t.rows).is_ok())
            && self
                .done
                .iter()
                .all(|t| diff::redo(&mut d, &t.rows).is_ok())
            && self
                .undone
                .iter()
                .rev()
                .all(|t| diff::redo(&mut d, &t.rows).is_ok())
    }
}

/// The journal's history and the session with the entries newer than
/// `session.json` replayed onto it, or None when they don't fit the file.
fn restore(
    lines: Vec<Line>,
    sha: &str,
    on_disk: &Session,
    on_disk_doc: &Value,
) -> Option<(History, Session, Value)> {
    let mut doc = on_disk_doc.clone();
    let history = replay(lines, sha, &mut doc).ok()?;
    let (session, doc) = if history.replayed == 0 {
        (on_disk.clone(), doc)
    } else {
        let session = Session::from_value(doc).ok()?;
        let doc = session.to_value();
        (session, doc)
    };
    history.fits(&doc).then_some((history, session, doc))
}

/// Rebuilds the undo history from the journal and replays onto `doc` (the
/// session as `session.json` holds it) the entries newer than it: those
/// after the last line naming `session.json`'s sha256. When a save
/// finished after that line, the file was put back from outside (a backup,
/// git) and the journal isn't its history.
fn replay(
    lines: Vec<Line>,
    sha: &str,
    doc: &mut Value,
) -> std::result::Result<History, diff::Mismatch> {
    let mismatch = || diff::Mismatch(String::new());
    let names_file =
        |l: &Line| matches!(l, Line::Start(m) | Line::Save(m) | Line::Saved(m) if m.sha256 == sha);
    let anchor = lines.iter().rposition(names_file).ok_or_else(mismatch)?;
    if lines[anchor + 1..]
        .iter()
        .any(|l| matches!(l, Line::Saved(_)))
    {
        return Err(mismatch());
    }
    let mut h = History::default();
    for (i, line) in lines.into_iter().enumerate() {
        let newer = i > anchor;
        let at = match line {
            Line::Start(_) | Line::Save(_) | Line::Saved(_) => continue,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part::tests::{change, start, Rng};
    use crate::SystemClock;

    fn assert_doc_is_the_session(pkg: &Package, at: &str) {
        assert_eq!(pkg.doc, pkg.session.to_value(), "{at}");
        assert_eq!(
            Session::from_value(pkg.doc.clone()).unwrap(),
            pkg.session,
            "{at}"
        );
    }

    #[test]
    fn the_document_stays_the_sessions_json_through_edits_undo_redo_and_relaunch() {
        for seed in 1..=6u64 {
            let tmp = tempfile::tempdir().unwrap();
            let clock: Arc<dyn Clock> = Arc::new(SystemClock);
            let mut pkg = Package::create(tmp.path(), start(), clock.clone()).unwrap();
            let mut rng = Rng(seed.wrapping_mul(0x2545_f491_4f6c_dd1d));
            for step in 0..120 {
                match rng.below(10) {
                    0..=5 => {
                        pkg.edit("change", |s| change(s, &mut rng)).unwrap();
                    }
                    6 | 7 => {
                        pkg.undo().unwrap();
                    }
                    8 => {
                        pkg.redo().unwrap();
                    }
                    _ => {
                        if rng.below(2) == 0 {
                            pkg.save().unwrap();
                        }
                        let dir = pkg.dir().to_path_buf();
                        drop(pkg);
                        pkg = Package::open(&dir, clock.clone()).unwrap().0;
                    }
                }
                assert_doc_is_the_session(&pkg, &format!("seed {seed}, step {step}"));
            }
        }
    }

    #[test]
    fn an_undo_that_does_not_fit_starts_a_new_history_and_loses_no_change() {
        // A journal damaged in a way open can't see: ⌘Z must not panic or
        // change the session; the session is saved and undo starts again.
        let tmp = tempfile::tempdir().unwrap();
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let mut pkg = Package::create(tmp.path(), start(), clock.clone()).unwrap();
        pkg.edit("rename", |s| s.title = "Kept".into()).unwrap();
        pkg.edit("move clip", |s| s.tracks[0].events[0].at_ms = 4321)
            .unwrap();
        // Undo checks what a row left: say it left something else.
        pkg.done[0].rows[0].after = Some(serde_json::json!("Not what it is"));
        assert_eq!(pkg.undo().unwrap().as_deref(), Some("move clip"));
        let held = pkg.session.to_json_bytes();

        let err = pkg.undo().unwrap_err();
        assert!(matches!(err, Error::History), "{err}");
        assert_eq!(pkg.session.to_json_bytes(), held, "the session changed");
        assert_eq!(pkg.undo_label(), "Nothing to undo.");
        assert_eq!(pkg.redo_label(), "Nothing to redo.");
        assert!(!pkg.is_dirty(), "saved as it stands");
        assert_doc_is_the_session(&pkg, "after the failed undo");
        // Still works, and reopens as it was.
        pkg.edit("move clip", |s| s.tracks[1].events[0].at_ms = 99)
            .unwrap();
        let held = pkg.session.to_json_bytes();
        let dir = pkg.dir().to_path_buf();
        drop(pkg);
        let (pkg, opened) = Package::open(&dir, clock).unwrap();
        assert!(!opened.history_reset);
        assert_eq!(pkg.session.to_json_bytes(), held);
        assert_eq!(pkg.undo_label(), "Undo move clip");
        let kept = fs::read_dir(dir.join("journal"))
            .unwrap()
            .filter(|e| {
                let name = e.as_ref().unwrap().file_name();
                name.to_string_lossy().starts_with("undo-")
            })
            .count();
        assert_eq!(kept, 1, "the old journal is kept beside the new one");
    }
}
