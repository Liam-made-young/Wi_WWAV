//! Recording a take (`docs/ENGINE.md` 3.10, 4.5).
//!
//! Input audio crosses two threads that must never wait on each other: the
//! one that has it (a card's input callback, or the audio thread itself for
//! the timer's loopback) pushes it into a ring, and a writer thread takes it
//! out, writes the take's file and shows it in shared memory. The ring is
//! atomics, so neither side locks, and a writer that falls behind costs
//! frames (counted), never a late block.
//!
//! Where a take belongs in the session is worked out from two facts the
//! threads publish: the audio thread says which session sample reaches the
//! speaker at which moment, and the input says at which moment each of its
//! frames was captured. A frame captured at the moment sample P was heard
//! belongs at P.

use crate::wav::TakeWriter;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use wwav_wire::shm::{Shm, TakeFields};

/// Samples the ring holds: over five seconds of stereo at 48 kHz.
const RING: usize = 1 << 19;
/// Frames one peak in shared memory covers.
pub const FRAMES_PER_PEAK: u32 = 256;

/// The ring between whoever has input audio and the take's writer.
pub struct Capture {
    ring: Box<[AtomicU32]>,
    /// Samples pushed and samples taken out, since the engine started.
    write: AtomicU64,
    read: AtomicU64,
    /// Frames that found the ring full.
    dropped: AtomicU64,
    /// Samples a frame, 1 or 2.
    channels: AtomicU32,
    /// The timer feeds its own master in: an input without hardware, so a
    /// take can be checked sample for sample.
    loopback: AtomicBool,
    /// The input's last callback: the index of its first frame, and when
    /// that frame was captured. `anchor_seq` is odd while they are written.
    anchor_seq: AtomicU64,
    anchor_frame: AtomicU64,
    anchor_ns: AtomicU64,
    /// What the audio thread said at the block a take began or ended: the
    /// playhead, when that sample reaches the speaker, whether it was
    /// playing, and how many frames the loopback had pushed.
    mark_pos: AtomicI64,
    mark_ns: AtomicU64,
    mark_playing: AtomicBool,
    mark_frame: AtomicU64,
}

impl Default for Capture {
    fn default() -> Capture {
        Capture {
            ring: (0..RING).map(|_| AtomicU32::new(0)).collect(),
            write: AtomicU64::new(0),
            read: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            channels: AtomicU32::new(2),
            loopback: AtomicBool::new(false),
            anchor_seq: AtomicU64::new(0),
            anchor_frame: AtomicU64::new(0),
            anchor_ns: AtomicU64::new(0),
            mark_pos: AtomicI64::new(0),
            mark_ns: AtomicU64::new(0),
            mark_playing: AtomicBool::new(false),
            mark_frame: AtomicU64::new(0),
        }
    }
}

impl Capture {
    pub fn channels(&self) -> usize {
        self.channels.load(Ordering::Relaxed) as usize
    }

    pub fn loopback(&self) -> bool {
        self.loopback.load(Ordering::Relaxed)
    }

