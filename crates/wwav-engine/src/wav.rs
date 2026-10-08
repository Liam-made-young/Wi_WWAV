//! A stereo WAVE writer that hashes what it writes, for `render` and for
//! takes: 32-bit float or 16-bit PCM.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavFormat {
    /// 32-bit IEEE float.
    F32,
    /// 16-bit PCM.
    S16,
}

impl WavFormat {
    fn bytes(self) -> u64 {
        match self {
            WavFormat::F32 => 4,
            WavFormat::S16 => 2,
        }
    }

    /// The float form is the non-PCM one: an 18-byte fmt and a fact chunk.
    fn head(self) -> u64 {
        match self {
            WavFormat::F32 => 58,
            WavFormat::S16 => 44,
        }
    }
}

/// Rounded to the nearest (ties to even) and clamped, with no dither: a
/// render is not an export (`docs/SPEC.md` 6.7).
pub fn to_s16(x: f32) -> i16 {
    let v = x * 32768.0;
    if v >= 32767.0 {
        32767
    } else if v <= -32768.0 {
        -32768
    } else {
        v.round_ties_even() as i16
    }
}

fn header(format: WavFormat, channels: u16, rate: u32, frames: u64) -> Vec<u8> {
    let align = channels as u32 * format.bytes() as u32;
    let data = (frames * align as u64) as u32;
    let mut h = Vec::with_capacity(58);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(format.head() as u32 - 8 + data).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    let float = format == WavFormat::F32;
    h.extend_from_slice(&(if float { 18u32 } else { 16 }).to_le_bytes());
    h.extend_from_slice(&(if float { 3u16 } else { 1 }).to_le_bytes());
    h.extend_from_slice(&channels.to_le_bytes());
    h.extend_from_slice(&rate.to_le_bytes());
    h.extend_from_slice(&(rate * align).to_le_bytes());
    h.extend_from_slice(&(align as u16).to_le_bytes());
    h.extend_from_slice(&(format.bytes() as u16 * 8).to_le_bytes());
    if float {
        h.extend_from_slice(&0u16.to_le_bytes());
        h.extend_from_slice(b"fact");
        h.extend_from_slice(&4u32.to_le_bytes());
        h.extend_from_slice(&(frames as u32).to_le_bytes());
    }
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data.to_le_bytes());
    h
}

/// Whether a WAVE file holds `frames` frames of `channels` in `format`.
pub fn fits(format: WavFormat, channels: u16, frames: u64) -> bool {
    frames
        .checked_mul(channels as u64 * format.bytes())
        .is_some_and(|data| data <= u32::MAX as u64 - format.head())
}

/// A file of a length known before it is written.
pub struct WavWriter {
    out: BufWriter<File>,
    hash: Sha256,
    format: WavFormat,
    bytes: Vec<u8>,
}

impl WavWriter {
    pub fn create(
        path: &Path,
        format: WavFormat,
        rate: u32,
        frames: u64,
    ) -> Result<WavWriter, String> {
        if !fits(format, 2, frames) {
            return Err(format!(
                "A WAVE file holds 4 GB; this render needs {} bytes.",
                frames.saturating_mul(2 * format.bytes())
            ));
        }
        let file =
            File::create(path).map_err(|e| format!("Can't write {} ({e}).", path.display()))?;
        let mut w = WavWriter {
            out: BufWriter::with_capacity(1 << 16, file),
            hash: Sha256::new(),
            format,
            bytes: Vec::new(),
        };
        w.put(&header(format, 2, rate, frames))
            .map_err(|e| format!("Can't write {} ({e}).", path.display()))?;
        Ok(w)
    }

    fn put(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.hash.update(bytes);
        self.out.write_all(bytes)
    }

