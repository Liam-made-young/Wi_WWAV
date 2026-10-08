//! The capture inbox (docs/NOTES.md, "The capture inbox"): a photo of a
//! notebook page or a scanned PDF, put in a folder, becomes a filed note.
//!
//! For each new file, once it has stopped changing: when it was taken (the
//! photo's own date, else the date in its name, else the file's); its text,
//! read on this Mac (`ocr.rs`); where it goes (the class it was taken in,
//! else, if Claude is allowed, a course, space or note Claude picks from the
//! person's own lists); then the note, in two entries so the filing can be
//! undone by itself; the original moved out of the inbox; and one quiet
//! notice saying what was done.
//!
//! Claude is asked at most once for a page, and only when
//! `Config::capture_claude` and the person's switch both allow it: to read
//! what Vision couldn't, to pick a place when no class did, and to suggest
//! tasks and key terms. What it suggests is kept beside the note and never
//! applied.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use base64::Engine as _;
use serde_json::{json, Value};
use wi_heat::commitments as fixed_rules;
use wi_heat::homes::squash;
use wi_heat::notes::{self as rules, Filing, Page, Place};
use wi_heat_store::notes::{self, Captured};
use wi_heat_store::{commit, kind, Outcome};
use wi_store::Actor;

use crate::args::Args;
use crate::bus::lock;
use crate::claude_cli::{self, Ask};
use crate::commit_cmd::{look_folder, IMAGE_TYPES};
use crate::heat_cmd::{announce, core_error, open_heat, wrote_outside};
use crate::{notes_cmd, ocr, CoreError, Inner};

/// The `heatSetting` that holds the person's switch.
const SETTING: &str = "capture";

/// A file is read once it has been still this long.
const STILL_FOR_MS: i64 = 1500;

/// Reading a page is the larger model's job; filing its text the smallest's.
const READ_MODEL: &str = "sonnet";
const FILE_MODEL: &str = "haiku";
const CLAUDE_TIMEOUT: Duration = Duration::from_secs(240);

/// The largest file the inbox takes.
const FILE_MAX_BYTES: u64 = 80 * 1024 * 1024;

/// What the inbox is doing, for the snapshot.
#[derive(Default)]
pub(crate) struct Capture {
    doing: Mutex<Option<String>>,
    /// Each waiting file's size and time at the last look: one that hasn't
    /// moved since is ready to read.
    seen: Mutex<BTreeMap<PathBuf, (u64, i64)>>,
    /// Files that couldn't be read, as they were then: not tried again
    /// until they change.
    failed: Mutex<BTreeMap<PathBuf, (u64, i64)>>,
    /// Set by `heat.capture.process`: look now, and don't wait for stillness.
    asked: AtomicBool,
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn takes(path: &Path) -> bool {
    let ext = extension(path);
    ext == "pdf" || IMAGE_TYPES.contains(&ext.as_str())
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn stamp(path: &Path) -> Option<(u64, i64)> {
    let m = std::fs::metadata(path).ok()?;
    let mtime = m
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis() as i64;
    Some((m.len(), mtime))
}

/// The folders watched: the ones the app names, and one inside the notes
/// folder that a drop or a paste in the Notes tab goes to.
pub(crate) fn inboxes(i: &Inner) -> Vec<PathBuf> {
    let mut out = i.capture_inboxes.clone();
    if let Some(dir) = &i.notes_dir {
        out.push(dir.join(".inbox"));
    }
    out
}

/// The files waiting in the inboxes, oldest first.
fn waiting(i: &Inner) -> Vec<PathBuf> {
    let mut out: Vec<(i64, PathBuf)> = Vec::new();
    for dir in inboxes(i) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // A dot file is the Finder's, or a file iCloud hasn't brought down yet.
            if name_of(&path).starts_with('.') || !path.is_file() || !takes(&path) {
                continue;
            }
            out.push((stamp(&path).map_or(0, |s| s.1), path));
        }
    }
    out.sort();
    out.into_iter().map(|(_, p)| p).collect()
}

/// Whether the person lets a captured page go to Claude. On until switched off.
fn switch_on(i: &Inner) -> bool {
    wi_heat_store::setting(&i.store(), SETTING)
        .ok()
        .flatten()
        .and_then(|v| v["claude"].as_bool())
        .unwrap_or(true)
}

fn claude_allowed(i: &Inner) -> bool {
    i.capture_claude && switch_on(i) && claude_cli::binary(i).is_ok()
}

