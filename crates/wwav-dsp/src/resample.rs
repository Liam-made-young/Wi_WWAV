//! A band-limited polyphase resampler for rational ratios: the export's
//! 48 → 44.1 kHz (6.7) and clips at another rate as they play (9.4).
//!
//! The ratio out ÷ in reduces to up ÷ down (48,000 → 44,100 is 147 ÷ 160).
//! Output frame k sits at input position k × down ÷ up, so it is read by one
//! of `up` phases of a windowed-sinc filter centred there. The filter keeps
//! 0–20 kHz flat to within 0.0001 dB and is designed to be 100 dB down from
//! the lower of the two Nyquist frequencies up, so nothing above the output
//! band folds into it and no image of the input lands in it (the tests hold
//! both to F9's ±0.1 dB and 90 dB). Centring the filter
//! on each output instant means the output has no delay: frame k is the
//! input at time k ÷ out_rate, which keeps sound on its film frame (9.5).
//!
//! Every sum runs in the same order whatever the block sizes, in f64, with
//! coefficients from `libm`, so the same input gives the same bits on every
//! run and every machine.

use crate::gcd;

/// Where the passband ends at 44.1 kHz and above. Below that the passband
/// keeps the same share of the band.
const PASS_HZ: f64 = 20_000.0;
/// The stopband's depth the filter is designed for. The tests hold it to
/// 90 dB (F9) and this leaves a margin over the Kaiser estimate.
const STOP_DB: f64 = 100.0;

pub struct Resampler {
    up: u64,
    down: u64,
    /// Taps per phase, even.
    taps: usize,
    /// `up` phases of `taps` coefficients each.
    table: Vec<f64>,
    channels: usize,
    /// Interleaved input frames, from absolute frame `first` on. Frames before
    /// 0 and after the end are silence.
    history: Vec<f32>,
    first: i64,
    produced: u64,
}

impl Resampler {
    /// A resampler for interleaved frames of `channels` samples. The two
    /// rates must differ: at the same rate there is nothing to resample, and
    /// a filter would only cut the top of the band.
    pub fn new(in_rate: u32, out_rate: u32, channels: usize) -> Self {
        assert!(in_rate > 0 && out_rate > 0, "rates must be positive");
        assert_ne!(in_rate, out_rate, "the same rate needs no resampler");
        assert!(channels > 0, "at least one channel");
        let g = gcd(in_rate as u64, out_rate as u64);
        let (up, down) = (out_rate as u64 / g, in_rate as u64 / g);

        // Frequencies as cycles per input sample.
        let stop_hz = in_rate.min(out_rate) as f64 / 2.0;
        let pass_hz = PASS_HZ.min(stop_hz * PASS_HZ / 22_050.0);
        let cutoff = (pass_hz + stop_hz) / 2.0 / in_rate as f64;
        let width = 2.0 * std::f64::consts::PI * (stop_hz - pass_hz) / in_rate as f64;

        // Kaiser's estimates for the length and shape that reach STOP_DB.
        let taps = ((STOP_DB - 7.95) / (2.285 * width)).ceil() as usize;
        let taps = taps + taps % 2;
        let beta = 0.1102 * (STOP_DB - 8.7);

        // Phase p, tap j reads input frame n0 − taps/2 + 1 + j, which sits
        // `d` input frames before the output instant n0 + p ÷ up.
        let half = (taps / 2) as f64;
        let mut table = Vec::with_capacity(up as usize * taps);
        for p in 0..up {
            for j in 0..taps {
                let d = p as f64 / up as f64 + (half - 1.0 - j as f64);
                table.push(2.0 * cutoff * sinc(2.0 * cutoff * d) * kaiser(d / half, beta));
            }
        }

        let lead = taps / 2 - 1;
        Resampler {
            up,
            down,
            taps,
            table,
            channels,
            history: vec![0.0; lead * channels],
            first: -(lead as i64),
            produced: 0,
        }
    }

