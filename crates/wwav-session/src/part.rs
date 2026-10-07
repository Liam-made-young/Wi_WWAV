//! What an edit changed, found without writing the whole session out.
//!
//! [`diff`](crate::diff) compares two documents, and on a long session
//! turning the session into JSON is most of an edit's cost. Here the
//! session's own types are compared first, which is cheap, and only the
//! parts that differ become JSON: moving a clip writes one event, adding a
//! note one note. The rows are the ones `diff` would give for the same
//! change. Rows applied to the document by undo and redo are applied to
//! the session the same way, part by part ([`apply`]).
//!
//! `Session`, `Track` and `Automation` are walked field by field, lists and
//! maps element by element; every other type is a part changed as one.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::diff::{self, Mismatch, Row};
use crate::model::*;
use crate::Error;

/// Why a part couldn't be done part by part.
pub(crate) enum Fault {
    /// The new value wouldn't read back (a NaN where a number must be).
    Unsaveable(String),
    /// The session and its document don't agree where they should, so the
    /// whole session is compared instead.
    Drift,
}

/// What a row does at its path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Op {
    Insert,
    Remove,
    Replace,
}

impl Op {
    fn of(from: Option<&Value>, to: Option<&Value>) -> Op {
        match (from, to) {
            (None, _) => Op::Insert,
            (_, None) => Op::Remove,
            _ => Op::Replace,
        }
    }
}

/// A part of the session.
pub(crate) trait Part: Serialize + DeserializeOwned + PartialEq + Sized {
    /// Reads the part from its JSON.
    fn read(v: Value) -> Result<Self, String> {
        serde_json::from_value(v).map_err(|e| e.to_string())
    }

    /// Pushes the rows that turn `old`, whose JSON is `doc` at `path`, into
    /// `new`, and leaves `new` as it reads back (a NaN in an optional field
    /// reads back as absent).
    fn rows(
        old: &Self,
        new: &mut Self,
        doc: &Value,
        path: &str,
        rows: &mut Vec<Row>,
    ) -> Result<(), Fault> {
        whole(old, new, doc, path, rows)
    }

    /// Brings the part up to date after a row at `tokens` below it was
    /// applied to its JSON, `doc`.
    fn patch(&mut self, doc: &Value, tokens: &[String], op: Op) -> Result<(), Fault> {
        let _ = (tokens, op);
        reread(self, doc)
    }
}

/// The rows that turn `old`, whose JSON is `doc`, into `new`, leaving `new`
/// as it reads back. An empty list means nothing changed.
pub(crate) fn rows(old: &Session, new: &mut Session, doc: &Value) -> Result<Vec<Row>, Error> {
    let mut rows = Vec::new();
    match Session::rows(old, new, doc, "", &mut rows) {
        Ok(()) => Ok(rows),
        Err(Fault::Unsaveable(e)) => Err(Error::Unsaveable(e)),
        Err(Fault::Drift) => {
            *new = read_back(new).map_err(|_| Error::Unsaveable("the session".into()))?;
            Ok(diff::diff(doc, &json(new)))
        }
    }
}

/// Applies rows forward (redo) or backward (undo) to the document and to
/// the session together: all of them, or, when one doesn't fit, none.
pub(crate) fn apply(
    doc: &mut Value,
    session: &mut Session,
    rows: &[Row],
    forward: bool,
) -> Result<(), Mismatch> {
    let ordered: Vec<&Row> = if forward {
        rows.iter().collect()
    } else {
        rows.iter().rev().collect()
    };
    for (k, row) in ordered.iter().enumerate() {
        let (from, to) = row.ends(forward);
        if let Err(m) = diff::put(doc, &row.path, from, to) {
            return Err(roll_back(doc, session, &ordered[..k], forward, m));
        }
        if session
            .patch(doc, &diff::tokens(&row.path), Op::of(from, to))
            .is_err()
        {
            // The session and its document parted: read it whole.
            match Session::from_value(doc.clone()) {
                Ok(s) => *session = s,
                Err(_) => {
                    let m = Mismatch(row.path.clone());
                    return Err(roll_back(doc, session, &ordered[..=k], forward, m));
                }
            }
        }
    }
    Ok(())
}

