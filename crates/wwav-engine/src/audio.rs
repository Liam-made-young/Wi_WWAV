//! The audio thread and what feeds it (`docs/SPEC.md` 9.2, 9.4): the
//! transport, the clock, the meters, and the graph, which the worker swaps
//! in with one atomic pointer exchange.
//!
//! There is always exactly one audio thread. With a sound card open it is
//! the card's callback; with none it is the timer, which runs a block every
//! block's worth of time, so the clock, the meters and the transport behave
//! the same either way (`docs/ENGINE.md` 1).
//!
//! Commands reach it through two single-producer queues: one from the
//! worker (the transport, the debug ops, a `param.set` that had to wait) and
//! one from the socket thread (`param.set`, so a change never waits behind a
//! load or a render).
//!
//! The audio thread never allocates, locks, logs or waits. What it shares
//! with other threads is atomics, the two queues, and the graph's pointer.

use crate::graph::{Graph, Index, Param, Streams};
use crate::media::MAX_BLOCK;
use crate::record::Capture;
use rtrb::{Consumer, Producer, RingBuffer};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicPtr, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wwav_wire::shm::{self, ClockFields, Shm, METER_SLOTS};

const SEQ: Ordering = Ordering::SeqCst;
const QUEUE: usize = 1024;

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    SetParam {
        node: u32,
        param: Param,
        value: f32,
        /// The graph it was meant for.
        gen: u64,
    },
    Play,
    Stop,
    Locate(i64),
    Loop {
        on: bool,
        start: i64,
        end: i64,
    },
    /// `debug.crash`, `debug.hang` and `debug.crumb`.
    Crash,
    Hang,
    CrashIn {
        node: u32,
        gen: u64,
    },
    /// A take begins or ends at this block.
    Record(bool),
    /// Nothing: the worker waits for it to know the queues were drained.
    Nop,
}

#[derive(Debug, Clone, Copy)]
pub struct Command {
    pub kind: Kind,
    seq: u64,
}