/// The inbox's share of the snapshot.
pub(crate) fn status(i: &Inner) -> Value {
    json!({
        "folders": inboxes(i),
        "watching": i.notes_dir.is_some(),
        "waiting": waiting(i).len(),
        "doing": lock(&i.capture.doing).clone(),
        "claude": i.capture_claude && switch_on(i),
        "reader": ocr::state(i),
    })
}

/// Runs one store write and tells the views, as `heat_cmd::write` does, and
/// answers the entry it made too: a notice's Undo names it.
fn write_entry(
    i: &Inner,
    f: impl FnOnce(&mut wi_store::Store, &wi_heat_store::Clock) -> wi_heat_store::Result<Outcome>,
) -> Result<(Value, Option<String>), CoreError> {
    let clock = i.clock();
    let (outcome, docs) = {
        let mut store = i.store();
        let outcome = f(&mut store, &clock).map_err(core_error)?;
        let mut docs = Vec::new();
        if let Some(txn) = &outcome.txn {
            i.note_own(txn);
            docs = store.entry_docs(txn)?.map(|e| e.docs).unwrap_or_default();
        }
        (outcome, docs)
    };
    announce(i, &docs, &outcome.outside)?;
    let txn = outcome.txn.clone();
    Ok((outcome.value(), txn))
}

/// "2026-10-07 10.22.31 IMG_2211.jpg": the time the Shortcut put in a name.
fn time_in_name(name: &str, zone: &jiff::tz::TimeZone) -> Option<f64> {
    let b = name.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let digits = |s: &str| s.bytes().all(|c| c.is_ascii_digit());
    let (date, time) = (name.get(0..10)?, name.get(11..19)?);
    let d: Vec<&str> = date.split('-').collect();
    let t: Vec<&str> = time.split(['.', ':', '-']).collect();
    if d.len() != 3 || t.len() != 3 || !d.iter().chain(&t).all(|p| digits(p)) {
        return None;
    }
    let civil: jiff::civil::DateTime = format!("{date}T{}:{}:{}", t[0], t[1], t[2]).parse().ok()?;
    let at = zone.to_ambiguous_timestamp(civil).compatible().ok()?;
    Some(at.as_millisecond() as f64)
}

/// When a capture was taken: the photo's own date, else the date in its
/// name, else the file's.
fn taken_at(i: &Inner, path: &Path, read: Option<&ocr::Read>) -> f64 {
    let clock = i.clock();
    if let Some(taken) = read.and_then(|r| r.taken.as_deref()) {
        let offset = read.and_then(|r| r.offset.as_deref());
        let at = match offset {
            Some(off) => format!("{taken}{off}").parse::<jiff::Timestamp>().ok(),
            None => taken
                .parse::<jiff::civil::DateTime>()
                .ok()
                .and_then(|c| clock.zone.to_ambiguous_timestamp(c).compatible().ok()),
        };
        if let Some(at) = at {
            return at.as_millisecond() as f64;
        }
    }
    if let Some(at) = time_in_name(&name_of(path), &clock.zone) {
        return at;
    }
    let made = std::fs::metadata(path).ok().and_then(|m| {
        let made = m
            .created()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok());
        let changed = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok());
        // Whichever is earlier: a copy keeps the photo's time as one of them.
        match (made, changed) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    });
    made.map_or(clock.now_ms, |d| d.as_millis() as f64)
}

/// The person's courses, spaces and notes, for Claude to pick from.
fn lists(i: &Inner) -> (Vec<String>, Vec<String>, Vec<String>) {
    let store = i.store();
    let names = |k: &str, field: &str| -> Vec<String> {
        wi_heat_store::all(&store, k)
            .unwrap_or_default()
            .iter()
            .filter_map(|r| {
                r[field]
                    .as_str()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            })
            .collect()
    };
    let courses = wi_heat_store::all(&store, kind::COURSE)
        .unwrap_or_default()
        .iter()
        .filter_map(|c| {
            let code = c["code"].as_str()?;
            Some(
                match c["name"].as_str().filter(|n| *n != code && !n.is_empty()) {
                    Some(name) => format!("{code} ({name})"),
                    None => code.to_string(),
                },
            )
        })
        .collect();
    let mut titles = names(kind::NOTE, "title");
    // A capture is never filed under another capture, and the list stays short.
    titles.retain(|t| !t.starts_with("Capture · "));
    titles.truncate(150);
    (courses, names(kind::SPACE, "name"), titles)
}