/// Takes back the rows applied so far, newest first, and reads the session
/// from the document they leave, which is the one it started from.
fn roll_back(
    doc: &mut Value,
    session: &mut Session,
    applied: &[&Row],
    forward: bool,
    m: Mismatch,
) -> Mismatch {
    for row in applied.iter().rev() {
        let (from, to) = row.ends(forward);
        let _ = diff::put(doc, &row.path, to, from);
    }
    if let Ok(s) = Session::from_value(doc.clone()) {
        *session = s;
    }
    m
}

fn json<T: Serialize>(x: &T) -> Value {
    serde_json::to_value(x).expect("the model always serializes")
}

fn read_back<T: Part>(x: &T) -> Result<T, Fault> {
    T::read(json(x)).map_err(Fault::Unsaveable)
}

fn reread<T: Part>(part: &mut T, doc: &Value) -> Result<(), Fault> {
    *part = T::read(doc.clone()).map_err(|_| Fault::Drift)?;
    Ok(())
}

/// The part as one: written as JSON and compared with its document.
fn whole<T: Part>(
    old: &T,
    new: &mut T,
    doc: &Value,
    path: &str,
    rows: &mut Vec<Row>,
) -> Result<(), Fault> {
    if old == new {
        return Ok(());
    }
    let back = read_back(new)?;
    diff::walk(path.to_string(), doc, &json(&back), rows);
    *new = back;
    Ok(())
}

/// A field of a walked struct, always present in its JSON as `key`.
fn field<T: Part>(
    old: &T,
    new: &mut T,
    doc: &Value,
    path: &str,
    key: &str,
    rows: &mut Vec<Row>,
) -> Result<(), Fault> {
    if old == new {
        return Ok(());
    }
    let node = doc.get(key).ok_or(Fault::Drift)?;
    T::rows(old, new, node, &format!("{path}/{key}"), rows)
}

fn patch_field<T: Part>(
    part: &mut T,
    doc: &Value,
    key: &str,
    rest: &[String],
    op: Op,
) -> Result<(), Fault> {
    part.patch(doc.get(key).ok_or(Fault::Drift)?, rest, op)
}

/// A key the model doesn't know is kept as the document has it.
fn patch_extra(extra: &mut Map<String, Value>, doc: &Value, key: &str) {
    match doc.get(key) {
        Some(v) => {
            extra.insert(key.to_string(), v.clone());
        }
        None => {
            extra.remove(key);
        }
    }
}

/// A struct walked field by field. Its unknown keys sit beside its fields
/// in the JSON, so when they change it is done as one, as it is when
/// `whole_if` holds. Every field is named, so a field added to the struct
/// and not here doesn't compile.
macro_rules! walked {
    (
        $ty:ident { $($field:ident: $key:literal),* $(,)? }
        $(whole_if |$o:ident, $n:ident| $cond:expr;)?
        $(read: $read:expr;)?
    ) => {
        impl Part for $ty {
            $(fn read(v: Value) -> Result<Self, String> {
                ($read)(v)
            })?

            fn rows(
                old: &Self,
                new: &mut Self,
                doc: &Value,
                path: &str,
                rows: &mut Vec<Row>,
            ) -> Result<(), Fault> {
                let as_one = old.extra != new.extra $(|| {
                    let ($o, $n): (&Self, &Self) = (old, &*new);
                    $cond
                })?;
                if as_one {
                    return whole(old, new, doc, path, rows);
                }
                let $ty { $($field,)* extra: _ } = new;
                $(field(&old.$field, $field, doc, path, $key, rows)?;)*
                Ok(())
            }

            fn patch(&mut self, doc: &Value, tokens: &[String], op: Op) -> Result<(), Fault> {
                let Some((key, rest)) = tokens.split_first() else {
                    return reread(self, doc);
                };
                match key.as_str() {
                    $($key => patch_field(&mut self.$field, doc, $key, rest, op),)*
                    _ => {
                        patch_extra(&mut self.extra, doc, key);
                        Ok(())
                    }
                }
            }
        }
    };
}

walked!(
    Session {
        wwavsession: "wwavsession",
        id: "id",
        kind: "kind",
        title: "title",
        key: "key",
        sample_rate: "sample_rate",
        frame: "frame",
        tempo_map: "tempo_map",
        buses: "buses",
        tracks: "tracks",
        master: "master",
        midi: "midi",
        video: "video",
        lineage: "lineage",
    }
    // A new version is refused as it is on open.
    whole_if |o, n| o.wwavsession != n.wwavsession;
    read: |v| Session::from_value(v).map_err(|e| e.to_string());
);

