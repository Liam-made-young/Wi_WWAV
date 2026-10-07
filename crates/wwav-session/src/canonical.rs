//! The canonical writer: `session.json` is UTF-8 with LF line endings, a
//! two-space indent and keys in a fixed order, "so saving the same session
//! twice gives the same bytes and a session kept in git diffs line by line"
//! (`docs/SPEC.md` 6.5).
//!
//! The order is [`KEY_ORDER`] and nowhere else: the writer reorders every
//! object by it, whatever order its keys were made or read in. Zero has one
//! form: -0.0 is written 0.0.

use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::Value;

const DEVICE: &[&str] = &[
    "id", "format", "uid", "name", "version", "on", "params", "state",
];
const STATE: &[&str] = &["inline", "file", "bytes", "sha256"];

/// The order keys are written in, for each kind of object, named by its
/// place in the document with `*` for any array index or map key.
///
/// Keys an object has that its entry doesn't list (a newer app's) follow
/// the listed ones, sorted. An object with no entry is a map keyed by data,
/// such as `midi` by clip ULID, and is written sorted.
pub const KEY_ORDER: &[(&str, &[&str])] = &[
    (
        "",
        &[
            "wwavsession",
            "id",
            "kind",
            "title",
            "key",
            "sample_rate",
            "frame",
            "tempo_map",
            "buses",
            "tracks",
            "master",
            "midi",
            "video",
            "lineage",
        ],
    ),
    ("/frame", &["w", "h", "fps"]),
    ("/tempo_map/*", &["at_beats", "bpm"]),
    (
        "/tracks/*",
        &[
            "id",
            "kind",
            "name",
            "role",
            "gain",
            "pan",
            "mute",
            "solo",
            "sends",
            "devices",
            "automation",
            "events",
        ],
    ),
    ("/tracks/*/sends", &["reverb", "delay"]),
    ("/tracks/*/devices/*", DEVICE),
    ("/tracks/*/devices/*/state", STATE),
    ("/tracks/*/automation/*", &["param", "points"]),
    ("/tracks/*/automation/*/points/*", &["at_beats", "value"]),
    (
        "/tracks/*/events/*",
        &[
            "clip_id",
            "source",
            "clip_in_ms",
            "clip_out_ms",
            "at_ms",
            "at_beats",
            "params",
        ],
    ),
    (
        "/tracks/*/events/*/params",
        &[
            "gain",
            "pan",
            "time",
            "transpose",
            "reverse",
            "loop",
            "grade",
        ],
    ),
    ("/master", &["gain", "devices"]),
    ("/master/devices/*", DEVICE),
    ("/master/devices/*/state", STATE),
    (
        "/midi/*/*",
        &[
            "pitch",
            "vel",
            "release_vel",
            "at_beats",
            "len_beats",
            "played_at_beats",
            "played_len_beats",
        ],
    ),
    ("/video", &["edits", "grade", "titles"]),
    ("/lineage", &["work", "from", "exports"]),
    ("/lineage/work", &["song_id", "film_id", "version"]),
    ("/lineage/from", &["song_id", "version", "title", "sha256"]),
    ("/lineage/exports/*", &["file", "version", "sha256", "at"]),
];

/// The listed keys for the object at `place`, or None for a map.
pub fn key_order(place: &str) -> Option<&'static [&'static str]> {
    KEY_ORDER
        .iter()
        .find(|(p, _)| *p == place)
        .map(|(_, keys)| *keys)
}

/// The document's canonical bytes, ending in a newline.
pub fn to_bytes(doc: &Value) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * 1024);
    let mut ser = serde_json::Serializer::with_formatter(
        &mut out,
        serde_json::ser::PrettyFormatter::with_indent(b"  "),
    );
    Ordered {
        v: doc,
        place: String::new(),
    }
    .serialize(&mut ser)
    .expect("a JSON value always serializes");
    out.push(b'\n');
    out
}

/// A value written with its objects' keys in canonical order, without
/// copying it.
struct Ordered<'a> {
    v: &'a Value,
    place: String,
}

impl Ordered<'_> {
    fn child<'b>(&self, v: &'b Value, step: &str) -> Ordered<'b> {
        // Only objects look their place up, so leaves don't build one.
        let place = match v {
            Value::Object(_) | Value::Array(_) => format!("{}/{step}", self.place),
            _ => String::new(),
        };
        Ordered { v, place }
    }
}

