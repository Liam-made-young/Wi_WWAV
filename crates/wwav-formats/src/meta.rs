//! What a `.wwav` and a `.swav` say about themselves, and their exact text:
//! `wmet` as `wwav_pack.py wmet_json` writes it, `wlin` as `wlin_json` and
//! PRANA's `writeWlin` write it, `wrmx` as PRANA's `writeWrmx` writes it.
//! Keys in fixed order with ": " and ", " between them, so the same song
//! written twice is the same bytes.

use crate::json::{num, quote};
use crate::text::py_strip;

/// A song's `type` (6.1). A film's is `Original` in 0.1; 6.10 adds `Remix`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Original,
    /// Demucs stems, from the WWAV disc.
    Split,
    Remix,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Original => "original",
            Kind::Split => "split",
            Kind::Remix => "remix",
        }
    }
}

/// A song's wmet, less `frames`, which the writer knows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SongMeta {
    /// 32 lowercase hex.
    pub song_id: String,
    pub title: String,
    pub artist: String,
    /// 0 for none; otherwise more than 20 and less than 400, written whole
    /// or to two decimals.
    pub bpm: f64,
    /// "" for none.
    pub key: String,
    pub kind: Kind,
    /// "" for none, e.g. "demucs".
    pub splitter: String,
    /// YYYY-MM-DD, or "" while the date is unknown.
    pub created: String,
}

impl SongMeta {
    /// This metadata as `wwav_pack.py unpack` then `pack` give it back, so
    /// an original written with it comes back byte for byte (6.13.1).
    /// unpack writes title, artist, key and created to song.txt a line
    /// each, and pack reads each line trimmed (python's `strip()`): so
    /// here line breaks become spaces and the ends are trimmed. pack keeps
    /// a bpm over 20 and under 400, and num() writes it whole or to 2
    /// decimals: so here it is rounded to 2 decimals, then kept only in
    /// that range (0 otherwise). song_id, kind and splitter stay as they
    /// are; an original also needs a title, a created date and a 32-hex
    /// song_id, which [`WwavWriter::create`](crate::writer::WwavWriter::create)
    /// checks.
    pub fn normalized(&self) -> SongMeta {
        let line = |s: &str| py_strip(&s.replace(['\r', '\n'], " ")).to_string();
        let rounded: f64 = format!("{:.2}", self.bpm).parse().unwrap_or(0.0);
        SongMeta {
            title: line(&self.title),
            artist: line(&self.artist),
            key: line(&self.key),
            created: line(&self.created),
            bpm: if 20.0 < rounded && rounded < 400.0 {
                rounded
            } else {
                0.0
            },
            ..self.clone()
        }
    }

    /// The wmet text for a master of `frames` frames.
    pub fn wmet(&self, frames: u64) -> String {
        let mut parts = vec![
            "\"wwav\": \"0.1\"".to_string(),
            format!("\"song_id\": {}", quote(&self.song_id)),
            format!("\"title\": {}", quote(&self.title)),
            format!("\"artist\": {}", quote(&self.artist)),
        ];
        if self.bpm != 0.0 {
            // a bpm that isn't a number has no place in wmet: python's
            // num() raises on it, and pack never passes one
            if let Ok(n) = num(self.bpm) {
                parts.push(format!("\"bpm\": {n}"));
            }
        }
        if !self.key.is_empty() {
            parts.push(format!("\"key\": {}", quote(&self.key)));
        }
        parts.push(format!("\"frames\": {frames}"));
        parts.push(format!("\"type\": {}", quote(self.kind.as_str())));
        if !self.splitter.is_empty() {
            parts.push(format!("\"splitter\": {}", quote(&self.splitter)));
        }
        parts.push(format!("\"created\": {}", quote(&self.created)));
        format!("{{{}}}", parts.join(", "))
    }
}

/// wlin: where a song or film came from. Songs and films share one id
/// space, so a film's parent can be a song.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lineage {
    /// None: an original.
    pub parent_id: Option<String>,
    pub root_id: String,
    pub generation: u32,
    /// An account name, or "".
    pub creator: String,
    /// "" until a device writes it (PRANA's M10); the desktop app isn't one.
    pub device_id: String,
}

impl Lineage {
    /// An original: no parent, its own id as root, generation 0.
    pub fn original(id: &str, creator: &str) -> Lineage {
        Lineage {
            parent_id: None,
            root_id: id.into(),
            generation: 0,
            creator: creator.into(),
            device_id: String::new(),
        }
    }

