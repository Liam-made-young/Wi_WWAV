//! `.wwav` and `.swav` 0.1 (`docs/SPEC.md` 6.1–6.4), written and read
//! byte for byte as the references do: `prana/tools/wwav_pack.py` for
//! songs and `formats/swav/swav_pack.py` for films, with Wi's `wrapWav`
//! (`wi/src/formats/wwav.js`) for wrapping a plain WAV.
//!
//! - [`wwav`] reads a `.wwav`: its chunks, the verdict (what PRANA makes of
//!   the file, in the reference's words), `info`, and the master and stems
//!   in runs of frames.
//! - [`writer`] writes one: [`writer::WwavWriter`] takes the master and the
//!   four stems in blocks, as the Console's export renders them, and
//!   writes originals, splits and remixes; [`writer::master_only`] and
//!   [`writer::wrap_wav`] write the master-only kinds.
//! - [`pack`] is `wwav_pack.py pack` and `unpack`: a song folder to a
//!   `.wwav` and back.
//! - [`swav`] reads, packs and unpacks a `.swav`.
//! - [`meta`] holds what goes in `wmet`, `wlin` and `wrmx`, written in the
//!   references' exact text.
//! - [`json`] and [`text`] are the python behaviour the bytes and sentences
//!   depend on (`json.loads`, `json.dumps`, `str()`, `str.strip`, `float()`).
//!
//! The `wwav` binary is the command line: `wwav pack|info|unpack` and
//! `wwav swav pack|info|unpack`, with the Python tools' arguments and
//! output.

pub mod json;
pub mod meta;
pub mod pack;
pub mod swav;
pub mod text;
pub mod writer;
pub mod wwav;

use std::fmt;
use std::io;
use std::path::Path;

/// What went wrong, in words fit to show.
#[derive(Debug)]
pub enum Error {
    /// Reading or writing failed part way.
    Io(io::Error),
    /// A refusal, in the reference's words ("01 Song: missing bass.wav").
    Msg(String),
    /// More than one RIFF file holds (4 GB), from the song's length in
    /// frames at 44.1 kHz. Shown as the export sheet says it (5.13, 6.7).
    TooLong { frames: u64 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => e.fmt(f),
            Error::Msg(m) => f.write_str(m),
            Error::TooLong { frames } => {
                let minutes = frames.div_ceil(44_100 * 60);
                write!(
                    f,
                    "A .wwav holds about 81 minutes. This session is {minutes}."
                )
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

fn msg(m: impl Into<String>) -> Error {
    Error::Msg(m.into())
}

/// swav_pack.py's refusal to write over its own input, under any name for
/// it (`os.path.samefile`: the same device and inode, so a hard link too).
/// The writers that read one file to write another refuse it too.
pub(crate) fn not_same(src: &Path, out: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    let same = |a: &Path, b: &Path| -> io::Result<bool> {
        use std::os::unix::fs::MetadataExt;
        let (a, b) = (std::fs::metadata(a)?, std::fs::metadata(b)?);
        Ok((a.dev(), a.ino()) == (b.dev(), b.ino()))
    };
    // std has no stable file identity off Unix: the same path will do
    #[cfg(not(unix))]
    let same = |a: &Path, b: &Path| -> io::Result<bool> {
        Ok(std::fs::canonicalize(a)? == std::fs::canonicalize(b)?)
    };
    if out.exists() && same(src, out)? {
        return Err(msg(format!(
            "{}: would write over {}",
            out.display(),
            src.display()
        )));
    }
    Ok(())
}
