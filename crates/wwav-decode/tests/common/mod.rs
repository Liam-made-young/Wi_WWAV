//! Fixtures made at test time and the measurements the tests share. Nothing
//! binary is kept in the repo: WAVs are written by hand here, and compressed
//! files come from the encoders on this Mac, each test skipping itself with a
//! note when its encoder is missing.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use wwav_decode::{Decoder, Error, Info};

pub const AFCONVERT: &str = "/usr/bin/afconvert";
pub const FLAC: &str = "/opt/homebrew/bin/flac";
pub const LAME: &str = "/opt/homebrew/bin/lame";
pub const FFMPEG: &str = "/opt/homebrew/bin/ffmpeg";

/// Whether an encoder is on this machine, with a note when it isn't.
pub fn have(tool: &str) -> bool {
    let there = Path::new(tool).exists();
    if !there {
        eprintln!("skipped: no {tool}");
    }
    there
}

pub fn run(command: &mut Command) {
    let out = command.output().unwrap();
    assert!(out.status.success(), "{command:?}: {out:?}");
}

/// ffmpeg takes a hand-written WAV of a sine for an MPEG transport stream
/// unless it is told the format.
pub fn ffmpeg(src: &Path, args: &[&str], dst: &Path) {
    run(Command::new(FFMPEG)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "wav",
            "-i",
        ])
        .arg(src)
        .args(args)
        .arg(dst));
}

/// The arguments for whichever Vorbis encoder this ffmpeg has: libvorbis, or
/// its own, which it calls experimental and which only does stereo.
pub fn vorbis_encoder() -> Option<&'static [&'static str]> {
    if !have(FFMPEG) {
        return None;
    }
    let out = Command::new(FFMPEG)
        .args(["-hide_banner", "-encoders"])
        .output()
        .unwrap();
    let list = String::from_utf8_lossy(&out.stdout).into_owned();
    if list.contains(" libvorbis ") {
        Some(&["-c:a", "libvorbis"])
    } else if list.contains(" vorbis ") {
        Some(&["-c:a", "vorbis", "-strict", "-2"])
    } else {
        eprintln!("skipped: {FFMPEG} has no Vorbis encoder");
        None
    }
}

/// The reference Vorbis encoder, built from `vorbis_encode.c` into `dir`
/// when Homebrew's libvorbis is here: `encoder in.wav out.ogg`.
pub fn libvorbis_encoder(dir: &Path) -> Option<PathBuf> {
    const HOMEBREW: &str = "/opt/homebrew";
    if !Path::new(HOMEBREW)
        .join("include/vorbis/vorbisenc.h")
        .exists()
    {
        eprintln!("skipped: no libvorbis in {HOMEBREW} to encode with");
        return None;
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/vorbis_encode.c");
    let encoder = dir.join("vorbis_encode");
    run(Command::new("cc")
        .arg(source)
        .arg(format!("-I{HOMEBREW}/include"))
        .arg(format!("-L{HOMEBREW}/lib"))
        .args(["-lvorbisenc", "-lvorbis", "-logg", "-o"])
        .arg(&encoder));
    Some(encoder)
}

/// A WAV with a 16-byte `fmt `, or the 40-byte extensible one when `tag` is
/// 0xFFFE (as PCM, with a speaker for each channel).
pub fn wav(tag: u16, channels: u16, rate: u32, bits: u16, data: &[u8]) -> Vec<u8> {
    let frame = channels * bits / 8;
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&tag.to_le_bytes());
    fmt.extend_from_slice(&channels.to_le_bytes());
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * frame as u32).to_le_bytes());
    fmt.extend_from_slice(&frame.to_le_bytes());
    fmt.extend_from_slice(&bits.to_le_bytes());
    if tag == 0xFFFE {
        fmt.extend_from_slice(&22u16.to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());
        fmt.extend_from_slice(&((1u32 << channels) - 1).to_le_bytes());
        fmt.extend_from_slice(&[
            1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71,
        ]);
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((20 + fmt.len() + data.len()) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    out.extend_from_slice(&fmt);
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    out
}

pub fn wav16(channels: u16, rate: u32, samples: &[i16]) -> Vec<u8> {
    let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    wav(1, channels, rate, 16, &data)
}

pub fn wav_float(channels: u16, rate: u32, samples: &[f32]) -> Vec<u8> {
    let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    wav(3, channels, rate, 32, &data)
}

/// The next of a fixed run of numbers, so every run tests the same samples.
pub fn next(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *seed >> 8
}

/// Samples over the whole 16-bit range, both ends included.
pub fn random16(count: usize, mut seed: u32) -> Vec<i16> {
    let mut samples: Vec<i16> = (0..count).map(|_| next(&mut seed) as i16).collect();
    samples[0] = i16::MIN;
    samples[1] = i16::MAX;
    samples
}

/// What 16-bit samples decode to.
pub fn over_32768(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&v| v as f32 / 32768.0).collect()
}

pub const TONE_HZ: f64 = 1000.0;
pub const LEVEL: f64 = 0.5;

/// Stereo: a 1 kHz sine on the left, to measure pitch and level by, and a
/// sweep from 200 Hz to 4 kHz on the right, which lines up with itself at
/// one lag only, to measure where the sound sits. Both at half scale.
pub fn tone_and_sweep(rate: u32, frames: usize) -> Vec<i16> {
    let length = frames as f64 / rate as f64;
    let mut out = Vec::with_capacity(frames * 2);
    for n in 0..frames {
        let t = n as f64 / rate as f64;
        let tone = (std::f64::consts::TAU * TONE_HZ * t).sin();
        let sweep = (std::f64::consts::TAU * (200.0 * t + 3800.0 * t * t / (2.0 * length))).sin();
        out.push((tone * LEVEL * 32767.0).round() as i16);
        out.push((sweep * LEVEL * 32767.0).round() as i16);
    }
    out
}

