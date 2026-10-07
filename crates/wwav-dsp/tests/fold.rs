//! F9: the fold check fails if it passes a pair whose difference is over
//! −80 dBFS (`docs/PLAN.md`); S3.6 fails if a clipping stem isn't named. 6.6
//! gives the sentences.

use wwav_dsp::fold::{fold_check, stem_peaks};

fn tone(len: usize, freq: f64, amp: f64) -> Vec<f32> {
    (0..len)
        .map(|n| (amp * (2.0 * std::f64::consts::PI * freq * n as f64 / 44_100.0).sin()) as f32)
        .collect()
}

/// Four stems of stereo audio and their exact sum.
fn stems_and_sum() -> ([Vec<f32>; 4], Vec<f32>) {
    let stems = [
        tone(8_820, 220.0, 0.2),
        tone(8_820, 55.0, 0.3),
        tone(8_820, 660.0, 0.1),
        tone(8_820, 41.0, 0.25),
    ];
    let sum = (0..8_820)
        .map(|i| stems.iter().map(|s| s[i] as f64).sum::<f64>() as f32)
        .collect();
    (stems, sum)
}

fn refs(stems: &[Vec<f32>; 4]) -> [&[f32]; 4] {
    [&stems[0], &stems[1], &stems[2], &stems[3]]
}

#[test]
fn stems_that_sum_to_the_master_pass() {
    let (stems, master) = stems_and_sum();
    let check = fold_check(refs(&stems), &master);
    assert!(check.passes());
    assert!(check.peak_dbfs() < -120.0, "{}", check.peak_dbfs());
    assert_eq!(check.line("anything"), "Stems sum to the master.");
}

#[test]
fn a_difference_just_over_minus_80_dbfs_fails_and_names_the_cause() {
    let (stems, mut master) = stems_and_sum();
    master[4_000] += 1.2e-4; // −78.4 dBFS, in one sample
    let check = fold_check(refs(&stems), &master);
    assert!(!check.passes());
    assert!(
        (check.peak_dbfs() - -78.4).abs() < 0.05,
        "{}",
        check.peak_dbfs()
    );
    assert_eq!(
        check.line("Tape Sat on the drums reverb return"),
        "Stems don't sum to the master: −78.4 dBFS apart, from Tape Sat on the drums reverb return."
    );
}

#[test]
fn a_difference_just_under_minus_80_dbfs_passes() {
    let (stems, mut master) = stems_and_sum();
    master[4_000] += 0.9e-4; // −80.9 dBFS
    assert!(fold_check(refs(&stems), &master).passes());
}

#[test]
fn a_non_linear_return_fails() {
    // A soft clipper after the sum, as a saturating plugin on a return does.
    let (stems, sum) = stems_and_sum();
    let master: Vec<f32> = sum.iter().map(|&s| (s as f64).tanh() as f32).collect();
    let check = fold_check(refs(&stems), &master);
    assert!(!check.passes());
    assert!(check.peak_dbfs() > -40.0);
}

#[test]
fn exactly_minus_80_dbfs_is_not_under_it() {
    let (stems, mut master) = stems_and_sum();
    master[100] = (master[100] as f64 + 1e-4) as f32;
    let check = fold_check(refs(&stems), &master);
    // f32 rounding moves the step by a hair either way; only "under" passes.
    assert_eq!(check.passes(), check.peak_dbfs() < -80.0);
}

#[test]
fn a_stem_over_full_scale_is_named_with_the_cut_rounded_up_to_a_whole_db() {
    let (mut stems, _) = stems_and_sum();
    stems[1] = tone(8_820, 55.0, 10f64.powf(1.8 / 20.0)); // drums at +1.8 dBFS
    let peaks = stem_peaks(refs(&stems));
    assert!((peaks.dbfs()[1] - 1.8).abs() < 0.01);
    assert_eq!(peaks.cut_db(), 2);
    assert_eq!(
        peaks.lines().unwrap(),
        [
            "drums peaks at +1.8 dBFS. Lower all four stems by 2 dB.".to_string(),
            "The master is unchanged. A player summing the stems will play 2 dB quieter."
                .to_string(),
        ]
    );
}

#[test]
fn every_clipping_stem_is_named_in_file_order_and_the_loudest_sets_the_cut() {
    let (mut stems, _) = stems_and_sum();
    stems[3] = tone(8_820, 41.0, 10f64.powf(0.4 / 20.0)); // bass +0.4
    stems[0] = tone(8_820, 220.0, 10f64.powf(3.2 / 20.0)); // vocals +3.2
    stems[1] = tone(8_820, 55.0, 10f64.powf(1.8 / 20.0)); // drums +1.8
    let peaks = stem_peaks(refs(&stems));
    assert_eq!(peaks.cut_db(), 4);
    assert_eq!(
        peaks.lines().unwrap()[0],
        "vocals peaks at +3.2 dBFS, drums at +1.8 dBFS and bass at +0.4 dBFS. \
         Lower all four stems by 4 dB."
    );
}

#[test]
fn stems_under_full_scale_say_nothing() {
    let (stems, _) = stems_and_sum();
    let peaks = stem_peaks(refs(&stems));
    assert_eq!(peaks.cut_db(), 0);
    assert!(peaks.lines().is_none());
    assert!((peaks.dbfs()[0] - 20.0 * 0.2f64.log10()).abs() < 0.01);
}

#[test]
fn a_silent_stem_has_no_peak() {
    let (mut stems, _) = stems_and_sum();
    stems[2] = vec![0.0; 8_820];
    assert_eq!(stem_peaks(refs(&stems)).dbfs()[2], f64::NEG_INFINITY);
}
