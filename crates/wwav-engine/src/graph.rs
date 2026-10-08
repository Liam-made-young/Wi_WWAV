//! The graph (`docs/SPEC.md` 9.4, 5.3, 6.6): tracks, then the four stem
//! buses, then the master. Every track sums into the bus of its role; the
//! master is the sum of the four buses, then its own gain. The same code runs
//! live on the audio thread and offline on the worker for a render, so what
//! you hear is what you render.
//!
//! Nothing here allocates, locks or waits once a graph is built: a block is
//! arithmetic over buffers the graph already owns.

use crate::media::{ClipSource, Stream, MAX_BLOCK};
use std::collections::HashMap;
use std::sync::Arc;
use wwav_wire::shm::{crumb_hash, Crumb};

/// Stem order is fixed: PRANA's fader order, Wi's, and the order in the file.
pub const ROLES: usize = 4;
pub const ROLE_NAMES: [&str; ROLES] = ["vocals", "drums", "other", "bass"];

pub fn role_of(name: &str) -> Option<usize> {
    ROLE_NAMES.iter().position(|r| *r == name)
}

/// A gain that ramps linearly to its target over one stretch of a block, so
/// a change never clicks. A gain that isn't changing steps by zero: unity
/// stays exact, which is what lets a `.wwav`'s stems come out bit for bit.
#[derive(Debug, Clone, Copy)]
pub struct Ramp {
    cur: f32,
    target: f32,
    step: f32,
}

impl Default for Ramp {
    fn default() -> Ramp {
        Ramp {
            cur: 1.0,
            target: 1.0,
            step: 0.0,
        }
    }
}

impl Ramp {
    fn begin(&mut self, n: usize) {
        self.step = (self.target - self.cur) / n as f32;
    }

    fn next(&mut self) -> f32 {
        self.cur += self.step;
        self.cur
    }

    fn end(&mut self) {
        self.cur = self.target;
        self.step = 0.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Param {
    GainDb,
    Pan,
    Mute,
    Solo,
}

impl Param {
    pub fn named(name: &str) -> Option<Param> {
        match name {
            "gain_db" => Some(Param::GainDb),
            "pan" => Some(Param::Pan),
            "mute" => Some(Param::Mute),
            "solo" => Some(Param::Solo),
            _ => None,
        }
    }
}

/// dB to a linear gain. 0 dB is exactly 1, and `libm` gives the same bits on
/// every machine, so a render doesn't depend on where it ran.
pub fn db_gain(db: f32) -> f32 {
    if db == 0.0 {
        1.0
    } else {
        libm::pow(10.0, db as f64 / 20.0) as f32
    }
}

/// A stereo balance: the centre leaves both sides at unity (so a `.wwav`'s
/// stems sum to its master at 0 dB); turning toward one side lowers the
/// other along a quarter cosine, to silence at the end.
pub fn balance(pan: f32) -> (f32, f32) {
    let quarter = std::f32::consts::FRAC_PI_2;
    let l = if pan > 0.0 {
        libm::cosf(pan * quarter)
    } else {
        1.0
    };
    let r = if pan < 0.0 {
        libm::cosf(-pan * quarter)
    } else {
        1.0
    };
    (l, r)
}

/// The fader end of a track, a bus or the master.
#[derive(Debug, Clone, Copy, Default)]
pub struct Strip {
    pub gain_db: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    l: Ramp,
    r: Ramp,
}

impl Strip {
    fn target(&mut self, audible: bool) {
        let g = if audible { db_gain(self.gain_db) } else { 0.0 };
        let (l, r) = balance(self.pan);
        self.l.target = g * l;
        self.r.target = g * r;
    }

    fn settle(&mut self) {
        self.l.end();
        self.r.end();
    }

    /// The strip's gain over one stretch, ramping from where it was.
    fn apply(&mut self, l: &mut [f32], r: &mut [f32]) {
        self.l.begin(l.len());
        self.r.begin(l.len());
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            *a *= self.l.next();
            *b *= self.r.next();
        }
        self.settle();
    }
}

pub struct Clip {
    /// The session frame of the clip's first frame.
    pub at: i64,
    pub gain: f32,
    pub source: ClipSource,
}

pub struct Track {
    pub role: usize,
    pub clips: Vec<Clip>,
    pub strip: Strip,
}

/// What a graph's nodes are called. It never changes once the graph is
/// built, so the socket thread reads it while the audio thread plays the
/// graph it belongs to.
pub struct Index {
    /// Which load this is: a param sent for another graph is dropped.
    pub gen: u64,
    pub sample_rate: u32,
    /// Node ids in meter-slot order: the tracks, the four buses, the master.
    ids: Vec<String>,
    by_id: HashMap<String, usize>,
    tracks: usize,
}

impl Index {
    pub fn new(gen: u64, sample_rate: u32, tracks: Vec<String>) -> Index {
        let count = tracks.len();
        let mut ids = tracks;
        ids.extend(ROLE_NAMES.iter().map(|r| format!("bus:{r}")));
        ids.push("master".into());
        let by_id = ids
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();
        Index {
            gen,
            sample_rate,
            ids,
            by_id,
            tracks: count,
        }
    }

