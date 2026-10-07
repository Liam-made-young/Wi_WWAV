//! Frame-indexed export (9.5). Export doesn't follow the clock: the engine
//! renders the sound first and the encoder pulls frames by number. Frame n
//! sits at n ÷ fps and its sound starts at sample ⌊n × rate ÷ fps⌋. Both are
//! integer arithmetic on the exact rate, so NTSC's 30000/1001 is still on the
//! sample after a day, where a float clock would have drifted.

use crate::gcd;

/// A frame rate as a fraction in lowest terms: 30, or 30000/1001 for 29.97.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fps {
    num: u32,
    den: u32,
}

impl Fps {
    /// `num` frames every `den` seconds.
    pub fn new(num: u32, den: u32) -> Fps {
        assert!(num > 0 && den > 0, "a frame rate must be positive");
        let g = gcd(num as u64, den as u64) as u32;
        Fps {
            num: num / g,
            den: den / g,
        }
    }

    pub fn whole(fps: u32) -> Fps {
        Fps::new(fps, 1)
    }

    /// Ticks a second of the film's clock: MP4's timescale, FFmpeg's
    /// time base 1/timescale.
    pub fn timescale(&self) -> u32 {
        self.num
    }

    /// Frame n's presentation time in timescale ticks: n ÷ fps seconds.
    pub fn pts(&self, n: u64) -> u64 {
        n * self.den as u64
    }

    /// The sample at `rate` where frame n's sound starts: ⌊n × rate ÷ fps⌋.
    pub fn start_sample(&self, n: u64, rate: u32) -> u64 {
        (n as u128 * rate as u128 * self.den as u128 / self.num as u128) as u64
    }

    /// Frames in a film whose sound is `samples` long: every frame that
    /// starts inside the sound.
    pub fn frame_count(&self, samples: u64, rate: u32) -> u64 {
        (samples as u128 * self.num as u128).div_ceil(rate as u128 * self.den as u128) as u64
    }
}
