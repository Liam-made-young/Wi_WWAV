//! Library search (docs/PLAN.md S1.8). It fails if a search takes over 50 ms
//! with 50,000 clips in the library. Timing means nothing in a debug build, so
//! this runs with `cargo test -p wi-store --release --test search_speed`.

mod common;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::{Duration, Instant};
use wi_store::*;

const CLIPS: usize = 50_000;
const BUDGET: Duration = Duration::from_millis(50);

fn word(rng: &mut StdRng) -> String {
    const SYLLABLES: [&str; 24] = [
        "lo", "ti", "de", "glas", "hour", "wor", "end", "ing", "be", "yon", "cé", "ra", "mu", "sa", "ka", "ne",
        "on", "vel", "dri", "ft", "noc", "tur", "ne", "ah",
    ];
    (0..rng.gen_range(1..=3)).map(|_| SYLLABLES[rng.gen_range(0..SYLLABLES.len())]).collect()
}

fn title(rng: &mut StdRng) -> String {
    (0..rng.gen_range(1..=4)).map(|_| word(rng)).collect::<Vec<_>>().join(" ")
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timing needs --release")]
fn search_answers_in_under_50_ms_at_50000_clips() {
    let (_dir, mut store) = common::library();
    let mut rng = StdRng::seed_from_u64(50_000);
    let built = Instant::now();
    for batch in 0..CLIPS / 5_000 {
        let mut tx = store.begin(Room::Library, "import 5,000 clips").unwrap();
        for n in 0..5_000 {
            let id = wwav_ids::ulid();
            let info = Inspection {
                artist: format!("{} {}", word(&mut rng), word(&mut rng)),
                bpm: Some(rng.gen_range(60..180) as f64),
                key: Some(["A minor", "C major", "F# minor", "Eb major"][rng.gen_range(0..4)].to_string()),
                duration_ms: rng.gen_range(30_000..400_000),
                verdict: "4 stems, and the master".into(),
                ..Inspection::new(Kind::Wwav, &title(&mut rng))
            };
            let clip = NewClip {
                file: format!("media/{id}.wwav"),
                id,
                sha256: "0".repeat(64),
                bytes: 52_900_000,
                info,
                from_sequence: None,
            };
            let id = tx.add_clip(clip).unwrap();
            if (batch * 5_000 + n) % 7 == 0 {
                tx.add_tag(&id, "live-drums", TagKind::User).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    eprintln!("built {CLIPS} clips in {:.1?}", built.elapsed());

    let any = SmartRule::default();
    let live_minor = SmartRule {
        tags: vec!["live-drums".into()],
        bpm_min: Some(128.0),
        bpm_max: Some(132.0),
        key: Some("minor".into()),
        ..SmartRule::default()
    };
    let queries: [(&str, &SmartRule); 9] = [
        ("", &any),
        ("l", &any),
        ("lo", &any),
        ("lotide", &any),
        ("glas hour", &any),
        ("beyonce", &any),
        ("zzz", &any),
        ("", &live_minor),
        ("ra", &live_minor),
    ];
    let mut worst = Duration::ZERO;
    for (text, rule) in queries {
        store.search(text, rule, 200).unwrap(); // warm the page cache once
        let mut slowest = Duration::ZERO;
        let mut found = 0;
        for _ in 0..5 {
            let start = Instant::now();
            found = store.search(text, rule, 200).unwrap().len();
            slowest = slowest.max(start.elapsed());
        }
        eprintln!("{text:>10?} {}: {found:>3} results, slowest of 5 {slowest:.2?}", if rule == &any { "" } else { "+rule" });
        worst = worst.max(slowest);
    }
    eprintln!("slowest search at {CLIPS} clips: {worst:.2?}");
    assert!(worst < BUDGET, "library search took {worst:?} at {CLIPS} clips; the budget is {BUDGET:?}");
}