    /// Frames pushed since the engine started.
    pub fn frames(&self) -> u64 {
        self.write.load(Ordering::Acquire) / self.channels() as u64
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// The worker, before an input starts: empties the ring and says how
    /// many samples a frame the input will push.
    pub fn begin(&self, channels: usize, loopback: bool) {
        self.read
            .store(self.write.load(Ordering::Acquire), Ordering::Release);
        // A frame count means frames of this many channels from here on.
        let at = self.write.load(Ordering::Acquire);
        let whole = at.div_ceil(channels as u64) * channels as u64;
        self.write.store(whole, Ordering::Release);
        self.read.store(whole, Ordering::Release);
        self.channels.store(channels as u32, Ordering::Relaxed);
        self.dropped.store(0, Ordering::Relaxed);
        self.anchor_seq.store(0, Ordering::Release);
        self.loopback.store(loopback, Ordering::Release);
    }

    pub fn end(&self) {
        self.loopback.store(false, Ordering::Release);
    }

    fn room(&self, samples: usize) -> Option<u64> {
        let (w, r) = (
            self.write.load(Ordering::Relaxed),
            self.read.load(Ordering::Acquire),
        );
        (RING as u64 - (w - r) >= samples as u64).then_some(w)
    }

    /// The audio thread, for the loopback: a block of the master.
    pub fn push_stereo(&self, l: &[f32], r: &[f32]) {
        let Some(w) = self.room(2 * l.len()) else {
            self.dropped.fetch_add(l.len() as u64, Ordering::Relaxed);
            return;
        };
        let mut at = (w % RING as u64) as usize;
        for (a, b) in l.iter().zip(r) {
            self.ring[at].store(a.to_bits(), Ordering::Relaxed);
            self.ring[(at + 1) % RING].store(b.to_bits(), Ordering::Relaxed);
            at = (at + 2) % RING;
        }
        self.write.store(w + 2 * l.len() as u64, Ordering::Release);
    }

    /// An input's callback: its frames, `of` samples each, the first one
    /// captured at `captured_ns`. The take keeps the first one or two samples
    /// of each frame.
    pub fn push_input(&self, data: &[f32], of: usize, captured_ns: u64) {
        let keep = self.channels().min(of);
        let frames = data.len() / of;
        let w = self.write.load(Ordering::Relaxed);
        let seq = self.anchor_seq.load(Ordering::Relaxed) | 1;
        self.anchor_seq.store(seq, Ordering::Release);
        self.anchor_frame.store(w / keep as u64, Ordering::Relaxed);
        self.anchor_ns.store(captured_ns, Ordering::Relaxed);
        self.anchor_seq.store(seq + 1, Ordering::Release);
        if self.room(frames * keep).is_none() {
            self.dropped.fetch_add(frames as u64, Ordering::Relaxed);
            return;
        }
        let mut at = (w % RING as u64) as usize;
        for f in data.chunks_exact(of) {
            for s in &f[..keep] {
                self.ring[at].store(s.to_bits(), Ordering::Relaxed);
                at = (at + 1) % RING;
            }
        }
        self.write
            .store(w + (frames * keep) as u64, Ordering::Release);
    }

    /// The input's last callback: a frame's index and when it was captured.
    /// `None` until the input has called back.
    pub fn anchor(&self) -> Option<(u64, u64)> {
        loop {
            let seq = self.anchor_seq.load(Ordering::Acquire);
            if seq == 0 {
                return None;
            }
            let at = (
                self.anchor_frame.load(Ordering::Relaxed),
                self.anchor_ns.load(Ordering::Relaxed),
            );
            if seq & 1 == 0 && self.anchor_seq.load(Ordering::Acquire) == seq {
                return Some(at);
            }
            std::hint::spin_loop();
        }
    }

    /// The audio thread, at the block a take begins or ends.
    pub fn mark(&self, pos: i64, playing: bool) {
        self.mark_pos.store(pos, Ordering::Relaxed);
        self.mark_playing.store(playing, Ordering::Relaxed);
        self.mark_frame.store(
            self.write.load(Ordering::Relaxed) / self.channels() as u64,
            Ordering::Relaxed,
        );
    }

    /// And when that block's first sample reaches the speaker.
    pub fn mark_time(&self, ns: u64) {
        self.mark_ns.store(ns, Ordering::Release);
    }

    pub fn marked(&self) -> Mark {
        Mark {
            ns: self.mark_ns.load(Ordering::Acquire),
            pos: self.mark_pos.load(Ordering::Relaxed),
            playing: self.mark_playing.load(Ordering::Relaxed),
            frame: self.mark_frame.load(Ordering::Relaxed),
        }
    }

    /// The writer: samples from the ring into `out`, which it clears.
    fn pop(&self, out: &mut Vec<f32>) {
        out.clear();
        let (r, w) = (
            self.read.load(Ordering::Relaxed),
            self.write.load(Ordering::Acquire),
        );
        for i in r..w {
            out.push(f32::from_bits(
                self.ring[(i % RING as u64) as usize].load(Ordering::Relaxed),
            ));
        }
        self.read.store(w, Ordering::Release);
    }
}

/// The audio thread's word at one block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    pub pos: i64,
    /// When `pos` reaches the speaker.
    pub ns: u64,
    pub playing: bool,
    /// Frames the loopback had pushed before this block.
    pub frame: u64,
}

/// Where a take that starts at input frame `first` belongs in the session.
/// The loopback's frames are the session's own, so its take starts at the
/// marked sample. A real input's frame is placed by when it was captured:
/// the sample being heard at that moment.
pub fn place(mark: Mark, rate: u32, first: u64, anchor: Option<(u64, u64)>) -> i64 {
    let (Some((frame, captured_ns)), true) = (anchor, mark.playing) else {
        return mark.pos;
    };
    let captured =
        captured_ns as i128 + (first as i128 - frame as i128) * 1_000_000_000 / rate as i128;
    let after = (captured - mark.ns as i128) * rate as i128;
    // Rounded to the nearest sample.
    let samples = (after + after.signum() * 500_000_000) / 1_000_000_000;
    mark.pos + samples as i64
}

/// What a take came to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Taken {
    pub frames: u64,
    /// The largest sample, as a linear level.
    pub peak: f32,
}

