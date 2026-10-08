//! Files the engine can't play where they lie: audio in another format
//! (MP3, AAC, FLAC, AIFF and the rest of what `wwav-decode` reads), and
//! audio at another rate than the session's. Each is made once into a
//! 32-bit float WAV at the session's rate, kept in the cache folder, and
//! played from there like any other WAV: so a seek, a loop and a render of
//! it are exact, and the audio thread does no decoding.
//!
//! The cache is keyed by the file's path, size and modified time, the part
//! of it played and the rate, so a file that changes is made again.

use crate::build::Converted;
use crate::media::{self, Failure, MediaPart, Probe, Source, RUN};
use crate::wav::TakeWriter;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use wwav_dsp::resample::Resampler;

fn failed(path: &str, e: impl std::fmt::Display) -> Failure {
    Failure::new(
        "convert_failed",
        format!("Can't make {path} ready to play ({e})."),
    )
}

/// Where the WAV made from this file, part and rate is kept.
fn cached(dir: &Path, path: &str, source: Source, rate: u32) -> Result<PathBuf, Failure> {
    let meta = std::fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Failure::new("no_such_file", format!("No file at {path}.")),
        _ => Failure::new("bad_clip", format!("Can't read {path} ({e}).")),
    })?;
    let changed = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let mut h = Sha256::new();
    h.update(format!(
        "1\0{path}\0{}\0{changed}\0{}\0{rate}",
        meta.len(),
        source.name()
    ));
    Ok(dir.join(format!("{}.wav", &hex::encode(h.finalize())[..32])))
}

/// The rate a cached WAV was made from, kept beside it.
fn rate_file(wav: &Path) -> PathBuf {
    wav.with_extension("rate")
}

/// A part the engine reads directly, resampled to `rate` into `dst`.
fn resample(
    path: &str,
    part: &MediaPart,
    rate: u32,
    dst: &Path,
    stopping: &AtomicBool,
) -> Result<(), Failure> {
    let file = File::open(path).map_err(|e| failed(path, e))?;
    let part_file = dst.with_extension("part");
    let channels = part.channels as usize;
    let write = || -> std::io::Result<bool> {
        let mut out = TakeWriter::create(&part_file, part.channels, rate)?;
        let mut resampler = Resampler::new(part.rate, rate, channels);
        let (mut l, mut r) = (vec![0.0f32; RUN], vec![0.0f32; RUN]);
        let mut raw = vec![0u8; RUN * part.stride as usize];
        let (mut frames, mut made) = (Vec::with_capacity(RUN * channels), Vec::new());
        let mut at = 0u64;
        while at < part.frames {
            if stopping.load(Ordering::Relaxed) {
                return Ok(false);
            }
            let n = RUN.min((part.frames - at) as usize);
            media::read_frames(&file, part, at as i64, &mut l[..n], &mut r[..n], &mut raw);
            frames.clear();
            for i in 0..n {
                frames.push(l[i]);
                if channels == 2 {
                    frames.push(r[i]);
                }
            }
            made.clear();
            resampler.process(&frames, &mut made);
            out.write(&made)?;
            at += n as u64;
        }
        made.clear();
        resampler.finish(&mut made);
        out.write(&made)?;
        out.finish()?;
        Ok(true)
    };
    let done = write();
    match done {
        Ok(true) => std::fs::rename(&part_file, dst).map_err(|e| failed(path, e)),
        Ok(false) => {
            let _ = std::fs::remove_file(&part_file);
            Err(Failure::new(
                "cancelled",
                "The engine stopped before the file was ready.",
            ))
        }
        Err(e) => {
            let _ = std::fs::remove_file(&part_file);
            Err(failed(path, e))
        }
    }
}

/// The WAV to play instead of `source` of `path` in a session at `rate`,
/// made now unless the cache has it. `direct` is the part when the engine
/// reads the file where it lies and only its rate is wrong. `progress` hears
/// source frames done and their total, when known.
pub fn convert(
    dir: &Path,
    path: &str,
    source: Source,
    direct: Option<&MediaPart>,
    rate: u32,
    stopping: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<Converted, Failure> {
    std::fs::create_dir_all(dir).map_err(|e| failed(path, e))?;
    let wav = cached(dir, path, source, rate)?;
    let shown = wav.to_string_lossy().into_owned();
    let known = std::fs::read_to_string(rate_file(&wav))
        .ok()
        .and_then(|r| r.trim().parse::<u32>().ok());
    let source_rate = match known.filter(|_| wav.is_file()) {
        Some(rate) => rate,
        None => {
            let from = match direct {
                Some(part) => {
                    resample(path, part, rate, &wav, stopping)?;
                    part.rate
                }
                None => {
                    let made = wwav_decode::to_wav(Path::new(path), &wav, rate, stopping, progress);
                    made.map_err(|e| match e {
                        wwav_decode::Error::NoSuchFile(m) => Failure::new("no_such_file", m),
                        wwav_decode::Error::Unsupported(m) => Failure::new("unsupported", m),
                        wwav_decode::Error::Bad(m) => Failure::new("bad_clip", m),
                        wwav_decode::Error::Cancelled => Failure::new(
                            "cancelled",
                            "The engine stopped before the file was ready.",
                        ),
                        wwav_decode::Error::Io(e) => failed(path, e),
                    })?
                    .source_rate
                }
            };
            std::fs::write(rate_file(&wav), from.to_string()).map_err(|e| failed(path, e))?;
            from
        }
    };
    match media::probe(&shown, Source::Master)? {
        Probe::Direct(part) if part.rate == rate => Ok(Converted {
            path: shown,
            part,
            source_rate,
        }),
        _ => Err(failed(
            path,
            "what was made isn't a WAV at the session's rate",
        )),
    }
}