    /// A child of `parent_id`, whose own wlin is `parent`: its root is the
    /// parent's root and its generation the parent's + 1 (6.1, 6.8).
    pub fn child_of(parent_id: &str, parent: &Lineage, creator: &str) -> Lineage {
        Lineage {
            parent_id: Some(parent_id.into()),
            root_id: parent.root_id.clone(),
            generation: parent.generation + 1,
            creator: creator.into(),
            device_id: String::new(),
        }
    }

    pub fn wlin(&self) -> String {
        let parent = self.parent_id.as_deref().map_or("null".into(), quote);
        format!(
            "{{\"parent_id\": {parent}, \"root_id\": {}, \"generation\": {}, \"creator\": {}, \"device_id\": {}}}",
            quote(&self.root_id),
            self.generation,
            quote(&self.creator),
            quote(&self.device_id)
        )
    }
}

/// One stem's settings in wrmx: fader positions from 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Track {
    pub vol: f32,
    pub mute: bool,
    /// reverb, delay, distortion, tremolo.
    pub fx: [f32; 4],
}

/// Whether the device plays the stems or the master.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Stems,
    Master,
}

/// wrmx, "the song as you hear it" (6.1): the four stems in wstm order,
/// then the master's pitch in semitones, speed (varispeed) and time as
/// ratios, the filters (lpf 1.0 when open, hpf 0.0 when off), the mode,
/// and the frames the remix covers. Values are f32, as the device holds
/// them, so the text rounds as the device's does.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wrmx {
    pub tracks: [Track; 4],
    pub pitch: i32,
    pub speed: f32,
    pub time: f32,
    pub lpf: f32,
    pub hpf: f32,
    pub mode: Mode,
    pub start: u32,
    pub length: u32,
}

impl Wrmx {
    /// The text PRANA's `writeWrmx` writes for these settings.
    pub fn json(&self) -> String {
        const STEMS: [&str; 4] = ["vocals", "drums", "other", "bass"];
        const FX: [&str; 4] = ["reverb", "delay", "distortion", "tremolo"];
        let tracks: Vec<String> = self
            .tracks
            .iter()
            .zip(STEMS)
            .map(|(t, stem)| {
                let fx: Vec<String> =
                    t.fx.iter()
                        .zip(FX)
                        .map(|(v, name)| format!("\"{name}\": {}", fixed3(*v)))
                        .collect();
                format!(
                    "{{\"stem\": \"{stem}\", \"vol\": {}, \"mute\": {}, \"fx\": {{{}}}}}",
                    fixed3(t.vol),
                    t.mute,
                    fx.join(", ")
                )
            })
            .collect();
        format!(
            "{{\"tracks\": [{}], \"master\": {{\"vol\": 1.0, \"pitch\": {}, \"speed\": {}, \"time\": {}, \"lpf\": {}, \"hpf\": {}}}, \
             \"mode\": \"{}\", \"start\": {}, \"length\": {}}}",
            tracks.join(", "),
            self.pitch,
            fixed3(self.speed),
            fixed3(self.time),
            fixed3(self.lpf),
            fixed3(self.hpf),
            if self.mode == Mode::Master { "master" } else { "stems" },
            self.start,
            self.length
        )
    }
}

/// PRANA's `fmtFixed(v, 3)` (core/base/text.cpp): in f32, v × 1000 + 0.5,
/// truncated, capped at 4e9, and printed as whole thousandths.
fn fixed3(v: f32) -> String {
    let neg = v < 0.0;
    let scaled = (v.abs() * 1000.0 + 0.5).min(4.0e9);
    let q = scaled as u32;
    format!(
        "{}{}.{:03}",
        if neg && q != 0 { "-" } else { "" },
        q / 1000,
        q % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wmet_and_wlin_are_the_tools_text() {
        let m = SongMeta {
            song_id: "0123456789abcdef0123456789abcdef".into(),
            title: "x".into(),
            ..SongMeta::default()
        };
        assert_eq!(
            m.wmet(1),
            r#"{"wwav": "0.1", "song_id": "0123456789abcdef0123456789abcdef", "title": "x", "artist": "", "frames": 1, "type": "original", "created": ""}"#
        );
        let parent = Lineage::original("p", "");
        let child = Lineage::child_of(
            "p",
            &Lineage {
                generation: 4,
                ..parent
            },
            "a",
        );
        assert_eq!(
            child.wlin(),
            r#"{"parent_id": "p", "root_id": "p", "generation": 5, "creator": "a", "device_id": ""}"#
        );
    }

    #[test]
    fn fixed3_rounds_as_the_device() {
        assert_eq!(fixed3(0.8), "0.800");
        assert_eq!(fixed3(0.65), "0.650");
        assert_eq!(fixed3(0.0005), "0.001");
        assert_eq!(fixed3(-0.0001), "0.000");
        assert_eq!(fixed3(1.0), "1.000");
    }
}