walked!(Track {
    id: "id",
    kind: "kind",
    name: "name",
    role: "role",
    gain_db: "gain",
    pan: "pan",
    mute: "mute",
    solo: "solo",
    sends: "sends",
    devices: "devices",
    automation: "automation",
    events: "events",
});

walked!(Automation {
    param: "param",
    points: "points",
});

/// Parts changed as one.
macro_rules! whole_parts {
    ($($ty:ty),* $(,)?) => {
        $(impl Part for $ty {})*
    };
}

whole_parts!(
    String,
    f64,
    u32,
    bool,
    Option<String>,
    Option<Frame>,
    Option<Role>,
    SequenceKind,
    TrackKind,
    Role,
    Sends,
    Master,
    Device,
    TempoPoint,
    Event,
    Note,
    AutomationPoint,
    Video,
    Lineage,
);

impl<T: Part> Part for Vec<T> {
    /// As `diff` does: keep what both ends share, pair up the middle
    /// element by element, then insert or remove the rest at the first
    /// unpaired index.
    fn rows(
        old: &Self,
        new: &mut Self,
        doc: &Value,
        path: &str,
        rows: &mut Vec<Row>,
    ) -> Result<(), Fault> {
        let Value::Array(was) = doc else {
            return Err(Fault::Drift);
        };
        if was.len() != old.len() {
            return Err(Fault::Drift);
        }
        let prefix = old
            .iter()
            .zip(new.iter())
            .take_while(|(a, b)| a == b)
            .count();
        let room = old.len().min(new.len()) - prefix;
        let suffix = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(room)
            .take_while(|(a, b)| a == b)
            .count();
        let (old_end, new_end) = (old.len() - suffix, new.len() - suffix);
        let at = prefix + (old_end - prefix).min(new_end - prefix);
        for i in prefix..at {
            T::rows(&old[i], &mut new[i], &was[i], &format!("{path}/{i}"), rows)?;
        }
        for (i, item) in new.iter_mut().enumerate().take(new_end).skip(at) {
            let back = read_back(item)?;
            rows.push(Row {
                path: format!("{path}/{i}"),
                before: None,
                after: Some(json(&back)),
            });
            *item = back;
        }
        for gone in &was[at..old_end] {
            rows.push(Row {
                path: format!("{path}/{at}"),
                before: Some(gone.clone()),
                after: None,
            });
        }
        Ok(())
    }

    fn patch(&mut self, doc: &Value, tokens: &[String], op: Op) -> Result<(), Fault> {
        let Some((first, rest)) = tokens.split_first() else {
            return reread(self, doc);
        };
        let i: usize = first.parse().map_err(|_| Fault::Drift)?;
        let node = || doc.get(i).ok_or(Fault::Drift);
        match (rest.is_empty(), op) {
            (true, Op::Insert) if i <= self.len() => {
                let item = T::read(node()?.clone()).map_err(|_| Fault::Drift)?;
                self.insert(i, item);
            }
            (true, Op::Remove) if i < self.len() => {
                self.remove(i);
            }
            (true, Op::Replace) | (false, _) => {
                self.get_mut(i)
                    .ok_or(Fault::Drift)?
                    .patch(node()?, rest, op)?;
            }
            _ => return Err(Fault::Drift),
        }
        Ok(())
    }
}

impl<T: Part> Part for BTreeMap<String, T> {
    fn rows(
        old: &Self,
        new: &mut Self,
        doc: &Value,
        path: &str,
        rows: &mut Vec<Row>,
    ) -> Result<(), Fault> {
        let Value::Object(was) = doc else {
            return Err(Fault::Drift);
        };
        for (key, o) in old {
            let p = format!("{path}/{}", diff::escape(key));
            let w = was.get(key).ok_or(Fault::Drift)?;
            match new.get_mut(key) {
                Some(n) => T::rows(o, n, w, &p, rows)?,
                None => rows.push(Row {
                    path: p,
                    before: Some(w.clone()),
                    after: None,
                }),
            }
        }
        for (key, n) in new.iter_mut() {
            if !old.contains_key(key) {
                let back = read_back(n)?;
                rows.push(Row {
                    path: format!("{path}/{}", diff::escape(key)),
                    before: None,
                    after: Some(json(&back)),
                });
                *n = back;
            }
        }
        Ok(())
    }

