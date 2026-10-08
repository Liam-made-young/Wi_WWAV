//! Converting a file to a 32-bit float WAV at a session's rate.
//!
//! The WAV is the plain 44-byte kind: `RIFF`, `WAVE`, a 16-byte `fmt ` with
//! format tag 3, then `data`. The lengths in its header aren't known until
//! the last packet has decoded (an MP3 needn't say how long it is), so they
//! are written as zeros and patched at the end, and the file only takes its
//! real name once that is done.

use std::fs::{self, File};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use wwav_dsp::resample::{resampled_len, Resampler};

use crate::{Decoder, Error};

const HEADER: u64 = 44;
/// A RIFF file's size field is 32 bits and counts all but its first 8 bytes.
const MAX_DATA: u64 = u32::MAX as u64 - (HEADER - 8);
const SAMPLE: u64 = 4;

/// What [`to_wav`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    /// Frames in the WAV written, at the rate asked for.
    pub frames: u64,
    /// 1 or 2, as the source.
    pub channels: u16,
    pub source_rate: u32,
    pub codec: String,
}

/// Decodes all of `src` and writes it to `dst` as a 32-bit float WAV at
/// `rate`, resampling when the file's rate differs and writing the decoded
/// samples untouched when it doesn't. The WAV is written under a temporary
/// name beside `dst` and renamed into place when whole, so `dst` is never a
/// cut-off file. `cancel` is checked between packets, and a cancelled or
/// failed conversion leaves nothing behind. `progress` is called about once
/// for each second of sound with the source frames decoded so far and the
/// total when the file says it.
pub fn to_wav(
    src: &Path,
    dst: &Path,
    rate: u32,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<Converted, Error> {
    convert(src, dst, rate, cancel, progress, MAX_DATA)
}

/// `to_wav` with the most `data` bytes a WAV may hold as an argument, so a
/// test can reach the limit without writing 4 GB.
fn convert(
    src: &Path,
    dst: &Path,
    rate: u32,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
    max_data: u64,
) -> Result<Converted, Error> {
    let mut decoder = Decoder::open(src)?;
    let info = decoder.info().clone();
    let name = src.file_name().unwrap_or(src.as_os_str()).to_string_lossy();
    if rate == 0 {
        return Err(Error::Unsupported(format!(
            "{name} can't be converted to a rate of 0 Hz."
        )));
    }
    let too_long = || {
        Error::Unsupported(format!(
            "{name} is too long to convert: it would be a WAV over 4 GB."
        ))
    };
    let frame = info.channels as u64 * SAMPLE;
    // Refused before any of it is written when the file says its length.
    if let Some(frames) = info.frames {
        if resampled_len(frames, info.rate, rate).saturating_mul(frame) > max_data {
            return Err(too_long());
        }
    }

    let mut partial = Partial::create(dst)?;
    let mut file = BufWriter::new(&partial.file);
    file.write_all(&header(info.channels, rate, 0))?;

    let mut resampler =
        (info.rate != rate).then(|| Resampler::new(info.rate, rate, info.channels as usize));
    let (mut decoded, mut resampled, mut bytes) = (Vec::new(), Vec::new(), Vec::new());
    let (mut done, mut told, mut data) = (0u64, 0u64, 0u64);
    progress(0, info.frames);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        decoded.clear();
        let frames = decoder.read(&mut decoded)? as u64;
        let last = frames == 0;
        let samples = match resampler.take() {
            None => &decoded,
            Some(mut r) => {
                resampled.clear();
                r.process(&decoded, &mut resampled);
                if last {
                    r.finish(&mut resampled);
                } else {
                    resampler = Some(r);
                }
                &resampled
            }
        };
        bytes.clear();
        bytes.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
        data += bytes.len() as u64;
        if data > max_data {
            return Err(too_long());
        }
        file.write_all(&bytes)?;
        if last {
            break;
        }
        done += frames;
        if done - told >= info.rate as u64 {
            told = done;
            progress(done, info.frames);
        }
    }
    progress(done, info.frames);

    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header(info.channels, rate, data))?;
    file.flush()?;
    drop(file);
    partial.keep(dst)?;
    Ok(Converted {
        frames: data / frame,
        channels: info.channels,
        source_rate: info.rate,
        codec: info.codec,
    })
}

fn header(channels: u16, rate: u32, data: u64) -> [u8; HEADER as usize] {
    let frame = channels * SAMPLE as u16;
    let mut h = [0u8; HEADER as usize];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&((HEADER - 8 + data) as u32).to_le_bytes());
    h[8..16].copy_from_slice(b"WAVEfmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    // Format tag 3: IEEE float.
    h[20..22].copy_from_slice(&3u16.to_le_bytes());
    h[22..24].copy_from_slice(&channels.to_le_bytes());
    h[24..28].copy_from_slice(&rate.to_le_bytes());
    h[28..32].copy_from_slice(&rate.wrapping_mul(frame as u32).to_le_bytes());
    h[32..34].copy_from_slice(&frame.to_le_bytes());
    h[34..36].copy_from_slice(&32u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&(data as u32).to_le_bytes());
    h
}

