//! The tempo and key the Console estimates (5.4) and labels as estimates,
//! ported from `wwav/src/audio/analysis.js`. Fails if a click track's tempo or
//! a chord progression's key comes back wrong, or a label doesn't say
//! "estimate".

use wwav_dsp::analysis::{analyze, estimate_key, estimate_tempo, first_onset, Audio, Key, Stems};
use wwav_dsp::dither::to_16_bit;

const RATE: u32 = 44_100;

/// Stereo kick-like hits (a decaying 80 Hz sine) on every beat.
fn drums(bpm: f64, seconds: f64, first: f64) -> Vec<f32> {
    drums_at(RATE, bpm, seconds, first)
}

fn drums_at(rate: u32, bpm: f64, seconds: f64, first: f64) -> Vec<f32> {
    let kick: Vec<f32> = (0..(0.25 * rate as f64) as usize)
        .map(|i| {
            let t = i as f64 / rate as f64;
            (0.8 * (-t / 0.03).exp() * (2.0 * std::f64::consts::PI * 80.0 * t).sin()) as f32
        })
        .collect();
    let frames = (seconds * rate as f64) as usize;
    let mut out = vec![0.0f32; frames * 2];
    let beat = 60.0 / bpm;
    let mut t0 = first;
    while t0 < seconds {
        let start = (t0 * rate as f64).round() as usize;
        for (i, &s) in kick.iter().enumerate() {
            if start + i >= frames {
                break;
            }
            out[(start + i) * 2] = s;
            out[(start + i) * 2 + 1] = s;
        }
        t0 += beat;
    }
    out
}

/// Sine chords, two seconds each, given as semitones from A4 (440 Hz).
fn chords(progression: &[[i32; 3]], seconds: f64) -> Vec<f32> {
    let frames = (seconds * RATE as f64) as usize;
    let per = 2 * RATE as usize;
    (0..frames)
        .flat_map(|n| {
            let chord = progression[(n / per) % progression.len()];
            let t = n as f64 / RATE as f64;
            let s: f64 = chord
                .iter()
                .map(|&st| {
                    let f = 440.0 * 2f64.powf(st as f64 / 12.0);
                    0.2 * (2.0 * std::f64::consts::PI * f * t).sin()
                })
                .sum();
            [s as f32, s as f32]
        })
        .collect()
}

fn stereo(samples: &[f32]) -> Audio<'_> {
    Audio {
        samples,
        channels: 2,
        rate: RATE,
    }
}

/// Every half BPM in `half_bpms` ÷ 2, as 30 s of clicks at `rate`. From 71
/// to 145 BPM each reads within 1 BPM (the 512-sample hop limits it to about
/// that). Outside, the prior around 120 can pull a reading an octave toward
/// it, as analysis.js does: 60 reads 120.2 and 150 reads 74.9. The
/// figure-eight folds tempo ratios by octaves for exactly this. Every
/// reading stays within 60-180 and within 1 BPM of the tempo, its half or
/// its double.
fn sweep(rate: u32, half_bpms: std::ops::RangeInclusive<u32>) {
    for half_bpm in half_bpms {
        let bpm = half_bpm as f64 / 2.0;
        let d = drums_at(rate, bpm, 30.0, 0.5);
        let got = estimate_tempo(&[Audio {
            samples: &d,
            channels: 2,
            rate,
        }])
        .expect("a tempo");
        assert!((60.0..=180.0).contains(&got), "{bpm} BPM read as {got}");
        if (71.0..=145.0).contains(&bpm) {
            assert!(
                (got - bpm).abs() <= 1.0,
                "{bpm} BPM at {rate} read as {got}"
            );
        } else {
            let near = [0.5, 1.0, 2.0].iter().any(|k| (got * k - bpm).abs() <= 1.0);
            assert!(near, "{bpm} BPM at {rate} read as {got}");
        }
    }
}

#[test]
fn click_tracks_from_60_to_120_bpm_at_44_1_khz() {
    sweep(44_100, 120..=240);
}

#[test]
fn click_tracks_from_120_to_180_bpm_at_44_1_khz() {
    sweep(44_100, 241..=360);
}

#[test]
fn click_tracks_from_60_to_120_bpm_at_48_khz() {
    sweep(48_000, 120..=240);
}

#[test]
fn click_tracks_from_120_to_180_bpm_at_48_khz() {
    sweep(48_000, 241..=360);
}

#[test]
fn tempo_needs_two_of_the_slowest_beats() {
    // Under two beats at 60 BPM, the longest lags would rest on a product
    // or two, or on none: 175 hops of 512 samples, just over 2 s.
    let d = drums(120.0, 0.9, 0.0);
    assert_eq!(estimate_tempo(&[stereo(&d)]), None);
    assert_eq!(estimate_tempo(&[]), None);
    let d = drums(120.0, 3.0, 0.0);
    assert_eq!(estimate_tempo(&[stereo(&d[..2 * (175 * 512 - 1)])]), None);
    assert!(estimate_tempo(&[stereo(&d[..2 * 175 * 512])]).is_some());
}

