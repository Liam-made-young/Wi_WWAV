//! F9: dither fails if it is applied more than once (`docs/PLAN.md`). 6.7:
//! TPDF dither once, at the last step, without noise shaping. That the API
//! can't take its own output again is the compile-fail example on
//! `wwav_dsp::dither::to_16_bit`.

use wwav_dsp::dither::to_16_bit;

const FULL_SCALE: f64 = 32_768.0;

/// A sine just under full scale, so dither never needs to clip it.
fn sine(len: usize) -> Vec<f32> {
    let amp = 32_766.0 / FULL_SCALE;
    (0..len)
        .map(|n| (amp * (2.0 * std::f64::consts::PI * 997.0 * n as f64 / 44_100.0).sin()) as f32)
        .collect()
}

#[test]
fn a_full_scale_sines_error_is_tpdf_and_rounding_and_nothing_else() {
    let x = sine(1 << 20);
    let y = to_16_bit(&x, 1);
    assert_eq!(y.samples().len(), x.len());

    // The error against the exact value, in LSBs, grouped by where the exact
    // value falls between two codes. TPDF makes its mean and power the same
    // in every group: no distortion and no noise that follows the signal.
    // Rounding alone gives 1/12 LSB² and the triangle 1/6, 1/4 in all.
    let groups = 8;
    let mut sum = vec![0.0; groups];
    let mut sq = vec![0.0; groups];
    let mut count = vec![0usize; groups];
    let mut worst: f64 = 0.0;
    for (&s, &q) in x.iter().zip(y.samples()) {
        let exact = s as f64 * FULL_SCALE;
        let e = q as f64 - exact;
        let g = ((exact - exact.floor()) * groups as f64) as usize;
        sum[g] += e;
        sq[g] += e * e;
        count[g] += 1;
        worst = worst.max(e.abs());
    }
    for g in 0..groups {
        let n = count[g] as f64;
        let mean = sum[g] / n;
        let power = sq[g] / n - mean * mean;
        assert!(mean.abs() < 0.01, "group {g}: mean error {mean}");
        assert!(
            (power - 0.25).abs() < 0.01,
            "group {g}: error power {power}"
        );
    }
    // Triangle (under 1 LSB) plus rounding (half an LSB).
    assert!(worst < 1.5, "an error of {worst} LSB");
}

#[test]
fn the_error_is_white() {
    // No noise shaping: the error's power is the same in the low and high
    // halves of the band.
    let x = sine(1 << 16);
    let y = to_16_bit(&x, 2);
    let e: Vec<f64> = x
        .iter()
        .zip(y.samples())
        .map(|(&s, &q)| q as f64 - s as f64 * FULL_SCALE)
        .collect();
    // First differences carry the high half of the band, sums the low half;
    // for white noise their powers match.
    let diff: f64 = e.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
    let sum: f64 = e.windows(2).map(|w| (w[1] + w[0]).powi(2)).sum();
    let ratio = diff / sum;
    assert!((ratio - 1.0).abs() < 0.03, "high/low power {ratio}");
}

#[test]
fn the_same_seed_gives_the_same_samples() {
    let x = sine(10_000);
    assert_eq!(to_16_bit(&x, 42).samples(), to_16_bit(&x, 42).samples());
    assert_ne!(to_16_bit(&x, 42).samples(), to_16_bit(&x, 43).samples());
}

#[test]
fn stems_with_their_own_seeds_sum_as_plain_noise() {
    // 6.7: "so the four stems' dither sums as plain noise". Four stems of
    // the same signal, each with its own seed: the summed error's power is
    // four stems' worth (independent), not sixteen (identical).
    let x = sine(1 << 18);
    let stems: Vec<_> = (0..4).map(|seed| to_16_bit(&x, seed)).collect();
    let mut power = 0.0;
    for (i, &s) in x.iter().enumerate() {
        let exact = s as f64 * FULL_SCALE;
        let e: f64 = stems.iter().map(|y| y.samples()[i] as f64 - exact).sum();
        power += e * e;
    }
    power /= x.len() as f64;
    assert!((power - 1.0).abs() < 0.03, "summed error power {power}");
}

#[test]
fn values_past_full_scale_saturate() {
    let y = to_16_bit(&[1.5, -1.5, 1.0, -1.0, 0.0], 3);
    assert_eq!(y.samples()[0], i16::MAX);
    assert_eq!(y.samples()[1], i16::MIN);
    assert_eq!(y.samples()[2], i16::MAX);
    assert!(y.samples()[3] <= -32_767);
    assert!(y.samples()[4].abs() <= 1);
}
