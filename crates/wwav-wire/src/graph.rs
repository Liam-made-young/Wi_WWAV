//! The graph `session.load` sends (`docs/ENGINE.md` §3.3). The session lives
//! in the app; the engine only renders what this describes.
//!
//! Fields the contract marks optional, and the numbers a fresh track starts
//! with, have defaults, so a hand-written graph (a test, a CLI script) can
//! leave them out.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub sample_rate: u32,
    #[serde(default)]
    pub tempo_map: Vec<Tempo>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub master: Master,
    /// The session's `plugin-state/` folder, where states over 256 KB go.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tempo {
    pub at_beats: f64,
    pub bpm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKind {
    Audio,
    Instrument,
    Stem,
    Bus,
}

/// The four stem roles, in the order the meter slots and stems use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Vocals,
    Drums,
    Other,
    Bass,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Vocals, Role::Drums, Role::Other, Role::Bass];

    pub fn name(self) -> &'static str {
        match self {
            Role::Vocals => "vocals",
            Role::Drums => "drums",
            Role::Other => "other",
            Role::Bass => "bass",
        }
    }

    /// The node id of this role's stem bus: `bus:vocals` …
    pub fn bus(self) -> String {
        format!("bus:{}", self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub kind: TrackKind,
    pub role: Role,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub pan: f64,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub solo: bool,
    #[serde(default)]
    pub sends: Sends,
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default)]
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub notes: Vec<Note>,
}

/// 0..1, to the role's reverb and delay returns.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Sends {
    #[serde(default)]
    pub reverb: f64,
    #[serde(default)]
    pub delay: f64,
}

/// Which part of a file a clip plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Master,
    Vocals,
    Drums,
    Other,
    Bass,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: String,
    pub path: String,
    pub source: Source,
    /// Session sample.
    pub at: i64,
    /// File frame.
    #[serde(rename = "in", default)]
    pub in_frame: i64,
    /// Frames.
    pub len: i64,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub reverse: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub pitch: u8,
    pub vel: u8,
    pub at: i64,
    pub len: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Builtin,
    Vst3,
    Au,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub format: Format,
    pub uid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default)]
    pub params: Map<String, Value>,
    #[serde(default)]
    pub state: Option<PluginState>,
    #[serde(default = "on")]
    pub on: bool,
}

fn on() -> bool {
    true
}

/// A plugin's saved state: inline as base64, or in a file when over 256 KB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginState {
    Inline(String),
    File(String),
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Master {
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub devices: Vec<Device>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_contracts_graph_reads() {
        let g: Graph = serde_json::from_value(json!({
            "sample_rate": 48000,
            "tempo_map": [{"at_beats": 0, "bpm": 86.0}],
            "tracks": [{
                "id": "01JC5R0000000000000000VOX1", "kind": "stem", "role": "vocals",
                "gain_db": -3.0, "pan": 0.25, "mute": false, "solo": true,
                "sends": {"reverb": 0.2, "delay": 0.0},
                "devices": [{
                    "id": "01JC5T0000000000000000TAPE", "format": "vst3", "uid": "ABCDEF",
                    "path": "/Library/Audio/Plug-Ins/VST3/Tape Echo.vst3",
                    "params": {"p0": 0.5}, "state": {"inline": "AAEC"}, "on": true
                }],
                "clips": [{
                    "id": "01JC5C0000000000000000CLP1", "path": "/abs/01J.wwav", "source": "vocals",
                    "at": 0, "in": 1024, "len": 48000, "gain_db": 0.0, "reverse": false
                }],
                "notes": [{"pitch": 60, "vel": 100, "at": 0, "len": 4800}]
            }],
            "master": {"gain_db": 0.0, "devices": [{"id": "01JC5T0000000000000000LIMT", "format": "builtin",
                       "uid": "limiter", "params": {}, "state": null, "on": false}]},
            "state_dir": "/abs/session/plugin-state"
        }))
        .unwrap();
        let t = &g.tracks[0];
        assert_eq!(
            (t.kind, t.role, t.solo),
            (TrackKind::Stem, Role::Vocals, true)
        );
        assert_eq!(t.clips[0].in_frame, 1024);
        assert_eq!(t.devices[0].state, Some(PluginState::Inline("AAEC".into())));
        assert!(!g.master.devices[0].on);
        assert_eq!(g.master.devices[0].state, None);
        // And it writes back to the same JSON.
        let again: Graph = serde_json::from_value(serde_json::to_value(&g).unwrap()).unwrap();
        assert_eq!(again, g);
        assert_eq!(
            serde_json::to_value(&t.devices[0].state).unwrap(),
            json!({"inline": "AAEC"})
        );
    }

    #[test]
    fn a_short_graph_takes_defaults() {
        let g: Graph = serde_json::from_value(json!({
            "sample_rate": 44100,
            "tracks": [{"id": "T1", "kind": "audio", "role": "drums",
                        "devices": [{"id": "D1", "format": "builtin", "uid": "reverb"}]}]
        }))
        .unwrap();
        let t = &g.tracks[0];
        assert_eq!((t.gain_db, t.pan, t.mute, t.solo), (0.0, 0.0, false, false));
        assert!(t.devices[0].on, "a device is on unless it says otherwise");
        assert_eq!(g.master, Master::default());
    }

    #[test]
    fn unknown_kinds_and_roles_are_refused() {
        for bad in [
            json!({"sample_rate": 48000, "tracks": [{"id": "T", "kind": "video", "role": "drums"}]}),
            json!({"sample_rate": 48000, "tracks": [{"id": "T", "kind": "audio", "role": "keys"}]}),
            json!({"sample_rate": 48000, "tracks": [{"kind": "audio", "role": "bass"}]}),
            json!({"tracks": []}),
        ] {
            assert!(
                serde_json::from_value::<Graph>(bad.clone()).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn roles_name_their_buses_in_slot_order() {
        let buses: Vec<String> = Role::ALL.iter().map(|r| r.bus()).collect();
        assert_eq!(buses, ["bus:vocals", "bus:drums", "bus:other", "bus:bass"]);
    }
}
