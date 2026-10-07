//! `wwav_pack.py pack` and `unpack`: a song folder to a `.wwav` and back,
//! the same bytes and the same words.
//!
//! A song folder holds master.wav and the four stems (or a breadboard
//! folder's 1.wav to 5.wav, 1 being the master), every file 44.1 kHz 16-bit
//! stereo PCM and all the same length, and song.txt (title, artist, bpm,
//! key, song_id, created). unpack writes the folder back with the song's id
//! in song.txt, so packing it again gives the same bytes.
//!
//! Paths are taken and shown as the tool shows them (`os.path` on POSIX).

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::json::{self, Value};
use crate::meta::{Kind, Lineage, SongMeta};
use crate::text::{basename, join, key_values, py_float, title_of, today};
use crate::writer::{wav_header, WwavWriter};
use crate::wwav::{read_at, ChunkWalk, Fmt, Wwav, FRAME, STEMS, STEM_FRAME};
use crate::{msg, not_same, Error};

const NAMED: [&str; 5] = [
    "master.wav",
    "vocals.wav",
    "drums.wav",
    "other.wav",
    "bass.wav",
];
const LEGACY: [&str; 5] = ["1.wav", "2.wav", "3.wav", "4.wav", "5.wav"];
/// Frames per pass, as the tool streams them.
const BLOCK: u64 = 1 << 16;

/// What pack wrote, for its line.
#[derive(Clone, Debug, PartialEq)]
pub struct Packed {
    pub title: String,
    pub frames: u64,
    pub total: u64,
    pub song_id: String,
}

impl Packed {
    /// The line pack prints, `out` being the .wwav as given.
    pub fn line(&self, out: &str) -> String {
        format!(
            "{out}: {}, {} frames ({:.1} s), {} bytes, song_id {}",
            self.title,
            self.frames,
            self.frames as f64 / 44_100.0,
            self.total,
            self.song_id
        )
    }
}

/// A WAV to pack from, at its first frame (the tool's `Wav` class).
struct Input {
    file: File,
    path: String,
    frames: u64,
}

impl Input {
    fn open(path: String) -> Result<Input, Error> {
        let mut file = File::open(&path).map_err(|e| msg(format!("{path}: {e}")))?;
        let size = file.metadata()?.len();
        let head = read_at(&mut file, 0, 12)?;
        if head.len() < 12 || &head[..4] != b"RIFF" || &head[8..] != b"WAVE" {
            return Err(msg(format!("{path}: not a WAV")));
        }
        let mut fmt = None;
        let mut walk = ChunkWalk::new(size);
        while let Some(c) = walk.next_chunk(&mut file)? {
            if &c.id == b"fmt " {
                fmt = Fmt::parse(&read_at(&mut file, c.at, c.size.min(40))?, 26);
            } else if &c.id == b"data" {
                if fmt != Some(Fmt::CD) {
                    let have = fmt.map_or("no fmt".into(), |f| {
                        format!(
                            "format {}, {} ch, {} Hz, {}-bit",
                            f.format, f.channels, f.rate, f.bits
                        )
                    });
                    return Err(msg(format!(
                        "{path}: needs 44.1 kHz 16-bit stereo PCM ({have})"
                    )));
                }
                // size 0: to the end, as the device reads it
                let frames = if c.size == 0 { size - c.at } else { c.size } / FRAME;
                file.seek(SeekFrom::Start(c.at))?;
                return Ok(Input { file, path, frames });
            }
        }
        Err(msg(format!("{path}: no data chunk")))
    }

    fn read(&mut self, frames: u64) -> Result<Vec<i16>, Error> {
        let mut bytes = vec![0u8; (frames * FRAME) as usize];
        self.file.read_exact(&mut bytes)?;
        Ok(bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect())
    }
}

