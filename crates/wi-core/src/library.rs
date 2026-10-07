//! The library (docs/SPEC.md 2.5, 2.14; docs/COMMANDS.md library.*):
//! wi-store, with wwav-formats reading each file as it comes in, so a song's
//! verdict is `wwav_pack.py info`'s sentence word for word.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use wi_store::{
    ByExtension, Clip, Colour, Inspection, Inspector, Kind, Pin, SmartRule, Store, TagKind,
};
use wwav_formats::json::{self as pyjson, Value as Py};
use wwav_formats::swav::Swav;
use wwav_formats::wwav::{Wwav, RATE};

use crate::args::Args;
use crate::{history, CoreError, Inner};

/// What 2.5 says of plain audio, and Get Info shows for it.
pub const PLAIN_AUDIO: &str = "Plain audio comes in as master only.";

const AUDIO: [&str; 9] = [
    "wav", "aif", "aiff", "flac", "mp3", "m4a", "aac", "ogg", "opus",
];

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn py_text(v: Option<&Py>) -> Option<String> {
    match v {
        Some(Py::Str(s)) if !s.trim().is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn py_number(v: Option<&Py>) -> Option<f64> {
    match v {
        Some(Py::Int(n)) => n.parse().ok(),
        Some(Py::Float(f)) if f.is_finite() => Some(*f),
        _ => None,
    }
}

/// The formats crate's readers, plugged into wi-store: a `.wwav`'s verdict
/// and its wmet; a film's verdict; a plain WAV's length and verdict, and
/// other plain audio's 2.5 sentence.
pub struct Formats;

impl Inspector for Formats {
    fn inspect(&self, path: &Path) -> wi_store::Result<Inspection> {
        let refused = |e: wwav_formats::Error| {
            wi_store::Error::Refused(format!("Wi_WWAV can't read this file: {e}"))
        };
        match extension(path).as_str() {
            "wwav" => {
                let w = Wwav::open(path).map_err(refused)?;
                let mut found = Inspection::new(Kind::Wwav, &stem_of(path));
                found.verdict = w.verdict().to_string();
                found.duration_ms = (w.master_frames() * 1000 / RATE as u64) as i64;
                if let Some(Py::Dict(m)) = &w.wmet {
                    if let Some(t) = py_text(pyjson::get(m, "title")) {
                        found.title = t;
                    }
                    found.artist = py_text(pyjson::get(m, "artist")).unwrap_or_default();
                    found.bpm = py_number(pyjson::get(m, "bpm"));
                    found.key = py_text(pyjson::get(m, "key"));
                }
                Ok(found)
            }
            ext @ ("swav" | "mp4" | "mov" | "m4v") => {
                let kind = if ext == "swav" {
                    Kind::Swav
                } else {
                    Kind::Video
                };
                let s = match Swav::open(path) {
                    Ok(s) => s,
                    Err(e) if kind == Kind::Swav => return Err(refused(e)),
                    // A camera's file that isn't plain MP4 boxes is still a film.
                    Err(_) => return ByExtension.inspect(path),
                };
                let mut found = Inspection::new(kind, &stem_of(path));
                found.verdict = s.verdict().to_string();
                if let Some(Py::Dict(m)) = &s.wmet {
                    if let Some(t) = py_text(pyjson::get(m, "title")) {
                        found.title = t;
                    }
                    found.artist = py_text(pyjson::get(m, "artist")).unwrap_or_default();
                }
                Ok(found)
            }
            "wav" => {
                let mut found = ByExtension.inspect(path)?;
                if let Ok(w) = Wwav::open(path) {
                    // Get Info says what `wwav_pack.py info` says of the file:
                    // "the master only: no wmet and wlin, so a plain WAV", or
                    // that PRANA lists no 48 kHz or 24-bit one. Other plain
                    // audio (MP3, FLAC…) is beyond the reference and keeps
                    // 2.5's sentence.
                    found.verdict = w.verdict().to_string();
                    if let Some(f) = w.fmt.filter(|f| f.rate > 0) {
                        let frame = (f.channels as u64 * f.bits as u64 / 8).max(1);
                        let frames = w.first(b"data").map_or(0, |c| c.size / frame);
                        found.duration_ms = (frames * 1000 / f.rate as u64) as i64;
                    }
                }
                Ok(found)
            }
            _ => ByExtension.inspect(path),
        }
    }
}

fn published(c: &Clip) -> Value {
    if c.is_up() {
        json!("up")
    } else if c.is_queued() {
        json!("queued")
    } else {
        Value::Null
    }
}

/// `Clip` as docs/COMMANDS.md gives it. `duration` is in seconds and
/// `created` in ms since 1970, from the clip's ULID.
pub(crate) fn clip_json(store: &Store, c: &Clip) -> Result<Value, CoreError> {
    let tags: Vec<String> = store.tags_of(&c.id)?.into_iter().map(|t| t.name).collect();
    let pinned = store
        .pins()?
        .iter()
        .position(|p| p.as_ref() == Some(&Pin::Clip(c.id.clone())));
    Ok(json!({
        "id": c.id,
        "kind": c.kind.as_str(),
        "title": c.title,
        "artist": c.artist,
        "bpm": c.bpm,
        "key": c.key,
        "duration": c.duration_ms as f64 / 1000.0,
        "colour": c.colour.map(Colour::as_str),
        "tags": tags,
        "pinned": pinned,
        "verdict": c.verdict,
        "published": published(c),
        "sha256": c.sha256,
        "bytes": c.bytes,
        "created": wwav_ids::ulid_ms(&c.id),
    }))
}

fn clips_json(store: &Store, clips: &[Clip]) -> Result<Vec<Value>, CoreError> {
    clips.iter().map(|c| clip_json(store, c)).collect()
}

fn rule_from(v: Option<&Value>) -> Result<SmartRule, CoreError> {
    match v {
        None => Ok(SmartRule::default()),
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| {
            CoreError::new(
                "bad_args",
                format!("A filter is tags, colour, bpm_min, bpm_max, key and kind ({e})."),
            )
        }),
    }
}

pub(crate) fn list(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let store = i.store();
    let mut rule = rule_from(a.get("filter"))?;
    if let Some(id) = a.opt_str("smart") {
        let folder = store
            .smart_folders()?
            .into_iter()
            .find(|f| f.id == id)
            .ok_or_else(|| {
                CoreError::new(
                    "refused",
                    "That smart folder isn't in the library any more.",
                )
            })?;
        let mut tags = folder.rule.tags.clone();
        tags.extend(rule.tags);
        rule = SmartRule {
            tags,
            colour: rule.colour.or(folder.rule.colour),
            bpm_min: rule.bpm_min.or(folder.rule.bpm_min),
            bpm_max: rule.bpm_max.or(folder.rule.bpm_max),
            key: rule.key.or(folder.rule.key),
            kind: rule.kind.or(folder.rule.kind),
        };
    }
    let mut clips = store.matching(&rule)?;
    match a.opt_str("sort").unwrap_or("newest") {
        "newest" => {}
        "oldest" => clips.reverse(),
        "title" => clips.sort_by_key(|c| c.title.to_lowercase()),
        "artist" => clips.sort_by_key(|c| (c.artist.to_lowercase(), c.title.to_lowercase())),
        "bpm" => clips.sort_by(|a, b| {
            a.bpm
                .partial_cmp(&b.bpm)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        other => {
            return Err(CoreError::new(
                "bad_args",
                format!(
                    "The library sorts by newest, oldest, title, artist or bpm, not '{other}'."
                ),
            ))
        }
    }
    if let Some(after) = a.opt_str("after") {
        let at = clips
            .iter()
            .position(|c| c.id == after)
            .map_or(clips.len(), |n| n + 1);
        clips.drain(..at);
    }
    let limit = a.opt_usize("limit")?.unwrap_or(200).max(1);
    let next = (clips.len() > limit).then(|| clips[limit - 1].id.clone());
    clips.truncate(limit);
    Ok(json!({"clips": clips_json(&store, &clips)?, "next": next}))
}

/// The palette's filters (2.7): `tag:live-drums`, `key:minor`, `bpm:128` or
/// `bpm:120-130`, `kind:wwav`, `colour:red`; the other words are searched.
fn parse_query(q: &str) -> (String, SmartRule) {
    let mut rule = SmartRule::default();
    let mut words = Vec::new();
    for w in q.split_whitespace() {
        match w.split_once(':') {
            Some(("tag", t)) if !t.is_empty() => rule.tags.push(t.to_lowercase()),
            Some(("key", k)) if !k.is_empty() => rule.key = Some(k.replace('-', " ")),
            Some(("kind", k)) if Kind::parse(k).is_some() => rule.kind = Kind::parse(k),
            Some(("colour" | "color", c)) if Colour::parse(c).is_some() => {
                rule.colour = Colour::parse(c)
            }
            Some(("bpm", b)) => {
                let (lo, hi) = b.split_once('-').unwrap_or((b, b));
                match (lo.parse::<f64>(), hi.parse::<f64>()) {
                    (Ok(lo), Ok(hi)) => {
                        rule.bpm_min = Some(lo.min(hi));
                        rule.bpm_max = Some(lo.max(hi));
                    }
                    _ => words.push(w),
                }
            }
            _ => words.push(w),
        }
    }
    (words.join(" "), rule)
}

pub(crate) fn search(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let (text, rule) = parse_query(a.str("q")?);
    let limit = a.opt_usize("limit")?.unwrap_or(50).max(1);
    let store = i.store();
    let clips = store.search(&text, &rule, limit)?;
    Ok(json!({"clips": clips_json(&store, &clips)?}))
}

/// Get Info: the clip, the reader's verdict, and each chunk (or box) with
/// what `wwav_pack.py info` (or `swav_pack.py info`) says of it.
pub(crate) fn get(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let store = i.store();
    let id = a.str("id")?;
    let clip = store
        .clip(id)?
        .ok_or_else(|| CoreError::new("refused", "That clip isn't in the library any more."))?;
    let path = store.path_of(&clip);
    let chunks = chunks_of(&clip, &path);
    Ok(json!({"clip": clip_json(&store, &clip)?, "verdict": clip.verdict, "chunks": chunks}))
}

fn chunks_of(clip: &Clip, path: &Path) -> Vec<Value> {
    let shown = path.to_string_lossy();
    let (lines, places): (String, Vec<(String, u64, u64)>) = match clip.kind {
        Kind::Wwav | Kind::Audio => match Wwav::open(path) {
            Ok(w) => {
                let mut seen = BTreeSet::new();
                let places = w
                    .chunks
                    .iter()
                    .filter(|c| seen.insert(c.id))
                    .map(|c| (c.id.iter().map(|&b| b as char).collect(), c.at, c.size))
                    .collect();
                (w.info(&shown), places)
            }
            Err(_) => return Vec::new(),
        },
        Kind::Swav | Kind::Video => match Swav::open(path) {
            Ok(s) => {
                let places = s
                    .boxes
                    .iter()
                    .map(|b| (b.kind.iter().map(|&c| c as char).collect(), b.at, b.size))
                    .collect();
                (s.info(&shown), places)
            }
            Err(_) => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    // info's lines after the first, one per chunk, in the same order.
    let details: Vec<&str> = lines.lines().skip(1).collect();
    places
        .into_iter()
        .zip(details)
        .map(|((id, at, size), line)| {
            let detail = line
                .split_once(" bytes")
                .map_or("", |(_, d)| d.trim_start_matches(": "));
            json!({"id": id, "at": at, "size": size, "detail": detail})
        })
        .collect()
}

/// Every file under `paths`, hidden ones left out, in a stable order.
fn walk(paths: &[PathBuf]) -> Result<Vec<PathBuf>, CoreError> {
    let mut out = Vec::new();
    let mut todo: Vec<PathBuf> = paths.iter().rev().cloned().collect();
    while let Some(p) = todo.pop() {
        let meta = std::fs::metadata(&p).map_err(|e| {
            CoreError::new(
                "not_found",
                format!("Wi_WWAV can't open '{}': {e}", p.display()),
            )
        })?;
        if meta.is_dir() {
            let mut kids: Vec<PathBuf> = std::fs::read_dir(&p)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|k| {
                    !k.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                })
                .collect();
            kids.sort();
            todo.extend(kids.into_iter().rev());
        } else if meta.is_file() {
            out.push(p);
        }
    }
    Ok(out)
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// 2.14's sentence: "214 files: 38 .wwav, 12 .swav, 160 plain audio, 4
/// other. Plain audio comes in as master only."
pub fn summary(files: &[PathBuf]) -> String {
    if files.is_empty() {
        return "This folder holds no files.".to_string();
    }
    let (mut wwav, mut swav, mut plain) = (0, 0, 0);
    for f in files {
        match extension(f).as_str() {
            "wwav" => wwav += 1,
            "swav" => swav += 1,
            e if AUDIO.contains(&e) => plain += 1,
            _ => {}
        }
    }
    let other = files.len() - wwav - swav - plain;
    let parts: Vec<String> = [
        (wwav, ".wwav"),
        (swav, ".swav"),
        (plain, "plain audio"),
        (other, "other"),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, what)| format!("{n} {what}"))
    .collect();
    let mut s = format!(
        "{}: {}.",
        plural(files.len(), "file", "files"),
        parts.join(", ")
    );
    if plain > 0 {
        s += " ";
        s += PLAIN_AUDIO;
    }
    s
}

pub(crate) fn inspect(a: &Args) -> Result<Value, CoreError> {
    let files = walk(&[PathBuf::from(a.str("path")?)])?;
    Ok(json!({"summary": summary(&files), "files": files.len()}))
}

fn sha256_file(path: &Path) -> Result<String, CoreError> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(hex::encode(h.finalize()))
}

/// Brings files and folders in as one change. Each file is copied (or left
/// in place) and hashed first, holding the library only one file at a time;
/// then all of them are recorded together under `label`. A file already in
/// the library, byte for byte, is skipped, so pressing again after an
/// interruption picks up where it stopped.
pub(crate) fn import(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let label = a.label()?;
    let paths: Vec<PathBuf> = a.strings("paths")?.into_iter().map(PathBuf::from).collect();
    let files = walk(&paths)?;
    let total = files.len();
    let mut brought = Vec::new();
    let mut skipped = Vec::new();
    for (n, f) in files.iter().enumerate() {
        let size = std::fs::metadata(f)?.len();
        let same_size = i.kv.shas_of_size(size)?;
        let have = !same_size.is_empty() && same_size.contains(&sha256_file(f)?);
        if have {
            skipped.push(json!({"path": f, "why": "It's already in the library."}));
        } else {
            match i.store().bring_in(f, &Formats) {
                Ok(new) => brought.push(new),
                Err(wi_store::Error::Refused(why)) => skipped.push(json!({"path": f, "why": why})),
                Err(e) => return Err(e.into()),
            }
        }
        i.bus.emit(
            "library.import",
            json!({"done": n + 1, "total": total, "file": f}),
        );
    }
    let ids: Vec<String> = brought.iter().map(|c| c.id.clone()).collect();
    if !brought.is_empty() {
        let mut store = i.store();
        let mut tx = store.begin(wi_store::Room::Library, label)?;
        for new in brought {
            tx.add_clip(new)?;
        }
        tx.commit()?;
    }
    let clips = {
        let store = i.store();
        let mut clips = Vec::new();
        for id in &ids {
            if let Some(c) = store.clip(id)? {
                clips.push(clip_json(&store, &c)?);
            }
        }
        clips
    };
    history::changed(i, &ids);
    Ok(json!({"clips": clips, "skipped": skipped}))
}

/// One labelled change over some clips, answered with the clips as they now
/// stand.
fn edit_clips(
    i: &Inner,
    a: &Args,
    f: impl Fn(&mut wi_store::Txn, &str) -> wi_store::Result<()>,
) -> Result<Value, CoreError> {
    let label = a.label()?;
    let ids = a.strings("ids")?;
    {
        let mut store = i.store();
        let mut tx = store.begin(wi_store::Room::Library, label)?;
        for id in &ids {
            f(&mut tx, id)?;
        }
        tx.commit()?;
    }
    history::changed(i, &ids);
    let store = i.store();
    let mut clips = Vec::new();
    for id in &ids {
        if let Some(c) = store.clip(id)? {
            clips.push(clip_json(&store, &c)?);
        }
    }
    Ok(json!({"clips": clips}))
}

pub(crate) fn tag(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let add = a.opt_strings("add")?;
    let remove = a.opt_strings("remove")?;
    edit_clips(i, a, |tx, id| {
        for t in &remove {
            tx.remove_tag(id, t, TagKind::User)?;
        }
        for t in &add {
            tx.add_tag(id, t, TagKind::User)?;
        }
        Ok(())
    })
}

/// Get Info's title: the library's only. The file keeps its name and bytes.
pub(crate) fn rename(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let title = a.str("title")?.trim().to_string();
    if title.is_empty() {
        return Err(CoreError::new("bad_args", "A title needs a word in it."));
    }
    let id = a.str("id")?.to_string();
    edit_one(i, a, &id, |tx| tx.rename_clip(&id, &title))
}

fn edit_one(
    i: &Inner,
    a: &Args,
    id: &str,
    f: impl FnOnce(&mut wi_store::Txn) -> wi_store::Result<()>,
) -> Result<Value, CoreError> {
    let label = a.label()?;
    {
        let mut store = i.store();
        let mut tx = store.begin(wi_store::Room::Library, label)?;
        f(&mut tx)?;
        tx.commit()?;
    }
    history::changed(i, &[id.to_string()]);
    let store = i.store();
    let clip = store
        .clip(id)?
        .ok_or_else(|| CoreError::new("refused", "That clip isn't in the library any more."))?;
    Ok(json!({"clips": [clip_json(&store, &clip)?]}))
}

pub(crate) fn colour(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let colour = match a.opt_str("colour") {
        None => None,
        Some(c) => Some(Colour::parse(c).ok_or_else(|| {
            CoreError::new(
                "bad_args",
                "The colour labels are red, orange, yellow, green, blue and purple.",
            )
        })?),
    };
    edit_clips(i, a, |tx, id| tx.set_colour(id, colour))
}

fn pins_json(store: &Store) -> Result<Value, CoreError> {
    let pins: Vec<Value> = store
        .pins()?
        .iter()
        .map(|p| match p {
            Some(Pin::Clip(id)) => json!({"clip": id}),
            Some(Pin::Folder(id)) => json!({"folder": id}),
            None => Value::Null,
        })
        .collect();
    Ok(json!(pins))
}

/// Pins fill a row of four from the left (2.5): `slot` null unpins; any
/// other slot pins the clip in the first free one, as wi-store places pins.
pub(crate) fn pin(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let label = a.label()?;
    let id = a.str("id")?;
    let pin = Pin::Clip(id.to_string());
    {
        let mut store = i.store();
        let mut tx = store.begin(wi_store::Room::Library, label)?;
        if a.get("slot").is_some() {
            tx.pin(&pin)?;
        } else {
            tx.unpin(&pin)?;
        }
        tx.commit()?;
    }
    history::changed(i, &[id.to_string()]);
    Ok(json!({"pins": pins_json(&i.store())?}))
}

pub(crate) fn smart_save(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let label = a.label()?;
    let name = a.str("name")?;
    let rule = rule_from(a.get("rules"))?;
    let id = {
        let mut store = i.store();
        let mut tx = store.begin(wi_store::Room::Library, label)?;
        // A saved folder changes by being replaced, in the same change.
        if let Some(old) = a.opt_str("id") {
            tx.delete_smart_folder(old)?;
        }
        let id = tx.save_smart_folder(name, &rule)?;
        tx.commit()?;
        id
    };
    history::changed(i, &[]);
    Ok(json!({"folder": {"id": id, "name": name, "rules": rule}}))
}

/// Rows only: the files stay, so undo brings a clip back with its sound.
pub(crate) fn delete(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    edit_clips(i, a, |tx, id| tx.delete_clip(id))?;
    Ok(json!({}))
}

pub(crate) fn cleanup_preview(i: &Inner) -> Result<Value, CoreError> {
    let unused = i.store().unused_media()?;
    Ok(json!({"files": unused.paths, "bytes": unused.bytes, "sentence": unused.sentence()}))
}

pub(crate) fn cleanup_run(i: &Inner) -> Result<Value, CoreError> {
    let store = i.store();
    let moved = store.move_to_trash(&store.unused_media()?)?;
    Ok(json!({"moved": moved.paths, "bytes": moved.bytes}))
}

pub(crate) fn trash_empty(i: &Inner) -> Result<Value, CoreError> {
    let gone = i.store().empty_trash()?;
    Ok(json!({"deleted": gone.paths, "bytes": gone.bytes}))
}
