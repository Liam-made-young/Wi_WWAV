//! Clip audio: finding a clip's part of a WAV or `.wwav` file, and reading
//! it on time (`docs/SPEC.md` 9.4). Files under 32 MB are held whole; bigger
//! ones stream: the reader thread keeps about 2 s of each clip ahead of the
//! playhead in a ring, in 1024-frame runs, as PRANA reads `wstm`.
//!
//! A streamed clip's ring is shared by the reader thread, which writes it,
//! and the audio thread, which reads it and never waits. Its samples are
//! atomics, so that sharing needs no unsafe code: a copy the reader wrote
//! over while it was being taken is noticed by the window's generation
//! afterwards and thrown away.

use std::collections::HashMap;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::sync::atomic::{fence, AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};
use wwav_formats::wwav::{Verdict, Wwav};

/// Frames per read.
pub const RUN: usize = 1024;
/// The most frames one stretch of a block asks for.
pub const MAX_BLOCK: usize = 4096;
/// Files under this are held whole.
pub const HOLD_WHOLE: u64 = 32 * 1024 * 1024;
/// The furthest sample a position or a length may name: about 800 years at
/// 44.1 kHz, and far enough below 2^63 that no sum of them overflows.
pub const MAX_SAMPLE: i64 = 1 << 50;
/// Frames a streamed clip keeps: about 3 s.
const RING: i64 = 1 << 17;
/// And from a loop's start: about 1.5 s.
const LOOP_RING: i64 = 1 << 16;

/// Why a file can't be a clip: a code from `docs/ENGINE.md` and a sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: &'static str,
    pub message: String,
}

impl Failure {
    pub fn new(code: &'static str, message: impl Into<String>) -> Failure {
        Failure {
            code,
            message: message.into(),
        }
    }
}

/// Where one part of a file is: the master of a WAV or `.wwav` (its data
/// chunk), or one stem of a `.wwav` (its two channels inside `wstm`'s
/// frames).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MediaPart {
    pub rate: u32,
    /// 1 or 2; mono plays on both sides.
    pub channels: u16,
    /// 16, 24 or 32.
    pub bits: u16,
    pub is_float: bool,
    /// The file offset of frame 0.
    pub data_at: u64,
    /// Bytes from one frame to the next.
    pub stride: u32,
    /// Bytes into a frame where this part's samples are.
    pub offset: u32,
    pub frames: u64,
    pub file_size: u64,
}

/// Which part of a file a clip plays: the master, or stem 0 to 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    Master,
    Stem(usize),
}

impl Source {
    pub fn named(name: &str) -> Option<Source> {
        if name == "master" {
            return Some(Source::Master);
        }
        crate::graph::role_of(name).map(Source::Stem)
    }

    pub fn name(self) -> &'static str {
        match self {
            Source::Master => "master",
            Source::Stem(s) => crate::graph::ROLE_NAMES[s],
        }
    }
}

