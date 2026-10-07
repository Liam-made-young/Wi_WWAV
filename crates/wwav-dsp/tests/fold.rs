//! F9: the fold check fails if it passes a pair whose difference is over
//! −80 dBFS (`docs/PLAN.md`); S3.6 fails if a clipping stem isn't named. 6.6
//! gives the sentences.

use wwav_dsp::dither::to_16_bit;
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
    // A peak shows rounded up to a tenth, so each stem here sits just under
    // its tenth: one built at +1.8 can peak a hair over it, from f32
    // rounding, and read +1.9.
    stems[1] = tone(8_820, 55.0, 10f64.powf(1.795 / 20.0)); // drums at +1.8 dBFS
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
    // Just under each tenth, as above: built at +0.4 exactly, bass peaks at
    // +0.40000008 dBFS and reads +0.5.
    stems[3] = tone(8_820, 41.0, 10f64.powf(0.395 / 20.0)); // bass +0.4
    stems[0] = tone(8_820, 220.0, 10f64.powf(3.195 / 20.0)); // vocals +3.2
    stems[1] = tone(8_820, 55.0, 10f64.powf(1.795 / 20.0)); // drums +1.8
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

#[test]
fn a_difference_that_isnt_a_number_is_said_plainly() {
    // A plugin that fails can put NaN or infinity in the master.
    for bad in [f32::NAN, f32::INFINITY] {
        let (stems, mut master) = stems_and_sum();
        master[10] = bad;
        let check = fold_check(refs(&stems), &master);
        assert!(!check.passes());
        assert_eq!(
            check.line("Tape Sat on the drums reverb return"),
            "Stems don't sum to the master: some samples aren't numbers, \
             from Tape Sat on the drums reverb return."
        );
    }
}

#[test]
fn a_stem_with_samples_that_arent_numbers_is_named_without_a_cut() {
    // No cut fixes these, so they're named instead, before any clipping.
    let (mut stems, _) = stems_and_sum();
    stems[1][100] = f32::NAN;
    stems[0] = tone(8_820, 220.0, 10f64.powf(3.195 / 20.0)); // vocals +3.2
    let peaks = stem_peaks(refs(&stems));
    assert_eq!(peaks.dbfs()[1], f64::INFINITY);
    assert_eq!(
        peaks.cut_db(),
        4,
        "the cut still covers the stems that are numbers"
    );
    assert_eq!(
        peaks.lines().unwrap(),
        [
            "drums has samples that aren't numbers.".to_string(),
            "At 16 bits they would be written as silence or full scale.".to_string(),
        ]
    );
    stems[3][7] = f32::NEG_INFINITY;
    assert_eq!(
        peaks_line(&stems),
        "drums and bass have samples that aren't numbers."
    );
}

fn peaks_line(stems: &[Vec<f32>; 4]) -> String {
    stem_peaks(refs(stems)).lines().unwrap()[0].clone()
}

#[test]
fn a_stem_at_full_scale_is_written_within_the_dithers_own_error() {
    // 6.6 names stems over 0 dBFS. One at or just under it can have dither
    // push a sample past 32,767, where to_16_bit clamps it, unnamed. The
    // clamped sample is still within 1 LSB of its value, inside the 1.5 LSB
    // any dithered sample moves, so the sheet stays quiet about it.
    let quiet = vec![0.1f32; 4];
    for x in [1.0f32, 0.99999, 32_767.0 / 32_768.0, -1.0, -0.99999] {
        let stem = vec![x; 100_000];
        assert!(stem_peaks([&quiet, &stem, &quiet, &quiet])
            .lines()
            .is_none());
        let out = to_16_bit(&stem, 5);
        let worst = out
            .samples()
            .iter()
            .map(|&q| (q as f64 - x as f64 * 32_768.0).abs())
            .fold(0.0, f64::max);
        assert!(worst <= 1.0, "{x}: off by {worst} LSB");
    }
    // The reviewer's case, a constant 0.99999 (32,767.67 codes): dither
    // would carry about two thirds of its samples past 32,767, so every
    // sample is written as 32,767, 0.67 LSB under its value.
    let stem = vec![0.99999f32; 10_000];
    assert!(to_16_bit(&stem, 5).samples().iter().all(|&q| q == i16::MAX));
}
