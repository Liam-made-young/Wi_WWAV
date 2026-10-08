//! `session.load`'s graph, checked and built (`docs/ENGINE.md` 3.3). It is
//! read from the JSON as it came, not through `wwav_wire::graph`, so each
//! thing wrong with it is refused with its own code and sentence, and a
//! feature of a later phase is refused as `unsupported` rather than ignored.

use crate::graph::{db_gain, role_of, Clip, Graph, Strip, Track, ROLES, ROLE_NAMES};
use crate::media::{
    self, ClipSource, Failure, HeldCache, MediaPart, Probe, Source, HOLD_WHOLE, MAX_SAMPLE,
};
use serde_json::{Map, Value};
use std::collections::HashSet;
use wwav_wire::shm::METER_SLOTS;

/// The loudest a fader or clip gain goes. Anything above is a mistake, and
/// would overflow to infinity long before it was a sound.
const MAX_GAIN_DB: f64 = 24.0;
/// A track id is a ULID (26 bytes). `session.load`'s answer lists every
/// track's id, so a cap on them keeps that answer far below a frame's 16 MiB.
const MAX_ID: usize = 1024;

/// A whole number that may have come as a float: 48000.0 is whole, 0.5 isn't.
pub fn whole_of(v: Option<&Value>) -> Option<i64> {
    let v = v?;
    v.as_i64().or_else(|| {
        v.as_f64()
            .filter(|f| f.is_finite() && f.abs() <= 9e15 && f.fract() == 0.0)
            .map(|f| f as i64)
    })
}

pub fn number_of(v: Option<&Value>) -> Option<f64> {
    v?.as_f64().filter(|f| f.is_finite())
}

/// A string field. One holding U+0000 names nothing.
pub fn text(v: Option<&Value>) -> &str {
    v.and_then(Value::as_str)
        .filter(|s| !s.contains('\0'))
        .unwrap_or("")
}

fn bad(message: impl Into<String>) -> Failure {
    Failure::new("bad_session", message)
}

fn unsupported(message: impl Into<String>) -> Failure {
    Failure::new("unsupported", message)
}

/// gain_db, pan, mute and solo, each optional. The master has no pan or solo.
fn read_strip(o: Option<&Value>, what: &str, master: bool) -> Result<Strip, Failure> {
    let mut s = Strip::default();
    let Some(o) = o.and_then(Value::as_object) else {
        return Ok(s);
    };
    if let Some(v) = o.get("gain_db") {
        s.gain_db = number_of(Some(v))
            .filter(|v| *v <= MAX_GAIN_DB)
            .ok_or_else(|| bad(format!("{what}'s gain_db is a number of dB up to +24.")))?
            as f32;
    }
    if let Some(v) = o.get("pan").filter(|_| !master) {
        s.pan = number_of(Some(v))
            .filter(|v| (-1.0..=1.0).contains(v))
            .ok_or_else(|| bad(format!("{what}'s pan is a number from -1 to 1.")))?
            as f32;
    }
    if let Some(v) = o.get("mute") {
        s.mute = v
            .as_bool()
            .ok_or_else(|| bad(format!("{what}'s mute is true or false.")))?;
    }
    if let Some(v) = o.get("solo").filter(|_| !master) {
        s.solo = v
            .as_bool()
            .ok_or_else(|| bad(format!("{what}'s solo is true or false.")))?;
    }
    Ok(s)
}

/// Devices are a later phase. One that is off, or that load switched off,
/// needs nothing from this one; one that is on is refused.
fn devices_off(devices: Option<&Value>, off: &HashSet<&str>, place: &str) -> Result<(), Failure> {
    let list = match devices {
        None | Some(Value::Null) => return Ok(()),
        Some(Value::Array(list)) => list,
        Some(_) => return Err(bad(format!("The devices on {place} are a list."))),
    };
    for d in list {
        let (id, format) = (text(d.get("id")), text(d.get("format")));
        if !d.is_object() || id.is_empty() {
            return Err(bad(format!("Every device on {place} needs an id.")));
        }
        if !matches!(format, "builtin" | "vst3" | "au") {
            return Err(bad(format!(
                "Device {id} has format \"{format}\"; it is builtin, vst3 or au."
            )));
        }
        let on = d.get("on").is_none_or(|v| v.as_bool() != Some(false));
        if !on || off.contains(id) {
            continue;
        }
        return Err(unsupported(if format == "builtin" {
            format!(
                "Built-in devices come in a later phase: \"{}\" ({id}) on {place} is on.",
                text(d.get("uid"))
            )
        } else {
            format!(
                "Plugins come in a later phase: {id} on {place} is a {} that is on.",
                if format == "vst3" { "VST3" } else { "AU" }
            )
        }));
    }
    Ok(())
}

/// `(path, source, the part if it reads directly, the session's rate)` to
/// the WAV to play instead.
pub type Convert<'a> =
    dyn FnMut(&str, Source, Option<&MediaPart>, u32) -> Result<Converted, Failure> + 'a;