    fn patch(&mut self, doc: &Value, tokens: &[String], op: Op) -> Result<(), Fault> {
        let Some((key, rest)) = tokens.split_first() else {
            return reread(self, doc);
        };
        let node = || doc.get(key).ok_or(Fault::Drift);
        match (rest.is_empty(), op) {
            (true, Op::Remove) => {
                self.remove(key).ok_or(Fault::Drift)?;
            }
            (true, _) => {
                let item = T::read(node()?.clone()).map_err(|_| Fault::Drift)?;
                self.insert(key.clone(), item);
            }
            (false, _) => {
                self.get_mut(key)
                    .ok_or(Fault::Drift)?
                    .patch(node()?, rest, op)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    /// A tiny deterministic generator, so a failure names its seed.
    pub(crate) struct Rng(pub u64);

    impl Rng {
        pub fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        pub fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    fn note(k: u64) -> Note {
        Note {
            pitch: (k % 128) as u8,
            vel: 1 + (k % 127) as u8,
            at_beats: (k % 64) as f64 * 0.25,
            len_beats: 0.5,
            played_at_beats: (k % 64) as f64 * 0.25 + 0.0131,
            played_len_beats: 0.4871,
            ..Note::default()
        }
    }

    /// Three audio tracks of four clips and an instrument clip of twenty
    /// notes.
    pub(crate) fn start() -> Session {
        let mut s = Session::new("Parts");
        for i in 0..3 {
            let mut t = Track::new(&format!("T{i}"), TrackKind::Audio);
            t.events = (0..4).map(|k| Event::new("C", k * 1000)).collect();
            s.tracks.push(t);
        }
        s.midi.insert("M1".into(), (0..20).map(note).collect());
        s
    }

    /// One change of a kind the Console makes, anywhere in the session.
    pub(crate) fn change(s: &mut Session, rng: &mut Rng) {
        let k = rng.next();
        let pick = |n: usize| (k as usize / 11) % n.max(1);
        let nt = s.tracks.len();
        match rng.below(21) {
            0 => {
                if let Some(t) = s.tracks.get_mut(pick(nt)) {
                    let n = t.events.len();
                    if let Some(e) = t.events.get_mut(pick(n)) {
                        e.at_ms = (k % 99_999) as i64;
                    }
                }
            }
            1 => s.midi.entry("M1".into()).or_default().push(note(k)),
            2 => {
                let notes = s.midi.entry("M1".into()).or_default();
                if !notes.is_empty() {
                    notes.remove(pick(notes.len()));
                }
            }
            3 => {
                let notes = s.midi.entry("M1".into()).or_default();
                notes.insert(pick(notes.len() + 1), note(k));
            }
            4 => {
                s.midi
                    .insert(format!("M{}", k % 5), vec![note(k), note(k + 1)]);
            }
            5 => {
                if let Some(key) = s.midi.keys().nth(pick(3)).cloned() {
                    s.midi.remove(&key);
                }
            }
            6 => {
                let mut t = Track::new(&format!("N{k}"), TrackKind::Instrument);
                t.events.push(Event::new("M1", 0));
                s.tracks.insert(pick(nt + 1), t);
            }
            7 => {
                if nt > 0 {
                    s.tracks.remove(pick(nt));
                }
            }
            8 => {
                if let Some(t) = s.tracks.get_mut(pick(nt)) {
                    match k % 4 {
                        0 => t.gain_db = -((k % 600) as f64) / 10.0,
                        1 => t.pan = -t.pan + 0.25,
                        2 => t.mute = !t.mute,
                        _ => t.role = [None, Some(Role::Bass)][(k / 4 % 2) as usize].clone(),
                    }
                }
            }
            9 => {
                if let Some(t) = s.tracks.get_mut(pick(nt)) {
                    let n = t.events.len();
                    if let Some(e) = t.events.get_mut(pick(n)) {
                        // NaN reads back as absent.
                        e.at_beats = [None, Some(2.5), Some(f64::NAN)][(k % 3) as usize];
                    }
                }
            }
            10 => {
                if let Some(t) = s.tracks.get_mut(pick(nt)) {
                    if t.automation.is_empty() || k.is_multiple_of(3) {
                        t.automation.push(Automation {
                            param: "pan".into(),
                            ..Automation::default()
                        });
                    }
                    let lanes = t.automation.len();
                    let lane = &mut t.automation[pick(lanes)];
                    match lane.points.len() {
                        0 => lane.points.push(AutomationPoint::default()),
                        n => lane.points[pick(n)].value = (k % 100) as f64 / 100.0,
                    }
                }
            }
            11 => s.title = format!("Take {}", k % 7),
            12 => s.key = [None, Some("A minor".to_string())][(k % 2) as usize].clone(),
            13 => {
                s.frame = k.is_multiple_of(2).then(|| Frame {
                    w: 1920,
                    h: 1080,
                    fps: 24,
                    ..Frame::default()
                })
            }
            14 => {
                // An unknown key at the top: the session is done as one.
                s.extra.insert(format!("x{}", k % 3), json!(k % 5));
            }
            15 => {
                if let Some(t) = s.tracks.get_mut(pick(nt)) {
                    t.extra.insert("color".into(), json!(k % 4));
                }
            }
            16 => {
                let notes = s.midi.entry("M1".into()).or_default();
                let n = notes.len();
                if let Some(x) = notes.get_mut(pick(n)) {
                    x.extra.insert("chance".into(), json!(0.5));
                }
            }
            17 => s.tempo_map[0].bpm = 60.0 + (k % 120) as f64,
            18 => s.video.edits.push(json!({"cut": k % 1000})),
            19 => s.tracks.reverse(),
            _ => {
                if let Some(t) = s.tracks.get_mut(pick(nt)) {
                    match t.devices.first_mut() {
                        Some(d) => {
                            d.params.insert("mix".into(), (k % 10) as f64 / 10.0);
                        }
                        None => t.devices.push(Device::default()),
                    }
                }
            }
        }
    }

    #[test]
    fn rows_are_what_diff_gives_and_apply_both_ways() {
        for seed in 1..=40u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
            let mut s = Session::from_value(json(&start())).unwrap();
            let mut doc = json(&s);
            for step in 0..150 {
                let mut new = s.clone();
                for _ in 0..=rng.below(3) {
                    change(&mut new, &mut rng);
                }
                let raw = new.clone();
                let fresh = json(&s);
                let rows = rows(&s, &mut new, &fresh).unwrap();
                let at = format!("seed {seed}, step {step}");
                // What the session reads back as, and the rows the whole
                // documents' diff gives. A NaN that reads back as absent
                // can pair list elements differently (NaN isn't equal to
                // itself): other rows, as correct, which the checks below
                // still hold them to.
                assert_eq!(new, Session::from_value(json(&raw)).unwrap(), "{at}");
                if raw == new {
                    assert_eq!(rows, diff::diff(&fresh, &json(&new)), "{at}");
                }
                // Applied to the document and the session together, forward
                // and back.
                let (mut d, mut t) = (doc.clone(), s.clone());
                apply(&mut d, &mut t, &rows, true).unwrap();
                assert_eq!(d, json(&new), "{at}: redo");
                assert_eq!(t, new, "{at}: redo");
                apply(&mut d, &mut t, &rows, false).unwrap();
                assert_eq!(d, doc, "{at}: undo");
                assert_eq!(t, s, "{at}: undo");
                apply(&mut doc, &mut s, &rows, true).unwrap();
            }
        }
    }

    #[test]
    fn a_change_that_would_not_read_back_is_refused() {
        let s = Session::from_value(json(&start())).unwrap();
        let doc = json(&s);
        let mut nan = s.clone();
        nan.tracks[1].gain_db = f64::NAN;
        assert!(matches!(
            rows(&s, &mut nan, &doc),
            Err(Error::Unsaveable(_))
        ));
        let mut newer = s.clone();
        newer.wwavsession = "1.0".into();
        assert!(matches!(
            rows(&s, &mut newer, &doc),
            Err(Error::Unsaveable(_))
        ));
    }

    #[test]
    fn a_row_that_does_not_fit_changes_neither() {
        let s = Session::from_value(json(&start())).unwrap();
        let mut new = s.clone();
        new.title = "Other".into();
        new.tracks[0].events[1].at_ms = 7;
        let mut rows = rows(&s, &mut new, &json(&s)).unwrap();
        assert_eq!(rows.len(), 2);
        rows[1].before = Some(json!(12345)); // not what the document holds
        let (mut d, mut t) = (json(&s), s.clone());
        assert!(apply(&mut d, &mut t, &rows, true).is_err());
        assert_eq!(d, json(&s));
        assert_eq!(t, s);
    }
}