    /// Takes interleaved input frames and appends every output frame they
    /// complete. Blocks may be any size, including empty.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        assert_eq!(input.len() % self.channels, 0, "whole frames only");
        self.history.extend_from_slice(input);
        self.run(out);
    }

    /// Ends the stream. The filter reads silence past the end, and the total
    /// output is exactly `resampled_len` of the frames given.
    pub fn finish(mut self, out: &mut Vec<f32>) {
        let tail = self.taps / 2 * self.channels;
        self.history.resize(self.history.len() + tail, 0.0);
        self.run(out);
    }

    fn run(&mut self, out: &mut Vec<f32>) {
        let (ch, taps) = (self.channels, self.taps);
        let half = (taps / 2) as i64;
        let end = self.first + (self.history.len() / ch) as i64;
        loop {
            let pos = self.produced * self.down;
            let n0 = (pos / self.up) as i64;
            if n0 + half >= end {
                break;
            }
            let phase = (pos % self.up) as usize;
            let coefs = &self.table[phase * taps..(phase + 1) * taps];
            let from = (n0 - half + 1 - self.first) as usize * ch;
            let frames = &self.history[from..from + taps * ch];
            for c in 0..ch {
                let mut acc = 0.0f64;
                for (h, &x) in coefs.iter().zip(frames[c..].iter().step_by(ch)) {
                    acc += h * x as f64;
                }
                out.push(acc as f32);
            }
            self.produced += 1;
        }
        // Forget the frames no later output reads.
        let next = (self.produced * self.down / self.up) as i64 - half + 1;
        let held = (self.history.len() / ch) as i64;
        let drop = (next - self.first).clamp(0, held);
        self.history.drain(..drop as usize * ch);
        self.first += drop;
    }
}

/// Output frames for `frames` input frames: every output instant inside the
/// input's span, ⌈frames × out_rate ÷ in_rate⌉, in integers so hours of audio
/// don't drift.
pub fn resampled_len(frames: u64, in_rate: u32, out_rate: u32) -> u64 {
    (frames as u128 * out_rate as u128).div_ceil(in_rate as u128) as u64
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        let px = std::f64::consts::PI * x;
        libm::sin(px) / px
    }
}

/// The Kaiser window at x in [−1, 1].
fn kaiser(x: f64, beta: f64) -> f64 {
    bessel_i0(beta * (1.0 - x * x).max(0.0).sqrt()) / bessel_i0(beta)
}

/// The modified Bessel function I0, by its power series.
fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    while term > sum * 1e-17 {
        let t = x / (2.0 * k);
        term *= t * t;
        sum += term;
        k += 1.0;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustfft::num_complex::Complex;
    use rustfft::FftPlanner;

    /// The whole prototype filter's response, from the table: flat to 20 kHz
    /// and at least 90 dB down from the lower Nyquist to the top of the
    /// up-sampled band. The tone tests in `tests/resampler.rs` check the
    /// streaming path; this checks every frequency between their probes.
    #[test]
    fn the_filter_is_flat_then_deep_everywhere() {
        for (from, to) in [
            (48_000, 44_100),
            (44_100, 48_000),
            (96_000, 44_100),
            (88_200, 44_100),
        ] {
            let r = Resampler::new(from, to, 1);
            let (up, taps) = (r.up as usize, r.taps);
            // Lay the phases out in time order: phase p, tap j sits at
            // up × (taps/2 − 1 − j) + p from the centre.
            let n = 1 << 21;
            let mut h = vec![Complex::new(0.0, 0.0); n];
            for p in 0..up {
                for j in 0..taps {
                    let at = up * (taps - 1 - j) + p;
                    h[at] = Complex::new(r.table[p * taps + j], 0.0);
                }
            }
            FftPlanner::new().plan_fft_forward(n).process(&mut h);
            // Each phase sums to about 1, so the prototype's gain is `up`.
            let rate = up as f64 * from as f64;
            let stop = from.min(to) as f64 / 2.0;
            for (k, c) in h[..n / 2].iter().enumerate() {
                let f = k as f64 * rate / n as f64;
                let db = 20.0 * (c.norm() / up as f64).max(1e-30).log10();
                if f <= PASS_HZ {
                    assert!(db.abs() <= 0.1, "{from} → {to}: {f:.1} Hz at {db:+.5} dB");
                } else if f >= stop {
                    assert!(db <= -90.0, "{from} → {to}: {f:.1} Hz at {db:.1} dB");
                }
            }
        }
    }

    #[test]
    fn bessel_i0_matches_known_values() {
        assert_eq!(bessel_i0(0.0), 1.0);
        assert!((bessel_i0(1.0) - 1.266_065_877_752_008_4).abs() < 1e-15);
        assert!((bessel_i0(10.0) - 2_815.716_628_466_254).abs() < 1e-9);
    }
}
