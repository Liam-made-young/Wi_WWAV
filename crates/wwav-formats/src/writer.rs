//! Writing a `.wwav`.
//!
//! [`WwavWriter`] writes a song with its stems (an original, a split or a
//! remix) from the master and the four stems handed over in blocks, in any
//! sizes and either order, as the Console's export renders them: the
//! layout is fixed once the length is known, so each block goes straight to
//! its place in the file. It writes the same bytes as `wwav_pack.py pack`
//! for the same audio and metadata:
//!
//! ```text
//! RIFF WAVE  fmt (44.1 kHz 16-bit stereo PCM)  data (the master)
//!            wmet  wstm (16-byte header, zeros to a 512-byte boundary,
//!            then VOC_L VOC_R DRM_L DRM_R OTH_L OTH_R BAS_L BAS_R frames)
//!            wlin  [wrmx, remixes as PRANA writes them: last]
//! ```
//!
//! [`master_only`] makes the store's ".wwav, master only" from a song, and
//! [`wrap_wav`] makes a plain WAV a master-only `.wwav` as Wi does.

use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::json::{self, Value};
use crate::meta::{Lineage, SongMeta, Wrmx};
use crate::wwav::{read_at, Fmt, Verdict, Wwav, ALIGN, FRAME, RATE, STEM_FRAME, WSTM_HEADER};
use crate::{msg, not_same, Error};

/// A RIFF file's size field is 32 bits: no file is longer.
const MAX_FILE: u64 = u32::MAX as u64;

/// An 8-byte chunk header, the payload, and a pad byte if it's odd.
pub(crate) fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 9);
    out.extend_from_slice(id);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() & 1 == 1 {
        out.push(0);
    }
    out
}

/// `wav_header`: RIFF, a 16-byte PCM fmt and the data chunk's header for
/// `frames` stereo 16-bit frames, with this RIFF size.
pub(crate) fn wav_header(frames: u64, riff: u64) -> Vec<u8> {
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(riff as u32).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&2u16.to_le_bytes());
    h.extend_from_slice(&RATE.to_le_bytes());
    h.extend_from_slice(&(RATE * FRAME as u32).to_le_bytes());
    h.extend_from_slice(&(FRAME as u16).to_le_bytes());
    h.extend_from_slice(&16u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&((frames * FRAME) as u32).to_le_bytes());
    h
}

/// Where everything goes for a song of `frames` frames.
struct Layout {
    /// wmet, then wstm's header and the zeros after it: right after the master.
    middle: Vec<u8>,
    stems_at: u64,
    /// wlin, and wrmx for a remix: right after the stems.
    tail: Vec<u8>,
    total: u64,
}

fn layout(frames: u64, meta: &SongMeta, lineage: &Lineage, wrmx: Option<&Wrmx>) -> Layout {
    let wmet_at = 44 + frames * FRAME;
    let mut middle = chunk(b"wmet", meta.wmet(frames).as_bytes());
    let payload = wmet_at + middle.len() as u64 + 8;
    let pad = (ALIGN - (payload + WSTM_HEADER) % ALIGN) % ALIGN;
    let wstm_size = WSTM_HEADER + pad + frames * STEM_FRAME;
    middle.extend_from_slice(b"wstm");
    middle.extend_from_slice(&(wstm_size as u32).to_le_bytes());
    // <HBBBBHII: version 1, 4 stems, 2 channels, 16 bits, 0, pad, rate, frames
    middle.extend_from_slice(&1u16.to_le_bytes());
    middle.extend_from_slice(&[4, 2, 16, 0]);
    middle.extend_from_slice(&(pad as u16).to_le_bytes());
    middle.extend_from_slice(&RATE.to_le_bytes());
    middle.extend_from_slice(&(frames as u32).to_le_bytes());
    middle.resize(middle.len() + pad as usize, 0);
    let mut tail = chunk(b"wlin", lineage.wlin().as_bytes());
    if let Some(w) = wrmx {
        tail.extend(chunk(b"wrmx", w.json().as_bytes()));
    }
    let total = payload + wstm_size + tail.len() as u64;
    Layout {
        middle,
        stems_at: payload + WSTM_HEADER + pad,
        tail,
        total,
    }
}

