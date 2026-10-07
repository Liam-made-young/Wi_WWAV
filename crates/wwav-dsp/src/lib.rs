//! The export maths, and the estimates the Console labels as estimates.
//!
//! - `resample`: the band-limited polyphase resampler, 48 → 44.1 kHz on
//!   export (6.7) and clips at another rate (9.4).
//! - `dither`: TPDF dither to 16 bits, once, at the last step (6.7).
//! - `fold`: the fold check and the clipping lines for the four stem buses
//!   (6.6).
//! - `sheet`: the export sheet's size, length limit and conversion lines
//!   (5.13, 6.1, 6.7).
//! - `film`: frame-indexed export, where frame n's sound starts (9.5).
//! - `analysis`: tempo, key and first onset, ported from v4's `analysis.js`
//!   (5.4).
//! - `matching`: the figure-eight's tempo, key and downbeat matching (4.6,
//!   5.9).
//!
//! Pure Rust and deterministic: no fast maths, no platform intrinsics, and
//! `libm` for every transcendental function, so the same input gives the
//! same bits on every run and every machine.

pub mod analysis;
pub mod dither;
pub mod film;
pub mod fold;
pub mod matching;
pub mod resample;
pub mod sheet;

pub(crate) fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
