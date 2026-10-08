//! `to_wav`: the WAV it writes is what the file decodes to, at the rate asked
//! for, with a header any parser reads; the same every run; and nothing is
//! left behind when it is cancelled or fails.

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use common::*;
use wwav_decode::{to_wav, Converted, Decoder, Error};
use wwav_dsp::resample::resampled_len;

fn convert(src: &Path, dst: &Path, rate: u32) -> Result<Converted, Error> {
    to_wav(src, dst, rate, &AtomicBool::new(false), &mut |_, _| {})
}

/// The WAV at `path` as the plainest parser reads it, after checking that
/// hound and this crate's own decoder read the same thing.
fn read_back(path: &Path) -> FloatWav {
    let wav = parse_float_wav(&fs::read(path).unwrap());

    let mut reader = hound::WavReader::open(path).unwrap();
    let spec = reader.spec();
    assert_eq!((spec.channels, spec.sample_rate), (wav.channels, wav.rate));
    assert_eq!(
        (spec.bits_per_sample, spec.sample_format),
        (32, hound::SampleFormat::Float)
    );
    let heard: Vec<f32> = reader.samples::<f32>().map(Result::unwrap).collect();
    assert_same(&heard, &wav.samples);

    let (info, decoded) = decode(path).unwrap();
    assert_eq!(
        (info.rate, info.channels, info.codec.as_str()),
        (wav.rate, wav.channels, "pcm")
    );
    assert_eq!(
        info.frames,
        Some((wav.samples.len() / wav.channels as usize) as u64)
    );
    assert_same(&decoded, &wav.samples);
    wav
}

#[test]
fn at_the_files_own_rate_the_wav_holds_the_decoded_samples_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("out.wav");

    let samples = random16(40_000, 4);
    let src = dir.path().join("in.wav");
    for (channels, rate) in [(1u16, 44_100u32), (2, 48_000)] {
        fs::write(&src, wav16(channels, rate, &samples)).unwrap();
        let done = convert(&src, &dst, rate).unwrap();
        let want = Converted {
            frames: 40_000 / channels as u64,
            channels,
            source_rate: rate,
            codec: "pcm".into(),
        };
        assert_eq!(done, want);
        let wav = read_back(&dst);
        assert_eq!((wav.channels, wav.rate), (channels, rate));
        assert_same(&wav.samples, &over_32768(&samples));
    }

    // Floats over full scale and a signed zero pass through as they are.
    let floats = [1.5f32, -3.25, -0.0, 0.0, f32::MIN_POSITIVE, 0.1];
    fs::write(&src, wav_float(2, 96_000, &floats)).unwrap();
    assert_eq!(convert(&src, &dst, 96_000).unwrap().frames, 3);
    assert_same(&read_back(&dst).samples, &floats);

    if have(LAME) {
        fs::write(&src, wav16(2, 48_000, &tone_and_sweep(48_000, 30_000))).unwrap();
        let mp3 = dir.path().join("in.mp3");
        run(Command::new(LAME).arg("--quiet").arg(&src).arg(&mp3));
        let done = convert(&mp3, &dst, 48_000).unwrap();
        assert_eq!((done.frames, done.codec.as_str()), (30_000, "mp3"));
        assert_same(&read_back(&dst).samples, &decode(&mp3).unwrap().1);
    }
}