impl Serialize for Ordered<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.v {
            Value::Object(m) => {
                let mut keys: Vec<&String> = m.keys().collect();
                let mut out = s.serialize_map(Some(m.len()))?;
                match key_order(&self.place) {
                    Some(listed) => {
                        let rank =
                            |k: &str| listed.iter().position(|l| *l == k).unwrap_or(listed.len());
                        keys.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.cmp(b)));
                        for k in keys {
                            out.serialize_entry(k, &self.child(&m[k], k))?;
                        }
                    }
                    None => {
                        keys.sort();
                        for k in keys {
                            out.serialize_entry(k, &self.child(&m[k], "*"))?;
                        }
                    }
                }
                out.end()
            }
            Value::Array(a) => {
                let mut out = s.serialize_seq(Some(a.len()))?;
                for x in a {
                    out.serialize_element(&self.child(x, "*"))?;
                }
                out.end()
            }
            // One zero: -0.0 and 0.0 are the same number to `==`, to the
            // journal and to every reader, so they are written alike (as
            // JSON.stringify and RFC 8785 write them).
            Value::Number(n) if n.is_f64() && n.as_f64() == Some(0.0) => s.serialize_f64(0.0),
            leaf => leaf.serialize(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use serde_json::json;
    use std::collections::BTreeSet;

    /// Every object a session holds, by place.
    fn objects(v: &Value, place: &str, out: &mut Vec<(String, Vec<String>)>) {
        match v {
            Value::Object(m) => {
                out.push((place.to_string(), m.keys().cloned().collect()));
                let is_map = key_order(place).is_none();
                for (k, x) in m {
                    let step = if is_map { "*" } else { k.as_str() };
                    objects(x, &format!("{place}/{step}"), out);
                }
            }
            Value::Array(a) => {
                for x in a {
                    objects(x, &format!("{place}/*"), out);
                }
            }
            _ => {}
        }
    }

    /// One of everything the model can hold.
    fn full_session() -> Session {
        let device = Device {
            id: "D".into(),
            format: DeviceFormat::Vst3,
            uid: "0123456789abcdef0123456789abcdef".into(),
            name: "Tape Echo".into(),
            version: "2.1.4".into(),
            params: [("mix".to_string(), 0.5)].into(),
            state: Some(PluginState {
                inline: Some("AAEC".into()),
                file: Some("plugin-state/X.bin".into()),
                bytes: 3,
                sha256: "ab".into(),
                ..PluginState::default()
            }),
            ..Device::default()
        };
        let mut track = Track::new("T", TrackKind::Instrument);
        track.role = Some(Role::Other);
        track.devices.push(device.clone());
        track.automation.push(Automation {
            param: "pan".into(),
            points: vec![AutomationPoint::default()],
            ..Automation::default()
        });
        let mut event = Event::new("C", 0);
        event.source = Some("vocals".into());
        event.at_beats = Some(1.0);
        event.params.grade = Some(json!({"saturation": 100}));
        track.events.push(event);
        let mut s = Session::new("Full");
        s.key = Some("A minor".into());
        s.frame = Some(Frame {
            w: 1920,
            h: 1080,
            fps: 24,
            ..Frame::default()
        });
        s.tracks.push(track);
        s.master.devices.push(device);
        s.midi.insert("C".into(), vec![Note::default()]);
        s.video.edits.push(json!({"cut": 1}));
        s.video.grade.insert("contrast".into(), json!(100));
        s.video.titles.push(json!({"text": "Low Tide"}));
        s.lineage.from = Some(LineageFrom::default());
        s.lineage.exports.push(Export::default());
        s
    }

    #[test]
    fn every_key_the_model_writes_has_its_place_in_the_order() {
        // Fails if a key falls to the sorted tail because the table forgot
        // it, or the table names a place no session has.
        let mut found = Vec::new();
        objects(&full_session().to_value(), "", &mut found);
        let maps = [
            "/midi",
            "/tracks/*/devices/*/params",
            "/master/devices/*/params",
            "/tracks/*/events/*/params/grade",
            "/video/grade",
            "/video/edits/*",
            "/video/titles/*",
        ];
        let mut places = BTreeSet::new();
        for (place, keys) in &found {
            match key_order(place) {
                Some(listed) => {
                    for k in keys {
                        assert!(
                            listed.contains(&k.as_str()),
                            "{place}: {k} isn't in KEY_ORDER"
                        );
                    }
                    places.insert(place.clone());
                }
                None => assert!(maps.contains(&place.as_str()), "{place} has no order"),
            }
        }
        for (place, _) in KEY_ORDER {
            assert!(
                places.contains(*place),
                "{place:?} is in KEY_ORDER but no session has it"
            );
        }
    }

    #[test]
    fn keys_follow_the_table_then_unknown_keys_sorted() {
        let doc = json!({
            "zz": 1, "tracks": [], "id": "X", "aa": {"b": 1, "a": 2}, "wwavsession": "0.1",
            "midi": {"02": [], "01": [{"vel": 9, "pitch": 60, "x": true}]}
        });
        let text = String::from_utf8(to_bytes(&doc)).unwrap();
        assert_eq!(
            text,
            r#"{
  "wwavsession": "0.1",
  "id": "X",
  "tracks": [],
  "midi": {
    "01": [
      {
        "pitch": 60,
        "vel": 9,
        "x": true
      }
    ],
    "02": []
  },
  "aa": {
    "a": 2,
    "b": 1
  },
  "zz": 1
}
"#
        );
    }

    #[test]
    fn zero_is_written_one_way() {
        let doc = json!({"tracks": [{"pan": -0.0, "gain": 0.0}], "x": [-0.0, -1.5, 0]});
        let text = String::from_utf8(to_bytes(&doc)).unwrap();
        assert!(!text.contains("-0.0"), "{text}");
        assert!(text.contains("-1.5"));
        assert!(text.contains("\"pan\": 0.0"));
        assert!(
            text.contains("\n    0\n"),
            "an integer zero stays an integer"
        );
    }

    #[test]
    fn a_new_session_is_written_like_this() {
        let mut s = Session::new("Low Tide");
        s.id = "01JC5Q8V3M2T7R9X4K6W0YHZNB".into();
        s.lineage.work.song_id = "9f2c".into();
        s.lineage.work.film_id = "41ab".into();
        s.tempo_map[0].bpm = 86.0;
        let mut t = Track::new("01JC5Q8V3M2T7R9X4K6W0YHZT0", TrackKind::Audio);
        t.role = Some(Role::Vocals);
        t.events
            .push(Event::new("01JC5Q8V3M2T7R9X4K6W0YHZC0", 1500));
        s.tracks.push(t);
        let text = String::from_utf8(s.to_json_bytes()).unwrap();
        assert_eq!(
            text,
            r#"{
  "wwavsession": "0.1",
  "id": "01JC5Q8V3M2T7R9X4K6W0YHZNB",
  "kind": "session",
  "title": "Low Tide",
  "key": null,
  "sample_rate": 48000,
  "frame": null,
  "tempo_map": [
    {
      "at_beats": 0.0,
      "bpm": 86.0
    }
  ],
  "buses": [
    "vocals",
    "drums",
    "other",
    "bass"
  ],
  "tracks": [
    {
      "id": "01JC5Q8V3M2T7R9X4K6W0YHZT0",
      "kind": "audio",
      "name": "",
      "role": "vocals",
      "gain": 0.0,
      "pan": 0.0,
      "mute": false,
      "solo": false,
      "sends": {
        "reverb": 0.0,
        "delay": 0.0
      },
      "devices": [],
      "automation": [],
      "events": [
        {
          "clip_id": "01JC5Q8V3M2T7R9X4K6W0YHZC0",
          "clip_in_ms": 0,
          "clip_out_ms": 0,
          "at_ms": 1500,
          "params": {
            "gain": 0.0,
            "pan": 0.0,
            "time": "as_played",
            "transpose": 0.0,
            "reverse": false,
            "loop": false
          }
        }
      ]
    }
  ],
  "master": {
    "gain": 0.0,
    "devices": []
  },
  "midi": {},
  "video": {
    "edits": [],
    "grade": {},
    "titles": []
  },
  "lineage": {
    "work": {
      "song_id": "9f2c",
      "film_id": "41ab",
      "version": 1
    },
    "from": null,
    "exports": []
  }
}
"#
        );
    }
}