    pub fn write(&mut self, l: &[f32], r: &[f32]) -> io::Result<()> {
        let mut bytes = std::mem::take(&mut self.bytes);
        bytes.clear();
        for (a, b) in l.iter().zip(r) {
            match self.format {
                WavFormat::F32 => {
                    bytes.extend_from_slice(&a.to_le_bytes());
                    bytes.extend_from_slice(&b.to_le_bytes());
                }
                WavFormat::S16 => {
                    bytes.extend_from_slice(&to_s16(*a).to_le_bytes());
                    bytes.extend_from_slice(&to_s16(*b).to_le_bytes());
                }
            }
        }
        let done = self.put(&bytes);
        self.bytes = bytes;
        done
    }

    /// Finishes the file; returns its SHA-256 as lowercase hex.
    pub fn finish(mut self) -> io::Result<String> {
        self.out.flush()?;
        Ok(hex::encode(self.hash.finalize()))
    }
}

/// A take: 32-bit float frames of a length known only when it ends. The
/// header is written again with the real length at the end.
pub struct TakeWriter {
    out: BufWriter<File>,
    channels: u16,
    rate: u32,
    frames: u64,
}

impl TakeWriter {
    pub fn create(path: &Path, channels: u16, rate: u32) -> io::Result<TakeWriter> {
        let mut out = BufWriter::with_capacity(1 << 16, File::create(path)?);
        out.write_all(&header(WavFormat::F32, channels, rate, 0))?;
        Ok(TakeWriter {
            out,
            channels,
            rate,
            frames: 0,
        })
    }

    /// Interleaved samples, whole frames.
    pub fn write(&mut self, samples: &[f32]) -> io::Result<()> {
        for s in samples {
            self.out.write_all(&s.to_le_bytes())?;
        }
        self.frames += samples.len() as u64 / self.channels as u64;
        Ok(())
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Writes the header with the take's length; returns its frames.
    pub fn finish(mut self) -> io::Result<u64> {
        // A take past 4 GB keeps its audio; its header says what fits.
        let frames = if fits(WavFormat::F32, self.channels, self.frames) {
            self.frames
        } else {
            (u32::MAX as u64 - 58) / (self.channels as u64 * 4)
        };
        self.out.seek(SeekFrom::Start(0))?;
        self.out
            .write_all(&header(WavFormat::F32, self.channels, self.rate, frames))?;
        self.out.flush()?;
        self.out.get_ref().sync_all()?;
        Ok(self.frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixteen_bits_round_to_even_and_clamp() {
        assert_eq!(to_s16(0.5), 16384);
        assert_eq!(to_s16(1.0), 32767);
        assert_eq!(to_s16(-1.0), -32768);
        assert_eq!(to_s16(2.0), 32767);
        assert_eq!(to_s16(0.5 / 32768.0), 0, "a tie goes to the even side");
        assert_eq!(to_s16(1.5 / 32768.0), 2);
        assert_eq!(
            to_s16(12345.0 / 32768.0),
            12345,
            "a 16-bit sample comes back as it was"
        );
    }

    #[test]
    fn a_float_file_says_its_length_twice_and_hashes_as_it_lies_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let mut w = WavWriter::create(&path, WavFormat::F32, 44100, 2).unwrap();
        w.write(&[0.25, -0.5], &[0.5, 1.0]).unwrap();
        let sha = w.finish().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), 58 + 16);
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
            bytes.len() - 8
        );
        assert_eq!(&bytes[38..42], b"fact");
        assert_eq!(u32::from_le_bytes(bytes[46..50].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(bytes[54..58].try_into().unwrap()), 16);
        assert_eq!(sha, hex::encode(Sha256::digest(&bytes)));
    }

    #[test]
    fn a_render_over_4_gb_is_refused_before_a_file_is_made() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.wav");
        assert!(WavWriter::create(&path, WavFormat::F32, 44100, 1 << 30).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn a_take_learns_its_length_when_it_ends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("take.wav");
        let mut t = TakeWriter::create(&path, 2, 48000).unwrap();
        t.write(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6]).unwrap();
        assert_eq!(t.finish().unwrap(), 3);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[54..58].try_into().unwrap()), 24);
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
            bytes.len() - 8
        );
    }
}