/// What a build needs beyond the graph: the clips already held, and a way
/// to turn a file the engine can't read where it lies (compressed, or at
/// another rate) into one it can.
pub struct Sources<'a> {
    pub held: &'a mut HeldCache,
    /// `(path, source, part if it reads directly, session rate)` to the WAV
    /// to play instead and the rate the original was at.
    pub convert: &'a mut Convert<'a>,
}

/// A file made ready to play at the session's rate.
pub struct Converted {
    pub path: String,
    pub part: MediaPart,
    pub source_rate: u32,
}

fn read_clip(c: &Value, rate: u32, track: &str, sources: &mut Sources) -> Result<Clip, Failure> {
    let (id, path) = (text(c.get("id")), text(c.get("path")));
    let what = format!(
        "Clip {} on track {track}",
        if id.is_empty() { "?" } else { id }
    );
    if !c.is_object() || !path.starts_with('/') {
        return Err(bad(format!("{what} needs an absolute path.")));
    }
    let frame = |v: Option<&Value>| whole_of(v).filter(|n| (0..=MAX_SAMPLE).contains(n));
    let (Some(at), Some(first)) = (frame(c.get("at")), frame(c.get("in"))) else {
        return Err(bad(format!(
            "{what} needs at and in as whole numbers of frames from 0."
        )));
    };
    let len = frame(c.get("len")).filter(|n| *n > 0).ok_or_else(|| {
        bad(format!(
            "{what} needs len as a whole number of frames above 0."
        ))
    })?;
    let gain = match c.get("gain_db") {
        None => 0.0,
        v => number_of(v).filter(|g| *g <= MAX_GAIN_DB).ok_or_else(|| {
            bad(format!(
                "{what} has a gain_db that isn't a number of dB up to +24."
            ))
        })?,
    };
    if c.get("reverse").and_then(Value::as_bool) == Some(true) {
        return Err(unsupported(format!(
            "Reversed clips come in a later phase: {what} is reversed."
        )));
    }
    let name = text(c.get("source"));
    let source = Source::named(name).ok_or_else(|| {
        Failure::new(
            "bad_clip",
            format!("A clip's source is master, vocals, drums, other or bass, not \"{name}\"."),
        )
    })?;
    // A file read where it lies, or the WAV made from it at this rate. `in`
    // counts the file's own frames, so it moves with the rate.
    let (path, part, first) = match media::probe(path, source)? {
        Probe::Direct(part) if part.rate == rate => (path.to_string(), part, first),
        probed => {
            let direct = match &probed {
                Probe::Direct(part) => Some(part),
                Probe::Encoded(_) => None,
            };
            let made = (sources.convert)(path, source, direct, rate)?;
            let first = (first as i128 * rate as i128 / made.source_rate.max(1) as i128) as i64;
            (made.path, made.part, first)
        }
    };
    // Files under 32 MB are held whole; bigger ones stream (docs/SPEC.md 9.4).
    let whole = part.file_size < HOLD_WHOLE;
    Ok(Clip {
        at,
        gain: db_gain(gain as f32),
        source: ClipSource::open(&path, part, first, len, whole, sources.held)?,
    })
}

fn read_track(
    t: &Value,
    rate: u32,
    off: &HashSet<&str>,
    ids: &mut HashSet<String>,
    sources: &mut Sources,
) -> Result<(String, Track), Failure> {
    let (id, kind) = (text(t.get("id")), text(t.get("kind")));
    if !t.is_object() || id.is_empty() {
        return Err(bad("Every track needs an id."));
    }
    if id.len() > MAX_ID {
        return Err(bad(format!("A track's id is {MAX_ID} bytes at most.")));
    }
    if ids.contains(id) {
        return Err(bad(format!("Two tracks have the id {id}.")));
    }
    if id == "master" || id.starts_with("bus:") {
        return Err(bad(format!(
            "A track can't be called {id}; the buses and the master are."
        )));
    }
    ids.insert(id.to_string());
    match kind {
        "audio" | "stem" => {}
        "instrument" => {
            return Err(unsupported(format!(
                "Instrument tracks come with MIDI, in a later phase: {id}."
            )))
        }
        "bus" => {
            return Err(unsupported(format!(
                "Bus tracks (returns and groups) come in a later phase: {id}."
            )))
        }
        _ => {
            return Err(bad(format!(
                "Track {id} has kind \"{kind}\"; it is audio, instrument, stem or bus."
            )))
        }
    }
    let role = role_of(text(t.get("role"))).ok_or_else(|| {
        bad(format!(
            "Track {id}'s role is vocals, drums, other or bass."
        ))
    })?;
    let strip = read_strip(Some(t), &format!("Track {id}"), false)?;
    for send in ["reverb", "delay"] {
        if number_of(t.get("sends").and_then(|s| s.get(send))).is_some_and(|v| v > 0.0) {
            return Err(unsupported(format!(
                "Sends feed the built-in reverb and delay returns, which come in a later phase: track {id} sends to its {send}."
            )));
        }
    }
    devices_off(t.get("devices"), off, &format!("track {id}"))?;
    let mut clips = Vec::new();
    match t.get("clips") {
        None | Some(Value::Null) => {}
        Some(Value::Array(list)) => {
            for c in list {
                clips.push(read_clip(c, rate, id, sources)?);
            }
        }
        Some(_) => return Err(bad(format!("Track {id}'s clips are a list."))),
    }
    Ok((id.to_string(), Track { role, clips, strip }))
}

