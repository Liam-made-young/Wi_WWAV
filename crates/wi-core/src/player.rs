//! The one listening player (docs/SPEC.md 2.3; docs/COMMANDS.md player.*).
//! The engine owns it: loading a song sends the engine a graph of four stem
//! tracks (or one master track for a song that has no stems), and every
//! stem change is a `param.set`. Where it stands comes from the engine's
//! shared-memory clock, never from a timer here.
//!
//! Pressing play in the Console, or opening a film, pauses it with
//! `pausedFor` set; nothing resumes on its own.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Weak};
use std::time::Duration;

use serde_json::{json, Value};
use wi_store::Kind;
use wwav_wire::graph::{Clip, Graph, Master, Role, Sends, Source, Track, TrackKind};
use wwav_wire::shm::{ClockFields, STATE_PLAYING};

use crate::args::Args;
use crate::bus::lock;
use crate::engine::EngineSession;
use crate::{CoreError, Inner};

const CALL: Duration = Duration::from_secs(2);
/// A level of 0 is silence: the engine takes decibels, and -144 dB is below
/// anything 24 bits can carry.
const SILENT_DB: f64 = -144.0;
/// What wwav-formats says of a file whose four stems play.
pub(crate) const STEMS_VERDICT: &str = "4 stems, and the master";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PausedFor {
    Console,
    Film,
}

impl PausedFor {
    fn as_str(self) -> &'static str {
        match self {
            PausedFor::Console => "console",
            PausedFor::Film => "film",
        }
    }
}

struct Stem {
    role: Role,
    track: String,
    level: f64,
    mute: bool,
    solo: bool,
}

struct Loaded {
    clip: String,
    title: String,
    key: Option<String>,
    bpm: Option<f64>,
    frames: u64,
    /// The file's own sample rate, which the session runs at: 44.1 kHz for a
    /// .wwav, and a plain WAV's own. The device follows it (docs/SPEC.md 8.4).
    rate: u32,
    session: EngineSession,
    /// The four stems in PRANA's order, or none for a master-only song.
    stems: Vec<Stem>,
    slots: BTreeMap<String, usize>,
}

#[derive(Default)]
pub(crate) struct Player {
    loaded: Option<Loaded>,
    paused_for: Option<PausedFor>,
    /// False once something else has the engine (the Console, a film).
    on_engine: bool,
    /// Where the player stood when it last let go of the engine, in samples.
    kept_at: i64,
}

fn level_db(level: f64) -> f64 {
    if level <= 0.0 {
        SILENT_DB
    } else {
        20.0 * level.log10()
    }
}

/// A WAVE file's master: its frames (its data chunk over its frame size) and
/// the sample rate they are at.
fn measure(path: &Path) -> Result<(u64, u32), CoreError> {
    let w = wwav_formats::wwav::Wwav::open(path)
        .map_err(|e| CoreError::new("unreadable", e.to_string()))?;
    let fmt = w
        .fmt
        .ok_or_else(|| CoreError::new("unreadable", "This file has no fmt chunk."))?;
    if fmt.rate == 0 {
        return Err(CoreError::new(
            "unreadable",
            "This file's sample rate is 0 Hz.",
        ));
    }
    let frame = (fmt.channels as u64 * fmt.bits as u64 / 8).max(1);
    Ok((w.first(b"data").map_or(0, |c| c.size / frame), fmt.rate))
}

fn stem_name(role: Role) -> &'static str {
    match role {
        Role::Vocals => "Vocals",
        Role::Drums => "Drums",
        Role::Other => "Other",
        Role::Bass => "Bass",
    }
}

fn track(id: &str, kind: TrackKind, role: Role, path: &str, source: Source, frames: u64) -> Track {
    Track {
        id: id.to_string(),
        kind,
        role,
        gain_db: 0.0,
        pan: 0.0,
        mute: false,
        solo: false,
        sends: Sends::default(),
        devices: Vec::new(),
        clips: vec![Clip {
            id: wwav_ids::ulid(),
            path: path.to_string(),
            source,
            at: 0,
            in_frame: 0,
            len: frames as i64,
            gain_db: 0.0,
            reverse: false,
        }],
        notes: Vec::new(),
    }
}

impl Player {
    fn clock(&self, i: &Inner) -> Option<ClockFields> {
        self.on_engine.then(|| i.engine.clock()).flatten()
    }

