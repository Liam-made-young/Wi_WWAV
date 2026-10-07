//! `media/`: every file a clip points at, ULID-named. "Media costs nothing
//! twice" (`docs/SPEC.md` 6.5): a file comes in as a clone where the
//! filesystem can make one (APFS, btrfs, XFS, ReFS), else as a hard link,
//! which is safe because media is never written after import, else as a
//! copy, and the app is told which so its sheet can say "Copying 1.4 GB of
//! media into the session."

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use crate::{fsx, Package, Result};

/// How a file came into `media/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    /// A copy-on-write clone: no space used until one side changes.
    Clone,
    /// A second name for the same file.
    HardLink,
    /// The bytes copied, on a filesystem that can do neither (exFAT, or
    /// another volume).
    Copy,
    /// The file was already in `media/`.
    Present,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    /// The ULID an event's `clip_id` names it by.
    pub id: String,
    /// `media/<ULID>.<ext>`, relative to the package.
    pub file: String,
    pub link: Link,
    pub bytes: u64,
}

impl Package {
    /// Brings a library media file into `media/` the cheapest correct way.
    /// A file already named by a ULID (as the library names its media) keeps
    /// it, so the session and the library name a song the same way; any
    /// other file gets a new one.
    pub fn import_media(&self, src: &Path) -> Result<Imported> {
        let stem = src.file_stem().and_then(OsStr::to_str).unwrap_or("");
        let id = if wwav_ids::is_ulid(stem) {
            stem.to_string()
        } else {
            wwav_ids::ulid()
        };
        let file = match src.extension().and_then(OsStr::to_str) {
            Some(ext) if !ext.is_empty() => format!("media/{id}.{}", ext.to_ascii_lowercase()),
            _ => format!("media/{id}"),
        };
        let bytes = fs::metadata(src)?.len();
        let dst = self.dir().join(&file);
        let link = if dst.exists() {
            Link::Present
        } else {
            let link = link(src, &dst)?;
            fsx::sync_dir(&self.dir().join("media"))?;
            link
        };
        Ok(Imported {
            id,
            file,
            link,
            bytes,
        })
    }

    /// Where the media file with this ULID is, if the package has it.
    pub fn media_path(&self, id: &str) -> Option<PathBuf> {
        fs::read_dir(self.dir().join("media"))
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| p.file_stem().and_then(OsStr::to_str) == Some(id))
    }
}

fn link(src: &Path, dst: &Path) -> io::Result<Link> {
    if reflink_copy::reflink(src, dst).is_ok() {
        return Ok(Link::Clone);
    }
    if fs::hard_link(src, dst).is_ok() {
        return Ok(Link::HardLink);
    }
    // A copy takes time, so it is made under another name: a crash never
    // leaves half a file under the media's name.
    let tmp = fsx::tmp_path(dst);
    fs::copy(src, &tmp)?;
    File::open(&tmp)?.sync_all()?;
    fs::rename(&tmp, dst)?;
    Ok(Link::Copy)
}

/// "Copying 1.4 GB of media into the session."
pub fn copying_sentence(bytes: u64) -> String {
    format!("Copying {} of media into the session.", size(bytes))
}

/// Sizes as Finder gives them, in powers of ten.
fn size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{} MB", (b / 1e6).round())
    } else {
        format!("{} KB", (b / 1e3).round().max(1.0))
    }
}