/// The graph of `session.load`'s args, numbered `gen`. `device_rate` is the
/// rate the audio thread runs at, which the session's must be.
pub fn build(
    args: &Map<String, Value>,
    device_rate: u32,
    gen: u64,
    sources: &mut Sources,
) -> Result<Graph, Failure> {
    let graph = args
        .get("graph")
        .filter(|g| g.is_object())
        .ok_or_else(|| bad("session.load needs a graph."))?;
    let rate = whole_of(graph.get("sample_rate"))
        .filter(|r| (8000..=384_000).contains(r))
        .ok_or_else(|| bad("The graph's sample_rate is a rate in Hz."))? as u32;
    if rate != device_rate {
        return Err(Failure::new(
            "rate_mismatch",
            format!(
                "This session runs at {rate} Hz and the device at {device_rate} Hz. Open the device at {rate} Hz first."
            ),
        ));
    }
    let off: HashSet<&str> = match args.get("off") {
        Some(Value::Array(ids)) => ids.iter().map(|id| text(Some(id))).collect(),
        _ => HashSet::new(),
    };
    let list = graph
        .get("tracks")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("The graph's tracks are a list."))?;
    let most = METER_SLOTS - ROLES - 1;
    if list.len() > most {
        return Err(bad(format!("A session holds {most} tracks at most.")));
    }
    let mut ids = HashSet::new();
    let mut tracks = Vec::with_capacity(list.len());
    for t in list {
        tracks.push(read_track(t, rate, &off, &mut ids, sources)?);
    }
    // The stem buses and the master. The contract's graph has the master's
    // gain_db; a bus's settings and the master's mute are read when present.
    let master = read_strip(graph.get("master"), "The master", true)?;
    devices_off(
        graph.get("master").and_then(|m| m.get("devices")),
        &off,
        "the master",
    )?;
    let mut buses = [Strip::default(); ROLES];
    for (r, name) in ROLE_NAMES.iter().enumerate() {
        buses[r] = read_strip(
            graph.get("buses").and_then(|b| b.get(name)),
            &format!("bus:{name}"),
            false,
        )?;
    }
    Ok(Graph::new(gen, rate, tracks, buses, master))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn whole_numbers_may_come_as_floats() {
        assert_eq!(whole_of(Some(&json!(48000))), Some(48000));
        assert_eq!(whole_of(Some(&json!(48000.0))), Some(48000));
        assert_eq!(whole_of(Some(&json!(0.5))), None);
        assert_eq!(whole_of(Some(&json!("1"))), None);
        assert_eq!(whole_of(Some(&json!(u64::MAX))), None);
        assert_eq!(whole_of(None), None);
    }

    #[test]
    fn a_string_holding_nul_names_nothing() {
        assert_eq!(text(Some(&json!("x\u{0}"))), "");
        assert_eq!(text(Some(&json!(5))), "");
        assert_eq!(text(Some(&json!("x"))), "x");
    }

    fn refuse(graph: Value) -> Failure {
        let args = json!({"graph": graph});
        let mut held = HeldCache::default();
        let mut sources = Sources {
            held: &mut held,
            convert: &mut |_, _, _, _| {
                Err(Failure::new("unsupported", "no converter in this test."))
            },
        };
        build(args.as_object().unwrap(), 48000, 1, &mut sources)
            .err()
            .expect("refused")
    }

    #[test]
    fn what_is_wrong_with_a_graph_has_its_own_code() {
        let track = |extra: Value| {
            let mut t = json!({"id": "T", "kind": "audio", "role": "bass"});
            t.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            json!({"sample_rate": 48000, "tracks": [t]})
        };
        assert_eq!(
            refuse(json!({"sample_rate": 44100, "tracks": []})).code,
            "rate_mismatch"
        );
        assert_eq!(refuse(json!({"tracks": []})).code, "bad_session");
        assert_eq!(refuse(track(json!({"role": "keys"}))).code, "bad_session");
        assert_eq!(
            refuse(track(json!({"kind": "instrument"}))).code,
            "unsupported"
        );
        assert_eq!(
            refuse(track(json!({"sends": {"reverb": 0.5}}))).code,
            "unsupported"
        );
        assert_eq!(refuse(track(json!({"gain_db": 30.0}))).code, "bad_session");
        assert_eq!(refuse(track(json!({"id": "bus:bass"}))).code, "bad_session");
        let device = json!({"devices": [{"id": "D", "format": "vst3", "uid": "U"}]});
        assert_eq!(refuse(track(device)).code, "unsupported");
        let clip = json!({"clips": [{"id": "C", "path": "/no/such.wav", "source": "master", "at": 0, "in": 0, "len": 9}]});
        assert_eq!(refuse(track(clip)).code, "no_such_file");
    }
}
