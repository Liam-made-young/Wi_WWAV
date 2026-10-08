//! Notes in the core (docs/NOTES.md): the `heat.note.*` commands, and
//! keeping the notes folder and the library the same.
//!
//! A note is a record the app writes and a markdown file the folder holds.
//! [`reconcile`] makes them agree, whichever side moved: a record that
//! changed (an edit, an undo, Claude's `add_note`) is written to its file; a
//! file that changed outside the app is written into its record, as one
//! journal entry, "notes from disk"; a new file becomes a note; a file that
//! is gone takes its note with it; a note that is gone leaves its file in
//! `.trash`. When both moved the file wins, and the app's version is one ⌘Z
//! away. It runs after every note command, when anything else changes a
//! note, and every few seconds for the folder.
//!
//! What was last seen of each file is a `noteFile` record, outside the
//! journal: its path, the hash of its text, and the hash of the note it was
//! written from. A file whose size and time haven't changed isn't read.
//!
//! `Config::notes_dir` names the folder. With none, notes are records only:
//! a test's core writes no files unless it asks to.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, UNIX_EPOCH};

use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wi_heat::notes::{self as rules, Meta, ATTACHMENTS};
use wi_heat_store::notes::{self, OnDisk};
use wi_heat_store::{all, kind};
use wi_store::Actor;

use crate::args::Args;
use crate::bus::lock;
use crate::heat_cmd::{core_error, open_heat, write, wrote_outside};
use crate::{capture, CoreError, Inner};

/// How often the folder is looked at for what changed outside the app.
const LOOK_EVERY: Duration = Duration::from_secs(3);

/// The largest attachment a view is handed whole.
const ATTACHMENT_MAX: u64 = 24 * 1024 * 1024;

/// One reconcile at a time.
static ONE: Mutex<()> = Mutex::new(());

fn no_folder() -> CoreError {
    CoreError::new("refused", "Notes aren't kept in a folder here.")
}

/// The notes folder, made if it isn't there.
pub(crate) fn folder(i: &Inner) -> Result<PathBuf, CoreError> {
    let dir = i.notes_dir.clone().ok_or_else(no_folder)?;
    std::fs::create_dir_all(dir.join(ATTACHMENTS))?;
    Ok(dir)
}

fn hash(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// What of a note its file holds: when any of this moves, the file is stale.
fn note_hash(note: &Value, meta: &Meta) -> String {
    hash(&format!(
        "{}\u{0}{}\u{0}{:?}\u{0}{:?}\u{0}{:?}\u{0}{}",
        note["title"].as_str().unwrap_or(""),
        note["markdown"].as_str().unwrap_or(""),
        meta.course,
        meta.space,
        meta.captured,
        meta.inbox
    ))
}

/// What a note's front matter says, from its record.
fn meta_of(i: &Inner, note: &Value, courses: &[Value], spaces: &[Value]) -> Meta {
    let named = |list: &[Value], id: Option<&str>, field: &str| {
        id.and_then(|id| list.iter().find(|r| r["id"] == id))
            .and_then(|r| r[field].as_str().map(String::from))
    };
    Meta {
        id: note["id"].as_str().map(String::from),
        course: named(courses, note["courseId"].as_str(), "code"),
        space: named(spaces, note["spaceId"].as_str(), "name"),
        captured: note["capturedAt"].as_f64().map(|at| i.clock().iso(at)),
        inbox: note["inbox"] == true,
        other: Vec::new(),
    }
}

struct Seen {
    rel: String,
    path: PathBuf,
    size: u64,
    mtime: i64,
}

/// Every markdown file in the folder, but what is in `attachments` or in a
/// folder whose name starts with a dot.
fn files(dir: &Path) -> Vec<Seen> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<Seen>, depth: usize) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(info) = entry.metadata() else {
                continue;
            };
            if info.is_dir() {
                if name != ATTACHMENTS && depth < 6 {
                    walk(root, &path, out, depth + 1);
                }
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"))
            {
                let mtime = info
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis() as i64);
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(Seen {
                    rel,
                    path,
                    size: info.len(),
                    mtime,
                });
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out, 0);
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

