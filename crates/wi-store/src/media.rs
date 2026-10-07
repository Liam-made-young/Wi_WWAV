//! Media is reclaimed only on a press (docs/SPEC.md 9.6). Deleting removes
//! rows, never files. **Clean up media…** lists files that no row and no
//! journal entry names and moves them to `trash/`; emptying the trash deletes
//! them. So undo can always bring a deleted clip back with its sound.
//!
//! And the nightly backup: `VACUUM INTO` a dated copy, keeping seven.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::{refused, Result, Store};

/// Some files in the library, as paths relative to its folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Files {
    pub paths: Vec<String>,
    pub bytes: u64,
}

impl Files {
    /// "1.8 GB in 214 files", or "Nothing to clean up."
    pub fn sentence(&self) -> String {
        if self.paths.is_empty() {
            return "Nothing to clean up.".to_string();
        }
        let files = match self.paths.len() {
            1 => "1 file".to_string(),
            n => format!("{n} files"),
        };
        format!("{} in {files}", size(self.bytes))
    }
}

/// Bytes as Finder counts them, in thousands: "1.8 GB", "52.9 MB", "210 MB".
fn size(bytes: u64) -> String {
    let units = [("GB", 1e9), ("MB", 1e6), ("KB", 1e3)];
    for (unit, scale) in units {
        let n = bytes as f64 / scale;
        if n >= 1.0 {
            return if n < 100.0 {
                format!("{n:.1} {unit}")
            } else {
                format!("{n:.0} {unit}")
            };
        }
    }
    format!("{bytes} bytes")
}

fn size_on_disk(path: &Path) -> Result<u64> {
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_dir() {
        return Ok(meta.len());
    }
    let mut total = 0;
    for entry in std::fs::read_dir(path)? {
        total += size_on_disk(&entry?.path())?;
    }
    Ok(total)
}

fn remove(path: &Path) -> Result<()> {
    if std::fs::symlink_metadata(path)?.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Folders whose entries rows name: media files, and session packages.
const NAMED_FOLDERS: [&str; 2] = ["media", "sessions"];

/// A backup's name: `library-YYYY-MM-DD.sqlite`.
fn backup_name(date: &str) -> Option<String> {
    let b = date.as_bytes();
    let dated = b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        });
    dated.then(|| format!("library-{date}.sqlite"))
}

const BACKUPS_KEPT: usize = 7;

impl Store {
    /// Every path a row or a journal entry names, relative to the library.
    fn named(&self) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT file FROM clips
             UNION SELECT package FROM sequences
             UNION SELECT json_extract(before, '$.file') FROM txn_row WHERE tbl = 'clips'
             UNION SELECT json_extract(after, '$.file') FROM txn_row WHERE tbl = 'clips'
             UNION SELECT json_extract(before, '$.package') FROM txn_row WHERE tbl = 'sequences'
             UNION SELECT json_extract(after, '$.package') FROM txn_row WHERE tbl = 'sequences'",
        )?;
        let names = stmt.query_map([], |r| r.get::<_, Option<String>>(0))?;
        Ok(names
            .filter_map(|n| n.transpose())
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Files in `media/` and packages in `sessions/` that nothing names.
    pub fn unused_media(&self) -> Result<Files> {
        let named = self.named()?;
        let mut unused = Files::default();
        for folder in NAMED_FOLDERS {
            for entry in std::fs::read_dir(self.root.join(folder))? {
                let entry = entry?;
                let rel = format!("{folder}/{}", entry.file_name().to_string_lossy());
                if !named.contains(&rel) {
                    unused.bytes += size_on_disk(&entry.path())?;
                    unused.paths.push(rel);
                }
            }
        }
        unused.paths.sort();
        Ok(unused)
    }

    /// Moves the listed files to `trash/`, skipping any that something names
    /// again since they were listed. Returns what moved.
    pub fn move_to_trash(&self, unused: &Files) -> Result<Files> {
        let named = self.named()?;
        let trash = self.root.join("trash");
        let mut moved = Files::default();
        for rel in &unused.paths {
            let path = Path::new(rel);
            let in_folder = NAMED_FOLDERS
                .iter()
                .any(|f| path.parent() == Some(Path::new(f)));
            let from = self.root.join(path);
            if !in_folder || named.contains(rel) || !from.exists() {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let mut to = trash.join(&*name);
            if to.exists() {
                to = trash.join(format!("{}-{name}", wwav_ids::ulid()));
            }
            moved.bytes += size_on_disk(&from)?;
            std::fs::rename(&from, &to)?;
            moved.paths.push(rel.clone());
        }
        Ok(moved)
    }

    /// What is in `trash/`.
    pub fn trash(&self) -> Result<Files> {
        let mut found = Files::default();
        for entry in std::fs::read_dir(self.root.join("trash"))? {
            let entry = entry?;
            found.bytes += size_on_disk(&entry.path())?;
            found
                .paths
                .push(format!("trash/{}", entry.file_name().to_string_lossy()));
        }
        found.paths.sort();
        Ok(found)
    }

    /// Deletes everything in `trash/`, for good. Returns what was deleted.
    pub fn empty_trash(&self) -> Result<Files> {
        let emptied = self.trash()?;
        for rel in &emptied.paths {
            remove(&self.root.join(rel))?;
        }
        Ok(emptied)
    }

    /// Writes `library-<date>.sqlite` into `dir` with `VACUUM INTO`, replacing
    /// one from the same day, and keeps only the seven newest. When it runs
    /// is the app's to decide.
    pub fn backup(&self, dir: &Path, date: &str) -> Result<PathBuf> {
        let Some(name) = backup_name(date) else {
            return refused("A backup is named by its date, as 2026-10-07.");
        };
        std::fs::create_dir_all(dir)?;
        let part = dir.join(format!(".{name}.part"));
        if part.exists() {
            std::fs::remove_file(&part)?;
        }
        self.conn
            .execute("VACUUM INTO ?1", [part.to_string_lossy()])?;
        let target = dir.join(&name);
        std::fs::rename(&part, &target)?;

        let mut dated: Vec<String> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| {
                n.strip_prefix("library-")
                    .and_then(|d| d.strip_suffix(".sqlite"))
                    .and_then(backup_name)
                    .is_some()
            })
            .collect();
        dated.sort();
        let surplus = dated.len().saturating_sub(BACKUPS_KEPT);
        for old in &dated[..surplus] {
            std::fs::remove_file(dir.join(old))?;
        }
        Ok(target)
    }
}
