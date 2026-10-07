//! The port gives what `analysis.js` gives. These five songs were written out
//! as f32 samples and read by Mi-WWAV's `wwav/src/audio/analysis.js`
//! (`analyzeTrack`, Node 22, 2026-10-07); its results are below. On the same
//! samples the port matched it to the bit, before rounding too: the tempo
//! (86.09430881177664 for the first) and the key's Pearson score.

use wwav_dsp::analysis::{analyze, Audio, Stems};

const RATE: u32 = 44_100;

fn drums(bpm: f64, seconds: f64, first: f64) -> Vec<f32> {
    let frames = (seconds * RATE as f64) as usize;
    let mut out = vec![0.0f32; frames * 2];
    let mut t0 = first;
    while t0 < seconds {
        let start = (t0 * RATE as f64).round() as usize;
        for i in 0..(0.25 * RATE as f64) as usize {
            if start + i >= frames {
                break;
            }
            let t = i as f64 / RATE as f64;
            let s = 0.8 * (-t / 0.03).exp() * (2.0 * std::f64::consts::PI * 80.0 * t).sin();
            out[(start + i) * 2] = s as f32;
            out[(start + i) * 2 + 1] = s as f32;
        }
        t0 += 60.0 / bpm;
    }
    out
}

fn chords(progression: &[[i32; 3]], seconds: f64) -> Vec<f32> {
    let frames = (seconds * RATE as f64) as usize;
    let per = 2 * RATE as usize;
    (0..frames)
        .flat_map(|n| {
            let chord = progression[(n / per) % progression.len()];
            let t = n as f64 / RATE as f64;
            let s: f64 = chord
                .iter()
                .map(|&st| {
                    let f = 440.0 * 2f64.powf(st as f64 / 12.0);
                    0.2 * (2.0 * std::f64::consts::PI * f * t).sin()
                })
                .sum();
            [s as f32, (s * 0.7) as f32]
        })
        .collect()
}

/// Noise in bursts, from a 64-bit LCG.
fn noise(seconds: f64, seed: u64) -> Vec<f32> {
    let mut s = seed;
    let frames = (seconds * RATE as f64) as usize;
    (0..frames * 2)
        .map(|i| {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let v = ((s >> 33) as f64 / (1u64 << 31) as f64 - 1.0) * 0.3;
            let env = if (i / 2) % 20_000 < 3_000 { 1.0 } else { 0.1 };
            (v * env) as f32
        })
        .collect()
}

fn audio(x: &[f32]) -> Option<Audio<'_>> {
    Some(Audio {
        samples: x,
        channels: 2,
        rate: RATE,
    })
}

#[test]
fn the_port_matches_analysis_js() {
    let am = [[0, 3, 7], [5, 8, 12], [7, 11, 14], [0, 3, 7]];
    let cm = [[3, 7, 10], [8, 12, 15], [10, 14, 17], [3, 7, 10]];
    let fsm = [[9, 12, 16], [14, 17, 21], [16, 20, 23], [9, 12, 16]];

    // vocals, drums, other, bass; then what analysis.js returned.
    type Song = [Option<Vec<f32>>; 4];
    let songs: [(Song, f64, &str, f64); 5] = [
        (
            [
                Some(chords(&am, 70.0)),
                Some(drums(86.0, 70.0, 0.75)),
                None,
                None,
            ],
            86.1,
            "A minor",
            0.74,
        ),
        (
            [
                None,
                Some(drums(60.0, 63.3, 0.31)),
                Some(chords(&cm, 63.3)),
                None,
            ],
            120.2,
            "C major",
            0.3,
        ),
        (
            [
                None,
                Some(drums(176.0, 50.0, 2.0)),
                None,
                Some(chords(&fsm, 50.0)),
            ],
            87.8,
            "F♯ minor",
            2.0,
        ),
        (
            [Some(noise(40.0, 3)), None, Some(chords(&am, 41.0)), None],
            135.6,
            "A minor",
            0.0,
        ),
        (
            [
                Some(noise(30.0, 5)),
                Some(noise(65.0, 9)),
                Some(drums(131.0, 66.0, 1.1)),
                Some(chords(&cm, 64.0)),
            ],
            132.4,
            "C major",
            0.0,
        ),
    ];

    for (i, (song, bpm, key, onset)) in songs.iter().enumerate() {
        let [vocals, drums, other, bass] = song;
        let e = analyze(&Stems {
            vocals: vocals.as_deref().and_then(audio),
            drums: drums.as_deref().and_then(audio),
            other: other.as_deref().and_then(audio),
            bass: bass.as_deref().and_then(audio),
        });
        assert_eq!(e.bpm, Some(*bpm), "song {i}");
        assert_eq!(
            e.key.map(|k| k.to_string()).as_deref(),
            Some(*key),
            "song {i}"
        );
        assert_eq!(e.drum_onset, *onset, "song {i}");
    }
}
