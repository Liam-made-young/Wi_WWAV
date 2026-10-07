//! The mock's audio: not music, but a deterministic stand-in that follows
//! the session's routing. Every track plays its role's tone at its role's
//! level, scaled by its fader and pan and silenced by mute and solo; each
//! stem bus is the sum of its tracks; the master is the sum of the buses
//! (the fold rule, docs/SPEC.md 6.6). Devices pass audio through unchanged.

use wwav_wire::graph::{Graph, Role};

/// A fader: gain, pan, mute and solo. Tracks carry theirs in the graph;
/// the stem buses and the master only get one through `param.set`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Strip {
    pub gain_db: f64,
    pub pan: f64,
    pub mute: bool,
    pub solo: bool,
}

/// Each role's tone: frequency in Hz and peak level at 0 dB.
pub fn tone(role: Role) -> (f64, f32) {
    match role {
        Role::Vocals => (440.0, 0.25),
        Role::Drums => (110.0, 0.3),
        Role::Other => (330.0, 0.2),
        Role::Bass => (55.0, 0.25),
    }
}

pub fn gain(db: f64) -> f32 {
    10f64.powf(db / 20.0) as f32
}

/// The balance law: the far side falls as the pan moves away from it.
fn pan_gains(pan: f64) -> [f32; 2] {
    let pan = pan.clamp(-1.0, 1.0) as f32;
    [(1.0 - pan).min(1.0), (1.0 + pan).min(1.0)]
}

/// Peak amplitude, left and right, of every track, bus and the master.
#[derive(Debug, Clone, PartialEq)]
pub struct Mix {
    pub tracks: Vec<[f32; 2]>,
    /// In `Role::ALL` order.
    pub buses: [[f32; 2]; 4],
    pub master: [f32; 2],
    /// The master's gain, applied after the buses are summed.
    pub master_gain: f32,
}

pub fn mix(graph: &Graph, buses: &[Strip; 4], master: &Strip) -> Mix {
    let any_solo = graph.tracks.iter().any(|t| t.solo);
    let tracks: Vec<[f32; 2]> = graph
        .tracks
        .iter()
        .map(|t| {
            if t.mute || (any_solo && !t.solo) {
                return [0.0, 0.0];
            }
            let level = tone(t.role).1 * gain(t.gain_db);
            pan_gains(t.pan).map(|p| level * p)
        })
        .collect();
    let any_bus_solo = buses.iter().any(|b| b.solo);
    let mut bus_peaks = [[0.0f32; 2]; 4];
    for (r, role) in Role::ALL.iter().enumerate() {
        let strip = &buses[r];
        if strip.mute || (any_bus_solo && !strip.solo) {
            continue;
        }
        // The tracks of one role play the same tone in phase, so they add.
        let sum = graph
            .tracks
            .iter()
            .zip(&tracks)
            .filter(|(t, _)| t.role == *role)
            .fold([0.0f32; 2], |s, (_, a)| [s[0] + a[0], s[1] + a[1]]);
        let g = gain(strip.gain_db);
        let p = pan_gains(strip.pan);
        bus_peaks[r] = [sum[0] * g * p[0], sum[1] * g * p[1]];
    }
    let master_gain = if master.mute {
        0.0
    } else {
        gain(master.gain_db)
    };
    let master_peak = bus_peaks
        .iter()
        .fold([0.0f32; 2], |s, b| [s[0] + b[0], s[1] + b[1]])
        .map(|v| v * master_gain);
    Mix {
        tracks,
        buses: bus_peaks,
        master: master_peak,
        master_gain,
    }
}

/// The meter slots for one block: tracks, then the buses, then the master,
/// each peak L, peak R, RMS L, RMS R. A sine's RMS is its peak over √2; the
/// master's tones differ, so their RMS adds as power.
pub fn meter_slots(m: &Mix) -> Vec<[f32; 4]> {
    let sine = |a: [f32; 2]| {
        [
            a[0],
            a[1],
            a[0] * std::f32::consts::FRAC_1_SQRT_2,
            a[1] * std::f32::consts::FRAC_1_SQRT_2,
        ]
    };
    let mut slots: Vec<[f32; 4]> = m.tracks.iter().chain(&m.buses).map(|a| sine(*a)).collect();
    let power = |side: usize| {
        m.buses
            .iter()
            .map(|b| (b[side] * m.master_gain).powi(2) / 2.0)
            .sum::<f32>()
            .sqrt()
    };
    slots.push([m.master[0], m.master[1], power(0), power(1)]);
    slots
}

