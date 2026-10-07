//! What `session.json` holds: the Console's Sequence (`docs/SPEC.md` 5.12)
//! with the package's additions (6.5).
//!
//! Every object keeps the keys this version doesn't know in `extra`, and
//! every closed set of words (`kind`, `role`, …) keeps a word it doesn't
//! know as `Unknown`, so a session saved by a newer app loses nothing here.
//! The order keys are written in lives in one place, [`crate::canonical`].

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::canonical;
use crate::Error;

/// A closed set of words written as strings, which keeps any word it
/// doesn't know (MI-WWAV-OS's `str_enum!`, without its lossy fallback).
macro_rules! str_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $word:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum $name {
            $($variant,)+
            /// A word from a newer version, kept as written.
            Unknown(String),
        }

        impl $name {
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $word,)+
                    Self::Unknown(s) => s,
                }
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                match s {
                    $($word => Self::$variant,)+
                    other => Self::Unknown(other.to_string()),
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                Ok(Self::from(String::deserialize(d)?.as_str()))
            }
        }
    };
}

str_enum!(
    /// 5.3's track kinds.
    TrackKind {
        Audio = "audio",
        Instrument = "instrument",
        Stem = "stem",
        Video = "video",
        Titles = "titles",
        Generator = "generator",
        Bus = "bus",
    }
);

str_enum!(
    /// The four stems, in a `.wwav` frame's order; a track's role picks the
    /// stem bus it folds into (6.6).
    Role {
        Vocals = "vocals",
        Drums = "drums",
        Other = "other",
        Bass = "bass",
    }
);

str_enum!(
    /// Where a device comes from.
    DeviceFormat {
        Builtin = "builtin",
        Vst3 = "vst3",
        Au = "au",
    }
);

str_enum!(
    /// How a clip keeps time (5.12): as played, following the bar lines, or
    /// bending pitch with speed like tape.
    TimeMode {
        AsPlayed = "as_played",
        Follow = "follow",
        Tape = "tape",
    }
);

/// The version this crate writes.
pub const VERSION: &str = "0.1";

/// The whole of `session.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub wwavsession: String,
    /// A ULID, which also names the package.
    pub id: String,
    pub title: String,
    /// "A minor"; null until known, never guessed.
    pub key: Option<String>,
    pub sample_rate: u32,
    /// From the first video clip; null for a session without picture.
    pub frame: Option<Frame>,
    pub tempo_map: Vec<TempoPoint>,
    pub buses: Vec<Role>,
    pub tracks: Vec<Track>,
    /// The master chain, after the four stem buses are summed (6.6).
    pub master: Master,
    /// Each instrument clip's notes, by clip ULID.
    pub midi: BTreeMap<String, Vec<Note>>,
    pub video: Video,
    pub lineage: Lineage,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            wwavsession: VERSION.into(),
            id: String::new(),
            title: String::new(),
            key: None,
            sample_rate: 48_000,
            frame: None,
            tempo_map: vec![TempoPoint {
                at_beats: 0.0,
                bpm: 120.0,
                extra: Map::new(),
            }],
            buses: vec![Role::Vocals, Role::Drums, Role::Other, Role::Bass],
            tracks: Vec::new(),
            master: Master::default(),
            midi: BTreeMap::new(),
            video: Video::default(),
            lineage: Lineage::default(),
            extra: Map::new(),
        }
    }
}

impl Session {
    /// A new session at 48 kHz (6.7). It reserves its `song_id` and
    /// `film_id` now, so every export of it is a version of one work (6.8).
    pub fn new(title: &str) -> Session {
        Session {
            id: wwav_ids::ulid(),
            title: title.into(),
            lineage: Lineage {
                work: Work {
                    song_id: wwav_ids::new_work_id(),
                    film_id: wwav_ids::new_work_id(),
                    version: 1,
                    extra: Map::new(),
                },
                ..Lineage::default()
            },
            ..Session::default()
        }
    }

    /// `session.json`'s bytes: the same session always gives the same bytes.
    pub fn to_json_bytes(&self) -> Vec<u8> {
        canonical::to_bytes(&self.to_value())
    }

