//! F9: the resampler fails if its passband ripple exceeds ±0.1 dB to 20 kHz or
//! its image and alias rejection is under 90 dB (`docs/PLAN.md`). These tones
//! go through the streaming path the export uses, and an FFT of the output
//! measures them.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use wwav_dsp::resample::{resampled_len, Resampler};

const RATIOS: [(u32, u32); 4] = [
    (48_000, 44_100),
    (44_100, 48_000),
    (96_000, 44_100),
    (88_200, 44_100),
];

/// Output samples measured per tone.
const N: usize = 1 << 16;
/// Output samples skipped at the start, where the filter still reads the
/// silence before the signal.
const SKIP: usize = 4096;
/// Bins either side of a tone that belong to the measuring window's main lobe.
const LOBE: usize = 10;

fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    while term > sum * 1e-17 {
        term *= (x / (2.0 * k)) * (x / (2.0 * k));
        sum += term;
        k += 1.0;
    }
    sum
}

/// A Kaiser window with beta 20: its sidelobes sit near −190 dB, far below
/// what is being measured.
fn window() -> Vec<f64> {
    let beta = 20.0;
    let m = (N - 1) as f64;
    (0..N)
        .map(|n| {
            let r = 2.0 * n as f64 / m - 1.0;
            bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / bessel_i0(beta)
        })
        .collect()
}

/// Resamples a full-scale sine and returns N output samples from its middle.
fn resample_tone(from: u32, to: u32, freq: f64) -> Vec<f32> {
    let want = (N + 2 * SKIP) as u64;
    let frames = (want * from as u64).div_ceil(to as u64) as usize;
    let input: Vec<f32> = (0..frames)
        .map(|n| (2.0 * std::f64::consts::PI * freq * n as f64 / from as f64).sin() as f32)
        .collect();
    let mut r = Resampler::new(from, to, 1);
    let mut out = Vec::new();
    r.process(&input, &mut out);
    r.finish(&mut out);
    assert_eq!(out.len() as u64, resampled_len(frames as u64, from, to));
    out[SKIP..SKIP + N].to_vec()
}

/// Magnitude per bin in dB against a full-scale sine.
fn spectrum_db(x: &[f32]) -> Vec<f64> {
    let w = window();
    let gain: f64 = w.iter().sum::<f64>() / 2.0;
    let mut buf: Vec<Complex<f64>> = x
        .iter()
        .zip(&w)
        .map(|(&s, &w)| Complex::new(s as f64 * w, 0.0))
        .collect();
    FftPlanner::new().plan_fft_forward(N).process(&mut buf);
    buf[..N / 2]
        .iter()
        .map(|c| 20.0 * (c.norm() / gain).max(1e-30).log10())
        .collect()
}

/// The loudest bin outside the given tone bins' main lobes, in dB.
fn loudest_spur(db: &[f64], tones: &[usize]) -> (usize, f64) {
    db.iter()
        .enumerate()
        .filter(|(k, _)| tones.iter().all(|t| k.abs_diff(*t) > LOBE))
        .map(|(k, &v)| (k, v))
        .fold((0, f64::NEG_INFINITY), |a, b| if b.1 > a.1 { b } else { a })
}

#[test]
fn passband_is_flat_within_a_tenth_of_a_db_to_20_khz_with_nothing_else_above_minus_90() {
    let probes = [
        20.0, 100.0, 1_000.0, 5_000.0, 10_000.0, 15_000.0, 18_000.0, 19_000.0, 19_700.0, 20_000.0,
    ];
    for (from, to) in RATIOS {
        for f in probes {
            // The bin at or just under f, so the tone is periodic in the window.
            let bin = (f * N as f64 / to as f64).floor() as usize;
            let freq = bin as f64 * to as f64 / N as f64;
            let db = spectrum_db(&resample_tone(from, to, freq));
            let gain = db[bin];
            assert!(
                gain.abs() <= 0.1,
                "{from} → {to}: {freq:.1} Hz comes out at {gain:+.4} dB"
            );
            let (spur, level) = loudest_spur(&db, &[bin]);
            assert!(
                level <= -90.0,
                "{from} → {to}: a {freq:.1} Hz tone leaves {level:.1} dB at {:.1} Hz",
                spur as f64 * to as f64 / N as f64
            );
        }
    }
}