/// Asks Claude once about a page: to read it when `images` are given, to
/// place it when `needs_place`, and for tasks and key terms either way.
fn ask_claude(
    i: &Inner,
    id: &str,
    text: Option<&str>,
    images: &[PathBuf],
    needs_place: bool,
) -> Option<Filing> {
    let (courses, spaces, titles) = lists(i);
    let today = i.clock().today();
    let schema = rules::filing_schema(!images.is_empty());
    let answer = if images.is_empty() {
        let prompt =
            rules::filing_prompt(text, &[], &courses, &spaces, &titles, &today, needs_place);
        claude_cli::run_json(
            i,
            &Ask {
                prompt: &prompt,
                allowed_tools: &[],
                json_schema: Some(&schema),
                model: Some(FILE_MODEL),
                timeout: CLAUDE_TIMEOUT,
            },
        )
    } else {
        // The pages, alone in a folder made for this run: all Claude can read.
        let folder = look_folder(i, id).ok()?;
        let mut names = Vec::new();
        for (n, image) in images.iter().enumerate() {
            let name = format!("page-{}.{}", n + 1, extension(image));
            if std::fs::copy(image, folder.join(&name)).is_ok() {
                names.push(name);
            }
        }
        let prompt = rules::filing_prompt(
            None,
            &names,
            &courses,
            &spaces,
            &titles,
            &today,
            needs_place,
        );
        let answer = claude_cli::run_json_looking(
            i,
            &Ask {
                prompt: &prompt,
                allowed_tools: &[],
                json_schema: Some(&schema),
                model: Some(READ_MODEL),
                timeout: CLAUDE_TIMEOUT,
            },
            &folder,
        );
        let _ = std::fs::remove_dir_all(&folder);
        answer
    };
    answer.ok().map(|a| rules::parse_filing(&a))
}

/// A course's code out of what Claude answered: "JPN 101 (Beginning Japanese I)".
fn code_in(name: &str) -> &str {
    name.split(" (").next().unwrap_or(name).trim()
}

/// One file read, made a note, and filed. `from_inbox`: the original is
/// moved into the notes' attachments; otherwise it is copied and left.
pub(crate) fn process(i: &Inner, path: &Path, from_inbox: bool) -> Result<Value, CoreError> {
    notes_cmd::folder(i)?;
    if !path.is_file() || !takes(path) {
        return Err(CoreError::new(
            "refused",
            "A capture is a photo, a screenshot or a PDF.",
        ));
    }
    if std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) > FILE_MAX_BYTES {
        return Err(CoreError::new(
            "refused",
            "That file is too large to capture.",
        ));
    }
    let name = name_of(path);
    *lock(&i.capture.doing) = Some(format!("Reading {name}"));
    wrote_outside(i, &["notice"]);
    let done = process_one(i, path, &name, from_inbox);
    *lock(&i.capture.doing) = None;
    done
}

