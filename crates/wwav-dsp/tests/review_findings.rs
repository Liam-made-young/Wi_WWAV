//! Reviewer's tests (build/dsp review, 2026-10-07). Each one exposes a defect
//! found in review and is `#[ignore]`d until it is fixed; the comment on each
//! names the finding. Run them with `cargo test -p wwav-dsp --test
//! review_findings -- --ignored`.

use wwav_dsp::analysis::{analyze, estimate_tempo, Audio, Estimates, Key, Stems};
use wwav_dsp::fold::stem_peaks;
use wwav_dsp::matching::match_to_parent;
use wwav_dsp::sheet::{refusal, size_line, wwav_bytes};

const RATE: u32 = 44_100;

fn stereo(samples: &[f32]) -> Audio<'_> {
    Audio {
        samples,
        channels: 2,
        rate: RATE,
    }
}

/// Finding (high, 5.4): silence reads as "≈ 185 BPM (estimate)". With no
/// onsets every lag scores 0, the first lag (28 hops) wins, and the tempo
/// comes out as 184.6 BPM, outside 60–180 and from nothing. analysis.js does
/// the same, so the port is faithful, but a drum bus with no tracks is
/// "written as silence" (6.6), so any drumless session or song gets an
/// invented tempo, and the figure-eight then bends a drumless child by
/// 184.6 ÷ the parent's tempo instead of leaving its rate alone.
#[test]
#[ignore = "finding: silence is estimated at 184.6 BPM"]
fn silence_has_no_tempo() {
    let silent = vec![0.0f32; 2 * 30 * RATE as usize];
    assert_eq!(estimate_tempo(&[stereo(&silent)]), None);

    let pad: Vec<f32> = (0..30 * RATE as usize)
        .flat_map(|n| {
            let s = 0.2 * (2.0 * std::f64::consts::PI * 220.0 * n as f64 / RATE as f64).sin();
            [s as f32, s as f32]
        })
        .collect();
    let child = analyze(&Stems {
        vocals: Some(stereo(&pad)),
        drums: Some(stereo(&silent)),
        other: None,
        bass: None,
    });
    assert_eq!(child.bpm_label(), None, "a silent drum stem has no tempo");

    let parent = Estimates {
        bpm: Some(120.0),
        key: Some(Key::parse("A minor").unwrap()),
        drum_onset: 0.5,
    };
    // A child with no tempo keeps its own rate (matching.rs: "A missing tempo
    // or key leaves that part alone").
    assert_eq!(match_to_parent(&parent, &child).rel_rate, 1.0);
}

/// Finding (medium, 5.4): a clip of 87 × 512 samples (1.01 s) has 86 onset
/// frames, so the autocorrelation at lag 86 is 0 ÷ 0. When lag 85 wins, the
/// parabolic refinement reads that NaN and the estimate is NaN, shown as
/// "≈ NaN BPM (estimate)". analysis.js returns NaN here too.
#[test]
#[ignore = "finding: a 1.01 s clip is estimated at NaN BPM"]
fn a_short_clip_never_reads_nan_bpm() {
    let mut x = vec![0.0f32; 87 * 512];
    for i in 0..512 {
        x[i] = 0.5;
        x[85 * 512 + i] = 0.5;
    }
    let clip = Audio {
        samples: &x,
        channels: 1,
        rate: RATE,
    };
    let bpm = estimate_tempo(&[clip]);
    assert!(bpm.map_or(true, f64::is_finite), "estimated {bpm:?}");
    let e = analyze(&Stems {
        vocals: None,
        drums: Some(clip),
        other: None,
        bass: None,
    });
    assert_ne!(e.bpm_label().as_deref(), Some("≈ NaN BPM (estimate)"));
}

/// Finding (medium, F9 and 6.7's table, read literally): "a session over 81
/// minutes | refused". The sheet refuses only past the 4 GB byte limit, which
/// is 81:09, so a session of 81:00–81:09 is accepted and reads "81:05 →
/// about 4.3 GB.".
#[test]
#[ignore = "finding: sessions of 81:00 to 81:09 are not refused"]
fn a_session_just_over_81_minutes_is_refused() {
    let frames = (81 * 60 + 5) * RATE as u64;
    let bytes = wwav_bytes(frames, 195, 115, None);
    assert_eq!(size_line(frames, bytes), "81:05 → about 4.3 GB.");
    assert_eq!(
        refusal(frames, bytes).as_deref(),
        Some("A .wwav holds about 81 minutes. This session is 82.")
    );
}

/// Finding (low, 6.6): the clipping line rounds the shown peak to a tenth
/// but the cut up from the exact peak, so a stem just over full scale reads
/// "drums peaks at 0.0 dBFS" (not over 0 dBFS at all), and one at +2.04
/// reads "+2.0 dBFS. Lower all four stems by 3 dB.". A non-finite peak reads
/// "+inf dBFS. Lower all four stems by 4294967295 dB.".
#[test]
#[ignore = "finding: clipping lines can contradict their own cut"]
fn the_shown_peak_agrees_with_the_cut() {
    let quiet = vec![0.1f32; 3];
    for db in [0.01f64, 0.04, 2.04] {
        let loud = vec![0.1f32, 10f64.powf(db / 20.0) as f32, -0.2];
        let peaks = stem_peaks([&quiet, &loud, &quiet, &quiet]);
        let line = &peaks.lines().unwrap()[0];
        let shown: f64 = line
            .split(" at +")
            .nth(1)
            .and_then(|r| r.split(' ').next())
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| panic!("{db}: the peak isn't shown as over 0: {line}"));
        assert_eq!(
            shown.ceil() as u32,
            peaks.cut_db(),
            "{db}: shown and cut disagree: {line}"
        );
    }
    let blown = vec![0.1f32, f32::INFINITY];
    let quiet = vec![0.1f32; 2];
    let peaks = stem_peaks([&quiet, &blown, &quiet, &quiet]);
    assert!(
        !peaks.lines().unwrap()[0].contains("4294967295"),
        "{:?}",
        peaks.lines()
    );
}

/// Finding (low, 5.4 and `Estimates::bpm`'s "60–180 BPM"): the estimate can
/// leave the range the spec gives it. The lags run from ⌊60 ÷ 180 × fps⌋ to
/// ⌈fps⌉, so the extremes are 184.6 and 59.4 BPM at 44.1 kHz, and a clean
/// 180 BPM click at 48 kHz reads 181.5, shown as "≈ 182 BPM (estimate)".
/// analysis.js gives the same (the port is faithful).
#[test]
#[ignore = "finding: a 180 BPM click at 48 kHz is estimated at 181.5 BPM"]
fn a_tempo_estimate_stays_within_60_to_180_bpm() {
    let rate = 48_000u32;
    let seconds = 70.0;
    let frames = (seconds * rate as f64) as usize;
    let mut x = vec![0.0f32; frames * 2];
    let mut t0 = 0.5;
    while t0 < seconds {
        let start = (t0 * rate as f64).round() as usize;
        for i in 0..(0.25 * rate as f64) as usize {
            if start + i >= frames {
                break;
            }
            let t = i as f64 / rate as f64;
            let s = 0.8 * (-t / 0.03).exp() * (2.0 * std::f64::consts::PI * 80.0 * t).sin();
            x[(start + i) * 2] = s as f32;
            x[(start + i) * 2 + 1] = s as f32;
        }
        t0 += 60.0 / 180.0;
    }
    let bpm = estimate_tempo(&[Audio {
        samples: &x,
        channels: 2,
        rate,
    }])
    .expect("a tempo");
    assert!((60.0..=180.0).contains(&bpm), "estimated {bpm} BPM");
}