/// Writes a `.wwav` with stems from blocks of audio.
///
/// ```no_run
/// # use wwav_formats::{meta::{Lineage, SongMeta}, writer::WwavWriter};
/// # fn render(_: usize) -> (Vec<i16>, [Vec<i16>; 4]) { unimplemented!() }
/// # let (meta, frames) = (SongMeta::default(), 44_100 * 60);
/// let lineage = Lineage::original(&meta.song_id, "liam_made_young");
/// let mut w = WwavWriter::create("Low Tide.wwav".as_ref(), frames, &meta, &lineage, None)?;
/// for block in 0..(frames as usize).div_ceil(65_536) {
///     let (master, [v, d, o, b]) = render(block); // L R interleaved, at 44.1 kHz
///     w.write_master(&master)?;
///     w.write_stems([&v, &d, &o, &b])?;
/// }
/// w.finish()?;
/// # Ok::<(), wwav_formats::Error>(())
/// ```
pub struct WwavWriter {
    file: BufWriter<File>,
    /// Where the file's cursor is, so writing on from there needs no seek.
    pos: u64,
    frames: u64,
    stems_at: u64,
    tail: Vec<u8>,
    total: u64,
    master_done: u64,
    stems_done: u64,
}

impl WwavWriter {
    /// The file's size for a song of `frames` frames, before writing it.
    pub fn size(frames: u64, meta: &SongMeta, lineage: &Lineage, wrmx: Option<&Wrmx>) -> u64 {
        layout(frames, meta, lineage, wrmx).total
    }

    /// Starts `path` for a song of `frames` frames at 44.1 kHz. `wrmx` makes
    /// it a remix "as settings" (6.8); a baked remix has none. Refuses a
    /// song that won't fit in one RIFF file (4 GB, about 81 minutes) before
    /// creating anything.
    pub fn create(
        path: &Path,
        frames: u64,
        meta: &SongMeta,
        lineage: &Lineage,
        wrmx: Option<&Wrmx>,
    ) -> Result<WwavWriter, Error> {
        let l = layout(frames, meta, lineage, wrmx);
        if l.total > MAX_FILE {
            return Err(Error::TooLong { frames });
        }
        let mut w = WwavWriter {
            file: BufWriter::with_capacity(1 << 20, File::create(path)?),
            pos: 0,
            frames,
            stems_at: l.stems_at,
            tail: l.tail,
            total: l.total,
            master_done: 0,
            stems_done: 0,
        };
        w.put(0, &wav_header(frames, l.total - 8))?;
        w.put(44 + frames * FRAME, &l.middle)?;
        Ok(w)
    }

    fn put(&mut self, at: u64, bytes: &[u8]) -> io::Result<()> {
        if at != self.pos {
            self.file.seek(SeekFrom::Start(at))?;
        }
        self.file.write_all(bytes)?;
        self.pos = at + bytes.len() as u64;
        Ok(())
    }

    /// The next frames of the master, L R interleaved.
    pub fn write_master(&mut self, samples: &[i16]) -> Result<(), Error> {
        let n = self.count(samples, "the master", self.master_done)?;
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        self.put(44 + self.master_done * FRAME, &bytes)?;
        self.master_done += n;
        Ok(())
    }

    /// The next frames of the four stems, in wstm's order (vocals, drums,
    /// other, bass), each L R interleaved and all the same length.
    pub fn write_stems(&mut self, stems: [&[i16]; 4]) -> Result<(), Error> {
        if stems.iter().any(|s| s.len() != stems[0].len()) {
            return Err(msg("the four stems' blocks aren't the same length"));
        }
        let n = self.count(stems[0], "the stems", self.stems_done)?;
        let mut bytes = Vec::with_capacity(n as usize * STEM_FRAME as usize);
        for f in 0..n as usize {
            for s in stems {
                bytes.extend_from_slice(&s[f * 2].to_le_bytes());
                bytes.extend_from_slice(&s[f * 2 + 1].to_le_bytes());
            }
        }
        self.put(self.stems_at + self.stems_done * STEM_FRAME, &bytes)?;
        self.stems_done += n;
        Ok(())
    }

