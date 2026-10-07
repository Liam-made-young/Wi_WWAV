//! The figure-eight's matching maths (4.6, 5.9), ported from `analysis.js`
//! and `PlayerContext.jsx`: an octave-folded tempo ratio, a key shift within
//! ±6 semitones through the relative major, and first downbeats aligned.
//! "The child bends entirely to the parent."

use wwav_dsp::analysis::{Estimates, Key};
use wwav_dsp::matching::{key_meet_shifts, match_to_parent, octave_fold};

fn key(name: &str) -> Key {
    Key::parse(name).unwrap()
}

fn song(bpm: Option<f64>, k: Option<&str>, onset: f64) -> Estimates {
    Estimates {
        bpm,
        key: k.map(key),
        drum_onset: onset,
    }
}

#[test]
fn tempo_ratios_fold_to_within_half_an_octave() {
    assert_eq!(octave_fold(1.0), 1.0);
    assert_eq!(octave_fold(2.0), 1.0);
    assert_eq!(octave_fold(0.5), 1.0);
    assert_eq!(octave_fold(1.5), 0.75);
    assert_eq!(octave_fold(0.6), 1.2);
    assert_eq!(octave_fold(170.0 / 85.0), 1.0);
    for r in [0.01, 0.3, 0.70, 0.71, 1.41, 1.42, 3.0, 99.0] {
        let f = octave_fold(r);
        assert!((std::f64::consts::FRAC_1_SQRT_2..std::f64::consts::SQRT_2).contains(&f));
    }
    // Nothing to fold: play at the child's own rate.
    assert_eq!(octave_fold(0.0), 1.0);
    assert_eq!(octave_fold(-2.0), 1.0);
    assert_eq!(octave_fold(f64::NAN), 1.0);
    assert_eq!(octave_fold(f64::INFINITY), 1.0);
}

#[test]
fn keys_meet_through_the_relative_major() {
    // A minor is C major's relative minor: nothing moves.
    assert_eq!(key_meet_shifts(key("A minor"), key("C major")), (0, 0));
    // C major to D major: two semitones, met halfway.
    assert_eq!(key_meet_shifts(key("C major"), key("D major")), (1, -1));
    for (a, b) in [
        ("C major", "F♯ major"),
        ("E minor", "G♯ major"),
        ("D♯ minor", "A♯ minor"),
    ] {
        let (sa, sb) = key_meet_shifts(key(a), key(b));
        let rel = |k: Key| if k.minor { (k.pc + 3) % 12 } else { k.pc } as i32;
        assert_eq!(
            (rel(key(a)) + sa).rem_euclid(12),
            (rel(key(b)) + sb).rem_euclid(12),
            "{a} and {b}"
        );
        assert!((sb - sa).abs() <= 6);
    }
}

#[test]
fn the_child_bends_entirely_to_the_parent() {
    let parent = song(Some(120.0), Some("A minor"), 0.5);
    let child = song(Some(86.0), Some("E minor"), 0.2);
    let m = match_to_parent(&parent, &child);
    assert!((m.rel_rate - 120.0 / 86.0).abs() < 1e-12);
    // E minor's relative major is G, five below C: the child goes up 5.
    assert_eq!(m.key_shift, 5);
    let child_key = key("E minor").transposed(m.key_shift);
    assert_eq!(child_key.to_string(), "A minor");
    // The child comes in at s so its first drum hit lands on the parent's.
    assert!((m.enter_at - (0.5 - 0.2 / m.rel_rate)).abs() < 1e-12);
    assert_eq!(m.child_from, 0.0);
}

#[test]
fn a_child_whose_drums_start_later_starts_part_way_in() {
    let parent = song(Some(100.0), None, 0.1);
    let child = song(Some(200.0), None, 1.3);
    let m = match_to_parent(&parent, &child);
    assert_eq!(m.rel_rate, 1.0); // 100 ÷ 200 folds to 1
    assert_eq!(m.enter_at, 0.0);
    assert!((m.child_from - 1.2).abs() < 1e-12);
    // Child position at timeline t is child_from + (t − enter_at) × rel_rate:
    // at the parent's first hit, the child is at its own.
    let at_parent_hit = m.child_from + (0.1 - m.enter_at) * m.rel_rate;
    assert!((at_parent_hit - 1.3).abs() < 1e-12);
}

#[test]
fn missing_estimates_leave_that_part_alone() {
    let m = match_to_parent(
        &song(None, None, 0.0),
        &song(Some(90.0), Some("C major"), 0.0),
    );
    assert_eq!((m.rel_rate, m.key_shift), (1.0, 0));
    let m = match_to_parent(
        &song(Some(90.0), Some("D major"), 0.0),
        &song(None, None, 0.0),
    );
    assert_eq!((m.rel_rate, m.key_shift), (1.0, 0));
}

#[test]
fn key_shifts_stay_within_six_semitones() {
    let names = ["major", "minor"];
    for pa in 0..12u8 {
        for pb in 0..12u8 {
            for ma in names {
                for mb in names {
                    let a = Key {
                        pc: pa,
                        minor: ma == "minor",
                    };
                    let b = Key {
                        pc: pb,
                        minor: mb == "minor",
                    };
                    let m = match_to_parent(
                        &Estimates {
                            bpm: None,
                            key: Some(a),
                            drum_onset: 0.0,
                        },
                        &Estimates {
                            bpm: None,
                            key: Some(b),
                            drum_onset: 0.0,
                        },
                    );
                    assert!(m.key_shift.abs() <= 6);
                    let rel = |k: Key| if k.minor { (k.pc + 3) % 12 } else { k.pc } as i32;
                    assert_eq!((rel(b) + m.key_shift).rem_euclid(12), rel(a));
                }
            }
        }
    }
}