#[test]
fn a_48k_file_comes_out_at_44100_on_pitch_and_at_its_level() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("in.wav"), dir.path().join("out.wav"));
    for (from, to, channels) in [(48_000u32, 44_100u32, 2usize), (44_100, 48_000, 1)] {
        let frames = from as usize * 3 / 2 + 17;
        let tone = sine(from, frames);
        // In stereo the right channel is the left upside down.
        let samples: Vec<f32> = tone
            .iter()
            .flat_map(|&s| [s, -s].into_iter().take(channels))
            .collect();
        fs::write(&src, wav_float(channels as u16, from, &samples)).unwrap();

        let done = convert(&src, &dst, to).unwrap();
        let wav = read_back(&dst);
        assert_eq!((wav.rate, wav.channels as usize), (to, channels));
        assert_eq!(done.frames, resampled_len(frames as u64, from, to));
        assert_eq!(wav.samples.len() as u64, done.frames * channels as u64);
        assert_eq!((done.source_rate, done.channels as usize), (from, channels));

        let left = channel(&wav.samples, channels, 0);
        let hz = peak_hz(&left, to);
        assert!(
            (hz - TONE_HZ).abs() < 0.1,
            "{from} → {to}: the tone is at {hz:.3} Hz"
        );
        let db = level_db(&left);
        assert!(
            db.abs() < 0.01,
            "{from} → {to}: the tone is {db:+.4} dB from its level"
        );

        // Frame k is the source at k ÷ rate seconds: no delay, no gain.
        let want = sine(to, left.len());
        let edge = 2_000;
        for k in edge..left.len() - edge {
            let off = (left[k] - want[k]).abs();
            assert!(
                off < 1e-4,
                "{from} → {to}: frame {k} is {off:e} from the sine"
            );
            if channels == 2 {
                assert_eq!(
                    wav.samples[2 * k + 1],
                    -left[k],
                    "the right channel at frame {k}"
                );
            }
        }
    }
}

#[test]
fn the_same_file_converts_to_the_same_bytes_every_time() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("in.wav");
    fs::write(&wav, wav16(2, 48_000, &tone_and_sweep(48_000, 30_011))).unwrap();
    let mut sources = vec![wav.clone()];
    if have(LAME) {
        let mp3 = dir.path().join("in.mp3");
        run(Command::new(LAME).arg("--quiet").arg(&wav).arg(&mp3));
        sources.push(mp3);
    }
    if have(AFCONVERT) {
        let m4a = dir.path().join("in.m4a");
        run(Command::new(AFCONVERT)
            .args(["-f", "m4af", "-d", "aac"])
            .arg(&wav)
            .arg(&m4a));
        sources.push(m4a);
    }
    for src in sources {
        for rate in [48_000, 44_100] {
            let (first, second) = (dir.path().join("first.wav"), dir.path().join("second.wav"));
            let a = convert(&src, &first, rate).unwrap();
            let b = convert(&src, &second, rate).unwrap();
            assert_eq!(a, b);
            assert_eq!(a.frames, resampled_len(30_011, 48_000, rate), "{src:?}");
            assert!(
                fs::read(&first).unwrap() == fs::read(&second).unwrap(),
                "{src:?} at {rate}"
            );
        }
    }
}

#[test]
fn a_cancelled_conversion_leaves_nothing_behind() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("in.wav"), dir.path().join("out.wav"));
    fs::write(&src, wav16(2, 48_000, &random16(400_000, 9))).unwrap();

    // Cancelled before it starts.
    let cancel = AtomicBool::new(true);
    let got = to_wav(&src, &dst, 44_100, &cancel, &mut |_, _| {});
    assert!(matches!(got, Err(Error::Cancelled)), "{got:?}");
    assert_eq!(names(dir.path()), ["in.wav"]);

    // Cancelled part-way, with some of the WAV already written.
    for rate in [48_000, 44_100] {
        let cancel = AtomicBool::new(false);
        let mut seen = 0;
        let got = to_wav(&src, &dst, rate, &cancel, &mut |done, _| {
            seen = done;
            cancel.store(done >= 100_000, Ordering::Relaxed);
        });
        assert!(matches!(got, Err(Error::Cancelled)), "{got:?}");
        assert!((100_000..200_000).contains(&seen), "stopped at {seen}");
        assert_eq!(names(dir.path()), ["in.wav"]);
    }

    // A file already at `dst` is still the file it was.
    fs::write(&dst, b"the last conversion").unwrap();
    let got = to_wav(&src, &dst, 44_100, &AtomicBool::new(true), &mut |_, _| {});
    assert!(matches!(got, Err(Error::Cancelled)), "{got:?}");
    assert_eq!(fs::read(&dst).unwrap(), b"the last conversion");
    assert_eq!(names(dir.path()), ["in.wav", "out.wav"]);
}

