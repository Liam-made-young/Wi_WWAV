//! Reading a `.wwav` as `wwav_pack.py`'s `Wwav` class reads one: the chunk
//! walk, the verdict (what PRANA makes of the file, in the tool's words)
//! and `info`'s printout, then the master and the stems in runs of frames.
//!
//! The rules, as the tool has them:
//! - Chunks are walked from byte 12. A chunk that runs past the end of the
//!   file is what's there of it, and the walk ends with it.
//! - The first chunk of each id is the one read; of the fmt chunks, the
//!   last one (wherever it is) says what the master is.
//! - A fmt of WAVE_FORMAT_EXTENSIBLE (0xFFFE) of 26 bytes or more takes
//!   its format from the subformat.
//! - wmet, wlin and wrmx are read with python's json.loads.

use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use crate::json::{self, Value};
use crate::{msg, Error};

/// The stems, in wstm's frame order: PRANA's and Wi's fader order.
pub const STEMS: [&str; 4] = ["vocals", "drums", "other", "bass"];
pub const RATE: u32 = 44_100;
/// Bytes in a stereo 16-bit frame of the master.
pub const FRAME: u64 = 4;
/// Bytes in a frame of the four stems.
pub const STEM_FRAME: u64 = 16;
/// The stem audio starts at a file offset divisible by this.
pub const ALIGN: u64 = 512;
pub const WSTM_HEADER: u64 = 16;

/// A fmt chunk's format, channels, rate and bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fmt {
    pub format: u16,
    pub channels: u16,
    pub rate: u32,
    pub bits: u16,
}

impl Fmt {
    /// 44.1 kHz, 16-bit, stereo PCM: the only master a `.wwav` has.
    pub const CD: Fmt = Fmt {
        format: 1,
        channels: 2,
        rate: RATE,
        bits: 16,
    };

    /// What the fmt chunk's first 40 bytes say, or None under 16 bytes.
    /// `extensible`: the smallest fmt whose subformat is read, 26 bytes in
    /// wwav_pack.py and Wi, 40 on the device (Wi's wrapWav asks for both).
    pub fn parse(body: &[u8], extensible: usize) -> Option<Fmt> {
        if body.len() < 16 {
            return None;
        }
        let u16_at = |i: usize| u16::from_le_bytes([body[i], body[i + 1]]);
        let mut format = u16_at(0);
        if format == 0xFFFE && body.len() >= extensible {
            format = u16_at(24);
        }
        Some(Fmt {
            format,
            channels: u16_at(2),
            rate: u32::from_le_bytes([body[4], body[5], body[6], body[7]]),
            bits: u16_at(14),
        })
    }
}

/// A chunk: its id, and where its payload is (`at`, `size`). A chunk cut
/// off by the end of the file has the size of what's there and `cut` set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub id: [u8; 4],
    pub at: u64,
    pub size: u64,
    pub cut: bool,
}

/// The chunks after a RIFF header, as `wwav_pack.py chunks()` yields them.
pub fn chunks(f: &mut (impl Read + Seek), size: u64) -> io::Result<Vec<Chunk>> {
    let mut out = Vec::new();
    let mut at = 12;
    while at + 8 <= size {
        let mut h = [0; 8];
        f.seek(SeekFrom::Start(at))?;
        f.read_exact(&mut h)?;
        let n = u32::from_le_bytes([h[4], h[5], h[6], h[7]]) as u64;
        let id = [h[0], h[1], h[2], h[3]];
        if at + 8 + n > size {
            out.push(Chunk {
                id,
                at: at + 8,
                size: size - at - 8,
                cut: true,
            });
            break;
        }
        out.push(Chunk {
            id,
            at: at + 8,
            size: n,
            cut: false,
        });
        at += 8 + n + (n & 1);
    }
    Ok(out)
}

pub(crate) fn read_at(f: &mut (impl Read + Seek), at: u64, n: u64) -> io::Result<Vec<u8>> {
    f.seek(SeekFrom::Start(at))?;
    let mut buf = Vec::new();
    f.take(n).read_to_end(&mut buf)?;
    Ok(buf)
}

/// A wstm header, and where its audio is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wstm {
    pub version: u16,
    pub stems: u8,
    pub channels: u8,
    pub bits: u8,
    pub pad: u16,
    pub rate: u32,
    pub frames: u32,
    /// The file offset of the first stem frame.
    pub audio: u64,
    /// The chunk holds every frame the header counts.
    pub fits: bool,
}

/// What PRANA makes of a file (`Library::wwavDone`), as the tool says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// "not listed: the master isn't 44.1 kHz 16-bit stereo PCM"
    NotListed,
    /// "the master only: " and the reason.
    MasterOnly(String),
    /// "4 stems, and the master"
    Stems,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::NotListed => {
                f.write_str("not listed: the master isn't 44.1 kHz 16-bit stereo PCM")
            }
            Verdict::MasterOnly(why) => write!(f, "the master only: {why}"),
            Verdict::Stems => f.write_str("4 stems, and the master"),
        }
    }
}

