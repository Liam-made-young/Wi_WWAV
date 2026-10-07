//! The tempo and key the Console estimates (5.4) and labels as estimates,
//! ported from `wwav/src/audio/analysis.js`. Fails if a click track's tempo or
//! a chord progression's key comes back wrong, or a label doesn't say
//! "estimate".

use wwav_dsp::analysis::{analyze, estimate_key, estimate_tempo, first_onset, Audio, Key, Stems};

const RATE: u32 = 44_100;

/// Stereo kick-like hits (a decaying 80 Hz sine) on every beat.
fn drums(bpm: f64, seconds: f64, first: f64) -> Vec<f32> {
    let frames = (seconds * RATE as f64) as usize;
    let mut out = vec![0.0f32; frames * 2];
    let beat = 60.0 / bpm;
    let mut t0 = first;
    while t0 < seconds {
        let start = (t0 * RATE as f64).round() as usize;
        for i in 0..(0.25 * RATE as f64) as usize {
            if start + i >= frames {
                break;
            }
            let t = i as f64 / RATE as f64;
            let s = 0.8 * (-t / 0.03).exp() * (2.0 * std::f64::consts::PI * 80.0 * t).sin();
            out[(start + i) * 2] = s as f32;
            out[(start + i) * 2 + 1] = s as f32;
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

#[test]
fn tempo_of_click_tracks_to_within_a_bpm() {
    // The 512-sample hop limits the estimate to about a beat a minute.
    for bpm in (76..=160).step_by(4).chain([86]) {
        let d = drums(bpm as f64, 70.0, 0.5);
        let got = estimate_tempo(&[stereo(&d)]).expect("a tempo");
        assert!((got - bpm as f64).abs() <= 1.0, "{bpm} BPM read as {got}");
    }
}

#[test]
fn tempo_at_the_ends_of_the_range_may_read_an_octave_away() {
    // The prior around 120 pulls 60 up to 120.2 and 176 down to 87.8, as
    // analysis.js does; the figure-eight folds tempo ratios by octaves for
    // exactly this.
    for bpm in [60, 64, 68, 72, 164, 168, 172, 176, 180] {
        let d = drums(bpm as f64, 70.0, 0.5);
        let got = estimate_tempo(&[stereo(&d)]).expect("a tempo");
        let near = [0.5, 1.0, 2.0]
            .iter()
            .any(|k| (got * k - bpm as f64).abs() <= 1.0);
        assert!(near, "{bpm} BPM read as {got}");
    }
}

#[test]
fn tempo_needs_enough_audio() {
    let d = drums(120.0, 0.9, 0.0);
    assert_eq!(estimate_tempo(&[stereo(&d)]), None);
    assert_eq!(estimate_tempo(&[]), None);
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
