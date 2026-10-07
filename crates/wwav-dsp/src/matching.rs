//! The figure-eight's matching (4.6) and the Console's **Match to session**
//! (5.9): "The child bends entirely to the parent." Ported from
//! `analysis.js` (`octaveFold`, `keyMeetShifts`, `matchToParent`) and the
//! drum alignment in v4's `PlayerContext.jsx`.

use crate::analysis::{js_round, Estimates, Key};
use std::f64::consts::{FRAC_1_SQRT_2, SQRT_2};

/// How the child plays against the parent's timeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Match {
    /// The child's tempo ratio, in [1/√2, √2).
    pub rel_rate: f64,
    /// Semitones the child moves, −6 to +6.
    pub key_shift: i32,
    /// Where on the parent's timeline the child comes in, in seconds.
    pub enter_at: f64,
    /// How far into its own audio the child starts, in seconds. The child's
    /// position at parent time t is `child_from + (t − enter_at) × rel_rate`.
    pub child_from: f64,
}

/// Folds a tempo ratio into [1/√2, √2), within half an octave of unity.
/// Tempo estimates are octave-ambiguous (a 170 song reads as 85 and back),
/// so a half or double misreading still gives the same musical result, and
/// rates stay where stretching sounds clean. Anything that isn't a positive
/// number folds to 1.
pub fn octave_fold(ratio: f64) -> f64 {
    if !ratio.is_finite() || ratio <= 0.0 {
        return 1.0;
    }
    let mut r = ratio;
    while r < FRAC_1_SQRT_2 {
        r *= 2.0;
    }
    while r >= SQRT_2 {
        r /= 2.0;
    }
    r
}

/// Semitone shifts that land two keys on matching tonics with the least
/// total movement: (shift for `a`, shift for `b`). Minor keys are compared
/// through their relative major (tonic + 3), so A minor already agrees with
/// C major.
pub fn key_meet_shifts(a: Key, b: Key) -> (i32, i32) {
    let relative_major = |k: Key| (if k.minor { (k.pc + 3) % 12 } else { k.pc }) as i32;
    let mut d = (relative_major(b) - relative_major(a)) % 12;
    if d > 6 {
        d -= 12;
    }
    if d < -6 {
        d += 12;
    }
    let shift_a = js_round(d as f64 / 2.0) as i32;
    (shift_a, shift_a - d)
}

/// The parent keeps its own tempo and key and the child adopts both: the
/// octave-folded tempo ratio, the key shift that moves the child's tonic
/// onto the parent's (meeting halfway, then handing the parent's share to
/// the child), and first drum hits aligned. A missing tempo or key leaves
/// that part alone. The parent may be a session whose tempo and key were
/// typed rather than estimated; the maths is the same.
pub fn match_to_parent(parent: &Estimates, child: &Estimates) -> Match {
    let rel_rate = match (parent.bpm, child.bpm) {
        (Some(p), Some(c)) => octave_fold(p / c),
        _ => 1.0,
    };
    let key_shift = match (parent.key, child.key) {
        (Some(p), Some(c)) => {
            let (a, b) = key_meet_shifts(p, c);
            b - a
        }
        _ => 0,
    };
    // The child enters at s so the first onsets coincide; an onset earlier
    // (scaled) than the parent's means the child starts part way in instead.
    let mut enter_at = parent.drum_onset - child.drum_onset / rel_rate;
    let mut child_from = 0.0;
    if enter_at < 0.0 {
        child_from = -enter_at * rel_rate;
        enter_at = 0.0;
    }
    Match {
        rel_rate,
        key_shift,
        enter_at,
        child_from,
    }
}