/// `wwav_pack.py pack folder -o out --splitter S --creator C`. A song.txt
/// without a valid song_id gets a new one; without a created date, today's.
/// Where the tool would write the .wwav over one of the WAVs it is reading,
/// this refuses, as swav_pack.py does ("would write over").
pub fn pack(folder: &str, out: &str, splitter: &str, creator: &str) -> Result<Packed, Error> {
    if !Path::new(folder).is_dir() {
        return Err(msg(format!("{folder}: not a folder")));
    }
    let mut have: HashMap<String, String> = HashMap::new();
    for entry in std::fs::read_dir(folder)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        have.insert(name.to_lowercase(), name);
    }
    let any = |names: &[&str]| names.iter().any(|n| have.contains_key(*n));
    let names = if any(&NAMED) || !any(&LEGACY) {
        NAMED
    } else {
        LEGACY
    };
    let missing: Vec<&str> = names
        .iter()
        .copied()
        .filter(|n| !have.contains_key(*n))
        .collect();
    if !missing.is_empty() {
        return Err(msg(format!("{folder}: missing {}", missing.join(", "))));
    }
    let mut wavs = names
        .iter()
        .map(|n| Input::open(join(folder, &have[*n])))
        .collect::<Result<Vec<_>, _>>()?;
    let frames = wavs[0].frames;
    if wavs.iter().any(|w| w.frames != frames) {
        let lengths: Vec<String> = wavs
            .iter()
            .map(|w| format!("{} {}", basename(&w.path), w.frames))
            .collect();
        return Err(msg(format!(
            "{folder}: the files aren't the same length ({} frames)",
            lengths.join(", ")
        )));
    }

    let info = key_values(Path::new(&join(folder, "song.txt")))?;
    let text = |k: &str| info.get(k).cloned().unwrap_or_default();
    let song_id = Some(text("song_id"))
        .filter(|id| wwav_ids::is_work_id(id))
        .unwrap_or_else(wwav_ids::new_work_id);
    let bpm = info
        .get("bpm")
        .map_or(Some(0.0), |b| py_float(b))
        .unwrap_or(0.0);
    let meta = SongMeta {
        song_id: song_id.clone(),
        title: Some(text("title"))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| title_of(folder)),
        artist: text("artist"),
        bpm: if 20.0 < bpm && bpm < 400.0 { bpm } else { 0.0 },
        key: text("key"),
        kind: Kind::Original,
        splitter: splitter.into(),
        created: Some(text("created"))
            .filter(|c| !c.is_empty())
            .unwrap_or_else(today),
    };
    let lineage = Lineage::original(&song_id, creator);
    // the tool would write over the WAV it is reading: refuse instead
    for w in &wavs {
        not_same(Path::new(&w.path), Path::new(out))?;
    }
    let mut w = match WwavWriter::create(Path::new(out), frames, &meta, &lineage, None) {
        Err(Error::TooLong { .. }) => {
            return Err(msg(format!("{folder}: too long for one WAV file (4 GB)")))
        }
        r => r?,
    };
    for f0 in (0..frames).step_by(BLOCK as usize) {
        w.write_master(&wavs[0].read(BLOCK.min(frames - f0))?)?;
    }
    for f0 in (0..frames).step_by(BLOCK as usize) {
        let n = BLOCK.min(frames - f0);
        let [v, d, o, b] = [
            wavs[1].read(n)?,
            wavs[2].read(n)?,
            wavs[3].read(n)?,
            wavs[4].read(n)?,
        ];
        w.write_stems([&v, &d, &o, &b])?;
    }
    let total = w.finish()?;
    Ok(Packed {
        title: meta.title,
        frames,
        total,
        song_id,
    })
}

/// The tool's `num(v) if isinstance(v, (int, float)) else v`, as song.txt
/// gets it.
fn song_txt_value(v: &Value) -> Result<String, Error> {
    match v {
        Value::Bool(b) => Ok(if *b { "1" } else { "0" }.into()),
        Value::Int(t) => Ok(t.clone()),
        Value::Float(f) => json::num(*f).map_err(msg),
        _ => Ok(json::py_str(v)),
    }
}

