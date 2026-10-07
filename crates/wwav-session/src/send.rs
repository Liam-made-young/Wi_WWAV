//! Send session… (`docs/SPEC.md` 6.5): one `<Title>.wwavsession.zip`,
//! stored rather than compressed (audio doesn't compress), without
//! `cache/`.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::{fsx, Package, Result};

impl Package {
    /// Saves, then writes `<dest_dir>/<Title>.wwavsession.zip` holding the
    /// package as a `<Title>.wwavsession/` folder: everything but `cache/`,
    /// which is rebuilt where it lands. Returns the zip's path.
    pub fn send(&mut self, dest_dir: &Path) -> Result<PathBuf> {
        self.save()?;
        let name = format!("{}.wwavsession", folder_name(&self.session().title));
        let out = dest_dir.join(format!("{name}.zip"));
        let part = dest_dir.join(format!("{name}.zip.part"));
        let mut zip = ZipWriter::new(File::create(&part)?);
        add_folder(&mut zip, self.dir(), &name, true)?;
        zip.finish()?.sync_all()?;
        fs::rename(&part, &out)?;
        fsx::sync_dir(dest_dir)?;
        Ok(out)
    }
}

/// Adds `dir` and everything in it under `name/`, in name order so the same
/// package gives the same zip.
fn add_folder(zip: &mut ZipWriter<File>, dir: &Path, name: &str, top: bool) -> Result<()> {
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.add_directory(format!("{name}/"), stored)?;
    let mut entries = fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if top && file_name == "cache" {
            continue;
        }
        let inner = format!("{name}/{file_name}");
        let meta = entry.metadata()?;
        if meta.is_dir() {
            add_folder(zip, &entry.path(), &inner, false)?;
        } else if meta.is_file() {
            let options = stored
                .large_file(meta.len() >= u32::MAX as u64)
                .last_modified_time(dos_time(meta.modified()?));
            zip.start_file(inner, options)?;
            io::copy(&mut File::open(entry.path())?, zip)?;
        }
    }
    Ok(())
}

/// A file's modified time in the zip's two-second DOS form, in UTC as the
/// zip crate writes its own times.
fn dos_time(t: SystemTime) -> DateTime {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let s = secs % 86_400;
    DateTime::from_date_and_time(
        y as u16,
        m,
        d,
        (s / 3600) as u8,
        (s % 3600 / 60) as u8,
        (s % 60) as u8,
    )
    .unwrap_or_default()
}

/// The proleptic Gregorian date of a day count since 1970-01-01 (Howard
/// Hinnant's `civil_from_days`).
fn civil_from_days(z: i64) -> (i64, u8, u8) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// A title as a file or folder name: `\ / : * ? " < > |` and control
/// characters become spaces, runs of spaces become one, and the ends are
/// trimmed, as PRANA's disc writer does (`prana/web/src/sim/wwavdisc.js`).
/// Unlike the disc, which only shows ASCII, letters in any script are kept.
pub fn folder_name(title: &str) -> String {
    let spaced: String = title
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let name = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        "Untitled".into()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_become_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2026-10-06, the day T0 in the tests falls on.
        assert_eq!(civil_from_days(1_791_321_120 / 86_400), (2026, 10, 6));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn modified_times_go_in_as_utc() {
        let t = UNIX_EPOCH + std::time::Duration::from_secs(1_791_321_120 + 3);
        let dt = dos_time(t);
        assert_eq!(
            (
                dt.year(),
                dt.month(),
                dt.day(),
                dt.hour(),
                dt.minute(),
                dt.second()
            ),
            (2026, 10, 6, 21, 12, 2)
        );
    }
}