#[test]
fn progress_counts_source_frames_up_to_the_total() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("in.wav"), dir.path().join("out.wav"));
    // Four seconds and a bit.
    fs::write(&src, wav16(1, 44_100, &random16(180_000, 2))).unwrap();
    for rate in [44_100, 48_000] {
        let mut calls = Vec::new();
        let never = AtomicBool::new(false);
        to_wav(&src, &dst, rate, &never, &mut |done, total| {
            calls.push((done, total))
        })
        .unwrap();
        assert_eq!(calls.first(), Some(&(0, Some(180_000))));
        assert_eq!(calls.last(), Some(&(180_000, Some(180_000))));
        assert!(
            calls.windows(2).all(|pair| pair[0].0 <= pair[1].0),
            "{calls:?}"
        );
        // About one call for each second of sound, not one for each packet.
        assert!((5..=8).contains(&calls.len()), "{} calls", calls.len());
    }
}

#[test]
fn a_conversion_that_fails_writes_no_file_and_keeps_the_one_that_was_there() {
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("out.wav");
    let words = dir.path().join("words.mp3");
    fs::write(&words, "not sound\n".repeat(500)).unwrap();
    let wide = dir.path().join("wide.wav");
    fs::write(&wide, wav(1, 6, 48_000, 16, &[0; 12_000])).unwrap();
    let fine = dir.path().join("fine.wav");
    fs::write(&fine, wav16(1, 48_000, &random16(1_000, 1))).unwrap();
    let before = names(dir.path());

    for kept in [None, Some(&b"the last conversion"[..])] {
        if let Some(bytes) = kept {
            fs::write(&dst, bytes).unwrap();
        }
        let got = convert(&dir.path().join("gone.flac"), &dst, 48_000);
        assert!(matches!(got, Err(Error::NoSuchFile(_))), "{got:?}");
        let got = convert(&words, &dst, 48_000);
        assert!(matches!(got, Err(Error::Unsupported(_))), "{got:?}");
        let got = convert(&wide, &dst, 48_000);
        assert!(matches!(got, Err(Error::Unsupported(_))), "{got:?}");
        let got = convert(&fine, &dst, 0);
        assert!(matches!(got, Err(Error::Unsupported(_))), "{got:?}");
        assert_eq!(fs::read(&dst).ok().as_deref(), kept);
    }
    fs::remove_file(&dst).unwrap();

    // Nowhere to write it.
    let got = convert(&fine, &dir.path().join("no/such/folder/out.wav"), 48_000);
    assert!(matches!(got, Err(Error::Io(_))), "{got:?}");
    assert_eq!(names(dir.path()), before);
}

#[test]
fn a_new_conversion_replaces_the_old_file() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("in.wav"), dir.path().join("out.wav"));
    let samples = random16(9_000, 6);
    fs::write(&src, wav16(2, 44_100, &samples)).unwrap();
    fs::write(&dst, vec![7u8; 500_000]).unwrap();
    assert_eq!(convert(&src, &dst, 44_100).unwrap().frames, 4_500);
    assert_same(&read_back(&dst).samples, &over_32768(&samples));
    assert_eq!(names(dir.path()), ["in.wav", "out.wav"]);
}

#[test]
fn a_file_with_no_sound_converts_to_a_wav_with_none() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("in.wav"), dir.path().join("out.wav"));
    fs::write(&src, wav16(2, 48_000, &[])).unwrap();
    for rate in [48_000, 44_100] {
        assert_eq!(convert(&src, &dst, rate).unwrap().frames, 0);
        assert_eq!(fs::read(&dst).unwrap().len(), 44);
        let wav = read_back(&dst);
        assert_eq!((wav.rate, wav.channels, wav.samples.len()), (rate, 2, 0));
    }
}

/// The engine converts on a worker thread and reads `Decoder` there.
#[test]
fn a_decoder_reads_the_wav_while_it_is_open_elsewhere() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dst) = (dir.path().join("in.wav"), dir.path().join("out.wav"));
    fs::write(&src, wav16(1, 48_000, &random16(50_000, 8))).unwrap();
    let mut reading = Decoder::open(&src).unwrap();
    let worker = std::thread::spawn({
        let (src, dst) = (src.clone(), dst.clone());
        move || convert(&src, &dst, 44_100).unwrap().frames
    });
    let mut all = Vec::new();
    while reading.read(&mut all).unwrap() > 0 {}
    assert_eq!(all.len(), 50_000);
    assert_eq!(
        worker.join().unwrap(),
        resampled_len(50_000, 48_000, 44_100)
    );
}
