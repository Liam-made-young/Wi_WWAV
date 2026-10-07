//! A stereo WAVE writer that hashes what it writes, for `render`.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    /// 32-bit IEEE float.
    F32,
    /// 16-bit PCM.
    S16,
}

impl SampleFormat {
    fn bytes(self) -> u32 {
        match self {
            SampleFormat::F32 => 4,
            SampleFormat::S16 => 2,
        }
    }
}

/// The data chunk's size for `frames` stereo frames, if a WAVE file can
/// hold it: the RIFF chunk's size, 36 header bytes plus the data, is a u32.
pub fn data_bytes(frames: u64, format: SampleFormat) -> Option<u32> {
    let data = u32::try_from(frames.checked_mul(2 * format.bytes() as u64)?).ok()?;
    data.checked_add(36).map(|_| data)
}

pub struct WavWriter {
    out: BufWriter<File>,
    hash: Sha256,
    format: SampleFormat,
}

impl WavWriter {
    /// Writes the 44-byte header for `frames` stereo frames at `rate`.
    pub fn create(
        path: &Path,
        rate: u32,
        format: SampleFormat,
        frames: u64,
    ) -> io::Result<WavWriter> {
        let data = data_bytes(frames, format)
            .ok_or_else(|| io::Error::other("a render over 4 GB doesn't fit a WAVE file"))?;
        let block_align = 2 * format.bytes();
        let mut h = Vec::with_capacity(44);
        h.extend_from_slice(b"RIFF");
        h.extend_from_slice(&(36 + data).to_le_bytes());
        h.extend_from_slice(b"WAVEfmt ");
        h.extend_from_slice(&16u32.to_le_bytes());
        h.extend_from_slice(
            &(if format == SampleFormat::F32 {
                3u16
            } else {
                1u16
            })
            .to_le_bytes(),
        );
        h.extend_from_slice(&2u16.to_le_bytes());
        h.extend_from_slice(&rate.to_le_bytes());
        h.extend_from_slice(&(rate * block_align).to_le_bytes());
        h.extend_from_slice(&(block_align as u16).to_le_bytes());
        h.extend_from_slice(&(format.bytes() as u16 * 8).to_le_bytes());
        h.extend_from_slice(b"data");
        h.extend_from_slice(&data.to_le_bytes());
        let mut w = WavWriter {
            out: BufWriter::new(File::create(path)?),
            hash: Sha256::new(),
            format,
        };
        w.put(&h)?;
        Ok(w)
    }

    pub fn write(&mut self, frames: &[[f32; 2]]) -> io::Result<()> {
        let mut bytes = Vec::with_capacity(frames.len() * 2 * self.format.bytes() as usize);
        for s in frames.iter().flatten() {
            match self.format {
                SampleFormat::F32 => bytes.extend_from_slice(&s.to_le_bytes()),
                SampleFormat::S16 => bytes.extend_from_slice(
                    &((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes(),
                ),
            }
        }
        self.put(&bytes)
    }

    /// Finishes the file; returns its SHA-256 as lowercase hex.
    pub fn finish(mut self) -> io::Result<String> {
        self.out.flush()?;
        Ok(hex::encode(self.hash.finalize()))
    }

    fn put(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.hash.update(bytes);
        self.out.write_all(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_float_file_is_laid_out_as_wave() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let mut w = WavWriter::create(&path, 48000, SampleFormat::F32, 2).unwrap();
        w.write(&[[0.5, -0.5], [1.0, 0.0]]).unwrap();
        let sha = w.finish().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), 44 + 16);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 36 + 16);
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), 3, "IEEE float");
        assert_eq!(
            u32::from_le_bytes(bytes[28..32].try_into().unwrap()),
            48000 * 8
        );
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(f32::from_le_bytes(bytes[44..48].try_into().unwrap()), 0.5);
        assert_eq!(sha, hex::encode(Sha256::digest(&bytes)));
    }

    #[test]
    fn the_largest_render_that_fits_a_wave_file() {
        // The RIFF size, 36 + data, must fit a u32.
        let f32_max = (u32::MAX as u64 - 36) / 8;
        assert_eq!(
            data_bytes(f32_max, SampleFormat::F32),
            Some(f32_max as u32 * 8)
        );
        assert_eq!(data_bytes(f32_max + 1, SampleFormat::F32), None);
        assert_eq!(data_bytes(536_870_911, SampleFormat::F32), None);
        let s16_max = (u32::MAX as u64 - 36) / 4;
        assert!(data_bytes(s16_max, SampleFormat::S16).is_some());
        assert_eq!(data_bytes(s16_max + 1, SampleFormat::S16), None);
        assert_eq!(data_bytes(u64::MAX, SampleFormat::S16), None);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        assert!(WavWriter::create(&path, 48000, SampleFormat::F32, f32_max + 1).is_err());
        assert!(
            !path.exists(),
            "nothing is written for a render that can't fit"
        );
    }

    #[test]
    fn pcm_clamps_and_rounds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let mut w = WavWriter::create(&path, 44100, SampleFormat::S16, 2).unwrap();
        w.write(&[[2.0, -2.0], [0.5, 0.0]]).unwrap();
        w.finish().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), 1, "PCM");
        let s: Vec<i16> = bytes[44..]
            .chunks(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(s, [32767, -32767, 16384, 0]);
    }
}
