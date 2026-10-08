//! Compressed files, made at test time by the encoders on this Mac. The
//! lossless ones decode to exactly the samples they were made from; the
//! lossy ones come back the right length, on pitch, at their level and in
//! their place; and what this doesn't read is refused in a sentence.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::*;
use tempfile::TempDir;
use wwav_decode::{probe, Error};

struct Source {
    dir: TempDir,
    wav: PathBuf,
    channels: usize,
    rate: u32,
    frames: u64,
    /// What the WAV decodes to.
    samples: Vec<f32>,
}

impl Source {
    fn new(channels: usize, rate: u32, samples: &[i16]) -> Source {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("source.wav");
        fs::write(&wav, wav16(channels as u16, rate, samples)).unwrap();
        Source {
            dir,
            wav,
            channels,
            rate,
            frames: (samples.len() / channels) as u64,
            samples: over_32768(samples),
        }
    }

    /// Samples of every value, for the formats that must return them all.
    fn random(channels: usize, rate: u32) -> Source {
        Source::new(channels, rate, &random16(30_002, 21))
    }

    /// Two seconds and a bit: the tone on the left and the sweep on the
    /// right, or the tone alone in mono.
    fn signal(channels: usize) -> Source {
        let stereo = tone_and_sweep(48_000, 96_123);
        match channels {
            2 => Source::new(2, 48_000, &stereo),
            _ => Source::new(
                1,
                48_000,
                &stereo.iter().step_by(2).copied().collect::<Vec<_>>(),
            ),
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn afconvert(&self, args: &[&str], name: &str) -> PathBuf {
        let out = self.path(name);
        run(Command::new(AFCONVERT).args(args).arg(&self.wav).arg(&out));
        out
    }

    fn ffmpeg(&self, args: &[&str], name: &str) -> PathBuf {
        let out = self.path(name);
        ffmpeg(&self.wav, args, &out);
        out
    }

    fn lame(&self, args: &[&str], name: &str) -> PathBuf {
        let out = self.path(name);
        run(Command::new(LAME)
            .arg("--quiet")
            .args(args)
            .arg(&self.wav)
            .arg(&out));
        out
    }

    /// A lossless file gives back every sample, and says so up front.
    fn assert_exact(&self, file: &Path, codec: &str, says_length: bool) {
        let name = file.file_name().unwrap().to_string_lossy();
        let (info, got) = decode(file).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(info.codec, codec, "{name}");
        assert_eq!(
            (info.rate, info.channels as usize),
            (self.rate, self.channels),
            "{name}"
        );
        assert_eq!(info.frames, says_length.then_some(self.frames), "{name}");
        assert_same(&got, &self.samples);
        assert_eq!(probe(file).unwrap(), info, "{name}");
    }

    /// A lossy file is still the tone, at 1 kHz and within 1 dB of its
    /// level, and the sweep is where it was to the frame. Returns the frames
    /// decoded.
    fn assert_sounds_right(&self, file: &Path, codec: &str, in_place: bool) -> u64 {
        let name = file.file_name().unwrap().to_string_lossy();
        let (info, got) = decode(file).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(info.codec, codec, "{name}");
        assert_eq!(
            (info.rate, info.channels as usize),
            (self.rate, self.channels),
            "{name}"
        );
        let tone = channel(&got, self.channels, 0);
        let hz = peak_hz(&tone, info.rate);
        assert!(
            (hz - TONE_HZ).abs() < 1.0,
            "{name}: the tone is at {hz:.2} Hz"
        );
        let db = level_db(&tone);
        assert!(
            db.abs() < 1.0,
            "{name}: the tone is {db:+.2} dB from its level"
        );
        if in_place {
            let last = self.channels - 1;
            let (late, alike) = lag(
                &channel(&self.samples, self.channels, last),
                &channel(&got, self.channels, last),
            );
            assert_eq!(late, 0, "{name}: the sound is {late} frames late");
            assert!(alike > 0.99, "{name}: only {alike:.3} like its source");
        }
        let frames = (got.len() / self.channels) as u64;
        if let Some(said) = info.frames {
            assert_eq!(
                frames, said,
                "{name}: decoded a different length than it says"
            );
        }
        assert_eq!(probe(file).unwrap(), info, "{name}");
        frames
    }
}

#[test]
fn a_flac_decodes_to_the_samples_of_its_wav() {
    if !have(FLAC) {
        return;
    }
    for source in [Source::random(1, 44_100), Source::random(2, 48_000)] {
        let flac = source.path("clip.flac");
        run(Command::new(FLAC)
            .args(["-s", "-f", "-o"])
            .arg(&flac)
            .arg(&source.wav));
        source.assert_exact(&flac, "flac", true);
    }
}

#[test]
fn flac_in_matroska_and_in_ogg_decodes_to_the_samples_of_its_wav() {
    if !have(FFMPEG) {
        return;
    }
    let source = Source::random(2, 48_000);
    // Matroska counts its length in milliseconds, which is not a length in
    // frames.
    source.assert_exact(&source.ffmpeg(&["-c:a", "flac"], "clip.mka"), "flac", false);
    source.assert_exact(
        &source.ffmpeg(&["-c:a", "flac", "-f", "ogg"], "clip.oga"),
        "flac",
        true,
    );
}

#[test]
fn an_alac_file_decodes_to_the_samples_of_its_wav() {
    if !have(AFCONVERT) {
        return;
    }
    // Symphonia's MP4 reader calls every ALAC track 44.1 kHz; the rate here
    // is the decoder's.
    for source in [Source::random(1, 44_100), Source::random(2, 48_000)] {
        source.assert_exact(
            &source.afconvert(&["-f", "m4af", "-d", "alac"], "clip.m4a"),
            "alac",
            true,
        );
        source.assert_exact(
            &source.afconvert(&["-f", "caff", "-d", "alac"], "clip.caf"),
            "alac",
            true,
        );
    }
}

#[test]
fn an_aiff_and_a_caf_decode_to_the_samples_of_their_wav() {
    for source in [Source::random(1, 44_100), Source::random(2, 48_000)] {
        if have(AFCONVERT) {
            // 16 bits, and the same 16 bits in 24.
            source.assert_exact(
                &source.afconvert(&["-f", "AIFF", "-d", "BEI16"], "clip.aiff"),
                "pcm",
                true,
            );
            source.assert_exact(
                &source.afconvert(&["-f", "AIFF", "-d", "BEI24"], "wide.aiff"),
                "pcm",
                true,
            );
            source.assert_exact(
                &source.afconvert(&["-f", "caff", "-d", "LEI16"], "clip.caf"),
                "pcm",
                true,
            );
        }
        if have(FFMPEG) {
            // AIFF-C's little-endian kind, and ffmpeg's layout of each.
            source.assert_exact(
                &source.ffmpeg(&["-c:a", "pcm_s16le", "-f", "aiff"], "little.aifc"),
                "pcm",
                true,
            );
            source.assert_exact(
                &source.ffmpeg(&["-c:a", "pcm_s16be", "-f", "aiff"], "ffmpeg.aiff"),
                "pcm",
                true,
            );
            source.assert_exact(
                &source.ffmpeg(&["-c:a", "pcm_s16le", "-f", "caf"], "ffmpeg.caf"),
                "pcm",
                true,
            );
        }
    }
}

#[test]
fn an_mp3_comes_back_the_same_length_on_pitch_and_in_place() {
    if !have(LAME) {
        return;
    }
    // LAME's header says how much delay and padding the encoder added, and
    // the decoder takes exactly that off.
    for source in [Source::signal(2), Source::signal(1)] {
        let frames =
            source.assert_sounds_right(&source.lame(&[], "clip.mp3"), "mp3", source.channels == 2);
        assert_eq!(frames, source.frames);
    }
}

#[test]
fn an_mp3_with_no_lame_header_keeps_its_padding_and_says_no_length() {
    if !have(LAME) {
        return;
    }
    let source = Source::signal(2);
    let file = source.lame(&["-t"], "bare.mp3");
    assert_eq!(probe(&file).unwrap().frames, None);
    let frames = source.assert_sounds_right(&file, "mp3", false);
    // 1105 frames of delay in front, and padding to a whole packet behind.
    let extra = frames - source.frames;
    assert!(
        (1105..1105 + 2 * 1152).contains(&extra),
        "{extra} frames more than the source"
    );
}

#[test]
fn an_aac_file_comes_back_the_same_length_on_pitch_and_in_place() {
    // Apple's encoder says its delay in an iTunSMPB tag, and ffmpeg's in an
    // edit list. Symphonia applies neither; `Decoder` does.
    if have(AFCONVERT) {
        for source in [Source::signal(2), Source::signal(1)] {
            let file = source.afconvert(&["-f", "m4af", "-d", "aac"], "apple.m4a");
            assert_eq!(probe(&file).unwrap().frames, Some(source.frames));
            let frames = source.assert_sounds_right(&file, "aac", source.channels == 2);
            assert_eq!(frames, source.frames);
        }
    }
    if have(FFMPEG) {
        let source = Source::signal(2);
        for (args, name) in [
            (&["-c:a", "aac"][..], "ffmpeg.m4a"),
            (&["-c:a", "aac", "-f", "mp4"], "ffmpeg.mp4"),
        ] {
            let file = source.ffmpeg(args, name);
            assert_eq!(probe(&file).unwrap().frames, Some(source.frames));
            let frames = source.assert_sounds_right(&file, "aac", true);
            assert_eq!(frames, source.frames);
        }
    }
}

#[test]
fn a_raw_aac_stream_keeps_its_encoder_delay_and_says_no_length() {
    if !have(FFMPEG) {
        return;
    }
    let source = Source::signal(2);
    let file = source.ffmpeg(&["-c:a", "aac"], "clip.aac");
    assert_eq!(probe(&file).unwrap().frames, None);
    let frames = source.assert_sounds_right(&file, "aac", false);
    // A packet of delay in front, and padding to a whole packet behind.
    let extra = frames - source.frames;
    assert!(
        (1024..3 * 1024).contains(&extra),
        "{extra} frames more than the source"
    );
}

#[test]
fn a_vorbis_file_comes_back_on_pitch_and_as_long_as_it_says() {
    let Some(encoder) = vorbis_encoder() else {
        return;
    };
    let source = Source::signal(2);
    let file = source.ffmpeg(encoder, "clip.ogg");
    // ffmpeg's own encoder smears the sweep and rounds the length up to a
    // block; libvorbis, below, is held to more.
    let frames = source.assert_sounds_right(&file, "vorbis", false);
    assert_eq!(probe(&file).unwrap().frames, Some(frames));
    assert!(
        (source.frames..source.frames + 1024).contains(&frames),
        "{frames} frames"
    );
}

/// Symphonia 0.5.5 decodes a Vorbis stream that fits in one Ogg page to the
/// end of its last block, past where the page says the sound ends.
#[test]
fn a_vorbis_file_from_libvorbis_stops_where_it_says_and_sits_in_place() {
    let source = Source::signal(2);
    let Some(encoder) = libvorbis_encoder(source.dir.path()) else {
        return;
    };
    let file = source.path("clip.ogg");
    run(Command::new(&encoder).arg(&source.wav).arg(&file));
    let frames = source.assert_sounds_right(&file, "vorbis", true);
    assert_eq!(frames, source.frames);

    for (channels, frames) in [(1, 1_000), (2, 5_000), (1, 44_100), (2, 44_100)] {
        let stereo = tone_and_sweep(44_100, frames);
        let mono: Vec<i16> = stereo.iter().step_by(2).copied().collect();
        let short = Source::new(
            channels,
            44_100,
            if channels == 2 { &stereo } else { &mono },
        );
        let file = short.path("short.ogg");
        run(Command::new(&encoder).arg(&short.wav).arg(&file));
        let (info, got) = decode(&file).unwrap();
        assert_eq!(info.frames, Some(frames as u64));
        assert_eq!(
            got.len(),
            frames * channels,
            "{channels} channels, {frames} frames"
        );
    }
}

#[test]
fn adpcm_and_companded_wavs_decode_on_pitch_and_in_place() {
    if !have(FFMPEG) {
        return;
    }
    let source = Source::signal(2);
    for (encoder, codec, exact) in [
        ("pcm_mulaw", "mulaw", true),
        ("pcm_alaw", "alaw", true),
        // ADPCM comes in blocks, and the last is decoded whole.
        ("adpcm_ms", "adpcm", false),
        ("adpcm_ima_wav", "adpcm", false),
    ] {
        let file = source.ffmpeg(&["-c:a", encoder], &format!("{encoder}.wav"));
        let frames = source.assert_sounds_right(&file, codec, true);
        if exact {
            assert_eq!(frames, source.frames, "{encoder}");
        } else {
            assert!(
                (source.frames..source.frames + 2048).contains(&frames),
                "{encoder}: {frames}"
            );
        }
    }
}

#[test]
fn what_this_doesnt_read_is_refused_in_a_sentence() {
    let source = Source::signal(2);
    let mut refused = Vec::new();
    if have(FFMPEG) {
        refused.extend([
            (
                source.ffmpeg(&["-c:a", "libopus"], "clip.opus"),
                "clip.opus holds opus sound, which this doesn't read.",
            ),
            (
                source.ffmpeg(&["-c:a", "wmav2"], "clip.wma"),
                "clip.wma is not a format this reads.",
            ),
            (
                source.ffmpeg(&["-c:a", "mp2"], "clip.mp2"),
                "clip.mp2 holds mp2 sound, which this doesn't read.",
            ),
            (
                source.ffmpeg(&["-c:a", "pcm_s16le", "-f", "w64"], "clip.w64"),
                "clip.w64 is not a format this reads.",
            ),
            (
                source.ffmpeg(&["-c:a", "pcm_s16le", "-rf64", "always"], "big.wav"),
                "big.wav is not a format this reads.",
            ),
            (
                source.ffmpeg(&["-c:a", "pcm_s24le"], "pcm.mka"),
                "pcm.mka holds pcm sound in a form this doesn't read.",
            ),
        ]);
    }
    if have(AFCONVERT) {
        refused.extend([
            (
                source.afconvert(&["-f", "caff", "-d", "aac"], "aac.caf"),
                "aac.caf holds aac sound in a form this doesn't read.",
            ),
            // Apple writes a raw AAC stream with the MPEG-2 bit set, which
            // symphonia's reader doesn't take for one.
            (
                source.afconvert(&["-f", "adts", "-d", "aac"], "apple.aac"),
                "apple.aac is not a format this reads.",
            ),
        ]);
    }
    for (file, sentence) in refused {
        let got = probe(&file).unwrap_err();
        assert!(
            matches!(&got, Error::Unsupported(s) if s == sentence),
            "{got:?}"
        );
        assert_eq!(decode(&file).unwrap_err().to_string(), sentence);
    }
}

/// Two Ogg streams end to end are one file to a player and two songs to
/// this. The first is all its headers describe, so the second is only met
/// at the end of the first.
#[test]
fn a_chained_ogg_is_refused_when_its_second_stream_comes() {
    let Some(encoder) = vorbis_encoder() else {
        return;
    };
    let source = Source::signal(2);
    let one = fs::read(source.ffmpeg(encoder, "one.ogg")).unwrap();
    let chained = source.path("chained.ogg");
    fs::write(&chained, [one.clone(), one].concat()).unwrap();
    assert_eq!(probe(&chained).unwrap().codec, "vorbis");
    let got = decode(&chained).map(|(info, samples)| (info, samples.len()));
    let want = "chained.ogg is several streams one after another; this reads a single stream.";
    assert!(
        matches!(&got, Err(Error::Unsupported(s)) if s == want),
        "{got:?}"
    );
}

/// Each kind of file with bytes overwritten, runs zeroed, and its end cut
/// off, the same way every run.
#[test]
fn a_damaged_file_is_an_error_or_a_shorter_clip_never_a_panic() {
    let source = Source::new(2, 48_000, &tone_and_sweep(48_000, 24_000));
    let mut files = vec![source.wav.clone()];
    if have(FLAC) {
        let flac = source.path("clip.flac");
        run(Command::new(FLAC)
            .args(["-s", "-f", "-o"])
            .arg(&flac)
            .arg(&source.wav));
        files.push(flac);
    }
    if have(LAME) {
        files.push(source.lame(&[], "clip.mp3"));
    }
    if have(AFCONVERT) {
        files.push(source.afconvert(&["-f", "m4af", "-d", "aac"], "aac.m4a"));
        files.push(source.afconvert(&["-f", "m4af", "-d", "alac"], "alac.m4a"));
        files.push(source.afconvert(&["-f", "AIFF", "-d", "BEI16"], "clip.aiff"));
        files.push(source.afconvert(&["-f", "caff", "-d", "LEI16"], "clip.caf"));
    }
    if let Some(encoder) = vorbis_encoder() {
        files.push(source.ffmpeg(encoder, "clip.ogg"));
        files.push(source.ffmpeg(&["-c:a", "flac"], "clip.mka"));
    }

    let mut seed = 2026;
    let (mut played, mut refused, mut gave_up) = (0, 0, 0);
    for file in &files {
        let whole = fs::read(file).unwrap();
        let name = file.file_name().unwrap().to_string_lossy();
        let damaged = source.path(&format!("damaged-{name}"));
        for round in 0..60 {
            let mut bytes = whole.clone();
            let at = next(&mut seed) as usize % bytes.len();
            match round % 4 {
                0 => bytes.truncate(at),
                // The headers, where a wrong byte does the most.
                1 => {
                    for _ in 0..1 + round / 8 {
                        let at = next(&mut seed) as usize % bytes.len().min(512);
                        bytes[at] = next(&mut seed) as u8;
                    }
                }
                2 => {
                    for _ in 0..64 {
                        let at = next(&mut seed) as usize % bytes.len();
                        bytes[at] = next(&mut seed) as u8;
                    }
                }
                _ => {
                    let end = (at + 256).min(bytes.len());
                    bytes[at..end].fill(0);
                }
            }
            fs::write(&damaged, &bytes).unwrap();
            match decode(&damaged) {
                Ok((info, samples)) => {
                    assert!(
                        matches!(info.channels, 1 | 2),
                        "{name}, round {round}: {info:?}"
                    );
                    assert!(
                        samples.len() < 100 * source.samples.len(),
                        "{name}, round {round}"
                    );
                    played += 1;
                }
                Err(e) => {
                    let sentence = e.to_string();
                    assert!(sentence.ends_with('.'), "{name}, round {round}: {sentence}");
                    if sentence.ends_with("the decoder stopped on it.") {
                        gave_up += 1;
                    }
                    refused += 1;
                }
            }
        }
    }
    eprintln!(
        "{} kinds of file damaged 60 ways each: {played} still played, {refused} were refused, \
         {gave_up} of those after the decoder panicked",
        files.len()
    );
}