/// One stereo frame of each stem at session sample `n`.
pub fn stem_frames(m: &Mix, n: i64, rate: u32) -> [[f32; 2]; 4] {
    let mut out = [[0.0f32; 2]; 4];
    for (r, role) in Role::ALL.iter().enumerate() {
        let (freq, _) = tone(*role);
        let s = (2.0 * std::f64::consts::PI * freq * n as f64 / rate as f64).sin() as f32;
        out[r] = [m.buses[r][0] * s, m.buses[r][1] * s];
    }
    out
}

/// The master frame: the stems summed, then the master's gain.
pub fn master_frame(m: &Mix, stems: &[[f32; 2]; 4]) -> [f32; 2] {
    let side =
        |c: usize| (((stems[0][c] + stems[1][c]) + stems[2][c]) + stems[3][c]) * m.master_gain;
    [side(0), side(1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn graph(tracks: serde_json::Value) -> Graph {
        serde_json::from_value(json!({"sample_rate": 48000, "tracks": tracks})).unwrap()
    }

    fn flat() -> [Strip; 4] {
        [Strip::default(); 4]
    }

    #[test]
    fn a_tracks_level_is_its_roles_tone_times_its_fader() {
        let g = graph(json!([
            {"id": "a", "kind": "audio", "role": "vocals"},
            {"id": "b", "kind": "audio", "role": "vocals", "gain_db": -6.0},
            {"id": "c", "kind": "audio", "role": "bass", "pan": 1.0}
        ]));
        let m = mix(&g, &flat(), &Strip::default());
        assert_eq!(m.tracks[0], [0.25, 0.25]);
        assert!((m.tracks[1][0] - 0.25 * 0.501_187).abs() < 1e-6);
        assert_eq!(m.tracks[2], [0.0, 0.25], "panned hard right");
        assert_eq!(m.buses[0][0], m.tracks[0][0] + m.tracks[1][0]);
        assert_eq!(m.buses[1], [0.0, 0.0], "no drum tracks");
        assert_eq!(
            m.master,
            [m.buses[0][0] + m.buses[3][0], m.buses[0][1] + m.buses[3][1]]
        );
    }

    #[test]
    fn mute_and_solo_silence_tracks_and_buses() {
        let g = graph(json!([
            {"id": "a", "kind": "audio", "role": "vocals", "solo": true},
            {"id": "b", "kind": "audio", "role": "drums"},
            {"id": "c", "kind": "audio", "role": "drums", "mute": true, "solo": true}
        ]));
        let m = mix(&g, &flat(), &Strip::default());
        assert_eq!(m.tracks[1], [0.0, 0.0], "not soloed");
        assert_eq!(m.tracks[2], [0.0, 0.0], "mute wins over solo");
        let mut buses = flat();
        buses[0].mute = true;
        assert_eq!(mix(&g, &buses, &Strip::default()).master, [0.0, 0.0]);
        let master = Strip {
            mute: true,
            ..Strip::default()
        };
        assert_eq!(mix(&g, &flat(), &master).master, [0.0, 0.0]);
    }

    #[test]
    fn the_master_is_the_sum_of_the_stems_sample_by_sample() {
        let g = graph(json!([
            {"id": "a", "kind": "audio", "role": "vocals"},
            {"id": "b", "kind": "audio", "role": "drums"},
            {"id": "c", "kind": "audio", "role": "other"}
        ]));
        let m = mix(&g, &flat(), &Strip::default());
        for n in [0, 1, 99, 48_000] {
            let s = stem_frames(&m, n, 48000);
            assert_eq!(
                master_frame(&m, &s)[0],
                s[0][0] + s[1][0] + s[2][0] + s[3][0]
            );
        }
    }

    #[test]
    fn meter_slots_are_tracks_then_buses_then_master() {
        let g = graph(json!([{"id": "a", "kind": "audio", "role": "vocals"}]));
        let slots = meter_slots(&mix(&g, &flat(), &Strip::default()));
        assert_eq!(slots.len(), 1 + 4 + 1);
        assert_eq!(
            slots[0], slots[1],
            "the vocals bus carries the one vocal track"
        );
        assert_eq!(slots[5][0], 0.25);
        assert!((slots[5][2] - 0.25 / 2f32.sqrt()).abs() < 1e-7);
    }
}
