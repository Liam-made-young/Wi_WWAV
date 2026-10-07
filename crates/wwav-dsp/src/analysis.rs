//! Tempo, key and the first drum hit, estimated from a song's stems: a port
//! of Mi-WWAV's `wwav/src/audio/analysis.js`, step for step. Tempo comes from
//! the drum stem's onset autocorrelation, key from a Krumhansl profile
//! matched to a chromagram of the harmonic stems.
//!
//! The port keeps the original's arithmetic: its `Float32Array`s stay `f32`
//! here and its sums stay `f64`, its `Math.round` rounds halves up, and
//! `libm` stands in for `Math`. The results are estimates, and the app labels
//! them as such: "≈ 86 BPM (estimate)" (5.4, "never invent metrics").
//!
//! It departs from the original only where that would invent a tempo or
//! leave the range it gives: silence, a level that never moves and clips
//! under two slow beats have no tempo; a silent drum stem gives way to the
//! whole song, as a missing one does; a refinement can't leave the three
//! lags it is fitted to; and the estimate stays within 60–180 BPM.
//! Everywhere else it gives what the original gives, to the bit.

use std::f64::consts::PI;
use std::fmt;

/// Interleaved audio. Buffers read together share one rate, as the original
/// assumes.
#[derive(Debug, Clone, Copy)]
pub struct Audio<'a> {
    pub samples: &'a [f32],
    pub channels: usize,
    pub rate: u32,
}

impl Audio<'_> {
    fn frames(&self) -> usize {
        self.samples.len() / self.channels
    }

    fn at(&self, frame: usize, channel: usize) -> f32 {
        self.samples[frame * self.channels + channel]
    }
}

/// A song's stems, any of which may be missing.
#[derive(Debug, Clone, Copy)]
pub struct Stems<'a> {
    pub vocals: Option<Audio<'a>>,
    pub drums: Option<Audio<'a>>,
    pub other: Option<Audio<'a>>,
    pub bass: Option<Audio<'a>>,
}

/// Pitch class 0 is A, so a tonic's index reads straight out as its name.
pub const NOTE_NAMES: [&str; 12] = [
    "A", "A♯", "B", "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯",
];

/// A key: "C♯ minor" is pitch class 4, minor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub pc: u8,
    pub minor: bool,
}

impl Key {
    /// Reads a name as `parseKey` does: a note from `NOTE_NAMES`, a space,
    /// and "minor" for a minor key; anything else after the note is major.
    pub fn parse(name: &str) -> Option<Key> {
        let mut words = name.split(' ');
        let note = words.next()?;
        let pc = NOTE_NAMES.iter().position(|&n| n == note)? as u8;
        Some(Key {
            pc,
            minor: words.next() == Some("minor"),
        })
    }

    /// The same mode, `semis` semitones away.
    pub fn transposed(self, semis: i32) -> Key {
        Key {
            pc: (self.pc as i32 + semis).rem_euclid(12) as u8,
            minor: self.minor,
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mode = if self.minor { "minor" } else { "major" };
        write!(f, "{} {mode}", NOTE_NAMES[self.pc as usize])
    }
}

/// What `analyze` estimated. Every field is an estimate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimates {
    /// 60–180 BPM, to a tenth. Like analysis.js, it can read an octave
    /// away, toward 120: under 71 BPM a click track often reads double and
    /// over 145 half (60 → 120.2, 150 → 74.9). The figure-eight folds tempo
    /// ratios by octaves for this. None when nothing in the audio has
    /// onsets to read, or none at a tempo in the range.
    pub bpm: Option<f64>,
    pub key: Option<Key>,
    /// Where the drums begin, in seconds; 0 when nothing could be found.
    pub drum_onset: f64,
}

impl Estimates {
    /// "≈ 86 BPM (estimate)"
    pub fn bpm_label(&self) -> Option<String> {
        self.bpm
            .map(|bpm| format!("≈ {} BPM (estimate)", js_round(bpm)))
    }

    /// "A minor (estimate)"
    pub fn key_label(&self) -> Option<String> {
        self.key.map(|key| format!("{key} (estimate)"))
    }
}