/// An open `.wwav` (or any RIFF WAVE).
pub struct Wwav {
    file: File,
    /// The file's size in bytes.
    pub size: u64,
    /// The size the RIFF header gives.
    pub riff: u32,
    /// Every chunk, in file order.
    pub chunks: Vec<Chunk>,
    /// The last fmt chunk's, if it had 16 bytes.
    pub fmt: Option<Fmt>,
    /// The first wmet, wlin and wrmx as JSON; None when missing, not UTF-8,
    /// not JSON, or JSON's null (which the tool can't tell from missing).
    pub wmet: Option<Value>,
    pub wlin: Option<Value>,
    pub wrmx: Option<Value>,
    /// The first wstm's header, if the chunk has one.
    pub wstm: Option<Wstm>,
}

impl Wwav {
    /// Opens and reads a file's chunks; refuses one that isn't a RIFF
    /// WAVE, as the tool does ("x.wav: not a WAV").
    pub fn open(path: &Path) -> Result<Wwav, Error> {
        let shown = path.to_string_lossy();
        let mut file = File::open(path).map_err(|e| msg(format!("{shown}: {e}")))?;
        let size = file.metadata()?.len();
        let head = read_at(&mut file, 0, 12)?;
        if head.len() < 12 || &head[..4] != b"RIFF" || &head[8..] != b"WAVE" {
            return Err(msg(format!("{shown}: not a WAV")));
        }
        let chunks = chunks(&mut file, size)?;
        let first = |id: &[u8; 4]| chunks.iter().find(|c| &c.id == id).copied();
        let mut fmt = None;
        for c in chunks.iter().filter(|c| &c.id == b"fmt ") {
            fmt = Fmt::parse(&read_at(&mut file, c.at, c.size.min(40))?, 26);
        }
        let mut json = |id| -> io::Result<Option<Value>> {
            let Some(c) = first(id) else { return Ok(None) };
            let v = json::loads(&read_at(&mut file, c.at, c.size)?, true);
            Ok(v.filter(|v| *v != Value::Null))
        };
        let (wmet, wlin, wrmx) = (json(b"wmet")?, json(b"wlin")?, json(b"wrmx")?);
        let wstm = match first(b"wstm") {
            Some(c) if c.size >= WSTM_HEADER => {
                let h = read_at(&mut file, c.at, WSTM_HEADER)?;
                let pad = u16::from_le_bytes([h[6], h[7]]);
                let frames = u32::from_le_bytes([h[12], h[13], h[14], h[15]]);
                Some(Wstm {
                    version: u16::from_le_bytes([h[0], h[1]]),
                    stems: h[2],
                    channels: h[3],
                    bits: h[4],
                    pad,
                    rate: u32::from_le_bytes([h[8], h[9], h[10], h[11]]),
                    frames,
                    audio: c.at + WSTM_HEADER + pad as u64,
                    fits: c.size >= WSTM_HEADER + pad as u64 + frames as u64 * STEM_FRAME,
                })
            }
            _ => None,
        };
        let riff = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
        Ok(Wwav {
            file,
            size,
            riff,
            chunks,
            fmt,
            wmet,
            wlin,
            wrmx,
            wstm,
        })
    }

    /// The first chunk with this id: the one the tool reads.
    pub fn first(&self, id: &[u8; 4]) -> Option<&Chunk> {
        self.chunks.iter().find(|c| &c.id == id)
    }

    /// The master's length: the first data chunk's whole frames.
    pub fn master_frames(&self) -> u64 {
        self.first(b"data").map_or(0, |c| c.size / FRAME)
    }

    /// What the device will do with the file (`Wwav.verdict()`).
    pub fn verdict(&self) -> Verdict {
        if self.fmt != Some(Fmt::CD) || self.first(b"data").is_none() {
            return Verdict::NotListed;
        }
        let only = |why: &str| Verdict::MasterOnly(why.into());
        let (Some(Value::Dict(wmet)), Some(Value::Dict(_))) = (&self.wmet, &self.wlin) else {
            return only("no wmet and wlin, so a plain WAV");
        };
        // python's str() of whatever "wwav" holds, then ^\d+ in any script
        let version = json::get(wmet, "wwav")
            .map(json::py_str)
            .unwrap_or_default();
        let major: Vec<u32> = version.chars().map_while(crate::text::decimal).collect();
        if major.is_empty() {
            return only("wmet has no version");
        }
        if major.iter().any(|&d| d > 0) {
            return Verdict::MasterOnly(format!(
                "version {version} is newer than this reader (0.x)"
            ));
        }
        let Some(s) = self.wstm else {
            return only("no wstm");
        };
        if (s.version, s.stems, s.channels, s.bits, s.rate) != (1, 4, 2, 16, RATE) {
            return only("wstm isn't v1, 4 stereo 16-bit stems at 44.1 kHz");
        }
        if !s.fits {
            return only("wstm is cut off");
        }
        let frames = self.master_frames();
        let wmet_frames = json::get(wmet, "frames");
        if !(s.frames as u64 == frames
            && wmet_frames.is_some_and(|v| json::py_eq(&Value::Int(frames.to_string()), v)))
        {
            let said = wmet_frames.map_or("None".into(), json::py_str);
            return Verdict::MasterOnly(format!(
                "frame counts differ (data {frames}, wstm {}, wmet {said})",
                s.frames
            ));
        }
        Verdict::Stems
    }