pub fn sine(rate: u32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|n| (LEVEL * (std::f64::consts::TAU * TONE_HZ * n as f64 / rate as f64).sin()) as f32)
        .collect()
}

/// One channel of interleaved frames.
pub fn channel(samples: &[f32], channels: usize, which: usize) -> Vec<f32> {
    samples
        .iter()
        .skip(which)
        .step_by(channels)
        .copied()
        .collect()
}

/// Everything a file decodes to.
pub fn decode(path: &Path) -> Result<(Info, Vec<f32>), Error> {
    let mut decoder = Decoder::open(path)?;
    let info = decoder.info().clone();
    let mut samples = Vec::new();
    loop {
        let before = samples.len();
        let frames = decoder.read(&mut samples)?;
        assert_eq!(samples.len() - before, frames * info.channels as usize);
        if frames == 0 {
            // The end stays the end.
            assert_eq!(decoder.read(&mut samples)?, 0);
            return Ok((info, samples));
        }
    }
}

/// The loudest frequency in the middle of a mono signal, to a fraction of a
/// bin by fitting a parabola to the peak.
pub fn peak_hz(mono: &[f32], rate: u32) -> f64 {
    const N: usize = 1 << 15;
    assert!(
        mono.len() >= N,
        "{} samples is too few to measure",
        mono.len()
    );
    let from = (mono.len() - N) / 2;
    let mut bins: Vec<Complex<f64>> = mono[from..from + N]
        .iter()
        .enumerate()
        .map(|(n, &x)| {
            let hann = 0.5 - 0.5 * (std::f64::consts::TAU * n as f64 / N as f64).cos();
            Complex::new(x as f64 * hann, 0.0)
        })
        .collect();
    FftPlanner::new().plan_fft_forward(N).process(&mut bins);
    let db: Vec<f64> = bins[..N / 2]
        .iter()
        .map(|c| c.norm().max(1e-30).ln())
        .collect();
    let peak = (1..N / 2 - 1)
        .max_by(|&a, &b| db[a].total_cmp(&db[b]))
        .unwrap();
    let (l, m, r) = (db[peak - 1], db[peak], db[peak + 1]);
    (peak as f64 + 0.5 * (l - r) / (l - 2.0 * m + r)) * rate as f64 / N as f64
}

/// The level of the middle half of a mono signal against a sine at `LEVEL`,
/// in dB.
pub fn level_db(mono: &[f32]) -> f64 {
    let middle = &mono[mono.len() / 4..mono.len() * 3 / 4];
    let mean = middle.iter().map(|&x| x as f64 * x as f64).sum::<f64>() / middle.len() as f64;
    20.0 * (mean.sqrt() / (LEVEL / std::f64::consts::SQRT_2)).log10()
}

/// How many frames late `got` is against `want`, from −64 to 64, and how
/// alike the two are at that lag (1 is identical).
pub fn lag(want: &[f32], got: &[f32]) -> (i64, f64) {
    const SPAN: usize = 8192;
    let from = want.len().min(got.len()) / 2 - SPAN / 2;
    let a = &want[from..from + SPAN];
    let alike = |lag: i64| {
        let b = &got[(from as i64 + lag) as usize..][..SPAN];
        let dot: f64 = a.iter().zip(b).map(|(&x, &y)| x as f64 * y as f64).sum();
        let (aa, bb): (f64, f64) = a.iter().zip(b).fold((0.0, 0.0), |(aa, bb), (&x, &y)| {
            (aa + x as f64 * x as f64, bb + y as f64 * y as f64)
        });
        dot / (aa * bb).sqrt().max(1e-30)
    };
    (-64..=64)
        .map(|lag| (lag, alike(lag)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
}

/// The names in a folder, sorted.
pub fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

pub struct FloatWav {
    pub channels: u16,
    pub rate: u32,
    pub samples: Vec<f32>,
}

/// Reads a WAV the way the plainest parser would: 44 bytes of header at
/// fixed places, every count in it checked against the file.
pub fn parse_float_wav(bytes: &[u8]) -> FloatWav {
    let u16_at = |at: usize| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(u32_at(4) as usize, bytes.len() - 8, "RIFF size");
    assert_eq!(&bytes[8..16], b"WAVEfmt ");
    assert_eq!(u32_at(16), 16, "fmt size");
    assert_eq!(u16_at(20), 3, "format tag: float");
    let (channels, rate) = (u16_at(22), u32_at(24));
    assert_eq!(u32_at(28), rate * channels as u32 * 4, "bytes a second");
    assert_eq!(u16_at(32), channels * 4, "bytes a frame");
    assert_eq!(u16_at(34), 32, "bits");
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(u32_at(40) as usize, bytes.len() - 44, "data size");
    assert_eq!(
        (bytes.len() - 44) % (channels as usize * 4),
        0,
        "whole frames"
    );
    let samples = bytes[44..]
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    FloatWav {
        channels,
        rate,
        samples,
    }
}

/// Bit for bit, so that −0.0 and 0.0 differ and the first difference is
/// named.
pub fn assert_same(got: &[f32], want: &[f32]) {
    assert_eq!(got.len(), want.len(), "sample count");
    for (n, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g.to_bits(), w.to_bits(), "sample {n}: {g} is not {w}");
    }
}