impl From<Kind> for Command {
    fn from(kind: Kind) -> Command {
        Command { kind, seq: 0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Transport {
    pub playing: bool,
    pub pos: i64,
}

/// Where a block's sound goes.
pub enum Output<'a> {
    /// Nowhere: the timer.
    None,
    /// A device's buffer, `channels` samples a frame.
    Interleaved {
        data: &'a mut [f32],
        channels: usize,
    },
}

/// The audio thread's own state.
struct Core {
    transport: Transport,
    looping: Option<(i64, i64)>,
    callbacks: u64,
    commands: Consumer<Command>,
    params: Consumer<Command>,
    slots: Box<[[f32; 4]; METER_SLOTS]>,
    /// A take is being recorded, and its first or last block is this one.
    recording: bool,
    marking: bool,
    /// What the loopback hears with no session loaded.
    silence: Box<[f32; MAX_BLOCK]>,
}

pub struct Shared {
    shm: Shm,
    /// Only ever touched inside `callback`, by whichever thread holds the
    /// `active` token: see `callback` and `Audio::quiet`.
    core: UnsafeCell<Core>,
    graph: AtomicPtr<Graph>,
    /// Odd while a callback runs.
    epoch: AtomicU64,
    /// Which source of callbacks may run a block; 0 for none.
    active: AtomicU64,
    /// A render has the graph: the audio thread plays silence and leaves
    /// the graph and the queues alone.
    parked: AtomicBool,
    /// The last worker command the audio thread applied, and the transport
    /// as that block left it.
    applied: AtomicU64,
    ack_playing: AtomicBool,
    ack_pos: AtomicI64,
    playhead: AtomicI64,
    rate: AtomicU32,
    output_latency: AtomicI64,
    /// Blocks the timer started more than a block late, and a device's own
    /// count of buffers it couldn't fill in time.
    late: AtomicU32,
    xruns: AtomicU32,
    /// Input audio on its way to a take.
    capture: Capture,
}

// SAFETY: every field but `core` is an atomic or the shared-memory region,
// which is atomics too. `core` is reached only from `callback`, and the
// `active` token with the `epoch` count keeps two threads from being inside
// it at once (see there).
unsafe impl Sync for Shared {}
unsafe impl Send for Shared {}

impl Shared {
    pub fn shm(&self) -> &Shm {
        &self.shm
    }

    pub fn playhead(&self) -> i64 {
        self.playhead.load(Ordering::Relaxed)
    }

    pub fn capture(&self) -> &Capture {
        &self.capture
    }

    pub fn count_xrun(&self) {
        self.xruns.fetch_add(1, Ordering::Relaxed);
    }

    /// One block of `n` frames. `token` says which source of callbacks this
    /// is; a source the worker has retired plays silence and touches nothing.
    /// `host_ns` is the device's own time for the block, when it gives one.
    pub fn callback(&self, token: u64, n: usize, mut out: Output, host_ns: Option<u64>) {
        // The count goes odd before the token is read, and `Audio::quiet`
        // clears the token before it reads the count: so either quiet sees
        // this callback and waits for it, or this callback sees it was
        // retired and leaves.
        self.epoch.fetch_add(1, SEQ);
        if self.active.load(SEQ) != token || n == 0 {
            if let Output::Interleaved { data, .. } = &mut out {
                data.fill(0.0);
            }
            self.epoch.fetch_add(1, SEQ);
            return;
        }
        let start = shm::monotonic_ns();
        // SAFETY: this thread holds the token, and the epoch is odd, so no
        // other thread is in here (above) and none will be until it is even.
        let core = unsafe { &mut *self.core.get() };
        let parked = self.parked.load(SEQ);
        let ptr = if parked {
            std::ptr::null_mut()
        } else {
            self.graph.load(SEQ)
        };
        // SAFETY: the worker frees a graph only after it has stored another
        // pointer and seen the epoch even or changed, so a pointer loaded in
        // this callback is live until this callback ends. While parked the
        // worker has the graph, and it isn't loaded here.
        let mut graph = unsafe { ptr.as_mut() };
        // While a render holds the graph, commands wait.
        if !parked {
            core.drain(self, graph.as_deref_mut());
        }
        let rate = self.rate.load(Ordering::Relaxed);
        if graph.as_ref().is_some_and(|g| g.index.sample_rate != rate) {
            // The device isn't at the session's rate, so the session isn't
            // played: at this rate it would run fast or slow and off pitch.
            core.transport.playing = false;
            graph = None;
        }

        let region = self.shm.region();
        let playing = core.transport.playing;
        let dropouts = self.late.load(Ordering::Relaxed) + self.xruns.load(Ordering::Relaxed);
        core.callbacks += 1;
        let latency = if playing {
            self.output_latency.load(Ordering::Relaxed)
        } else {
            0
        };
        let host = host_ns.unwrap_or(start);
        if core.marking {
            // When this block's first sample reaches the speaker.
            let ahead = self.output_latency.load(Ordering::Relaxed) as u64 * 1_000_000_000
                / rate.max(1) as u64;
            self.capture.mark_time(host + ahead);
            core.marking = false;
        }
        region.clock.write(&ClockFields {
            sample_pos: core.transport.pos - latency,
            host_time_ns: host,
            rate: if playing { rate as f64 } else { 0.0 },
            state: match (playing, core.recording) {
                (true, true) => shm::STATE_RECORDING,
                (true, false) => shm::STATE_PLAYING,
                (false, _) => shm::STATE_STOPPED,
            },
            dropouts,
            callbacks: core.callbacks,
        });

        let mut used = 0;
        let mut done = 0;
        while done < n {
            // A device asking for more than MAX_BLOCK frames gets them in
            // pieces; the meters show the last.
            let m = (n - done).min(MAX_BLOCK);
            if let Some(g) = graph.as_deref_mut() {
                g.begin_block(m);
            }
            let mut at = 0;
            while at < m {
                // A block that crosses the loop's end plays to it, then on
                // from its start.
                let mut k = m - at;
                let t = &mut core.transport;
                if let (true, Some((_, end))) = (t.playing, core.looping) {
                    if t.pos < end {
                        k = (k as i64).min(end - t.pos) as usize;
                    }
                }
                if let Some(g) = graph.as_deref_mut() {
                    g.process(t.pos, at, k, t.playing, true, Some(&region.crumb));
                }
                at += k;
                if t.playing {
                    t.pos += k as i64;
                    if let Some((from, end)) = core.looping {
                        if t.pos == end {
                            t.pos = from;
                        }
                    }
                }
            }
            if let Output::Interleaved { data, channels } = &mut out {
                let channels = *channels;
                let frames = &mut data[done * channels..(done + m) * channels];
                match graph.as_deref() {
                    None => frames.fill(0.0),
                    Some(g) => {
                        let (l, r) = g.master_out();
                        for (i, f) in frames.chunks_exact_mut(channels).enumerate() {
                            if channels == 1 {
                                f[0] = 0.5 * (l[i] + r[i]);
                            } else {
                                f[0] = l[i];
                                f[1] = r[i];
                                f[2..].fill(0.0);
                            }
                        }
                    }
                }
            }
            if self.capture.loopback() {
                match graph.as_deref() {
                    Some(g) => {
                        let (l, r) = g.master_out();
                        self.capture.push_stereo(&l[..m], &r[..m]);
                    }
                    None => self
                        .capture
                        .push_stereo(&core.silence[..m], &core.silence[..m]),
                }
            }
            if let Some(g) = graph.as_deref() {
                let last = done + m == n;
                used = g.end_block(m, last.then_some(&mut core.slots[..]));
            }
            done += m;
        }
        self.playhead.store(core.transport.pos, Ordering::Relaxed);
        let load = (shm::monotonic_ns() - start) as f64 * rate as f64 / (n as f64 * 1e9);
        region.write_meters(core.callbacks, load as f32, dropouts, &core.slots[..used]);
        self.epoch.fetch_add(1, SEQ);
    }
}

impl Core {
    fn run(&mut self, kind: Kind, graph: Option<&mut Graph>, shared: &Shared) {
        match kind {
            Kind::SetParam {
                node,
                param,
                value,
                gen,
            } => {
                if let Some(g) = graph.filter(|g| g.index.gen == gen) {
                    g.set(node as usize, param, value);
                }
            }
            Kind::Play => self.transport.playing = true,
            Kind::Stop => self.transport.playing = false,
            Kind::Locate(pos) => self.transport.pos = pos,
            Kind::Loop { on, start, end } => self.looping = on.then_some((start, end)),
            Kind::Crash => std::process::abort(),
            Kind::Hang => loop {
                std::thread::sleep(Duration::from_secs(1));
            },
            Kind::CrashIn { node, gen } => {
                if let Some(g) = graph.filter(|g| g.index.gen == gen) {
                    g.crash_in(node as usize);
                }
            }
            Kind::Record(on) => {
                self.recording = on;
                self.marking = true;
                shared
                    .capture
                    .mark(self.transport.pos, self.transport.playing);
            }
            Kind::Nop => {}
        }
    }

    fn drain(&mut self, shared: &Shared, mut graph: Option<&mut Graph>) {
        let mut last = 0;
        while let Ok(c) = self.commands.pop() {
            self.run(c.kind, graph.as_deref_mut(), shared);
            last = c.seq;
        }
        while let Ok(c) = self.params.pop() {
            self.run(c.kind, graph.as_deref_mut(), shared);
        }
        if last != 0 {
            shared
                .ack_playing
                .store(self.transport.playing, Ordering::Relaxed);
            shared.ack_pos.store(self.transport.pos, Ordering::Relaxed);
            shared.playhead.store(self.transport.pos, Ordering::Relaxed);
            shared.applied.store(last, Ordering::Release);
        }
    }
}

/// What is calling back, as `device.open` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceInfo {
    /// `None` when no device is open (the timer runs unnamed).
    pub name: Option<String>,
    pub rate: u32,
    pub block: u32,
    pub output_latency: i64,
    pub input_latency: i64,
}

/// The graph the audio thread plays, as the worker keeps hold of it.
struct Live {
    /// From `Box::into_raw`. The worker dereferences it only while the audio
    /// thread is parked, and frees it only once the audio thread has let go.
    graph: *mut Graph,
    index: Arc<Index>,
    streams: Streams,
}

/// A graph the audio thread has let go of. Dropped by the worker, never on
/// the audio thread.
pub struct Retired {
    graph: *mut Graph,
    pub streams: Streams,
}

// SAFETY: an owning pointer to a graph nothing else holds any more.
unsafe impl Send for Retired {}

impl Drop for Retired {
    fn drop(&mut self) {
        // SAFETY: `Audio::swap` made this only after the audio thread could
        // no longer hold the pointer; nothing else has it.
        drop(unsafe { Box::from_raw(self.graph) });
    }
}

enum Source {
    Timer(Arc<AtomicBool>),
    Device(crate::device::Stream),
}

/// The worker's handle on the audio thread.
pub struct Audio {
    shared: Arc<Shared>,
    commands: Producer<Command>,
    sent: u64,
    live: Option<Live>,
    source: Option<Source>,
    tokens: u64,
    info: DeviceInfo,
}

// SAFETY: `Live::graph` is the one field that isn't Send by itself; it is
// an owning pointer, used by the thread that holds this handle as described
// on `Live`.
unsafe impl Send for Audio {}

impl Audio {
    /// The handle, and the socket thread's end of the `param.set` queue.
    pub fn new(shm: Shm, rate: u32, block: u32) -> (Audio, Producer<Command>) {
        let (commands, from_worker) = RingBuffer::new(QUEUE);
        let (params, from_socket) = RingBuffer::new(QUEUE);
        let shared = Arc::new(Shared {
            shm,
            core: UnsafeCell::new(Core {
                transport: Transport::default(),
                looping: None,
                callbacks: 0,
                commands: from_worker,
                params: from_socket,
                slots: Box::new([[0.0; 4]; METER_SLOTS]),
                recording: false,
                marking: false,
                silence: Box::new([0.0; MAX_BLOCK]),
            }),
            graph: AtomicPtr::new(std::ptr::null_mut()),
            epoch: AtomicU64::new(0),
            active: AtomicU64::new(0),
            parked: AtomicBool::new(false),
            applied: AtomicU64::new(0),
            ack_playing: AtomicBool::new(false),
            ack_pos: AtomicI64::new(0),
            playhead: AtomicI64::new(0),
            rate: AtomicU32::new(rate),
            output_latency: AtomicI64::new(0),
            late: AtomicU32::new(0),
            xruns: AtomicU32::new(0),
            capture: Capture::default(),
        });
        let audio = Audio {
            shared,
            commands,
            sent: 0,
            live: None,
            source: None,
            tokens: 0,
            info: DeviceInfo {
                name: None,
                rate,
                block,
                ..DeviceInfo::default()
            },
        };
        (audio, params)
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    pub fn device(&self) -> &DeviceInfo {
        &self.info
    }

    /// Retires whatever is calling back, and returns once no callback of
    /// its is inside the audio thread's state.
    fn quiet(&mut self) {
        self.shared.active.store(0, SEQ);
        // A block is a tenth of a second at most. A callback that never
        // comes back (`debug.hang`) never touches the state again either.
        let until = Instant::now() + Duration::from_secs(1);
        while self.shared.epoch.load(SEQ) & 1 == 1 && Instant::now() < until {
            std::thread::sleep(Duration::from_micros(100));
        }
        match self.source.take() {
            Some(Source::Timer(stop)) => stop.store(true, Ordering::Relaxed),
            Some(Source::Device(stream)) => drop(stream),
            None => {}
        }
    }

    fn start(&mut self, info: DeviceInfo) -> u64 {
        self.tokens += 1;
        self.shared.rate.store(info.rate, Ordering::Relaxed);
        self.shared
            .output_latency
            .store(info.output_latency, Ordering::Relaxed);
        self.shared
            .shm
            .region()
            .header
            .set_format(info.rate, info.block);
        self.info = info;
        self.tokens
    }

    /// The timer: no hardware. `named` is whether it was asked for as the
    /// device "null", or stands in because no device is open.
    pub fn open_timer(&mut self, rate: u32, block: u32, named: bool) {
        self.quiet();
        let token = self.start(DeviceInfo {
            name: named.then(|| "null".to_string()),
            rate,
            block,
            output_latency: 0,
            input_latency: 0,
        });
        let stop = Arc::new(AtomicBool::new(false));
        let spawned = std::thread::Builder::new().name("audio".into()).spawn({
            let (shared, stop) = (self.shared.clone(), stop.clone());
            move || timer(&shared, token, rate, block, &stop)
        });
        if let Err(e) = spawned {
            eprintln!("wwav-engine: can't start the audio thread ({e})");
        }
        self.source = Some(Source::Timer(stop));
        self.shared.active.store(token, SEQ);
    }

    /// A sound card, by its name, or the system's default output for `None`.
    /// On failure the timer takes over unnamed, so there is still a clock.
    pub fn open_device(
        &mut self,
        name: Option<&str>,
        rate: u32,
        block: u32,
    ) -> Result<(), (&'static str, String)> {
        let found = crate::device::find(name)?;
        self.quiet();
        match crate::device::open(found, rate, block, self.shared.clone(), self.tokens + 1) {
            Ok((stream, info)) => {
                let token = self.start(info);
                self.source = Some(Source::Device(stream));
                self.shared.active.store(token, SEQ);
                Ok(())
            }
            Err(e) => {
                self.open_timer(rate, block, false);
                Err(e)
            }
        }
    }

    /// No thread at all: the caller runs the blocks itself, with the token
    /// this returns. For tests that must be the audio thread to watch it.
    pub fn open_manual(&mut self, rate: u32, block: u32) -> u64 {
        self.quiet();
        let token = self.start(DeviceInfo {
            name: Some("manual".into()),
            rate,
            block,
            output_latency: 0,
            input_latency: 0,
        });
        self.shared.active.store(token, SEQ);
        token
    }

    /// No device: the timer, unnamed, at the format it had.
    pub fn close_device(&mut self) {
        let (rate, block) = (self.info.rate, self.info.block);
        self.open_timer(rate, block, false);
    }

    /// Queues a command for the audio thread.
    pub fn post(&mut self, kind: Kind) {
        self.sent += 1;
        let mut c = Command {
            kind,
            seq: self.sent,
        };
        // The queue holds 1024 commands, so it is full only when the audio
        // thread has stopped taking them. Give up on this one rather than
        // wait for ever.
        let until = Instant::now() + Duration::from_secs(1);
        while let Err(rtrb::PushError::Full(back)) = self.commands.push(c) {
            c = back;
            if Instant::now() > until {
                eprintln!("wwav-engine: the audio thread isn't taking commands; one was dropped");
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Waits until the audio thread has applied every command posted so
    /// far; returns the transport as that block left it.
    pub fn wait_applied(&self) -> Transport {
        // A block is 0.1 s at most; an audio thread that has stopped gets a second.
        let until = Instant::now() + Duration::from_secs(1);
        while self.shared.applied.load(Ordering::Acquire) < self.sent && Instant::now() < until {
            std::thread::sleep(Duration::from_micros(100));
        }
        self.transport()
    }

    pub fn apply(&mut self, kind: Kind) -> Transport {
        self.post(kind);
        self.wait_applied()
    }

    /// The transport as the last command left it.
    pub fn transport(&self) -> Transport {
        // Pairs with drain()'s release: the ack below is that block's.
        self.shared.applied.load(Ordering::Acquire);
        Transport {
            playing: self.shared.ack_playing.load(Ordering::Relaxed),
            pos: self.shared.ack_pos.load(Ordering::Relaxed),
        }
    }

    pub fn playhead(&self) -> i64 {
        self.shared.playhead()
    }

    pub fn index(&self) -> Option<&Arc<Index>> {
        self.live.as_ref().map(|l| &l.index)
    }

    pub fn streams(&self) -> Option<&Streams> {
        self.live.as_ref().map(|l| &l.streams)
    }

    /// A callback that began before now may still hold the old graph: wait
    /// for it to end. Callbacks that begin later see what was just stored.
    fn wait_callback_end(&self) {
        let e = self.shared.epoch.load(SEQ);
        if e & 1 == 1 {
            let until = Instant::now() + Duration::from_secs(1);
            while self.shared.epoch.load(SEQ) == e && Instant::now() < until {
                std::thread::sleep(Duration::from_micros(50));
            }
        }
    }

    /// One atomic pointer exchange; returns the old graph once the audio
    /// thread has let go of it, to be freed here.
    pub fn swap(&mut self, graph: Option<Box<Graph>>) -> Option<Retired> {
        let live = graph.map(|g| Live {
            index: g.index.clone(),
            streams: g.streams(),
            graph: Box::into_raw(g),
        });
        let ptr = live.as_ref().map_or(std::ptr::null_mut(), |l| l.graph);
        self.shared.graph.store(ptr, SEQ);
        self.wait_callback_end();
        std::mem::replace(&mut self.live, live).map(|old| Retired {
            graph: old.graph,
            streams: old.streams,
        })
    }

    /// A render takes the graph: every change sent so far is in it, the
    /// audio thread plays silence and leaves it alone, and `f` runs with it.
    pub fn parked<T>(&mut self, f: impl FnOnce(&mut Graph) -> T) -> Option<T> {
        self.live.as_ref()?;
        // The audio thread drains both queues at the block that takes this,
        // so a param.set answered before the render was asked for is in.
        self.apply(Kind::Nop);
        self.shared.parked.store(true, SEQ);
        self.wait_callback_end();
        // SAFETY: the audio thread saw `parked` or will before it next
        // loads the pointer, and the callback that might have loaded it
        // already has ended; this thread owns the graph until `parked` clears.
        let out = self.live.as_ref().map(|l| f(unsafe { &mut *l.graph }));
        self.shared.parked.store(false, SEQ);
        out
    }

    /// The engine is going: no more blocks.
    pub fn stop(&mut self) {
        self.quiet();
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.quiet();
        drop(self.swap(None));
    }
}

/// Sleeps until `ns` on the clock's clock. The last stretch is spun, since
/// a sleep comes back late by more than a small block lasts.
fn sleep_until(ns: u64) {
    const SPIN: u64 = 150_000;
    let now = shm::monotonic_ns();
    if ns > now + SPIN {
        std::thread::sleep(Duration::from_nanos(ns - now - SPIN));
    }
    while shm::monotonic_ns() < ns {
        std::hint::spin_loop();
    }
}

/// Asks the system to run this thread ahead of ordinary work.
pub fn make_urgent() {
    #[cfg(target_vendor = "apple")]
    // SAFETY: sets this thread's own quality-of-service class.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE, 0);
    }
}

/// The timer: a block every block's worth of time. Each is due at its
/// nominal time, however late the last one woke, so the clock moves at
/// exactly the rate; a thread more than a block behind counts a dropout and
/// starts over from now rather than racing to catch up.
fn timer(shared: &Shared, token: u64, rate: u32, block: u32, stop: &AtomicBool) {
    make_urgent();
    const SECOND: u64 = 1_000_000_000;
    let (rate, block) = (rate as u64, block as u64);
    let period = block * SECOND / rate;
    let (mut start, mut k) = (shm::monotonic_ns(), 0u64);
    while !stop.load(Ordering::Relaxed) {
        shared.callback(token, block as usize, Output::None, None);
        k += 1;
        let frames = k * block;
        let mut next = start + frames / rate * SECOND + frames % rate * SECOND / rate;
        if shm::monotonic_ns() > next + period {
            shared.late.fetch_add(1, Ordering::Relaxed);
            (start, next, k) = (shm::monotonic_ns(), shm::monotonic_ns(), 0);
        }
        // In slices of 10 ms at most, so a new device never waits out a long block.
        loop {
            let now = shm::monotonic_ns();
            if now >= next || stop.load(Ordering::Relaxed) {
                break;
            }
            sleep_until(next.min(now + 10_000_000));
        }
    }
}