#[test]
fn content_above_the_output_band_is_rejected_by_at_least_90_db() {
    for (from, to) in RATIOS {
        if from < to {
            continue; // upsampling has no input above the output band
        }
        let top = from as f64 / 2.0;
        let probes = [
            22_100.0,
            22_500.0,
            23_000.0,
            23_900.0,
            top * 0.6,
            top * 0.8,
            top - 100.0,
        ];
        for f in probes.into_iter().filter(|&f| f > 22_050.0 && f < top) {
            let db = spectrum_db(&resample_tone(from, to, f));
            let (spur, level) = loudest_spur(&db, &[]);
            assert!(
                level <= -90.0,
                "{from} → {to}: {f:.0} Hz aliases to {:.1} Hz at {level:.1} dB",
                spur as f64 * to as f64 / N as f64
            );
        }
    }
}

#[test]
fn the_output_is_time_aligned_with_the_input() {
    // A 997 Hz sine resampled lands on the same sine sampled at the new
    // rate: no delay, so sound stays on its film frame (9.5).
    for (from, to) in RATIOS {
        let f = 997.0;
        let frames = from as usize; // one second
        let input: Vec<f32> = (0..frames)
            .map(|n| (2.0 * std::f64::consts::PI * f * n as f64 / from as f64).sin() as f32)
            .collect();
        let mut r = Resampler::new(from, to, 1);
        let mut out = Vec::new();
        r.process(&input, &mut out);
        r.finish(&mut out);
        assert_eq!(out.len(), to as usize);
        let worst = (SKIP..out.len() - SKIP)
            .map(|k| {
                let want = (2.0 * std::f64::consts::PI * f * k as f64 / to as f64).sin();
                (out[k] as f64 - want).abs()
            })
            .fold(0.0, f64::max);
        assert!(
            20.0 * worst.log10() <= -90.0,
            "{from} → {to}: off by {worst:e}"
        );
    }
}

/// A small deterministic generator for test signals and block sizes.
fn lcg(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state >> 33
}

#[test]
fn block_by_block_equals_one_shot_sample_for_sample() {
    for (from, to) in RATIOS {
        let channels = 2;
        let mut seed = 7;
        let input: Vec<f32> = (0..20_011 * channels)
            .map(|_| (lcg(&mut seed) as f64 / (1u64 << 31) as f64 - 1.0) as f32)
            .collect();

        let mut whole = Vec::new();
        let mut r = Resampler::new(from, to, channels);
        r.process(&input, &mut whole);
        r.finish(&mut whole);

        let mut blocks = Vec::new();
        let mut r = Resampler::new(from, to, channels);
        let mut at = 0;
        while at < input.len() {
            // Blocks of 0 to 700 frames, including empty and single frames.
            let frames = (lcg(&mut seed) % 701) as usize;
            let end = (at + frames * channels).min(input.len());
            r.process(&input[at..end], &mut blocks);
            at = end;
        }
        r.finish(&mut blocks);

        assert_eq!(whole.len(), blocks.len(), "{from} → {to}");
        let same = whole
            .iter()
            .zip(&blocks)
            .all(|(a, b)| a.to_bits() == b.to_bits());
        assert!(same, "{from} → {to}: blocks change the output");
    }
}

#[test]
fn the_output_length_is_exact() {
    // Every output instant inside the input's span: ⌈n × to ÷ from⌉.
    assert_eq!(resampled_len(48_000, 48_000, 44_100), 44_100);
    assert_eq!(resampled_len(1, 48_000, 44_100), 1);
    assert_eq!(resampled_len(160, 48_000, 44_100), 147);
    assert_eq!(resampled_len(161, 48_000, 44_100), 148);
    assert_eq!(resampled_len(147, 44_100, 48_000), 160);
    assert_eq!(resampled_len(3, 88_200, 44_100), 2);
    // Ten hours at 48 kHz, where a float would have drifted.
    assert_eq!(
        resampled_len(48_000 * 36_000, 48_000, 44_100),
        44_100 * 36_000
    );
    for (from, to) in RATIOS {
        for frames in [0usize, 1, 2, 3, 159, 160, 161, 1_000, 4_801] {
            let mut r = Resampler::new(from, to, 2);
            let mut out = Vec::new();
            r.process(&vec![0.5; frames * 2], &mut out);
            r.finish(&mut out);
            assert_eq!(
                out.len() as u64,
                2 * resampled_len(frames as u64, from, to),
                "{from} → {to}, {frames} frames"
            );
        }
    }
}