    /// Node numbers are also the meter slots.
    pub fn nodes(&self) -> usize {
        self.ids.len()
    }

    pub fn tracks(&self) -> usize {
        self.tracks
    }

    pub fn bus_node(&self, role: usize) -> usize {
        self.tracks + role
    }

    pub fn master_node(&self) -> usize {
        self.tracks + ROLES
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    pub fn find(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    /// The master has no pan and no solo.
    pub fn takes(&self, node: usize, p: Param) -> bool {
        node < self.nodes()
            && (node != self.master_node() || matches!(p, Param::GainDb | Param::Mute))
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Level {
    peak: [f32; 2],
    sum_sq: [f32; 2],
}

impl Level {
    fn meter(&mut self, l: &[f32], r: &[f32]) {
        for (a, b) in l.iter().zip(r) {
            self.peak[0] = self.peak[0].max(a.abs());
            self.peak[1] = self.peak[1].max(b.abs());
            self.sum_sq[0] += a * a;
            self.sum_sq[1] += b * b;
        }
    }
}

pub struct Graph {
    pub index: Arc<Index>,
    pub tracks: Vec<Track>,
    pub buses: [Strip; ROLES],
    pub master: Strip,
    /// Left then right of each bus, `MAX_BLOCK` frames each.
    bus: Vec<f32>,
    /// Left then right, for the master, the track being summed and the clip
    /// being read.
    out: Vec<f32>,
    track: Vec<f32>,
    clip: Vec<f32>,
    levels: Vec<Level>,
    crumbs: Vec<u64>,
    /// `debug.crumb`: the audio thread dies inside this node.
    crash_node: Option<usize>,
}

impl Graph {
    /// A graph of these tracks, with every gain at its target: a new graph
    /// starts at its levels, without ramping in.
    pub fn new(
        gen: u64,
        sample_rate: u32,
        tracks: Vec<(String, Track)>,
        buses: [Strip; ROLES],
        master: Strip,
    ) -> Graph {
        let (ids, tracks): (Vec<String>, Vec<Track>) = tracks.into_iter().unzip();
        let index = Arc::new(Index::new(gen, sample_rate, ids));
        let mut g = Graph {
            crumbs: index.ids().iter().map(|id| crumb_hash(id)).collect(),
            levels: vec![Level::default(); index.nodes()],
            index,
            tracks,
            buses,
            master,
            bus: vec![0.0; 2 * ROLES * MAX_BLOCK],
            out: vec![0.0; 2 * MAX_BLOCK],
            track: vec![0.0; 2 * MAX_BLOCK],
            clip: vec![0.0; 2 * MAX_BLOCK],
            crash_node: None,
        };
        g.retarget();
        g.settle();
        g
    }

    /// Every gain at its target now, without ramping there. A render starts
    /// so: what it writes can't depend on whether a block ran since the last
    /// `param.set`.
    pub fn settle(&mut self) {
        for t in &mut self.tracks {
            t.strip.settle();
        }
        for b in &mut self.buses {
            b.settle();
        }
        self.master.settle();
    }

    fn retarget(&mut self) {
        let track_solo = self.tracks.iter().any(|t| t.strip.solo);
        let bus_solo = self.buses.iter().any(|b| b.solo);
        for t in &mut self.tracks {
            let s = &mut t.strip;
            s.target(!s.mute && (!track_solo || s.solo));
        }
        for b in &mut self.buses {
            b.target(!b.mute && (!bus_solo || b.solo));
        }
        let audible = !self.master.mute;
        self.master.target(audible);
    }

    /// A parameter change, heard from the next stretch.
    pub fn set(&mut self, node: usize, p: Param, value: f32) {
        let tracks = self.tracks.len();
        let s = if node < tracks {
            &mut self.tracks[node].strip
        } else if node < tracks + ROLES {
            &mut self.buses[node - tracks]
        } else {
            &mut self.master
        };
        match p {
            Param::GainDb => s.gain_db = value,
            Param::Pan => s.pan = value,
            Param::Mute => s.mute = value != 0.0,
            Param::Solo => s.solo = value != 0.0,
        }
        self.retarget();
    }

    pub fn crash_in(&mut self, node: usize) {
        self.crash_node = Some(node);
    }

    /// One block in stretches of the session: `begin_block`, `process` for
    /// each stretch (two when the block crosses a loop's end), `end_block`.
    pub fn begin_block(&mut self, n: usize) {
        for b in 0..2 * ROLES {
            self.bus[b * MAX_BLOCK..b * MAX_BLOCK + n].fill(0.0);
        }
        self.levels.fill(Level::default());
    }

    /// Frames `pos..pos + n` of the session into frames `at..at + n` of the
    /// block. `realtime` reads streamed clips from their windows and never
    /// waits; a render reads them from the file. The crumb names the node now
    /// running, so a crash can be pinned on it (`docs/ENGINE.md` 4.3).
    pub fn process(
        &mut self,
        pos: i64,
        at: usize,
        n: usize,
        playing: bool,
        realtime: bool,
        crumbs: Option<&Crumb>,
    ) {
        let Graph {
            index,
            tracks,
            buses,
            master,
            bus,
            out,
            track,
            clip,
            levels,
            crumbs: hashes,
            crash_node,
        } = self;
        let enter = |node: usize| {
            if let Some(c) = crumbs {
                c.set(hashes[node]);
            }
            if *crash_node == Some(node) {
                std::process::abort();
            }
        };
        let leave = || {
            if let Some(c) = crumbs {
                c.clear();
            }
        };
        let (tl, tr) = track.split_at_mut(MAX_BLOCK);
        let (tl, tr) = (&mut tl[..n], &mut tr[..n]);
        let (cl, cr) = clip.split_at_mut(MAX_BLOCK);
        for (i, t) in tracks.iter_mut().enumerate() {
            enter(i);
            tl.fill(0.0);
            tr.fill(0.0);
            for c in &t.clips {
                let len = c.source.len();
                if realtime {
                    c.source.want((pos - c.at).clamp(0, len));
                }
                if !playing {
                    continue;
                }
                let (a, b) = (pos.max(c.at), (pos + n as i64).min(c.at + len));
                if a >= b {
                    continue;
                }
                let (m, dst) = ((b - a) as usize, (a - pos) as usize);
                let (l, r) = (&mut cl[..m], &mut cr[..m]);
                if realtime {
                    c.source.read(a - c.at, l, r);
                } else {
                    c.source.read_now(a - c.at, l, r);
                }
                for k in 0..m {
                    tl[dst + k] += l[k] * c.gain;
                    tr[dst + k] += r[k] * c.gain;
                }
            }
            t.strip.apply(tl, tr);
            levels[i].meter(tl, tr);
            let b = 2 * t.role * MAX_BLOCK;
            for k in 0..n {
                bus[b + at + k] += tl[k];
                bus[b + MAX_BLOCK + at + k] += tr[k];
            }
            leave();
        }
        for (r, strip) in buses.iter_mut().enumerate() {
            let node = index.bus_node(r);
            enter(node);
            let (l, rest) = bus[2 * r * MAX_BLOCK..].split_at_mut(MAX_BLOCK);
            let (l, rt) = (&mut l[at..at + n], &mut rest[at..at + n]);
            strip.apply(l, rt);
            levels[node].meter(l, rt);
            leave();
        }
        // The fold rule: the master is the sum of the four stems, in stem
        // order, then its own gain.
        let node = index.master_node();
        enter(node);
        let (ml, mr) = out.split_at_mut(MAX_BLOCK);
        let (ml, mr) = (&mut ml[at..at + n], &mut mr[at..at + n]);
        let side = |role: usize, right: usize| &bus[(2 * role + right) * MAX_BLOCK + at..][..n];
        for (right, m) in [&mut *ml, &mut *mr].into_iter().enumerate() {
            let (v, d, o, b) = (
                side(0, right),
                side(1, right),
                side(2, right),
                side(3, right),
            );
            for k in 0..n {
                m[k] = ((v[k] + d[k]) + o[k]) + b[k];
            }
        }
        master.apply(ml, mr);
        levels[node].meter(ml, mr);
        leave();
    }

    /// The block's levels into `slots` (peak L, peak R, RMS L, RMS R for each
    /// node); returns how many it used.
    pub fn end_block(&self, n: usize, slots: Option<&mut [[f32; 4]]>) -> usize {
        let used = self.index.nodes();
        if let Some(slots) = slots {
            for (s, lv) in slots.iter_mut().zip(&self.levels) {
                *s = [
                    lv.peak[0],
                    lv.peak[1],
                    (lv.sum_sq[0] / n as f32).sqrt(),
                    (lv.sum_sq[1] / n as f32).sqrt(),
                ];
            }
        }
        used
    }

    /// The last block's left and right of one stem bus, and of the master.
    pub fn bus(&self, role: usize) -> (&[f32], &[f32]) {
        let b = &self.bus[2 * role * MAX_BLOCK..];
        (&b[..MAX_BLOCK], &b[MAX_BLOCK..2 * MAX_BLOCK])
    }

    pub fn master_out(&self) -> (&[f32], &[f32]) {
        self.out.split_at(MAX_BLOCK)
    }

    /// The graph's streamed clips, for the worker to keep ahead of the
    /// playhead while the audio thread has the graph.
    pub fn streams(&self) -> Streams {
        Streams(
            self.tracks
                .iter()
                .flat_map(|t| &t.clips)
                .filter_map(|c| c.source.stream().map(|s| (c.at, s.clone())))
                .collect(),
        )
    }
}

/// A graph's streamed clips and where each starts in the session.
#[derive(Default)]
pub struct Streams(Vec<(i64, Arc<Stream>)>);

impl Streams {
    /// Before the reader thread knows them: fills each clip at `pos`.
    pub fn prefill(&self, pos: i64) {
        for (at, s) in &self.0 {
            s.prefill((pos - at).clamp(0, s.len()));
        }
    }

    /// Where the transport's loop jumps back to, as a session sample, or
    /// `None` when there is no loop. Streamed clips keep the frames from
    /// there ready.
    pub fn want_loop(&self, start: Option<i64>) {
        for (at, s) in &self.0 {
            // A loop that starts before the clip comes back to its first frame.
            let c = start
                .map(|start| (start - at).max(0))
                .filter(|c| *c < s.len());
            s.want_loop(c.unwrap_or(-1));
        }
    }

    /// Whether every clip holds `frames` frames from `pos`.
    pub fn ready(&self, pos: i64, frames: i64) -> bool {
        self.0.iter().all(|(at, s)| {
            let len = s.len();
            if pos + frames <= *at || pos >= at + len {
                return true;
            }
            let from = (pos - at).max(0);
            s.ready(from, len.min(pos + frames - at) - from)
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<Stream>> {
        self.0.iter().map(|(_, s)| s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_db_is_unity_and_minus_six_is_about_half() {
        assert_eq!(db_gain(0.0), 1.0);
        assert!((db_gain(-6.0206) - 0.5).abs() < 1e-5);
        assert!(db_gain(-120.0) < 1.1e-6);
    }

    #[test]
    fn the_centre_leaves_both_sides_at_unity() {
        assert_eq!(balance(0.0), (1.0, 1.0));
        let (l, r) = balance(1.0);
        assert!(l.abs() < 1e-6 && r == 1.0, "hard right: {l} {r}");
        let (l, r) = balance(-1.0);
        assert!(r.abs() < 1e-6 && l == 1.0, "hard left: {l} {r}");
    }

    #[test]
    fn the_master_takes_no_pan_and_no_solo() {
        let ix = Index::new(1, 44100, vec!["a".into()]);
        assert_eq!((ix.nodes(), ix.bus_node(0), ix.master_node()), (6, 1, 5));
        assert_eq!(ix.find("bus:other"), Some(3));
        assert!(ix.takes(5, Param::Mute) && !ix.takes(5, Param::Pan) && !ix.takes(5, Param::Solo));
        assert!(ix.takes(0, Param::Solo) && !ix.takes(6, Param::GainDb));
    }
}