    fn count(&self, samples: &[i16], what: &str, done: u64) -> Result<u64, Error> {
        if samples.len() % 2 == 1 {
            return Err(msg(format!(
                "{what}: a block of stereo frames has an even number of samples, not {}",
                samples.len()
            )));
        }
        let n = samples.len() as u64 / 2;
        if done + n > self.frames {
            return Err(msg(format!(
                "{what}: {} frames written to a song of {}",
                done + n,
                self.frames
            )));
        }
        Ok(n)
    }

    /// Writes wlin (and wrmx) once every frame is in. Returns the file's size.
    pub fn finish(mut self) -> Result<u64, Error> {
        if (self.master_done, self.stems_done) != (self.frames, self.frames) {
            return Err(msg(format!(
                "the song is {} frames, and {} of the master and {} of the stems were written",
                self.frames, self.master_done, self.stems_done
            )));
        }
        let tail = std::mem::take(&mut self.tail);
        self.put(self.total - tail.len() as u64, &tail)?;
        self.file.flush()?;
        Ok(self.total)
    }
}

/// Copies `n` bytes from `at` in `src` to `out`.
fn copy(src: &mut File, at: u64, n: u64, out: &mut impl Write) -> Result<(), Error> {
    src.seek(SeekFrom::Start(at))?;
    if io::copy(&mut src.take(n), out)? != n {
        return Err(msg("the file got shorter while being read"));
    }
    Ok(())
}

/// The store's ".wwav, master only" (6.12): `src`, a song with stems,
/// without them. The master, wmet and wlin are copied as they are, so it
/// keeps its song_id and every byte of its identity; wstm goes, and so
/// does a remix's wrmx, which only sets the stems up. Returns its size.
pub fn master_only(src: &Path, out: &Path) -> Result<u64, Error> {
    not_same(src, out)?;
    let w = Wwav::open(src)?;
    let verdict = w.verdict();
    if verdict != Verdict::Stems {
        return Err(msg(format!(
            "{}: {verdict}, so it has no stems to leave out",
            src.display()
        )));
    }
    let mut file = File::open(src)?;
    let mut tail = Vec::new();
    for id in [b"wmet", b"wlin"] {
        let c = *w.first(id).ok_or_else(|| msg("no identity"))?;
        tail.extend(chunk(id, &read_at(&mut file, c.at, c.size)?));
    }
    let (frames, data) = (w.master_frames(), w.first(b"data").map_or(0, |c| c.at));
    let total = 44 + frames * FRAME + tail.len() as u64;
    let mut o = BufWriter::with_capacity(1 << 20, File::create(out)?);
    o.write_all(&wav_header(frames, total - 8))?;
    copy(&mut file, data, frames * FRAME, &mut o)?;
    o.write_all(&tail)?;
    o.flush()?;
    Ok(total)
}