    fn state(&self, i: &Inner) -> Value {
        let Some(l) = &self.loaded else {
            return json!({
                "clip": null, "title": null, "key": null, "bpm": null, "playing": false,
                "position": 0.0, "duration": 0.0, "stems": [], "pausedFor": self.paused_for.map(PausedFor::as_str),
            });
        };
        let rate = l.rate as f64;
        let clock = self.clock(i);
        let sample = clock.map_or(self.kept_at, |c| c.sample_pos.max(0));
        let duration = l.frames as f64 / rate;
        let stems: Vec<Value> = l
            .stems
            .iter()
            .map(|s| {
                json!({
                    "stem": s.role.name(), "level": s.level, "mute": s.mute, "solo": s.solo,
                    "slot": l.slots.get(&s.track),
                })
            })
            .collect();
        json!({
            "clip": l.clip,
            "title": l.title,
            "key": l.key,
            "bpm": l.bpm,
            "playing": clock.is_some_and(|c| c.state == STATE_PLAYING),
            "position": (sample as f64 / rate).min(duration),
            "duration": duration,
            "stems": stems,
            "masterSlot": l.slots.get("master"),
            "pausedFor": self.paused_for.map(PausedFor::as_str),
        })
    }
}

fn emit(i: &Inner, p: &Player) -> Value {
    let state = p.state(i);
    i.bus.emit("player", state.clone());
    state
}

/// The engine's transport changes reach the UI as `player` events.
pub(crate) fn listen(inner: &Arc<Inner>) {
    let weak: Weak<Inner> = Arc::downgrade(inner);
    inner.engine.set_listener(Arc::new(move |ev| {
        if ev.ev != "transport" {
            return;
        }
        if let Some(i) = weak.upgrade() {
            let p = lock(&i.player);
            if p.loaded.is_some() && p.on_engine {
                emit(&i, &p);
            }
        }
    }));
}

fn nothing_loaded() -> CoreError {
    CoreError::new(
        "nothing_loaded",
        "Nothing is loaded. Select a song and press Space.",
    )
}

pub(crate) fn load(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let id = a.str("clip")?;
    let (clip, path) = {
        let store = i.store();
        let clip = store
            .clip(id)?
            .ok_or_else(|| CoreError::new("refused", "That clip isn't in the library any more."))?;
        let path = store.path_of(&clip);
        (clip, path)
    };
    if !matches!(clip.kind, Kind::Wwav | Kind::Audio) {
        return Err(CoreError::new(
            "not_a_song",
            "Only songs play in the player.",
        ));
    }
    let (frames, rate) = measure(&path)?;
    let file = path.to_string_lossy().into_owned();
    let mut names = BTreeMap::new();
    let mut stems = Vec::new();
    let tracks = if clip.kind == Kind::Wwav && clip.verdict == STEMS_VERDICT {
        Role::ALL
            .into_iter()
            .map(|role| {
                let id = wwav_ids::ulid();
                names.insert(id.clone(), stem_name(role).to_string());
                let source = match role {
                    Role::Vocals => Source::Vocals,
                    Role::Drums => Source::Drums,
                    Role::Other => Source::Other,
                    Role::Bass => Source::Bass,
                };
                stems.push(Stem {
                    role,
                    track: id.clone(),
                    level: 1.0,
                    mute: false,
                    solo: false,
                });
                track(&id, TrackKind::Stem, role, &file, source, frames)
            })
            .collect()
    } else {
        // Plain audio, or a song whose stems don't read: the master alone.
        let id = wwav_ids::ulid();
        names.insert(id.clone(), "Master".to_string());
        vec![track(
            &id,
            TrackKind::Audio,
            Role::Other,
            &file,
            Source::Master,
            frames,
        )]
    };
    let session = EngineSession {
        graph: Graph {
            sample_rate: rate,
            tempo_map: Vec::new(),
            tracks,
            master: Master::default(),
            state_dir: None,
        },
        names,
    };
    let mut p = lock(&i.player);
    if p.on_engine && p.clock(i).is_some_and(|c| c.state == STATE_PLAYING) {
        i.engine.call("transport.stop", Value::Null, CALL)?;
    }
    let slots = i.engine.load(session.clone(), 0)?;
    i.engine
        .call("transport.locate", json!({"sample": 0}), CALL)?;
    i.engine.next_block();
    p.loaded = Some(Loaded {
        clip: clip.id,
        title: clip.title,
        key: clip.key,
        bpm: clip.bpm,
        frames,
        rate,
        session,
        stems,
        slots,
    });
    p.on_engine = true;
    p.paused_for = None;
    p.kept_at = 0;
    Ok(json!({"state": emit(i, &p)}))
}

