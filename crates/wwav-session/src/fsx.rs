//! Writing files so a crash leaves the old file or the new one.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// `path` with ".tmp" added: where a new version is written before it
/// takes the name.
pub fn tmp_path(path: &Path) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(".tmp");
    PathBuf::from(name)
}

/// Writes and syncs a file, so its bytes are on disk before anything names
/// it.
pub fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut f = File::create(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// Writes `bytes` to a temporary file, syncs it, renames it over `path`,
/// then syncs the folder so the rename itself survives a power cut.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = tmp_path(path);
    write_synced(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    sync_dir(path.parent().unwrap_or(Path::new(".")))
}

/// Syncs a folder's entries. Windows can't open a folder to sync it, and
/// NTFS journals its renames, so there it does nothing.
pub fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}