/// The first wmet or wlin as python's `w.json(...) or {}`: a JSON object's
/// items, nothing for a false value. python dies on a true value that
/// isn't an object (`.get` of a list); this says so instead.
pub(crate) fn object_or_empty(v: &Option<Value>) -> Result<&[(String, Value)], Error> {
    match v {
        Some(Value::Dict(d)) => Ok(d),
        Some(v) if json::truthy(v) => Err(msg(format!(
            "'{}' object has no attribute 'get'",
            match v {
                Value::List(_) => "list",
                Value::Str(_) => "str",
                Value::Float(_) => "float",
                Value::Bool(_) => "bool",
                _ => "int",
            }
        ))),
        _ => Ok(&[]),
    }
}

/// `wwav_pack.py unpack file -o outdir`: master.wav, the four stems when
/// wstm holds them whole, and song.txt. Returns the names written. Where
/// the tool would write one of them over the .wwav it is reading (a .wwav
/// named master.wav unpacked into its own folder), this refuses first.
pub fn unpack(path: &str, outdir: &str) -> Result<Vec<String>, Error> {
    let mut w = Wwav::open(Path::new(path))?;
    if w.first(b"data").is_none() || w.fmt != Some(Fmt::CD) {
        return Err(msg(format!(
            "{path}: the master isn't 44.1 kHz 16-bit stereo PCM"
        )));
    }
    let stems = w
        .wstm
        .filter(|s| s.fits && (s.version, s.stems, s.channels, s.bits) == (1, 4, 2, 16));
    // the tool would write over the .wwav it is reading: refuse instead
    let mut names = vec!["master.wav".to_string()];
    if stems.is_some() {
        names.extend(STEMS.iter().map(|n| format!("{n}.wav")));
    }
    names.push("song.txt".into());
    for n in &names {
        not_same(Path::new(path), Path::new(&join(outdir, n)))?;
    }
    std::fs::create_dir_all(outdir)?;
    let frames = w.master_frames();
    let mut out = BufWriter::new(File::create(join(outdir, "master.wav"))?);
    out.write_all(&wav_header(frames, 36 + frames * FRAME))?;
    let mut buf = vec![0i16; BLOCK as usize * 2];
    for f0 in (0..frames).step_by(BLOCK as usize) {
        let n = w.read_master(f0, &mut buf)?;
        out.write_all(
            &buf[..n * 2]
                .iter()
                .flat_map(|s| s.to_le_bytes())
                .collect::<Vec<u8>>(),
        )?;
    }
    out.flush()?;

    if let Some(s) = stems {
        let stem_frames = s.frames as u64;
        let mut outs = STEMS
            .iter()
            .map(|n| {
                Ok(BufWriter::new(File::create(join(
                    outdir,
                    &format!("{n}.wav"),
                ))?))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        for o in &mut outs {
            o.write_all(&wav_header(stem_frames, 36 + stem_frames * FRAME))?;
        }
        let mut buf = vec![0i16; (BLOCK * STEM_FRAME / 2) as usize];
        for f0 in (0..stem_frames).step_by(BLOCK as usize) {
            let n = w.read_stems(f0, &mut buf)?;
            for (i, o) in outs.iter_mut().enumerate() {
                let bytes: Vec<u8> = buf[..n * 8]
                    .chunks_exact(8)
                    .flat_map(|f| [f[i * 2], f[i * 2 + 1]])
                    .flat_map(i16::to_le_bytes)
                    .collect();
                o.write_all(&bytes)?;
            }
        }
        for o in &mut outs {
            o.flush()?;
        }
    }

    let meta = object_or_empty(&w.wmet)?;
    let mut lines = Vec::new();
    for k in ["title", "artist", "bpm", "key", "song_id", "created"] {
        match json::get(meta, k) {
            None | Some(Value::Null) => {}
            Some(Value::Str(s)) if s.is_empty() => {}
            Some(v) => lines.push(format!("{k} = {}", song_txt_value(v)?)),
        }
    }
    std::fs::write(join(outdir, "song.txt"), lines.join("\n") + "\n")?;
    Ok(names)
}