pub(crate) fn play(i: &Inner) -> Result<Value, CoreError> {
    let mut p = lock(&i.player);
    let Some(l) = &p.loaded else {
        return Err(nothing_loaded());
    };
    if !p.on_engine {
        // Something else had the engine: put the song back where it stood.
        let slots = i.engine.load(l.session.clone(), p.kept_at)?;
        i.engine
            .call("transport.locate", json!({"sample": p.kept_at}), CALL)?;
        if let Some(l) = p.loaded.as_mut() {
            l.slots = slots;
        }
        p.on_engine = true;
    }
    i.engine.call("transport.play", Value::Null, CALL)?;
    i.engine.next_block();
    p.paused_for = None;
    Ok(json!({"state": emit(i, &p)}))
}

pub(crate) fn pause(i: &Inner) -> Result<Value, CoreError> {
    let mut p = lock(&i.player);
    if p.loaded.is_none() {
        return Err(nothing_loaded());
    }
    if p.on_engine {
        let r = i.engine.call("transport.stop", Value::Null, CALL)?;
        p.kept_at = r.get("sample").and_then(Value::as_i64).unwrap_or(p.kept_at);
        i.engine.next_block();
    }
    Ok(json!({"state": emit(i, &p)}))
}

pub(crate) fn seek(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let seconds = a.f64("seconds")?;
    let mut p = lock(&i.player);
    let Some(l) = &p.loaded else {
        return Err(nothing_loaded());
    };
    let rate = l.rate as f64;
    let duration = l.frames as f64 / rate;
    let sample = (seconds.clamp(0.0, duration) * rate).round() as i64;
    if p.on_engine {
        i.engine
            .call("transport.locate", json!({"sample": sample}), CALL)?;
        i.engine.next_block();
    }
    p.kept_at = sample;
    Ok(json!({"state": emit(i, &p)}))
}

pub(crate) fn stem(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let name = a.str("stem")?;
    let level = a.opt_f64("level")?;
    let mute = a.opt_bool("mute")?;
    let solo = a.opt_bool("solo")?;
    if level.is_some_and(|l| !(0.0..=1.0).contains(&l)) {
        return Err(CoreError::new("bad_args", "A stem's level is from 0 to 1."));
    }
    let mut p = lock(&i.player);
    let on_engine = p.on_engine;
    let l = p.loaded.as_mut().ok_or_else(nothing_loaded)?;
    let s = l
        .stems
        .iter_mut()
        .find(|s| s.role.name() == name)
        .ok_or_else(|| {
            CoreError::new(
                "no_such_stem",
                "This song has no stems: it plays as the master only.",
            )
        })?;
    let track = s.track.clone();
    let mut changes = Vec::new();
    if let Some(v) = mute {
        s.mute = v;
        changes.push(("mute", json!(v)));
    }
    if let Some(v) = solo {
        s.solo = v;
        changes.push(("solo", json!(v)));
    }
    if let Some(v) = level {
        s.level = v;
        changes.push(("gain_db", json!(level_db(v))));
    }
    for (param, value) in changes {
        // The player's own copy of the session holds every change, so a
        // reload after the Console put it back as it was.
        if let Some(t) = l.session.graph.tracks.iter_mut().find(|t| t.id == track) {
            match param {
                "mute" => t.mute = value.as_bool().unwrap_or(false),
                "solo" => t.solo = value.as_bool().unwrap_or(false),
                _ => t.gain_db = value.as_f64().unwrap_or(0.0),
            }
        }
        if on_engine {
            i.engine.param(&track, param, value)?;
        }
    }
    Ok(json!({"state": emit(i, &p)}))
}

/// 2.3: "Listening pauses and the strip reads 'Paused for the Console'.
/// Stopping the Console doesn't resume it."
pub(crate) fn pause_for(i: &Inner, why: PausedFor) -> Result<Value, CoreError> {
    let mut p = lock(&i.player);
    if p.loaded.is_none() {
        return Ok(json!({"state": p.state(i)}));
    }
    if p.on_engine {
        if let Some(c) = p.clock(i) {
            if c.state == STATE_PLAYING {
                let r = i.engine.call("transport.stop", Value::Null, CALL)?;
                p.kept_at = r
                    .get("sample")
                    .and_then(Value::as_i64)
                    .unwrap_or(c.sample_pos);
                p.paused_for = Some(why);
                i.engine.next_block();
            } else {
                p.kept_at = c.sample_pos.max(0);
            }
        }
    }
    // The Console or the film takes the engine from here.
    p.on_engine = false;
    Ok(json!({"state": emit(i, &p)}))
}