fn stem_of(rel: &str) -> String {
    Path::new(rel)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn stamp(path: &Path) -> (u64, i64) {
    std::fs::metadata(path)
        .map(|m| {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_millis() as i64);
            (m.len(), mtime)
        })
        .unwrap_or((0, 0))
}

/// A file moved out of the way, into `.trash`, never deleted.
fn trash(dir: &Path, path: &Path) {
    let bin = dir.join(".trash");
    if std::fs::create_dir_all(&bin).is_err() {
        return;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut to = bin.join(&name);
    let mut n = 1;
    while to.exists() {
        n += 1;
        to = bin.join(format!("{} {n}.md", stem_of(&name)));
    }
    let _ = std::fs::rename(path, to);
}

/// Makes the folder and the library agree. Answers how many notes or files
/// it had to write. With no folder configured it does nothing.
pub(crate) fn reconcile(i: &Inner) -> Result<usize, CoreError> {
    if i.notes_dir.is_none() {
        return Ok(0);
    }
    let dir = folder(i)?;
    let _one = lock(&ONE);
    let (records, states, courses, spaces) = {
        let store = i.store();
        (
            notes::notes(&store).map_err(core_error)?,
            notes::file_states(&store).map_err(core_error)?,
            all(&store, kind::COURSE).map_err(core_error)?,
            all(&store, kind::SPACE).map_err(core_error)?,
        )
    };
    let by_id: BTreeMap<&str, &Value> = records
        .iter()
        .filter_map(|n| n["id"].as_str().map(|id| (id, n)))
        .collect();
    let on_disk = files(&dir);
    let by_rel: BTreeMap<&str, &Seen> = on_disk.iter().map(|f| (f.rel.as_str(), f)).collect();
    let mut claimed: BTreeSet<String> = BTreeSet::new();
    let mut changes: Vec<OnDisk> = Vec::new();
    // What to remember once the changes are in: the note's id (None for a
    // new file, filled from what `apply_disk` answers), its path and the
    // file's hash.
    let mut new_files: Vec<(String, String, u64, i64)> = Vec::new();
    let mut to_write: Vec<(&Value, Option<String>)> = Vec::new();
    let mut state_sets: Vec<(String, Value)> = Vec::new();
    let mut state_drops: Vec<String> = Vec::new();
    let mut wrote = 0;

    // Files nothing remembers, read once: a new note, or a note's file
    // renamed outside the app, known by the id in its front matter.
    let known_paths: BTreeSet<&str> = states.values().filter_map(|s| s["path"].as_str()).collect();
    let mut strays: Vec<(&Seen, String, Meta, String)> = Vec::new();
    for f in on_disk
        .iter()
        .filter(|f| !known_paths.contains(f.rel.as_str()))
    {
        if let Ok(text) = std::fs::read_to_string(&f.path) {
            let (meta, body) = rules::from_file(&text);
            strays.push((f, text, meta, body));
        }
    }
    let mut adopted: BTreeSet<String> = BTreeSet::new();

    for (id, state) in &states {
        let path = state["path"].as_str().unwrap_or_default();
        let Some(note) = by_id.get(id.as_str()) else {
            // The note is gone: its file goes to the trash.
            if let Some(f) = by_rel.get(path) {
                trash(&dir, &f.path);
                wrote += 1;
            }
            state_drops.push(id.clone());
            continue;
        };
        let meta = meta_of(i, note, &courses, &spaces);
        let record_moved = state["noteHash"].as_str() != Some(note_hash(note, &meta).as_str());
        match by_rel.get(path) {
            Some(f) => {
                claimed.insert(f.rel.clone());
                let same_stamp = state["size"].as_u64() == Some(f.size)
                    && state["mtime"].as_i64() == Some(f.mtime);
                let text = if same_stamp {
                    None
                } else {
                    std::fs::read_to_string(&f.path).ok()
                };
                let disk_moved = text
                    .as_deref()
                    .is_some_and(|t| state["fileHash"].as_str() != Some(hash(t).as_str()));
                if disk_moved {
                    let text = text.unwrap_or_default();
                    let (file_meta, body) = rules::from_file(&text);
                    // The file's name is the title only when someone renamed it.
                    let stem = stem_of(&f.rel);
                    let title = if stem == rules::file_stem(note["title"].as_str().unwrap_or("")) {
                        note["title"].as_str().unwrap_or("").to_string()
                    } else {
                        stem
                    };
                    changes.push(OnDisk::Changed {
                        id: id.clone(),
                        title,
                        markdown: body,
                        meta: file_meta,
                    });
                    state_sets.push((
                        id.clone(),
                        json!({"path": f.rel, "fileHash": hash(&text), "size": f.size, "mtime": f.mtime, "noteHash": null}),
                    ));
                } else if record_moved {
                    to_write.push((note, Some(path.to_string())));
                } else if !same_stamp {
                    // Touched, not changed: remember the new time.
                    let mut s = state.clone();
                    s["size"] = json!(f.size);
                    s["mtime"] = json!(f.mtime);
                    state_sets.push((id.clone(), s));
                }
            }
            None => {
                // Renamed outside the app, or deleted.
                match strays
                    .iter()
                    .find(|(_, _, m, _)| m.id.as_deref() == Some(id.as_str()))
                {
                    Some((f, text, file_meta, body)) => {
                        adopted.insert(f.rel.clone());
                        claimed.insert(f.rel.clone());
                        changes.push(OnDisk::Changed {
                            id: id.clone(),
                            title: stem_of(&f.rel),
                            markdown: body.clone(),
                            meta: file_meta.clone(),
                        });
                        state_sets.push((
                            id.clone(),
                            json!({"path": f.rel, "fileHash": hash(text), "size": f.size, "mtime": f.mtime, "noteHash": null}),
                        ));
                    }
                    None if record_moved => to_write.push((note, None)),
                    None => {
                        changes.push(OnDisk::Gone { id: id.clone() });
                        state_drops.push(id.clone());
                    }
                }
            }
        }
    }
    // Notes with no file yet, and files with no note yet.
    for note in &records {
        let id = note["id"].as_str().unwrap_or_default();
        if states.contains_key(id) {
            continue;
        }
        match strays
            .iter()
            .find(|(f, _, m, _)| m.id.as_deref() == Some(id) && !adopted.contains(&f.rel))
        {
            // Its file is there already (a library put back from a backup).
            Some((f, text, file_meta, body)) => {
                adopted.insert(f.rel.clone());
                claimed.insert(f.rel.clone());
                changes.push(OnDisk::Changed {
                    id: id.to_string(),
                    title: stem_of(&f.rel),
                    markdown: body.clone(),
                    meta: file_meta.clone(),
                });
                state_sets.push((
                    id.to_string(),
                    json!({"path": f.rel, "fileHash": hash(text), "size": f.size, "mtime": f.mtime, "noteHash": null}),
                ));
            }
            None => to_write.push((note, None)),
        }
    }
    for (f, text, meta, body) in strays
        .iter()
        .filter(|(f, _, _, _)| !adopted.contains(&f.rel))
    {
        changes.push(OnDisk::New {
            title: stem_of(&f.rel),
            markdown: body.clone(),
            meta: meta.clone(),
        });
        new_files.push((f.rel.clone(), hash(text), f.size, f.mtime));
        claimed.insert(f.rel.clone());
    }

    // The records' side first: each note that moved is written to its file.
    let mut taken: BTreeSet<String> = on_disk.iter().map(|f| f.rel.to_lowercase()).collect();
    for (note, was) in to_write {
        let id = note["id"].as_str().unwrap_or_default();
        let mut meta = meta_of(i, note, &courses, &spaces);
        let within = was
            .as_deref()
            .and_then(|p| p.rsplit_once('/'))
            .map(|(d, _)| d.to_string());
        if let Some(old) = &was {
            // Lines of front matter the person wrote stay.
            if let Ok(text) = std::fs::read_to_string(dir.join(old)) {
                meta.other = rules::from_file(&text).0.other;
            }
            taken.remove(&old.to_lowercase());
        }
        let stem = rules::file_stem(note["title"].as_str().unwrap_or(""));
        let mut rel;
        let mut n = 1;
        loop {
            let name = if n == 1 {
                format!("{stem}.md")
            } else {
                format!("{stem} {n}.md")
            };
            rel = match &within {
                Some(d) => format!("{d}/{name}"),
                None => name,
            };
            if !taken.contains(&rel.to_lowercase()) {
                break;
            }
            n += 1;
        }
        taken.insert(rel.to_lowercase());
        let text = rules::to_file(&meta, note["markdown"].as_str().unwrap_or(""));
        let path = dir.join(&rel);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Some(old) = was.filter(|old| *old != rel) {
            let _ = std::fs::rename(dir.join(&old), &path);
        }
        if std::fs::write(&path, &text).is_err() {
            continue;
        }
        wrote += 1;
        let (size, mtime) = stamp(&path);
        meta.other.clear();
        state_sets.push((
            id.to_string(),
            json!({"path": rel, "fileHash": hash(&text), "size": size, "mtime": mtime, "noteHash": note_hash(note, &meta)}),
        ));
    }

    // Then the folder's side, as one entry.
    let mut made: Vec<String> = Vec::new();
    if !changes.is_empty() {
        let mut ids = Vec::new();
        write(i, |s, c| {
            let (outcome, new) = notes::apply_disk(s, c, &changes)?;
            ids = new;
            Ok(outcome)
        })?;
        made = ids;
        wrote += changes.len();
    }
    // What is remembered: for a note the folder changed, the hash of the
    // note as it now stands, so the next look finds nothing to do.
    let mut store = i.store();
    let now: BTreeMap<String, Value> = notes::notes(&store)
        .map_err(core_error)?
        .into_iter()
        .filter_map(|n| n["id"].as_str().map(|id| (id.to_string(), n.clone())))
        .collect();
    for (id, rel_hash) in made.iter().zip(new_files) {
        let (rel, file_hash, size, mtime) = rel_hash;
        state_sets.push((id.clone(), json!({"path": rel, "fileHash": file_hash, "size": size, "mtime": mtime, "noteHash": null})));
    }
    for (id, mut state) in state_sets {
        if state["noteHash"].is_null() {
            match now.get(&id) {
                Some(note) => {
                    let mut meta = meta_of(i, note, &courses, &spaces);
                    meta.other.clear();
                    state["noteHash"] = json!(note_hash(note, &meta));
                }
                None => continue,
            }
        }
        notes::file_state_set(&mut store, &id, &state).map_err(core_error)?;
    }
    for id in state_drops {
        notes::file_state_remove(&mut store, &id).map_err(core_error)?;
    }
    Ok(wrote)
}

/// A note command's answer, with the folder brought up to date behind it.
fn then_files(i: &Inner, out: Result<Value, CoreError>) -> Result<Value, CoreError> {
    let out = out?;
    reconcile(i)?;
    Ok(out)
}

/// A path as a note writes it (`attachments/page.jpg`) as a file inside the
/// notes folder, and nowhere else.
fn inside(dir: &Path, rel: &str) -> Result<PathBuf, CoreError> {
    let rel = rel.trim().trim_start_matches("./");
    let bad = rel.is_empty()
        || rel.starts_with('/')
        || rel.contains('\\')
        || rel.split('/').any(|part| part == ".." || part.is_empty());
    if bad {
        return Err(CoreError::new(
            "refused",
            "That file isn't in the notes folder.",
        ));
    }
    Ok(dir.join(rel))
}

fn mime_of(path: &Path) -> &'static str {
    match path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("heic" | "heif") => "image/heic",
        Some("tif" | "tiff") => "image/tiff",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// The path of a note's file, for the Finder.
fn file_of(i: &Inner, id: &str) -> Result<PathBuf, CoreError> {
    let dir = folder(i)?;
    let note = notes::find(&i.store(), id).map_err(core_error)?;
    let states = notes::file_states(&i.store()).map_err(core_error)?;
    states
        .get(note["id"].as_str().unwrap_or_default())
        .and_then(|s| s["path"].as_str())
        .map(|rel| dir.join(rel))
        .ok_or_else(|| CoreError::new("refused", "That note has no file yet."))
}

pub(crate) fn invoke(i: &Inner, cmd: &str, a: &Args) -> Result<Value, CoreError> {
    match cmd {
        "heat.note.create" => {
            open_heat(i)?;
            let args = a.object();
            then_files(i, write(i, |s, c| notes::create(s, c, &args, Actor::You)))
        }
        "heat.note.save" => {
            let (id, title, markdown) = (a.str("id")?, a.opt_str("title"), a.opt_str("markdown"));
            then_files(i, write(i, |s, c| notes::save(s, c, id, title, markdown)))
        }
        "heat.note.daily" => {
            open_heat(i)?;
            let (date, markdown) = (a.str("date")?, a.opt_str("markdown"));
            then_files(i, write(i, |s, c| notes::daily(s, c, date, markdown)))
        }
        "heat.note.search" => {
            let limit = a.opt_usize("limit")?.unwrap_or(30);
            notes::search(&i.store(), a.str("q")?, limit).map_err(core_error)
        }
        "heat.note.link" => {
            let (id, to) = (a.str("id")?, a.str("to")?);
            then_files(i, write(i, |s, c| notes::link(s, c, id, to, Actor::You)))
        }
        "heat.note.file" => {
            let id = a.str("id")?;
            let mut args = a.object();
            // A title is the capture pipeline's to give, not a caller's.
            args.remove("title");
            then_files(i, write(i, |s, c| notes::file(s, c, id, &args, Actor::You)))
        }
        "heat.note.taskFromLine" => {
            let id = a.str("id")?;
            let line = a.opt_usize("line")?.ok_or_else(|| {
                CoreError::new(
                    "bad_args",
                    "heat.note.taskFromLine needs line, a whole number.",
                )
            })?;
            then_files(i, write(i, |s, c| notes::task_from_line(s, c, id, line)))
        }
        "heat.note.toggleBox" => {
            let id = a.str("id")?;
            let line = a.opt_usize("line")?.ok_or_else(|| {
                CoreError::new(
                    "bad_args",
                    "heat.note.toggleBox needs line, a whole number.",
                )
            })?;
            let done = a.opt_bool("done")?.ok_or_else(|| {
                CoreError::new("bad_args", "heat.note.toggleBox needs done, true or false.")
            })?;
            then_files(i, write(i, |s, c| notes::toggle_box(s, c, id, line, done)))
        }
        "heat.note.suggestion.accept" => {
            let id = a.str("noteId")?;
            let index = a.opt_usize("index")?.ok_or_else(|| {
                CoreError::new(
                    "bad_args",
                    "heat.note.suggestion.accept needs index, a whole number.",
                )
            })?;
            write(i, |s, c| notes::suggestion_accept(s, c, id, index))
        }
        "heat.note.suggestion.dismiss" => {
            let id = a.str("noteId")?;
            write(i, |s, _| notes::suggestion_dismiss(s, id))
        }
        "heat.note.attachment" => {
            let path = inside(&folder(i)?, a.str("path")?)?;
            let size = std::fs::metadata(&path)
                .map_err(|_| {
                    CoreError::new("refused", "That file isn't in the notes folder any more.")
                })?
                .len();
            if size > ATTACHMENT_MAX {
                return Err(CoreError::new(
                    "refused",
                    "That file is too large to show here. Open it from the notes folder.",
                ));
            }
            let bytes = std::fs::read(&path)?;
            let data = base64::engine::general_purpose::STANDARD.encode(bytes);
            Ok(json!({"dataUrl": format!("data:{};base64,{data}", mime_of(&path))}))
        }
        // Shows the note's file, or the folder, in the Finder.
        "heat.note.reveal" => {
            let path = match a.opt_str("id") {
                Some(id) => file_of(i, id)?,
                None => folder(i)?,
            };
            if cfg!(target_os = "macos") && !a.opt_bool("quiet")?.unwrap_or(false) {
                let mut open = std::process::Command::new("/usr/bin/open");
                if path.is_file() {
                    open.arg("-R");
                }
                let _ = open.arg(&path).spawn();
            }
            Ok(json!({ "path": path }))
        }
        "heat.note.sync" => {
            open_heat(i)?;
            // Twice: what the first pass wrote into the library, the second writes out.
            let first = reconcile(i)?;
            Ok(json!({"changed": first + reconcile(i)?}))
        }
        "heat.notice.dismiss" => {
            let id = a.str("id")?;
            write(i, |s, _| notes::notice_dismiss(s, id))
        }
        _ => Err(CoreError::new(
            "unknown_command",
            format!("There is no command called '{cmd}'."),
        )),
    }
}

/// The notes' share of the snapshot: the store's, and what only the core
/// knows (the folder, the capture inbox).
pub(crate) fn for_snapshot(i: &Inner, snap: &mut Value) -> Result<(), CoreError> {
    {
        let store = i.store();
        notes::for_snapshot(&store, &i.clock(), snap).map_err(core_error)?;
    }
    snap["notes"]["folder"] = json!(i.notes_dir);
    snap["notes"]["capture"] = capture::status(i);
    Ok(())
}

/// Daily notes from before Notes are moved over, once.
pub(crate) fn bring_daily_notes(i: &Inner) -> Result<(), CoreError> {
    write(i, notes::bring_daily_notes)?;
    Ok(())
}

fn about_notes(payload: &Value) -> bool {
    payload["kinds"].as_array().is_some_and(|kinds| {
        kinds
            .iter()
            .filter_map(Value::as_str)
            .any(|k| matches!(k, "note" | "course" | "space"))
    })
}

pub(crate) fn start(inner: &Arc<Inner>) -> JoinHandle<()> {
    let i = inner.clone();
    let events = i.bus.subscribe();
    std::thread::Builder::new()
        .name("heat notes".into())
        .spawn(move || {
            let mut looked = Instant::now() - LOOK_EVERY;
            while !i.closing() {
                // A note changed by anything (an undo, the MCP helper) is
                // written to its file at once; the folder is looked at
                // every few seconds.
                let mut due = looked.elapsed() >= LOOK_EVERY;
                match events.recv_timeout(Duration::from_millis(250)) {
                    Ok(e) if e.event == "heat" && about_notes(&e.payload) => due = true,
                    Ok(_) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                }
                if !due || i.closing() {
                    continue;
                }
                looked = Instant::now();
                if i.notes_dir.is_some() {
                    let _ = reconcile(&i);
                }
                if capture::work(&i) {
                    wrote_outside(&i, &["notice"]);
                }
            }
        })
        .expect("a thread for Learn's notes")
}

/// Used by `capture.rs`: a file copied into `attachments/` under a name
/// nothing there has, and the path a note writes for it.
pub(crate) fn attach(
    i: &Inner,
    from: &Path,
    name: &str,
    moving: bool,
) -> Result<String, CoreError> {
    let dir = folder(i)?.join(ATTACHMENTS);
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.to_string(), String::new()),
    };
    let mut to = dir.join(name);
    let mut n = 1;
    while to.exists() {
        n += 1;
        to = dir.join(format!("{stem}-{n}{ext}"));
    }
    if !moving || std::fs::rename(from, &to).is_err() {
        std::fs::copy(from, &to)?;
        if moving {
            let _ = std::fs::remove_file(from);
        }
    }
    let file = to
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(format!("{ATTACHMENTS}/{file}"))
}