    /// Reads `session.json`'s bytes, refusing a newer major version.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Session, Error> {
        Session::from_value(serde_json::from_slice(bytes)?)
    }

    pub(crate) fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("a session always serializes")
    }

    pub(crate) fn from_value(v: Value) -> Result<Session, Error> {
        // Like the .wwav readers, only the major version is checked.
        if let Some(version) = v.get("wwavsession").and_then(Value::as_str) {
            let major = version.split('.').next().unwrap_or("");
            if major.parse::<u64>() != Ok(0) {
                return Err(Error::Newer(version.to_string()));
            }
        }
        Ok(serde_json::from_value(v)?)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub fps: u32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TempoPoint {
    pub at_beats: f64,
    pub bpm: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    /// A ULID; the engine's node id for the track.
    pub id: String,
    pub kind: TrackKind,
    pub name: String,
    /// The stem bus it folds into; null for video, titles and generators.
    pub role: Option<Role>,
    pub gain_db: f64,
    /// −1 hard left to 1 hard right.
    pub pan: f64,
    pub mute: bool,
    pub solo: bool,
    /// To the role's reverb and delay returns, 0 to 1.
    pub sends: Sends,
    pub devices: Vec<Device>,
    pub automation: Vec<Automation>,
    pub events: Vec<Event>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Track {
    fn default() -> Self {
        Track::new("", TrackKind::Audio)
    }
}

impl Track {
    pub fn new(id: &str, kind: TrackKind) -> Track {
        Track {
            id: id.into(),
            kind,
            name: String::new(),
            role: None,
            gain_db: 0.0,
            pan: 0.0,
            mute: false,
            solo: false,
            sends: Sends::default(),
            devices: Vec::new(),
            automation: Vec::new(),
            events: Vec::new(),
            extra: Map::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Sends {
    pub reverb: f64,
    pub delay: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Master {
    pub gain_db: f64,
    pub devices: Vec<Device>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A built-in effect or a plugin, with the version that saved its state
/// (6.5), so an older installed version can be named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Device {
    /// A ULID; the engine's node id for the device.
    pub id: String,
    pub format: DeviceFormat,
    /// "reverb", a VST3 class id as 32 hex, or "aumf:TpEc:Vndr".
    pub uid: String,
    pub name: String,
    pub version: String,
    /// Off after a crash until turned on, and while frozen (5.7).
    pub on: bool,
    /// A built-in's parameters by name; a plugin keeps its own in `state`.
    pub params: BTreeMap<String, f64>,
    pub state: Option<PluginState>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Device {
    fn default() -> Self {
        Device {
            id: String::new(),
            format: DeviceFormat::Builtin,
            uid: String::new(),
            name: String::new(),
            version: String::new(),
            on: true,
            params: BTreeMap::new(),
            state: None,
            extra: Map::new(),
        }
    }
}

impl Device {
    /// "Saved with 'Tape Echo' 2.1.4. You have 2.0.0, so its settings may
    /// not load." when the installed version is older than the one that
    /// saved the state.
    pub fn older_version_sentence(&self, installed: &str) -> Option<String> {
        crate::plugin::older_version_sentence(&self.name, &self.version, installed)
    }
}

/// A plugin's saved state: inline as base64 up to 256 KB, else a file in
/// `plugin-state/` (6.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PluginState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline: Option<String>,
    /// `plugin-state/<ULID>.bin`, relative to the package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub bytes: u64,
    pub sha256: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Automation {
    /// "gain_db", "pan", "send.reverb", or `<device id>.<param>`.
    pub param: String,
    pub points: Vec<AutomationPoint>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AutomationPoint {
    pub at_beats: f64,
    pub value: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A clip on a track: a media file named by its ULID, with in, out and
/// position. "No pixels or samples are touched until export."
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Event {
    /// The ULID of a file in `media/`, an instrument clip in `midi`, or a
    /// title.
    pub clip_id: String,
    /// Which part of a `.wwav` a stem lane plays: "vocals" … "bass", or
    /// "master". Absent means the whole file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub clip_in_ms: i64,
    /// 0 means to the end of the file.
    pub clip_out_ms: i64,
    pub at_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_beats: Option<f64>,
    pub params: EventParams,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Event {
    pub fn new(clip_id: &str, at_ms: i64) -> Event {
        Event {
            clip_id: clip_id.into(),
            at_ms,
            ..Event::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventParams {
    pub gain_db: f64,
    pub pan: f64,
    pub time: TimeMode,
    /// Semitones.
    pub transpose: f64,
    pub reverse: bool,
    #[serde(rename = "loop")]
    pub looped: bool,
    /// A video clip's grade (5.11); its shape isn't fixed yet, so it is kept
    /// as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for EventParams {
    fn default() -> Self {
        EventParams {
            gain_db: 0.0,
            pan: 0.0,
            time: TimeMode::AsPlayed,
            transpose: 0.0,
            reverse: false,
            looped: false,
            grade: None,
            extra: Map::new(),
        }
    }
}

/// A note (5.4). Quantizing moves `at_beats` and `len_beats`; the played
/// pair is never rewritten, so ⌥Q can always return to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Note {
    pub pitch: u8,
    pub vel: u8,
    pub release_vel: u8,
    pub at_beats: f64,
    pub len_beats: f64,
    pub played_at_beats: f64,
    pub played_len_beats: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// 6.5 names `edits`, `grade` and `titles` without their shape, so they are
/// kept exactly as written until 5.11's video work fixes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Video {
    pub edits: Vec<Value>,
    pub grade: Map<String, Value>,
    pub titles: Vec<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// What the session will become (6.5, 6.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Lineage {
    pub work: Work,
    /// The parent a fork came from; null for a session of your own.
    pub from: Option<LineageFrom>,
    pub exports: Vec<Export>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Work {
    pub song_id: String,
    pub film_id: String,
    pub version: u32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A forked parent's exact version, so it can be found after its maker
/// pushes a new one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LineageFrom {
    pub song_id: String,
    pub version: u32,
    pub title: String,
    pub sha256: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Export {
    /// `renders/<ULID>.wwav`, relative to the package.
    pub file: String,
    pub version: u32,
    pub sha256: String,
    /// "2026-10-06T21:14:03Z".
    pub at: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