/// The WAV while it is being written, under a name of its own beside where it
/// is going. Dropped before `keep`, it deletes itself.
struct Partial {
    path: PathBuf,
    file: File,
    kept: bool,
}

impl Partial {
    fn create(dst: &Path) -> Result<Partial, Error> {
        // Two conversions to one `dst` at once, in this process or another,
        // each get their own file.
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let mut name = std::ffi::OsString::from(".");
        name.push(dst.file_name().unwrap_or(dst.as_os_str()));
        name.push(format!(".{}-{n}.part", std::process::id()));
        let path = dst.with_file_name(name);
        let file = File::options().write(true).create_new(true).open(&path)?;
        Ok(Partial {
            path,
            file,
            kept: false,
        })
    }

    /// On the disk before it has its name, so a power cut can't leave `dst`
    /// with a whole header and half its sound.
    fn keep(&mut self, dst: &Path) -> Result<(), Error> {
        self.file.sync_all()?;
        fs::rename(&self.path, dst)?;
        self.kept = true;
        Ok(())
    }
}

impl Drop for Partial {
    fn drop(&mut self) {
        if !self.kept {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A second of 16-bit mono silence at 48 kHz.
    fn a_wav(path: &Path) {
        let data = vec![0u8; 96_000];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&96_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&data);
        fs::write(path, bytes).unwrap();
    }

    fn files_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_wav_over_the_size_limit_is_refused_and_nothing_is_left() {
        let dir = tempfile::tempdir().unwrap();
        let (src, dst) = (dir.path().join("in.wav"), dir.path().join("out.wav"));
        a_wav(&src);
        let never = AtomicBool::new(false);
        // 48,000 mono frames are 192,000 bytes of float.
        for rate in [48_000, 44_100] {
            let fits = convert(&src, &dst, rate, &never, &mut |_, _| {}, 192_000);
            assert_eq!(fits.unwrap().frames, resampled_len(48_000, 48_000, rate));
            fs::remove_file(&dst).unwrap();
            let over = convert(&src, &dst, rate, &never, &mut |_, _| {}, 100_000);
            assert!(
                matches!(&over, Err(Error::Unsupported(s)) if s == "in.wav is too long to convert: it would be a WAV over 4 GB."),
                "{over:?}"
            );
            assert_eq!(files_in(dir.path()), ["in.wav"]);
        }
    }

    /// A raw AAC stream doesn't say how long it is, so the limit is only met
    /// part-way through it.
    #[test]
    fn a_file_that_doesnt_say_its_length_is_stopped_at_the_limit() {
        let ffmpeg = Path::new("/opt/homebrew/bin/ffmpeg");
        if !ffmpeg.exists() {
            eprintln!("skipped: no /opt/homebrew/bin/ffmpeg to make an AAC stream");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let (wav, src) = (dir.path().join("in.wav"), dir.path().join("in.aac"));
        let dst = dir.path().join("out.wav");
        a_wav(&wav);
        let made = std::process::Command::new(ffmpeg)
            .args(["-loglevel", "error", "-f", "wav", "-i"])
            .arg(&wav)
            .args(["-c:a", "aac"])
            .arg(&src)
            .output()
            .unwrap();
        assert!(made.status.success(), "{made:?}");
        fs::remove_file(&wav).unwrap();
        assert_eq!(Decoder::open(&src).unwrap().info().frames, None);

        let never = AtomicBool::new(false);
        let over = convert(&src, &dst, 48_000, &never, &mut |_, _| {}, 100_000);
        assert!(matches!(over, Err(Error::Unsupported(_))), "{over:?}");
        assert_eq!(files_in(dir.path()), ["in.aac"]);
    }

    #[test]
    fn the_header_counts_its_own_bytes() {
        let h = header(2, 44_100, 800);
        assert_eq!(&h[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(h[4..8].try_into().unwrap()), 36 + 800);
        assert_eq!(u16::from_le_bytes(h[20..22].try_into().unwrap()), 3);
        assert_eq!(
            u32::from_le_bytes(h[28..32].try_into().unwrap()),
            44_100 * 8
        );
        assert_eq!(u16::from_le_bytes(h[32..34].try_into().unwrap()), 8);
        assert_eq!(u32::from_le_bytes(h[40..44].try_into().unwrap()), 800);
    }
}
