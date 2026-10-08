//! Reading text on this Mac (docs/NOTES.md, "Reading on this Mac"): a
//! photographed page or a scanned PDF goes to `wi-ocr`, a small Swift
//! program over Apple's Vision, and comes back as its pages, each with its
//! text, how sure the reading was, and a JPEG of the page. Nothing leaves
//! the machine.
//!
//! The reader is `Config::ocr`, else `WI_WWAV_OCR`, else `wi-ocr` in the
//! app's `Contents/Helpers` or beside its binary, else one the core builds
//! once with `swiftc` into the library's `cache` folder (the source is in
//! this crate). With none of those there is no reader, and a captured page
//! is filed with its image and no text.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::bus::lock;
use crate::{CoreError, Inner};

const SOURCE: &str = include_str!("ocr/wi-ocr.swift");

/// What is known of the reader. It is looked for once.
#[derive(Clone, Debug, Default)]
pub(crate) enum State {
    #[default]
    Unknown,
    Building,
    Ready(PathBuf),
    Missing,
}

#[derive(Default)]
pub(crate) struct Ocr {
    state: Mutex<State>,
}

/// One page as it was read.
#[derive(Clone, Debug)]
pub(crate) struct PageRead {
    pub text: String,
    pub confidence: f64,
    /// The page as a JPEG, upright, in the folder asked for.
    pub image: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Read {
    pub pages: Vec<PageRead>,
    /// The photo's own date, `2026-10-07T10:22:31`, when the file has one.
    pub taken: Option<String>,
    /// Its offset from UTC, `-04:00`, when the camera wrote one.
    pub offset: Option<String>,
}

fn named(i: &Inner) -> Option<PathBuf> {
    i.ocr_path.clone().or_else(|| {
        std::env::var_os("WI_WWAV_OCR")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
    })
}

fn beside_the_app() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [dir.join("../Helpers/wi-ocr"), dir.join("wi-ocr")]
        .into_iter()
        .find(|p| p.is_file())
}

/// Where a reader built from this crate's source is kept: named for the
/// source, so a new version of the app builds its own.
fn built(i: &Inner) -> PathBuf {
    let tag = hex::encode(&Sha256::digest(SOURCE.as_bytes())[..6]);
    i.root.join("cache").join(format!("wi-ocr-{tag}"))
}

/// Builds the reader with the Mac's own Swift compiler. None when there is
/// no compiler (Xcode's command-line tools aren't installed).
fn build(i: &Inner) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    // Asked first: the bare `swiftc` shim would put up an install dialog.
    let tools = Command::new("/usr/bin/xcode-select")
        .arg("-p")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;
    if !tools.success() {
        return None;
    }
    let out = built(i);
    let dir = out.parent()?;
    std::fs::create_dir_all(dir).ok()?;
    let source = dir.join("wi-ocr.swift");
    std::fs::write(&source, SOURCE).ok()?;
    let partial = out.with_extension("building");
    let done = Command::new("/usr/bin/xcrun")
        .args(["swiftc", "-O"])
        .arg(&source)
        .arg("-o")
        .arg(&partial)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;
    let _ = std::fs::remove_file(&source);
    if !done.success() {
        let _ = std::fs::remove_file(&partial);
        return None;
    }
    std::fs::rename(&partial, &out).ok()?;
    Some(out)
}

/// The reader, found or built. The first call may take a few seconds.
pub(crate) fn reader(i: &Inner) -> Option<PathBuf> {
    match lock(&i.ocr.state).clone() {
        State::Ready(path) => return Some(path),
        State::Missing => return None,
        State::Building | State::Unknown => {}
    }
    let found = named(i)
        .filter(|p| p.is_file())
        .or_else(beside_the_app)
        .or_else(|| Some(built(i)).filter(|p| p.is_file()));
    let found = match found {
        Some(path) => Some(path),
        None if i.ocr_build && named(i).is_none() => {
            *lock(&i.ocr.state) = State::Building;
            crate::heat_cmd::wrote_outside(i, &["heatSetting"]);
            build(i)
        }
        None => None,
    };
    *lock(&i.ocr.state) = match &found {
        Some(path) => State::Ready(path.clone()),
        None => State::Missing,
    };
    found
}

/// `ready`, `building` or `missing`, for the guide. Before anything has
/// been read it says what a look without building finds.
pub(crate) fn state(i: &Inner) -> &'static str {
    match &*lock(&i.ocr.state) {
        State::Ready(_) => "ready",
        State::Building => "building",
        State::Missing => "missing",
        State::Unknown => {
            let there = named(i).is_some_and(|p| p.is_file())
                || beside_the_app().is_some()
                || built(i).is_file();
            if there || (i.ocr_build && cfg!(target_os = "macos")) {
                "ready"
            } else {
                "missing"
            }
        }
    }
}

/// Reads `file` (an image or a PDF), writing each page's JPEG into `out`.
pub(crate) fn read(i: &Inner, file: &Path, out: &Path) -> Result<Read, CoreError> {
    let Some(reader) = reader(i) else {
        return Err(CoreError::new(
            "no_reader",
            "This Mac has no text reader yet.",
        ));
    };
    std::fs::create_dir_all(out)?;
    let run = Command::new(&reader)
        .arg(file)
        .arg(out)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| CoreError::new("reader", format!("The text reader wouldn't start: {e}")))?;
    if !run.status.success() {
        let why = String::from_utf8_lossy(&run.stderr);
        let why = why.lines().next().unwrap_or("").trim();
        return Err(CoreError::new(
            "reader",
            if why.is_empty() {
                "Learn couldn't read that file."
            } else {
                why
            },
        ));
    }
    let answer: Value = serde_json::from_slice(&run.stdout).map_err(|_| {
        CoreError::new("reader", "The text reader gave an answer Learn can't read.")
    })?;
    let pages = answer["pages"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|p| {
            Some(PageRead {
                text: p["text"].as_str().unwrap_or("").to_string(),
                confidence: p["confidence"].as_f64().unwrap_or(0.0),
                image: PathBuf::from(p["image"].as_str()?),
            })
        })
        .collect();
    Ok(Read {
        pages,
        taken: answer["taken"].as_str().map(String::from),
        offset: answer["offset"].as_str().map(String::from),
    })
}