/// What `probe` makes of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// PCM the engine reads where it lies.
    Direct(MediaPart),
    /// Audio in a format that has to be decoded to a WAV first: the reason,
    /// for when nothing can decode it.
    Encoded(String),
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Finds `source` in the file at `path`. A stem needs a `.wwav` that PRANA
/// would list with its four stems; otherwise the reason is the reference
/// reader's verdict (`formats/prana/tools/wwav_pack.py`), word for word.
pub fn probe(path: &str, source: Source) -> Result<Probe, Failure> {
    let name = file_name(path);
    let file = File::open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Failure::new("no_such_file", format!("No file at {path}.")),
        _ => Failure::new("bad_clip", format!("Can't read {path} ({e}).")),
    })?;
    let meta = file
        .metadata()
        .ok()
        .filter(|m| m.is_file())
        .ok_or_else(|| Failure::new("bad_clip", format!("{path} isn't a file.")))?;
    let file_size = meta.len();
    let mut head = [0u8; 12];
    let got = read_at(&file, 0, &mut head);
    let riff = got == 12 && &head[..4] == b"RIFF" && &head[8..] == b"WAVE";
    if !riff {
        if let Source::Stem(_) = source {
            return Err(Failure::new(
                "bad_clip",
                format!("{name} has no {} stem: it is not a .wwav.", source.name()),
            ));
        }
        return Ok(Probe::Encoded(format!("{name} is not a WAV file.")));
    }
    if file_size > u32::MAX as u64 {
        return Err(Failure::new(
            "bad_clip",
            format!("{name} is over 4 GB, more than a WAVE file can hold."),
        ));
    }
    drop(file);
    let not_wav = || Failure::new("bad_clip", format!("{name} is not a WAV file."));
    let w = Wwav::open(Path::new(path)).map_err(|_| not_wav())?;
    let (Some(fmt), Some(data)) = (w.fmt, w.first(b"data").copied()) else {
        return Err(not_wav());
    };

    if let Source::Stem(stem) = source {
        let verdict = w.verdict();
        let Some(s) = w.wstm.filter(|_| verdict == Verdict::Stems) else {
            return Err(Failure::new(
                "bad_clip",
                format!("{name} has no {} stem: {verdict}.", source.name()),
            ));
        };
        return Ok(Probe::Direct(MediaPart {
            rate: s.rate,
            channels: 2,
            bits: 16,
            is_float: false,
            data_at: s.audio,
            stride: wwav_formats::wwav::STEM_FRAME as u32,
            offset: stem as u32 * 4,
            frames: s.frames as u64,
            file_size,
        }));
    }
    let pcm = fmt.format == 1 && matches!(fmt.bits, 16 | 24 | 32);
    let float = fmt.format == 3 && fmt.bits == 32;
    if !pcm && !float {
        return Ok(Probe::Encoded(match fmt.format {
            1 | 3 => format!(
                "{name} is {}-bit {}; clips are 16-, 24- or 32-bit PCM or 32-bit float.",
                fmt.bits,
                if fmt.format == 1 { "PCM" } else { "float" }
            ),
            f => format!("{name} is WAVE format {f}, which the engine doesn't read."),
        }));
    }
    if fmt.channels != 1 && fmt.channels != 2 {
        return Err(Failure::new(
            "unsupported",
            format!(
                "{name} has {} channels; clips are mono or stereo.",
                fmt.channels
            ),
        ));
    }
    if fmt.rate == 0 {
        return Err(Failure::new(
            "bad_clip",
            format!("{name} says its rate is 0 Hz."),
        ));
    }
    let stride = fmt.channels as u32 * fmt.bits as u32 / 8;
    Ok(Probe::Direct(MediaPart {
        rate: fmt.rate,
        channels: fmt.channels,
        bits: fmt.bits,
        is_float: float,
        data_at: data.at,
        stride,
        offset: 0,
        frames: data.size / stride as u64,
        file_size,
    }))
}