fn process_one(i: &Inner, path: &Path, name: &str, from_inbox: bool) -> Result<Value, CoreError> {
    let id = wwav_ids::ulid();
    let scratch = i.root.join("cache").join("capture").join(&id);
    let is_pdf = extension(path) == "pdf";
    // 1. Read it here.
    let read = ocr::read(i, path, &scratch).ok();
    let at = taken_at(i, path, read.as_ref());
    let clock = i.clock();
    let (day, minute) = (
        clock.date_of(at),
        wi_heat::model::zone::minute_of_day(at, &clock.zone),
    );

    // 2. The class it was taken in, if there was one.
    let fixed = commit::Fixed::load(&i.store()).map_err(core_error)?;
    let all = fixed.occurrences(&clock, &day, &day);
    let class = fixed_rules::class_at(&all, &day, minute)
        .and_then(|o| o.course_id.clone())
        .and_then(|course| {
            wi_heat_store::one(&i.store(), kind::COURSE, &course)
                .ok()
                .flatten()
                .and_then(|c| c["code"].as_str().map(|code| (course, code.to_string())))
        });

    // 3. Claude, once, when it is allowed and there is something to ask.
    let mut texts: Vec<String> = read
        .as_ref()
        .map(|r| r.pages.iter().map(|p| p.text.clone()).collect())
        .unwrap_or_default();
    let images: Vec<PathBuf> = match &read {
        Some(r) => r.pages.iter().map(|p| p.image.clone()).collect(),
        // No reader: the photo itself is the page.
        None if !is_pdf => vec![path.to_path_buf()],
        None => Vec::new(),
    };
    let poor = match &read {
        Some(r) => {
            r.pages.is_empty()
                || r.pages
                    .iter()
                    .any(|p| rules::poorly_read(&p.text, p.confidence))
        }
        None => true,
    };
    let mut whole_text: Option<String> = None;
    let mut filing = Filing::default();
    if claude_allowed(i) {
        let look = poor
            && !images.is_empty()
            && images
                .iter()
                .all(|p| extension(p) != "heic" && extension(p) != "heif");
        let joined = texts.join("\n\n");
        if look {
            if let Some(f) = ask_claude(i, &id, None, &images, class.is_none()) {
                whole_text = f.text.clone();
                filing = f;
            }
        } else if !joined.trim().is_empty() {
            if let Some(f) = ask_claude(i, &id, Some(&joined), &[], class.is_none()) {
                filing = f;
            }
        }
    }
    if whole_text.is_some() {
        // Claude read the pages as one: its text goes under them all.
        texts.iter_mut().for_each(String::clear);
    }

    // 4. The files: each page's image, and the original, into attachments.
    let slug = match &class {
        Some((_, code)) => squash(code).to_lowercase(),
        None => "capture".to_string(),
    };
    let base = format!(
        "{day}-{:02}{:02}-{slug}",
        (minute / 60.0).floor() as i64,
        (minute % 60.0) as i64
    );
    let mut attachments = Vec::new();
    let mut pages = Vec::new();
    let many = images.len() > 1;
    if let Some(r) = &read {
        for (n, page) in r.pages.iter().enumerate() {
            let file = if many {
                format!("{base}-p{}.jpg", n + 1)
            } else {
                format!("{base}.jpg")
            };
            let rel = notes_cmd::attach(i, &page.image, &file, true)?;
            attachments.push(rel.clone());
            pages.push(Page {
                image: rel,
                text: texts.get(n).cloned().unwrap_or_default(),
            });
        }
    }
    let original = notes_cmd::attach(
        i,
        path,
        &format!(
            "{base}{}.{}",
            if read.is_some() && !is_pdf {
                "-original"
            } else {
                ""
            },
            extension(path)
        ),
        from_inbox,
    )?;
    attachments.push(original.clone());
    if read.is_none() && !is_pdf {
        pages.push(Page {
            image: original.clone(),
            text: String::new(),
        });
    }
    let _ = std::fs::remove_dir_all(&scratch);
    let mut markdown = rules::capture_markdown(&pages, is_pdf.then_some(original.as_str()));
    if let Some(text) = &whole_text {
        markdown.push_str(&format!("\n{}\n", text.trim()));
    }

    // 5. The note: made in the Notes inbox, then filed, as two entries.
    let captured = Captured {
        markdown,
        captured_at: at,
        attachments,
    };
    let (made, made_txn) = write_entry(i, |s, c| notes::capture_add(s, c, &captured))?;
    let note_id = made["note"]["id"].as_str().unwrap_or_default().to_string();
    let mut line = "Captured to the Notes inbox".to_string();
    let mut undo = made_txn;
    let mut title = made["note"]["title"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let place = match (&class, &filing.place) {
        (Some((course, code)), _) => {
            Some(json!({"courseId": course, "title": fixed_rules::capture_title(code, &day)}))
        }
        (None, Place::Course(name)) => Some(
            json!({"course": code_in(name), "title": fixed_rules::capture_title(code_in(name), &day)}),
        ),
        (None, Place::Space(name)) => Some(json!({"space": name})),
        _ => None,
    };
    if let Some(args) = place.as_ref().and_then(Value::as_object) {
        // A place Claude named that isn't there is no place: it stays in the inbox.
        if let Ok((filed, txn)) =
            write_entry(i, |s, c| notes::file(s, c, &note_id, args, Actor::You))
        {
            line = filed["line"].as_str().unwrap_or(&line).to_string();
            title = filed["note"]["title"]
                .as_str()
                .unwrap_or(&title)
                .to_string();
            undo = txn;
        }
    } else if let Place::Note(to) = &filing.place {
        if let Ok((linked, txn)) =
            write_entry(i, |s, c| notes::link(s, c, &note_id, to, Actor::You))
        {
            line = format!(
                "Captured, and {}",
                linked["line"].as_str().unwrap_or("linked").to_lowercase()
            );
            undo = txn;
        }
    }

    // 6. What Claude suggested, kept beside it; and one quiet notice.
    {
        let mut store = i.store();
        let _ = notes::suggestions_set(&mut store, &note_id, &filing.tasks, &filing.terms);
        let mut more = json!({"noteId": note_id});
        if let Some(txn) = &undo {
            more["undo"] = json!({ "txnId": txn });
        }
        let _ = notes::notice_add(
            &mut store,
            &clock,
            &format!("filed:{note_id}"),
            "filed",
            &line,
            more,
        );
    }
    wrote_outside(i, &["notice", "noteSuggestion"]);
    i.bus.status("learn", &format!("{line}."));
    notes_cmd::reconcile(i)?;
    let _ = name;
    Ok(json!({"id": note_id, "title": title, "line": line}))
}

/// Reads what waits in the inboxes and has stopped changing. True if
/// anything was read.
pub(crate) fn work(i: &Inner) -> bool {
    if i.notes_dir.is_none() {
        return false;
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let hurry = i.capture.asked.swap(false, Ordering::Relaxed);
    let mut did = false;
    for path in waiting(i) {
        if i.closing() {
            break;
        }
        let Some(now) = stamp(&path) else {
            continue;
        };
        if lock(&i.capture.failed).get(&path) == Some(&now) {
            continue;
        }
        let before = lock(&i.capture.seen).insert(path.clone(), now);
        // Still being written, or still coming down from iCloud.
        let still = before == Some(now) && now_ms - now.1 >= STILL_FOR_MS;
        if !still && !hurry {
            continue;
        }
        lock(&i.capture.seen).remove(&path);
        match process(i, &path, true) {
            Ok(_) => did = true,
            Err(e) => {
                lock(&i.capture.failed).insert(path.clone(), now);
                let text = format!("Learn couldn't read {}: {}", name_of(&path), e.message);
                let key = format!("capture:{}", name_of(&path));
                let _ = notes::notice_add(
                    &mut i.store(),
                    &i.clock(),
                    &key,
                    "capture",
                    &text,
                    json!({}),
                );
                did = true;
            }
        }
    }
    did
}

pub(crate) fn invoke(i: &Inner, cmd: &str, a: &Args) -> Result<Value, CoreError> {
    match cmd {
        // `{path}`: that one file, now. `{wait: true}`: everything waiting,
        // now. `{}`: the worker is asked to look, and the answer is at once.
        "heat.capture.process" => {
            open_heat(i)?;
            notes_cmd::folder(i)?;
            if let Some(path) = a.opt_str("path") {
                let file = Path::new(path);
                let in_inbox = inboxes(i)
                    .iter()
                    .any(|dir| file.parent() == Some(dir.as_path()));
                let note = process(i, file, in_inbox)?;
                return Ok(json!({"notes": [note]}));
            }
            if a.opt_bool("wait")?.unwrap_or(false) {
                let mut made = Vec::new();
                for path in waiting(i) {
                    made.push(process(i, &path, true)?);
                }
                return Ok(json!({ "notes": made }));
            }
            i.capture.asked.store(true, Ordering::Relaxed);
            Ok(json!({"started": true, "waiting": waiting(i).len()}))
        }
        // Files dropped on the Notes tab, or an image pasted there, go to
        // the inbox inside the notes folder and are read like any other.
        "heat.capture.inbox.add" => {
            open_heat(i)?;
            let dir = notes_cmd::folder(i)?.join(".inbox");
            std::fs::create_dir_all(&dir)?;
            let free = |name: &str| -> PathBuf {
                let mut to = dir.join(name);
                let mut n = 1;
                while to.exists() {
                    n += 1;
                    to = dir.join(format!("{n} {name}"));
                }
                to
            };
            let mut added = 0;
            if let Some(data) = a.opt_str("base64") {
                let name = name_of(Path::new(a.opt_str("name").unwrap_or("pasted.png")));
                if !takes(Path::new(&name)) {
                    return Err(CoreError::new(
                        "refused",
                        "A capture is a photo, a screenshot or a PDF.",
                    ));
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data.trim())
                    .map_err(|_| {
                        CoreError::new(
                            "bad_args",
                            "That image didn't arrive whole. Paste it again.",
                        )
                    })?;
                std::fs::write(free(&name), bytes)?;
                added += 1;
            }
            for path in a.opt_strings("paths")? {
                let from = Path::new(&path);
                if from.is_file() && takes(from) {
                    std::fs::copy(from, free(&name_of(from)))?;
                    added += 1;
                }
            }
            if added == 0 {
                return Err(CoreError::new(
                    "refused",
                    "A capture is a photo, a screenshot or a PDF.",
                ));
            }
            i.capture.asked.store(true, Ordering::Relaxed);
            wrote_outside(i, &["notice"]);
            Ok(json!({ "added": added }))
        }
        "heat.capture.settings.set" => {
            let on = a.opt_bool("claude")?.ok_or_else(|| {
                CoreError::new(
                    "bad_args",
                    "heat.capture.settings.set needs claude, true or false.",
                )
            })?;
            wi_heat_store::set_setting(&mut i.store(), SETTING, &json!({ "claude": on }))
                .map_err(core_error)?;
            wrote_outside(i, &["heatSetting"]);
            Ok(json!({}))
        }
        _ => Err(CoreError::new(
            "unknown_command",
            format!("There is no command called '{cmd}'."),
        )),
    }
}