/// `analyzeTrack`: tempo and the first onset from the drums (the whole song
/// without them), key from the harmonic stems so the kick doesn't smear the
/// chroma.
pub fn analyze(stems: &Stems) -> Estimates {
    // analysis.js lists the stems in this order, and sums them in it.
    let all: Vec<Audio> = [stems.vocals, stems.drums, stems.bass, stems.other]
        .into_iter()
        .flatten()
        .collect();
    let harmonic: Vec<Audio> = [stems.vocals, stems.bass, stems.other]
        .into_iter()
        .flatten()
        .collect();
    // The whole song stands in for drums that give no tempo, as it does for
    // the first onset below. analysis.js falls back only for a missing drum
    // stem, but a stem bus with no tracks is written as silence (6.6), and a
    // drumless song still has the tempo of what was played (5.4).
    let bpm = stems
        .drums
        .and_then(|drums| estimate_tempo(&[drums]))
        .or_else(|| estimate_tempo(&all));
    let key = estimate_key(if harmonic.is_empty() { &all } else { &harmonic });
    let drum_onset = stems
        .drums
        .and_then(|d| first_onset(&d))
        .or_else(|| all.first().and_then(first_onset))
        .unwrap_or(0.0);
    Estimates {
        bpm,
        key,
        drum_onset,
    }
}

const BPM_MIN: f64 = 60.0;
const BPM_MAX: f64 = 180.0;

/// `detectBpm`: the tempo of a minute from the middle of the audio. None
/// when there's nothing to read: under two beats at 60 BPM (about 2 s); no
/// onsets at all, such as silence, the dither on a silent bus or a level
/// that never moves; or onsets that never repeat within the range, such as
/// clicks at 55 BPM. analysis.js reads most of those as 184.6 BPM, and a
/// clip of a second or so as NaN or as a value resting on a product or two.
pub fn estimate_tempo(buffers: &[Audio]) -> Option<f64> {
    let (samples, rate) = mono_window(buffers, 60)?;
    let mono = Audio {
        samples: &samples,
        channels: 1,
        rate,
    };
    if blank(&mono) {
        return None;
    }

    // Onset envelope: half-wave-rectified energy flux over 512-sample hops.
    let hop = 512;
    let fps = rate as f64 / hop as f64; // onset frames per second
    let lag_min = ((60.0 / BPM_MAX) * fps).floor() as usize;
    let lag_max = ((60.0 / BPM_MIN) * fps).ceil() as usize;
    // Every lag is averaged over at least one slowest beat, so none rests on
    // a product or two, or on none (0 ÷ 0).
    let frames = (samples.len() / hop) as i64 - 1;
    if frames < 64.max(2 * lag_max as i64) {
        return None;
    }
    let frames = frames as usize;
    let mut onsets = vec![0.0f32; frames];
    let mut prev = 0.0f64;
    for (f, onset) in onsets.iter_mut().enumerate() {
        let e: f64 = samples[f * hop..f * hop + hop]
            .iter()
            .map(|&s| s as f64 * s as f64)
            .sum();
        *onset = (e - prev).max(0.0) as f32;
        prev = e;
    }

    let mut corr = vec![0.0f32; lag_max + 1];
    for (lag, c) in corr.iter_mut().enumerate().skip(lag_min) {
        let mut sum = 0.0f64;
        for i in 0..frames.saturating_sub(lag) {
            sum += onsets[i] as f64 * onsets[i + lag] as f64;
        }
        *c = (sum / (frames as f64 - lag as f64)) as f32;
    }

    // A lag supported by its own double and half (the beat's harmonics) is a
    // beat, not a fluke accent, and a log-Gaussian prior centred on 120 BPM
    // breaks the half/double-tempo tie a bare autocorrelation can't.
    let at = |lag: usize| {
        if (lag_min..=lag_max).contains(&lag) {
            corr[lag] as f64
        } else {
            0.0
        }
    };
    let mut best = lag_min;
    let mut best_score = f64::NEG_INFINITY;
    for (lag, &c) in corr.iter().enumerate().skip(lag_min) {
        let octaves = libm::log2((60.0 * fps) / lag as f64 / 120.0);
        let prior = libm::exp(-(octaves * octaves) / (2.0 * 0.6 * 0.6));
        let half = js_round(lag as f64 / 2.0) as usize;
        let score = (c as f64 + 0.5 * at(lag * 2) + 0.25 * at(half)) * prior;
        if score > best_score {
            best_score = score;
            best = lag;
        }
    }
    // Nothing correlated (every lag scored 0, and the first would win), or
    // the audio held samples that aren't numbers.
    if !(best_score > 0.0 && best_score.is_finite()) {
        return None;
    }

    // Parabolic refinement around the winning lag: the vertex of the
    // parabola through it and its neighbours. When the winner isn't a peak
    // of the autocorrelation itself, the vertex can fall outside those three
    // lags, which is extrapolation, so the winner stands.
    let mut lag = best as f64;
    if best > lag_min && best < lag_max {
        let (a, b, c) = (
            corr[best - 1] as f64,
            corr[best] as f64,
            corr[best + 1] as f64,
        );
        let denom = a - 2.0 * b + c;
        if denom != 0.0 {
            let vertex = best as f64 + (0.5 * (a - c)) / denom;
            if (vertex - lag).abs() <= 1.0 {
                lag = vertex;
            }
        }
    }
    // The lags reach a little past the range (59.4 to 184.6 BPM at
    // 44.1 kHz), and the estimate is promised within it.
    let bpm = (60.0 * fps) / lag;
    Some((js_round(bpm * 10.0) / 10.0).clamp(BPM_MIN, BPM_MAX))
}