/// Reads until `buf` is full, the file ends or an error; returns the bytes read.
fn read_at(file: &File, at: u64, buf: &mut [u8]) -> usize {
    let mut got = 0;
    while got < buf.len() {
        match file.read_at(&mut buf[got..], at + got as u64) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    got
}

#[inline]
fn sample(s: &[u8], p: &MediaPart) -> f32 {
    match p.bits {
        16 => i16::from_le_bytes([s[0], s[1]]) as f32 * (1.0 / 32768.0),
        24 => {
            let v = s[0] as i32 | (s[1] as i32) << 8 | (s[2] as i32) << 16;
            ((v ^ 0x80_0000) - 0x80_0000) as f32 * (1.0 / 8_388_608.0)
        }
        _ if p.is_float => f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
        _ => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 * (1.0 / 2_147_483_648.0),
    }
}

/// `l.len()` frames laid out as `part`, starting at a frame, to float left
/// and right.
pub fn decode_frames(raw: &[u8], part: &MediaPart, l: &mut [f32], r: &mut [f32]) {
    let bytes = part.bits as usize / 8;
    let (stride, offset) = (part.stride as usize, part.offset as usize);
    for i in 0..l.len() {
        let f = &raw[i * stride + offset..];
        l[i] = sample(f, part);
        r[i] = if part.channels == 2 {
            sample(&f[bytes..], part)
        } else {
            l[i]
        };
    }
}

/// Frames `frame..` of `part` from the file. Frames past the part's end are
/// silence, and so is whatever a cut-off file doesn't hold.
pub fn read_frames(
    file: &File,
    part: &MediaPart,
    frame: i64,
    l: &mut [f32],
    r: &mut [f32],
    raw: &mut [u8],
) {
    let n = l.len();
    let have = if frame < part.frames as i64 {
        (n as i64).min(part.frames as i64 - frame) as usize
    } else {
        0
    };
    let stride = part.stride as usize;
    let mut got = 0;
    while got < have {
        let m = RUN.min(have - got);
        let want = &mut raw[..m * stride];
        let read = read_at(
            file,
            part.data_at + (frame as u64 + got as u64) * stride as u64,
            want,
        );
        want[read..].fill(0);
        decode_frames(want, part, &mut l[got..got + m], &mut r[got..got + m]);
        got += m;
    }
    l[got..].fill(0.0);
    r[got..].fill(0.0);
}

/// A clip's frames, decoded once and kept.
pub struct Held {
    l: Vec<f32>,
    r: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HeldKey {
    path: String,
    part: MediaPart,
    first: i64,
    frames: usize,
    changed: Option<SystemTime>,
}

/// The clips held whole, by what they hold. A session is loaded again after
/// every change to its tracks, and most of its clips are the ones it had:
/// those are found here instead of read again. An entry lasts as long as a
/// graph uses it.
#[derive(Default)]
pub struct HeldCache {
    held: HashMap<HeldKey, Weak<Held>>,
}

impl HeldCache {
    fn get(&mut self, key: HeldKey, make: impl FnOnce() -> Option<Held>) -> Option<Arc<Held>> {
        if let Some(h) = self.held.get(&key).and_then(Weak::upgrade) {
            return Some(h);
        }
        self.held.retain(|_, h| h.strong_count() > 0);
        let h = Arc::new(make()?);
        self.held.insert(key, Arc::downgrade(&h));
        Some(h)
    }
}

/// Frames `first..first + len` of one part of a file. Frames past the file's
/// end are silence, and take no memory. Positions are relative to the clip.
pub enum ClipSource {
    Whole { held: Arc<Held>, len: i64 },
    Streamed(Arc<Stream>),
}

impl ClipSource {
    /// Held whole (decoded now, or found in `cache`) or streamed.
    pub fn open(
        path: &str,
        part: MediaPart,
        first: i64,
        len: i64,
        whole: bool,
        cache: &mut HeldCache,
    ) -> Result<ClipSource, Failure> {
        let cant = |e: &dyn std::fmt::Display| {
            Failure::new("bad_clip", format!("Can't open {path} ({e})."))
        };
        let file = File::open(path).map_err(|e| cant(&e))?;
        if !whole {
            return Ok(ClipSource::Streamed(Arc::new(Stream::new(
                file, part, first, len,
            ))));
        }
        // Only the frames the file has: past its end the clip is silence,
        // however long its len.
        let frames = len.min(part.frames as i64 - first).max(0) as usize;
        let key = HeldKey {
            path: path.into(),
            part,
            first,
            frames,
            changed: file.metadata().ok().and_then(|m| m.modified().ok()),
        };
        let held = cache.get(key, || {
            let mut l = Vec::new();
            let mut r = Vec::new();
            if l.try_reserve_exact(frames).is_err() || r.try_reserve_exact(frames).is_err() {
                return None;
            }
            l.resize(frames, 0.0);
            r.resize(frames, 0.0);
            let mut raw = vec![0u8; RUN * part.stride as usize];
            for c in (0..frames).step_by(RUN) {
                let n = RUN.min(frames - c);
                read_frames(
                    &file,
                    &part,
                    first + c as i64,
                    &mut l[c..c + n],
                    &mut r[c..c + n],
                    &mut raw,
                );
            }
            Some(Held { l, r })
        });
        match held {
            Some(held) => Ok(ClipSource::Whole { held, len }),
            None => Err(Failure::new(
                "too_big",
                "There isn't the memory to hold this session's clips.",
            )),
        }
    }

    pub fn len(&self) -> i64 {
        match self {
            ClipSource::Whole { len, .. } => *len,
            ClipSource::Streamed(s) => s.len,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn stream(&self) -> Option<&Arc<Stream>> {
        match self {
            ClipSource::Whole { .. } => None,
            ClipSource::Streamed(s) => Some(s),
        }
    }

    /// The audio thread: where the playhead needs this clip next.
    pub fn want(&self, c: i64) {
        if let ClipSource::Streamed(s) = self {
            s.want.store(c, Ordering::Relaxed);
        }
    }

    /// The audio thread: frames `c..` into `l` and `r`. A streamed clip whose
    /// reader hasn't got there gives silence and false. Never waits.
    pub fn read(&self, c: i64, l: &mut [f32], r: &mut [f32]) -> bool {
        match self {
            ClipSource::Whole { held, .. } => {
                let have = held.l.len() as i64;
                let m = if c < have {
                    (l.len() as i64).min(have - c) as usize
                } else {
                    0
                };
                if m > 0 {
                    let c = c as usize;
                    l[..m].copy_from_slice(&held.l[c..c + m]);
                    r[..m].copy_from_slice(&held.r[c..c + m]);
                }
                l[m..].fill(0.0);
                r[m..].fill(0.0);
                true
            }
            ClipSource::Streamed(s) => {
                if s.read_main(c, l, r) || s.read_loop(c, l, r) {
                    return true;
                }
                l.fill(0.0);
                r.fill(0.0);
                false
            }
        }
    }

    /// A render: the same frames, read from the file now if need be.
    pub fn read_now(&self, c: i64, l: &mut [f32], r: &mut [f32]) {
        match self {
            ClipSource::Whole { .. } => {
                self.read(c, l, r);
            }
            ClipSource::Streamed(s) => {
                let mut raw = s.now.lock().unwrap_or_else(|e| e.into_inner());
                read_frames(&s.file, &s.part, s.first + c, l, r, &mut raw);
            }
        }
    }
}

fn ring(frames: i64) -> [Box<[AtomicU32]>; 2] {
    let side = || {
        (0..frames)
            .map(|_| AtomicU32::new(0))
            .collect::<Box<[AtomicU32]>>()
    };
    [side(), side()]
}

/// The reader's buffers for one run.
struct Scratch {
    raw: Vec<u8>,
    l: Vec<f32>,
    r: Vec<f32>,
}

/// A clip too big to hold: two windows of it, kept by the reader thread.
pub struct Stream {
    part: MediaPart,
    file: File,
    first: i64,
    len: i64,
    /// Frames kept ahead of the playhead.
    ahead: i64,
    /// The main window holds clip frames `start..end`, frame `c` at `c mod
    /// RING`. `gen` is odd while the reader moves the window somewhere else.
    ring: [Box<[AtomicU32]>; 2],
    want: AtomicI64,
    start: AtomicI64,
    end: AtomicI64,
    gen: AtomicU32,
    /// The loop's window: clip frames `loop_start..loop_end` from the start
    /// of `loop_ring`, read once and kept while the loop stays where it is,
    /// so the jump back finds them while the main window starts over.
    loop_ring: [Box<[AtomicU32]>; 2],
    loop_want: AtomicI64,
    loop_start: AtomicI64,
    loop_end: AtomicI64,
    loop_gen: AtomicU32,
    /// The reader thread's, and the worker's before the reader knows this
    /// stream. Never the audio thread's.
    scratch: Mutex<Scratch>,
    /// A render's bytes for one run.
    now: Mutex<Vec<u8>>,
}

impl Stream {
    fn new(file: File, part: MediaPart, first: i64, len: i64) -> Stream {
        let raw = RUN.max(MAX_BLOCK) * part.stride as usize;
        Stream {
            part,
            file,
            first,
            len,
            ahead: (2 * part.rate as i64).min(RING - 2 * MAX_BLOCK as i64),
            ring: ring(RING),
            want: AtomicI64::new(0),
            start: AtomicI64::new(0),
            end: AtomicI64::new(0),
            gen: AtomicU32::new(0),
            loop_ring: ring(LOOP_RING),
            loop_want: AtomicI64::new(-1),
            loop_start: AtomicI64::new(0),
            loop_end: AtomicI64::new(0),
            loop_gen: AtomicU32::new(0),
            scratch: Mutex::new(Scratch {
                raw: vec![0; RUN * part.stride as usize],
                l: vec![0.0; RUN],
                r: vec![0.0; RUN],
            }),
            now: Mutex::new(vec![0; raw]),
        }
    }

    pub fn len(&self) -> i64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The worker: where a loop jumps back to in this clip, or -1 for none.
    pub fn want_loop(&self, c: i64) {
        self.loop_want.store(c, Ordering::Relaxed);
    }

    /// A seqlock over the window: copy, then check the reader hasn't moved
    /// the window or written over what was copied.
    fn read_main(&self, c: i64, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len() as i64;
        let gen = self.gen.load(Ordering::Acquire);
        let (start, end) = (
            self.start.load(Ordering::Acquire),
            self.end.load(Ordering::Acquire),
        );
        if gen & 1 == 1 || c < start || c + n > end {
            return false;
        }
        let mut at = (c % RING) as usize;
        for i in 0..l.len() {
            l[i] = f32::from_bits(self.ring[0][at].load(Ordering::Relaxed));
            r[i] = f32::from_bits(self.ring[1][at].load(Ordering::Relaxed));
            at += 1;
            if at == RING as usize {
                at = 0;
            }
        }
        fence(Ordering::Acquire);
        self.start.load(Ordering::Relaxed) <= c && self.gen.load(Ordering::Relaxed) == gen
    }

    /// The same seqlock. Within one generation the reader only appends, so a
    /// copy of frames already in the window can only be spoilt by a move.
    fn read_loop(&self, c: i64, l: &mut [f32], r: &mut [f32]) -> bool {
        let n = l.len() as i64;
        let gen = self.loop_gen.load(Ordering::Acquire);
        let (start, end) = (
            self.loop_start.load(Ordering::Acquire),
            self.loop_end.load(Ordering::Acquire),
        );
        if gen & 1 == 1 || c < start || c + n > end {
            return false;
        }
        let at = (c - start) as usize;
        for i in 0..l.len() {
            l[i] = f32::from_bits(self.loop_ring[0][at + i].load(Ordering::Relaxed));
            r[i] = f32::from_bits(self.loop_ring[1][at + i].load(Ordering::Relaxed));
        }
        fence(Ordering::Acquire);
        self.loop_gen.load(Ordering::Relaxed) == gen
    }

    /// The reader thread: moves the window to the wanted frame if it is
    /// outside it, then reads ahead; and keeps the loop's window. True when
    /// it read anything.
    fn fill(&self) -> bool {
        let mut s = self.scratch.lock().unwrap_or_else(|e| e.into_inner());
        let main = self.fill_main(&mut s);
        self.fill_loop(&mut s) || main
    }

    fn fill_loop(&self, s: &mut Scratch) -> bool {
        let want = self.loop_want.load(Ordering::Relaxed);
        if want < 0 || want >= self.len {
            return false;
        }
        let (mut start, mut end) = (
            self.loop_start.load(Ordering::Relaxed),
            self.loop_end.load(Ordering::Relaxed),
        );
        if want != start {
            // A new loop start: the window starts over there.
            let gen = self.loop_gen.load(Ordering::Relaxed);
            self.loop_gen.store(gen.wrapping_add(1), Ordering::Relaxed);
            fence(Ordering::Release);
            self.loop_start.store(want, Ordering::Relaxed);
            self.loop_end.store(want, Ordering::Relaxed);
            self.loop_gen.store(gen.wrapping_add(2), Ordering::Release);
            (start, end) = (want, want);
        }
        let limit = self.len.min(start + LOOP_RING);
        if end >= limit {
            return false;
        }
        let n = RUN.min((limit - end) as usize);
        let Scratch { raw, l, r } = s;
        read_frames(
            &self.file,
            &self.part,
            self.first + end,
            &mut l[..n],
            &mut r[..n],
            raw,
        );
        let at = (end - start) as usize;
        for i in 0..n {
            self.loop_ring[0][at + i].store(l[i].to_bits(), Ordering::Relaxed);
            self.loop_ring[1][at + i].store(r[i].to_bits(), Ordering::Relaxed);
        }
        self.loop_end.store(end + n as i64, Ordering::Release);
        true
    }

    fn fill_main(&self, s: &mut Scratch) -> bool {
        let w = self.want.load(Ordering::Relaxed).clamp(0, self.len);
        let (mut start, mut end) = (
            self.start.load(Ordering::Relaxed),
            self.end.load(Ordering::Relaxed),
        );
        if w < start || w > end {
            // The playhead jumped out of the window: start a new one where it is.
            let gen = self.gen.load(Ordering::Relaxed);
            self.gen.store(gen.wrapping_add(1), Ordering::Relaxed);
            fence(Ordering::Release);
            self.start.store(w, Ordering::Relaxed);
            self.end.store(w, Ordering::Relaxed);
            self.gen.store(gen.wrapping_add(2), Ordering::Release);
            (start, end) = (w, w);
        }
        let limit = self.len.min(w + self.ahead);
        if end >= limit {
            return false;
        }
        let n = RUN.min((limit - end) as usize);
        let Scratch { raw, l, r } = s;
        read_frames(
            &self.file,
            &self.part,
            self.first + end,
            &mut l[..n],
            &mut r[..n],
            raw,
        );
        // The run writes over the oldest frames: say so before writing.
        let oldest = start.max(end + n as i64 - RING);
        if oldest != start {
            self.start.store(oldest, Ordering::Relaxed);
        }
        fence(Ordering::Release);
        let mut at = (end % RING) as usize;
        for i in 0..n {
            self.ring[0][at].store(l[i].to_bits(), Ordering::Relaxed);
            self.ring[1][at].store(r[i].to_bits(), Ordering::Relaxed);
            at += 1;
            if at == RING as usize {
                at = 0;
            }
        }
        self.end.store(end + n as i64, Ordering::Release);
        true
    }

    /// Before the reader thread knows this stream: fills the window at `c`.
    pub fn prefill(&self, c: i64) {
        self.want.store(c, Ordering::Relaxed);
        while self.fill() {}
    }

    /// Whether `frames` frames from `c` are in the window.
    pub fn ready(&self, c: i64, frames: i64) -> bool {
        if c >= self.len {
            return true;
        }
        self.gen.load(Ordering::Acquire) & 1 == 0
            && self.start.load(Ordering::Acquire) <= c
            && self.end.load(Ordering::Acquire) >= self.len.min(c + frames)
    }
}

/// The disk thread: keeps every streamed clip ahead of the playhead.
pub struct Reader {
    sources: Arc<Mutex<Vec<Arc<Stream>>>>,
    quit: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Default for Reader {
    fn default() -> Reader {
        Reader::new()
    }
}

impl Reader {
    pub fn new() -> Reader {
        let sources: Arc<Mutex<Vec<Arc<Stream>>>> = Arc::default();
        let quit = Arc::new(AtomicBool::new(false));
        let thread = std::thread::Builder::new()
            .name("reader".into())
            .spawn({
                let (sources, quit) = (sources.clone(), quit.clone());
                move || {
                    while !quit.load(Ordering::Relaxed) {
                        // One run per clip per pass, so every clip moves
                        // ahead together. The lock is held for one run at a
                        // time, so remove() waits for one read at most.
                        let mut busy = false;
                        let mut i = 0;
                        loop {
                            let list = sources.lock().unwrap_or_else(|e| e.into_inner());
                            let Some(s) = list.get(i) else { break };
                            busy = s.fill() || busy;
                            i += 1;
                        }
                        if !busy {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                    }
                }
            })
            .ok();
        Reader {
            sources,
            quit,
            thread,
        }
    }

    pub fn add(&self, s: Arc<Stream>) {
        self.sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(s);
    }

    /// Returns once the reader no longer touches `s`.
    pub fn remove(&self, s: &Arc<Stream>) {
        self.sources
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|x| !Arc::ptr_eq(x, s));
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn part(bits: u16, is_float: bool, channels: u16) -> MediaPart {
        MediaPart {
            rate: 44100,
            channels,
            bits,
            is_float,
            data_at: 0,
            stride: channels as u32 * bits as u32 / 8,
            offset: 0,
            frames: 1,
            file_size: 0,
        }
    }

    #[test]
    fn a_16_bit_sample_is_its_value_over_32768() {
        let (mut l, mut r) = ([0.0], [0.0]);
        decode_frames(
            &[0x00, 0x40, 0x00, 0x80],
            &part(16, false, 2),
            &mut l,
            &mut r,
        );
        assert_eq!((l[0], r[0]), (0.5, -1.0));
        decode_frames(&[0xFF, 0x7F], &part(16, false, 1), &mut l, &mut r);
        assert_eq!(
            (l[0], r[0]),
            (32767.0 / 32768.0, 32767.0 / 32768.0),
            "mono plays on both sides"
        );
    }

    #[test]
    fn wider_samples_keep_their_sign() {
        let (mut l, mut r) = ([0.0], [0.0]);
        decode_frames(&[0x00, 0x00, 0x80], &part(24, false, 1), &mut l, &mut r);
        assert_eq!(l[0], -1.0);
        decode_frames(&0.25f32.to_le_bytes(), &part(32, true, 1), &mut l, &mut r);
        assert_eq!(l[0], 0.25);
        decode_frames(&i32::MIN.to_le_bytes(), &part(32, false, 1), &mut l, &mut r);
        assert_eq!(l[0], -1.0);
    }

    /// A streamed clip of `frames` frames whose left sample is its frame
    /// number over 32768 (so every frame says where it is from).
    fn counting(frames: usize) -> (tempfile::TempDir, Stream) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("count.raw");
        let mut f = File::create(&path).unwrap();
        let bytes: Vec<u8> = (0..frames)
            .flat_map(|i| ((i % 30000) as i16).to_le_bytes())
            .collect();
        f.write_all(&bytes).unwrap();
        let p = MediaPart {
            frames: frames as u64,
            ..part(16, false, 1)
        };
        (
            dir,
            Stream::new(File::open(&path).unwrap(), p, 0, frames as i64),
        )
    }

    fn frame_at(s: &Stream, c: i64) -> Option<i64> {
        let (mut l, mut r) = ([0.0f32; 16], [0.0f32; 16]);
        (s.read_main(c, &mut l, &mut r) || s.read_loop(c, &mut l, &mut r))
            .then(|| (l[0] * 32768.0).round() as i64)
    }

    #[test]
    fn a_stream_keeps_two_seconds_ahead_of_where_it_is_wanted() {
        let (_dir, s) = counting(400_000);
        assert!(!s.ready(0, 1000), "nothing is read before the first fill");
        s.prefill(0);
        assert!(s.ready(0, 2 * 44100));
        assert_eq!(frame_at(&s, 12_345), Some(12_345));
        assert_eq!(
            frame_at(&s, 300_000),
            None,
            "outside the window: silence, not a wait"
        );
        s.prefill(300_000);
        assert_eq!(frame_at(&s, 300_000), Some(300_000 % 30000));
        assert_eq!(frame_at(&s, 12_345), None, "the window moved");
    }

    #[test]
    fn the_loops_start_stays_ready_while_the_main_window_moves_on() {
        let (_dir, s) = counting(400_000);
        s.want_loop(20_000);
        s.prefill(150_000);
        assert_eq!(frame_at(&s, 150_010), Some(150_010 % 30000));
        assert_eq!(
            frame_at(&s, 20_000),
            Some(20_000),
            "the jump back finds the loop's window"
        );
    }

    #[test]
    fn frames_past_the_files_end_are_silence() {
        let (_dir, s) = counting(2000);
        let (mut l, mut r) = ([1.0f32; 64], [1.0f32; 64]);
        let mut raw = vec![0u8; RUN * 2];
        read_frames(&s.file, &s.part, 1990, &mut l, &mut r, &mut raw);
        assert_eq!(l[9], 1999.0 / 32768.0);
        assert!(l[10..].iter().all(|v| *v == 0.0) && r[10..].iter().all(|v| *v == 0.0));
    }
}
