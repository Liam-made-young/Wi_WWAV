#![allow(dead_code)] // each test file uses its own share of these

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use wwav_session::model::{Event, Note, Role, Session, Track, TrackKind};
use wwav_session::Clock;

/// 2026-10-06 21:12:00 UTC.
pub const T0: u64 = 1_791_321_120_000;

/// A clock the test moves by hand.
pub struct FakeClock(AtomicU64);

impl FakeClock {
    pub fn at(ms: u64) -> Arc<FakeClock> {
        Arc::new(FakeClock(AtomicU64::new(ms)))
    }

    pub fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

pub const CLIP: &str = "01JC5Q8V3M2T7R9X4K6W0YHZNB";
pub const MIDI_CLIP: &str = "01JC5Q8V3M2T7R9X4K6W0YHZNC";

/// A small session with two audio tracks, one clip each, and an
/// instrument clip of `notes` notes. Its ids are fixed, so two calls give
/// the same session.
pub fn sample_session(notes: usize) -> Session {
    let mut s = Session::new("Low Tide");
    s.id = "01JC5Q8V3M2T7R9X4K6W0YHZN0".into();
    s.lineage.work.song_id = "9f2c0000000000000000000000000001".into();
    s.lineage.work.film_id = "41ab0000000000000000000000000002".into();
    s.key = Some("A minor".into());
    for (i, role) in [Role::Vocals, Role::Drums].into_iter().enumerate() {
        let mut t = Track::new(&format!("01JC5Q8V3M2T7R9X4K6W0YHZT{i}"), TrackKind::Audio);
        t.name = format!("Track {}", i + 1);
        t.role = Some(role);
        t.events.push(Event::new(CLIP, (i as i64) * 1000));
        s.tracks.push(t);
    }
    let roll = s.midi.entry(MIDI_CLIP.into()).or_default();
    for i in 0..notes {
        roll.push(note(i as u64));
    }
    s
}

pub fn note(k: u64) -> Note {
    let at = (k % 64) as f64 * 0.25 + 0.0131;
    Note {
        pitch: 36 + (k % 48) as u8,
        vel: 1 + (k % 127) as u8,
        release_vel: 64,
        at_beats: at,
        len_beats: 0.231,
        played_at_beats: at,
        played_len_beats: 0.231,
        ..Note::default()
    }
}