    /// The stems' header, when the verdict is "4 stems, and the master".
    pub fn stems(&self) -> Option<Wstm> {
        (self.verdict() == Verdict::Stems)
            .then_some(self.wstm)
            .flatten()
    }

    /// `wwav_pack.py info`'s printout, `shown` being the path as given.
    pub fn info(&self, shown: &str) -> String {
        let riff = if self.riff as u64 + 8 == self.size {
            "ok".to_string()
        } else {
            format!("says {}, file is {}", self.riff, self.size as i64 - 8)
        };
        let mut out = format!("{shown}: {} bytes (RIFF size {riff})\n", self.size);
        let mut seen: Vec<[u8; 4]> = Vec::new();
        for c in &self.chunks {
            if seen.contains(&c.id) {
                continue;
            }
            seen.push(c.id);
            let name: String = c.id.iter().map(|&b| b as char).collect(); // latin-1
            out += &format!("  {name}  at {}, {} bytes", c.at, c.size);
            match &c.id {
                b"fmt " => {
                    if let Some(f) = self.fmt {
                        out += &format!(
                            ": format {}, {} ch, {} Hz, {}-bit",
                            f.format, f.channels, f.rate, f.bits
                        );
                    }
                }
                b"data" => {
                    out += &format!(
                        ": the master, {} frames ({:.2} s)",
                        c.size / FRAME,
                        c.size as f64 / FRAME as f64 / RATE as f64
                    );
                }
                b"wstm" => {
                    if let Some(s) = self.wstm {
                        let aligned = if s.audio % ALIGN == 0 {
                            " (512-aligned)"
                        } else {
                            ""
                        };
                        out += &format!(
                            ": v{}, {} stems, {} ch, {}-bit, {} Hz, {} frames, audio at {}{aligned}",
                            s.version, s.stems, s.channels, s.bits, s.rate, s.frames, s.audio
                        );
                    }
                }
                b"wmet" | b"wlin" | b"wrmx" => {
                    let j = match &c.id {
                        b"wmet" => &self.wmet,
                        b"wlin" => &self.wlin,
                        _ => &self.wrmx,
                    };
                    out += ": ";
                    out += &j.as_ref().map_or("not JSON".into(), json::dumps);
                }
                _ => {}
            }
            out.push('\n');
        }
        out + &format!("  on PRANA: {}\n", self.verdict())
    }

    /// Master frames from `start` into `out` (L R interleaved); returns how
    /// many frames it read, fewer at the end of the master.
    pub fn read_master(&mut self, start: u64, out: &mut [i16]) -> io::Result<usize> {
        let (at, frames) = self
            .first(b"data")
            .map_or((0, 0), |c| (c.at, c.size / FRAME));
        self.read_frames(at, frames, FRAME, start, out)
    }

    /// Stem frames from `start` into `out`, eight samples a frame (VOC_L
    /// VOC_R DRM_L DRM_R OTH_L OTH_R BAS_L BAS_R); returns how many frames
    /// it read, fewer at the end. This reads whatever a whole wstm chunk
    /// holds: check the verdict (or use [`Wwav::stems`]) before playing them.
    pub fn read_stems(&mut self, start: u64, out: &mut [i16]) -> io::Result<usize> {
        let (at, frames) = match self.wstm {
            Some(s) if s.fits => (s.audio, s.frames as u64),
            _ => (0, 0),
        };
        self.read_frames(at, frames, STEM_FRAME, start, out)
    }

    fn read_frames(
        &mut self,
        at: u64,
        frames: u64,
        frame: u64,
        start: u64,
        out: &mut [i16],
    ) -> io::Result<usize> {
        let per = frame as usize / 2;
        let n = (out.len() / per).min(frames.saturating_sub(start) as usize);
        let mut bytes = vec![0u8; n * frame as usize];
        self.file.seek(SeekFrom::Start(at + start * frame))?;
        self.file.read_exact(&mut bytes)?;
        for (o, b) in out.iter_mut().zip(bytes.chunks_exact(2)) {
            *o = i16::from_le_bytes([b[0], b[1]]);
        }
        Ok(n)
    }
}