/// The take's writer: frames `first..` of the capture into `path` until
/// `last` says where the take ends (`u64::MAX` while it runs), and into the
/// shared-memory rings as they come.
pub fn write_take(
    capture: &Capture,
    shm: &Shm,
    path: &Path,
    take: TakeFields,
    first: u64,
    last: &AtomicU64,
) -> std::io::Result<Taken> {
    let channels = take.channels as usize;
    let mut file = TakeWriter::create(path, take.channels as u16, take.sample_rate)?;
    if let (Some(peaks), Some(input)) = (shm.peaks(), shm.input()) {
        peaks.begin(&take);
        input.begin(&take);
    }
    let per_peak = take.frames_per_peak as usize;
    // Frames of the capture taken out so far, counted as `Capture::frames` does.
    let mut seen = capture.read.load(Ordering::Acquire) / channels as u64;
    let (mut samples, mut stereo) = (Vec::new(), Vec::new());
    let (mut peak, mut in_peak) = ([0.0f32; 4], 0usize);
    let mut loudest = 0.0f32;
    loop {
        let end = last.load(Ordering::Acquire);
        capture.pop(&mut samples);
        let got = (samples.len() / channels) as u64;
        // The part of what came that is the take's.
        let from = first.clamp(seen, seen + got);
        let to = end.clamp(from, seen + got);
        let mine = &samples[(from - seen) as usize * channels..(to - seen) as usize * channels];
        seen += got;
        if !mine.is_empty() {
            file.write(mine)?;
            stereo.clear();
            for f in mine.chunks_exact(channels) {
                let (l, r) = (f[0], f[channels - 1]);
                stereo.extend_from_slice(&[l, r]);
                loudest = loudest.max(l.abs()).max(r.abs());
                peak = if in_peak == 0 {
                    [l, l, r, r]
                } else {
                    [
                        peak[0].min(l),
                        peak[1].max(l),
                        peak[2].min(r),
                        peak[3].max(r),
                    ]
                };
                in_peak += 1;
                if in_peak == per_peak {
                    if let Some(ring) = shm.peaks() {
                        ring.push(peak);
                    }
                    in_peak = 0;
                }
            }
            if let Some(ring) = shm.input() {
                ring.push(&stereo);
            }
        }
        if seen >= end {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(Taken {
        frames: file.finish()?,
        peak: loudest,
    })
}

/// A take being recorded, as the worker holds it.
pub struct Take {
    pub path: String,
    pub at: i64,
    pub rate: u32,
    pub channels: u32,
    pub input: String,
    pub input_latency: i64,
    /// Where the take ends, as an input frame; `u64::MAX` while it runs.
    pub last: Arc<AtomicU64>,
    pub writer: std::thread::JoinHandle<std::io::Result<Taken>>,
    /// The card's input, kept open for as long as the take.
    pub stream: Option<crate::device::Input>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loopback_take_starts_at_the_marked_sample() {
        let mark = Mark {
            pos: 12_345,
            ns: 1_000_000_000,
            playing: true,
            frame: 77,
        };
        assert_eq!(place(mark, 48_000, 77, None), 12_345);
    }

    #[test]
    fn an_input_frame_belongs_at_the_sample_heard_when_it_was_captured() {
        let mark = Mark {
            pos: 48_000,
            ns: 5_000_000_000,
            playing: true,
            frame: 0,
        };
        // Frame 100 was captured exactly when sample 48000 was heard.
        assert_eq!(place(mark, 48_000, 100, Some((100, 5_000_000_000))), 48_000);
        // Frame 148 came a thousandth of a second later: 48 samples on.
        assert_eq!(place(mark, 48_000, 148, Some((100, 5_000_000_000))), 48_048);
        // A frame captured 10 ms before the mark was heard belongs 480 samples before it.
        assert_eq!(place(mark, 48_000, 100, Some((100, 4_990_000_000))), 47_520);
        // Stopped, nothing moves: the take starts where the playhead stands.
        let stopped = Mark {
            playing: false,
            ..mark
        };
        assert_eq!(
            place(stopped, 48_000, 148, Some((100, 4_990_000_000))),
            48_000
        );
    }

    #[test]
    fn a_full_ring_drops_whole_blocks_and_counts_them() {
        let c = Capture::default();
        c.begin(2, true);
        let block = vec![0.5f32; 4096];
        for _ in 0..RING / 2 / 4096 {
            c.push_stereo(&block, &block);
        }
        assert_eq!(c.dropped(), 0);
        c.push_stereo(&block, &block);
        assert_eq!(c.dropped(), 4096);
        let mut out = Vec::new();
        c.pop(&mut out);
        assert_eq!(out.len(), RING);
        c.push_stereo(&block, &block);
        assert_eq!(
            c.dropped(),
            4096,
            "room again once the writer has taken some out"
        );
    }

    #[test]
    fn a_mono_take_keeps_the_first_sample_of_each_input_frame() {
        let c = Capture::default();
        c.begin(1, false);
        assert_eq!(c.anchor(), None);
        c.push_input(&[0.1, 9.0, 0.2, 9.0, 0.3, 9.0], 2, 42);
        assert_eq!(c.anchor(), Some((0, 42)));
        c.push_input(&[0.4, 9.0], 2, 99);
        assert_eq!(c.anchor(), Some((3, 99)));
        let mut out = Vec::new();
        c.pop(&mut out);
        assert_eq!(out, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(c.frames(), 4);
    }
}