/// A plain 44.1 kHz 16-bit stereo WAV as a master-only `.wwav`, as Wi's
/// `wrapWav` (wi/src/formats/wwav.js) writes it, refusals in its words.
///
/// PRANA lists a file only if its first 1 KB walks to the data chunk with
/// the fmt it needs before it (prana/core/disc/wav.cpp). Most WAVs pass:
/// wmet and wlin then go after the last chunk, so nothing moves and it
/// still plays as the WAV it was. One that doesn't (a DAW's FLLR padding,
/// a recorder's bext and iXML) gets the plain 44-byte head in front of the
/// same audio, and its other chunks are left behind. wmet holds `meta`
/// (Wi passes no bpm, key or splitter) with the master's frames; wlin makes
/// it an original by `creator`. Returns the file's size.
pub fn wrap_wav(src: &Path, out: &Path, meta: &SongMeta, creator: &str) -> Result<u64, Error> {
    not_same(src, out)?;
    let w = match Wwav::open(src) {
        Err(Error::Msg(m)) if m.ends_with(": not a WAV") => {
            return Err(msg("This isn’t a WAV file"))
        }
        r => r?,
    };
    let mut file = File::open(src)?;
    // read as JSON.parse reads: NaN and Infinity aren't JSON
    let mut object = |id: &[u8; 4]| -> io::Result<bool> {
        Ok(match w.first(id) {
            Some(c) => matches!(
                json::loads(&read_at(&mut file, c.at, c.size)?, false),
                Some(Value::Dict(_))
            ),
            None => false,
        })
    };
    if object(b"wmet")? && object(b"wlin")? {
        return Err(msg("It’s already a .wwav"));
    }
    if w.chunks
        .iter()
        .any(|c| [b"wmet", b"wstm", b"wlin", b"wrmx"].contains(&&c.id))
    {
        return Err(msg("It has .wwav chunks, but they don’t read as a .wwav"));
    }
    if w.fmt != Some(Fmt::CD) {
        let have = match w.fmt {
            Some(f) => format!(
                "this is {} Hz, {}-bit, {} ch{}",
                f.rate,
                f.bits,
                f.channels,
                if f.format == 1 { "" } else { ", not PCM" }
            ),
            None => "it has no fmt chunk".into(),
        };
        return Err(msg(format!(
            "It needs to be 44.1 kHz 16-bit stereo PCM ({have})"
        )));
    }
    let data = *w
        .first(b"data")
        .ok_or_else(|| msg("It has no audio (no data chunk)"))?;
    let frames = data.size / FRAME;
    if frames == 0 {
        return Err(msg("It has no audio (the data chunk is empty)"));
    }
    let d = w.chunks.iter().position(|c| &c.id == b"data").unwrap_or(0);
    let before = w.chunks[..d].iter().rfind(|c| &c.id == b"fmt ").copied();
    let fmt_before = |file: &mut File, extensible| -> io::Result<Option<Fmt>> {
        Ok(match before {
            Some(c) => Fmt::parse(&read_at(file, c.at, c.size.min(40))?, extensible),
            None => None,
        })
    };
    if before.is_some() && fmt_before(&mut file, 26)? != Some(Fmt::CD) {
        return Err(msg(
            "The fmt chunk before the audio isn’t 44.1 kHz 16-bit stereo PCM",
        ));
    }
    let mut tail = chunk(b"wmet", meta.wmet(frames).as_bytes());
    tail.extend(chunk(
        b"wlin",
        Lineage::original(&meta.song_id, creator).wlin().as_bytes(),
    ));
    let too_long = || msg("It’s too long for one WAV file (4 GB)");

    if fmt_before(&mut file, 40)? != Some(Fmt::CD) || data.at > 1024 {
        // the 44-byte head wwav_pack.py writes, in front of the same audio
        let total = 44 + frames * FRAME + tail.len() as u64;
        if total - 8 > MAX_FILE {
            return Err(too_long());
        }
        let mut o = BufWriter::with_capacity(1 << 20, File::create(out)?);
        o.write_all(&wav_header(frames, total - 8))?;
        copy(&mut file, data.at, frames * FRAME, &mut o)?;
        o.write_all(&tail)?;
        o.flush()?;
        return Ok(total);
    }

    // The appended chunks are only found if the chunk walk ends exactly
    // where the file does (after a missing pad byte, which is added).
    let size = w.size;
    let last = *w.chunks.last().ok_or_else(|| msg("It has no chunks"))?;
    let h = read_at(&mut file, last.at - 4, 4)?;
    let n = u32::from_le_bytes([h[0], h[1], h[2], h[3]]) as u64;
    if last.at + n > size {
        // JavaScript's trim(): the id's spaces and line breaks, and U+00A0
        let id: String = last.id.iter().map(|&b| b as char).collect();
        let id = id
            .trim_matches(|c| matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}'));
        return Err(msg(format!(
            "The file is cut off: its {id} chunk runs past the end"
        )));
    }
    if last.at + n + (n & 1) != size + (size & 1) {
        return Err(msg("The file has stray bytes after its last chunk"));
    }
    let total = size + (size & 1) + tail.len() as u64;
    if total - 8 > MAX_FILE {
        return Err(too_long());
    }
    let mut o = BufWriter::with_capacity(1 << 20, File::create(out)?);
    o.write_all(b"RIFF")?;
    o.write_all(&((total - 8) as u32).to_le_bytes())?;
    copy(&mut file, 8, size - 8, &mut o)?;
    o.write_all(&vec![0; (size & 1) as usize])?;
    o.write_all(&tail)?;
    o.flush()?;
    Ok(total)
}