/// Krumhansl-Kessler probe-tone profiles, by semitones above the tonic.
const MAJOR_PROFILE: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR_PROFILE: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.6, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

/// `detectKey`: the key of 45 seconds from the middle of the audio, or None
/// for under a second of it or silence.
pub fn estimate_key(buffers: &[Audio]) -> Option<Key> {
    let (samples, rate) = mono_window(buffers, 45)?;

    const N: usize = 4096;
    let hop = N * 2; // sparse frames: key is a property of the whole song
    let hann: Vec<f32> = (0..N)
        .map(|i| (0.5 - 0.5 * libm::cos((2.0 * PI * i as f64) / (N - 1) as f64)) as f32)
        .collect();

    let mut chroma = [0.0f32; 12];
    let mut re = vec![0.0f32; N];
    let mut im = vec![0.0f32; N];
    let bin_hz = rate as f64 / N as f64;
    let lo_bin = 1.max((60.0 / bin_hz).ceil() as usize);
    let hi_bin = (N / 2 - 1).min((5000.0 / bin_hz).floor() as usize);

    let mut o = 0;
    while o + N <= samples.len() {
        for i in 0..N {
            re[i] = samples[o + i] * hann[i];
            im[i] = 0.0;
        }
        fft(&mut re, &mut im);
        for b in lo_bin..=hi_bin {
            let mag = (re[b] as f64 * re[b] as f64 + im[b] as f64 * im[b] as f64).sqrt();
            if mag <= 0.0 {
                continue;
            }
            // Pitch class 0 is A (440 Hz).
            let semis = js_round(12.0 * libm::log2((b as f64 * bin_hz) / 440.0));
            let pc = (semis as i64).rem_euclid(12) as usize;
            chroma[pc] = (chroma[pc] as f64 + mag) as f32;
        }
        o += hop;
    }
    if !chroma.iter().any(|&v| v > 0.0) {
        return None;
    }

    let pearson = |profile: &[f64; 12], tonic: usize| {
        let (mut sx, mut sy, mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (i, &y) in profile.iter().enumerate() {
            let x = chroma[(tonic + i) % 12] as f64;
            sx += x;
            sy += y;
            sxx += x * x;
            syy += y * y;
            sxy += x * y;
        }
        let cov = sxy - (sx * sy) / 12.0;
        let den = ((sxx - (sx * sx) / 12.0) * (syy - (sy * sy) / 12.0)).sqrt();
        if den > 0.0 {
            cov / den
        } else {
            0.0
        }
    };

    let mut best = None;
    let mut best_score = f64::NEG_INFINITY;
    for tonic in 0..12 {
        for (profile, minor) in [(&MAJOR_PROFILE, false), (&MINOR_PROFILE, true)] {
            let score = pearson(profile, tonic);
            if score > best_score {
                best_score = score;
                best = Some(Key {
                    pc: tonic as u8,
                    minor,
                });
            }
        }
    }
    best
}

/// `firstOnset`: where the drums begin, in seconds, so a child song's drums
/// can be slid into phase with its parent's.
///
/// 20 ms RMS windows; the threshold is the louder of 6× the noise floor
/// (from the first half second) and 12% of the stem's own peak, so neither
/// tape hiss nor a quiet count-in trips it. Two windows in a row must clear
/// it (one is a click, not a beat), then the time walks back down the rise
/// to the transient's foot.
pub fn first_onset(buffer: &Audio) -> Option<f64> {
    let energy = rms_windows(buffer);
    let frames = energy.len();
    if frames < 4 {
        return None;
    }

    let peak = loudest(&energy);
    if (peak as f64) < SILENCE_RMS {
        return None; // silence, such as a blank stem
    }

    let mut head = energy[..frames.min(25)].to_vec();
    head.sort_by(f32::total_cmp);
    let noise = head[head.len() / 2] as f64;
    let thresh = (noise * 6.0).max(peak as f64 * 0.12);

    for f in 0..frames - 1 {
        if energy[f] as f64 >= thresh && energy[f + 1] as f64 >= thresh {
            let mut start = f;
            while start > 0 && energy[start - 1] as f64 > thresh * 0.4 {
                start -= 1;
            }
            return Some(start as f64 * 0.02);
        }
    }
    None
}

/// Under this RMS in every 20 ms window (−80 dBFS) a stem is silent. The
/// dither on a silent bus, about −96 dBFS, is under it.
const SILENCE_RMS: f64 = 1e-4;

/// Nothing in it reaches −80 dBFS over 20 ms: `first_onset`'s test for a
/// blank stem.
fn blank(buffer: &Audio) -> bool {
    (loudest(&rms_windows(buffer)) as f64) < SILENCE_RMS
}

/// The RMS of each whole 20 ms window, averaged over the channels.
fn rms_windows(buffer: &Audio) -> Vec<f32> {
    let win = 1.max(js_round(buffer.rate as f64 * 0.02) as usize);
    let mut energy = vec![0.0f32; buffer.frames() / win];
    for ch in 0..buffer.channels {
        for (f, en) in energy.iter_mut().enumerate() {
            let e: f64 = (f * win..f * win + win)
                .map(|i| {
                    let s = buffer.at(i, ch) as f64;
                    s * s
                })
                .sum();
            *en = (*en as f64 + (e / win as f64).sqrt() / buffer.channels as f64) as f32;
        }
    }
    energy
}

fn loudest(energy: &[f32]) -> f32 {
    energy
        .iter()
        .fold(0.0f32, |p, &e| if e > p { e } else { p })
}

/// `monoWindow`: `seconds` from the middle of the buffers, summed to mono.
/// The middle avoids intros and outros, the least representative parts.
fn mono_window(buffers: &[Audio], seconds: u32) -> Option<(Vec<f32>, u32)> {
    let rate = buffers.first()?.rate;
    let sr = rate as f64;
    // AudioBuffer.duration is length ÷ sampleRate, and the window is cut
    // from durations as the original cuts it.
    let dur = buffers
        .iter()
        .map(|b| b.frames() as f64 / b.rate as f64)
        .fold(f64::NEG_INFINITY, f64::max);
    let len = ((seconds as f64 * sr).floor()).min((dur * sr).floor()) as usize;
    if len < rate as usize {
        return None;
    }
    let start = 0f64.max((((dur - seconds as f64) / 2.0) * sr).floor()) as usize;
    let mut out = vec![0.0f32; len];
    for buf in buffers {
        let scale = 1.0 / buf.channels as f64;
        let n = len.min(buf.frames().saturating_sub(start));
        for ch in 0..buf.channels {
            for (i, o) in out[..n].iter_mut().enumerate() {
                *o = (*o as f64 + buf.at(start + i, ch) as f64 * scale) as f32;
            }
        }
    }
    Some((out, rate))
}

/// In-place iterative radix-2 FFT of a real signal in `re` (`im` zeroed),
/// with the original's `Float32Array` storage between butterflies.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut size = 2;
    while size <= n {
        let half = size >> 1;
        let step = (-2.0 * PI) / size as f64;
        for i in (0..n).step_by(size) {
            for k in 0..half {
                let wr = libm::cos(step * k as f64);
                let wi = libm::sin(step * k as f64);
                let (a, b) = (i + k, i + k + half);
                let tr = re[b] as f64 * wr - im[b] as f64 * wi;
                let ti = re[b] as f64 * wi + im[b] as f64 * wr;
                re[b] = (re[a] as f64 - tr) as f32;
                im[b] = (im[a] as f64 - ti) as f32;
                re[a] = (re[a] as f64 + tr) as f32;
                im[a] = (im[a] as f64 + ti) as f32;
            }
        }
        size <<= 1;
    }
}

/// JavaScript's `Math.round`: the nearest integer, halves up (−2.5 → −2).
pub(crate) fn js_round(x: f64) -> f64 {
    let floor = x.floor();
    if x - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_follows_math_round() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(-0.5), 0.0);
        assert_eq!(js_round(0.499_999_999_999_999_94), 0.0);
        assert_eq!(js_round(86.04), 86.0);
    }

    #[test]
    fn the_fft_finds_a_bin_centred_tone() {
        let n = 64;
        let mut re: Vec<f32> = (0..n)
            .map(|i| libm::cos(2.0 * PI * 5.0 * i as f64 / n as f64) as f32)
            .collect();
        let mut im = vec![0.0f32; n];
        fft(&mut re, &mut im);
        for k in 0..n / 2 {
            let mag = (re[k] as f64).hypot(im[k] as f64);
            let want = if k == 5 { n as f64 / 2.0 } else { 0.0 };
            assert!((mag - want).abs() < 1e-4, "bin {k}: {mag}");
        }
    }
}