#[test]
fn nothing_played_has_no_tempo() {
    // Silence, the dither on a silent bus (what a stem bus with no tracks
    // becomes at 16 bits), and a level that never moves have no onsets to
    // read; analysis.js reads all three as 184.6 BPM.
    let silence = vec![0.0f32; 2 * 30 * RATE as usize];
    let dithered: Vec<f32> = to_16_bit(&silence, 3)
        .samples()
        .iter()
        .map(|&q| q as f32 / 32_768.0)
        .collect();
    assert!(dithered.iter().any(|&x| x != 0.0));
    let level = vec![0.5f32; 2 * 30 * RATE as usize];
    for x in [&silence, &dithered, &level] {
        assert_eq!(estimate_tempo(&[stereo(x)]), None);
    }
}

#[test]
fn clicks_slower_than_the_range_have_no_tempo() {
    // At 55 BPM no two onsets are within a 60-180 BPM lag of each other, so
    // nothing correlates; analysis.js reads the first lag, 184.6 BPM.
    for rate in [44_100, 48_000] {
        let d = drums_at(rate, 55.0, 30.0, 0.5);
        let audio = Audio {
            samples: &d,
            channels: 2,
            rate,
        };
        assert_eq!(estimate_tempo(&[audio]), None);
    }
}

#[test]
fn key_of_chord_progressions() {
    // i iv V i in A minor, I IV V I in C major, i iv V i in F♯ minor.
    let a_minor = [[0, 3, 7], [5, 8, 12], [7, 11, 14], [0, 3, 7]];
    let c_major = [[3, 7, 10], [8, 12, 15], [10, 14, 17], [3, 7, 10]];
    let fs_minor = [[9, 12, 16], [14, 17, 21], [16, 20, 23], [9, 12, 16]];
    for (prog, want) in [
        (a_minor, "A minor"),
        (c_major, "C major"),
        (fs_minor, "F♯ minor"),
    ] {
        let audio = chords(&prog, 50.0);
        let got = estimate_key(&[stereo(&audio)]).expect("a key");
        assert_eq!(got.to_string(), want);
    }
}

#[test]
fn key_needs_sound() {
    let silence = vec![0.0f32; 2 * 10 * RATE as usize];
    assert_eq!(estimate_key(&[stereo(&silence)]), None);
}

#[test]
fn the_first_onset_is_where_the_drums_begin() {
    let d = drums(120.0, 10.0, 1.5);
    let at = first_onset(&stereo(&d)).expect("an onset");
    assert!((at - 1.5).abs() < 1e-9, "{at}");
    let silence = vec![0.0f32; 2 * RATE as usize];
    assert_eq!(first_onset(&stereo(&silence)), None);
}

#[test]
fn analyze_reads_tempo_from_drums_and_key_from_the_harmonic_stems() {
    let d = drums(86.0, 70.0, 0.75);
    let h = chords(&[[0, 3, 7], [5, 8, 12], [7, 11, 14], [0, 3, 7]], 70.0);
    let stems = Stems {
        vocals: Some(stereo(&h)),
        drums: Some(stereo(&d)),
        other: None,
        bass: None,
    };
    let e = analyze(&stems);
    assert!((e.bpm.unwrap() - 86.0).abs() <= 0.5);
    assert_eq!(e.key, Some(Key::parse("A minor").unwrap()));
    assert!((e.drum_onset - 0.74).abs() < 0.03, "{}", e.drum_onset);
    // 5.4: "≈ 86 BPM (estimate)", and never a bare number.
    assert_eq!(e.bpm_label().as_deref(), Some("≈ 86 BPM (estimate)"));
    assert_eq!(e.key_label().as_deref(), Some("A minor (estimate)"));
}

#[test]
fn nothing_to_read_gives_no_estimate() {
    let e = analyze(&Stems {
        vocals: None,
        drums: None,
        other: None,
        bass: None,
    });
    assert_eq!((e.bpm, e.key, e.drum_onset), (None, None, 0.0));
    assert_eq!((e.bpm_label(), e.key_label()), (None, None));
}

#[test]
fn keys_read_and_print_as_analysis_js_names_them() {
    let k = Key::parse("C♯ minor").unwrap();
    assert_eq!((k.pc, k.minor), (4, true));
    assert_eq!(k.to_string(), "C♯ minor");
    assert_eq!(Key::parse("A major").unwrap().pc, 0);
    assert_eq!(Key::parse("H minor"), None);
    assert_eq!(Key::parse(""), None);
    assert_eq!(k.transposed(-5).to_string(), "G♯ minor");
    assert_eq!(k.transposed(8).to_string(), "A minor");
}
