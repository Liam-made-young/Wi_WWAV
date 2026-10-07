//! TPDF dither to 16 bits, once, at the last step of an export (6.7).
//!
//! The noise is the difference of two uniform values, a triangle one LSB
//! wide either side, which makes the error's mean and power the same
//! wherever the signal sits between two codes: no distortion and no noise
//! that follows the music. There is no noise shaping, so each stem's dither
//! is plain white noise, and stems dithered with their own seeds sum as
//! plain noise too.
//!
//! [`to_16_bit`] is the only way to make a [`Pcm16`], and nothing turns a
//! `Pcm16` back into floats, so a second dither can't happen by passing the
//! result along. It would take code that converts the samples back by hand:
//!
//! ```compile_fail
//! use wwav_dsp::dither::to_16_bit;
//! let once = to_16_bit(&[0.25f32, -0.5], 1);
//! let twice = to_16_bit(once.samples(), 2); // already 16-bit: refused
//! ```
//!
//! ```
//! use wwav_dsp::dither::to_16_bit;
//! let once = to_16_bit(&[0.25f32, -0.5], 1);
//! assert_eq!(once.samples().len(), 2);
//! ```

/// 16-bit samples, dithered. Only [`to_16_bit`] makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcm16 {
    samples: Vec<i16>,
}

impl Pcm16 {
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }

    pub fn into_samples(self) -> Vec<i16> {
        self.samples
    }
}

/// Dithers float samples at full scale ±1.0 to 16 bits, at 32,768 codes to
/// 1.0: the scale 16-bit readers use on the way in (code ÷ 32,768), so an
/// undithered round trip gives back the same codes. The same samples and
/// seed always give the same result. Give each stem and the master their own
/// seed, so their dithers are independent.
pub fn to_16_bit(samples: &[f32], seed: u64) -> Pcm16 {
    let mut rng = SplitMix64(seed);
    let samples = samples
        .iter()
        .map(|&x| {
            let noise = rng.unit() - rng.unit();
            let v = (x as f64 * 32_768.0 + noise + 0.5).floor();
            v.clamp(i16::MIN as f64, i16::MAX as f64) as i16
        })
        .collect();
    Pcm16 { samples }
}

/// SplitMix64: a 64-bit generator small enough to read, with no state but a
/// counter, so a seed fixes the noise exactly on every machine. `rand`'s
/// generators are free to change between versions.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1), from the top 53 bits.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_its_reference_output() {
        // The first outputs for seed 1234567, SplitMix64's published test vector.
        let mut r = SplitMix64(1_234_567);
        assert_eq!(r.next(), 6_457_827_717_110_365_317);
        assert_eq!(r.next(), 3_203_168_211_198_807_973);
        assert_eq!(r.next(), 9_817_491_932_198_370_423);
    }

    #[test]
    fn a_value_on_a_code_stays_within_one_code() {
        let x: Vec<f32> = (-5..5).map(|v| v as f32 / 32_768.0).collect();
        let y = to_16_bit(&x, 9);
        for (v, &q) in (-5i16..5).zip(y.samples()) {
            assert!((q - v).abs() <= 1);
        }
    }
}
