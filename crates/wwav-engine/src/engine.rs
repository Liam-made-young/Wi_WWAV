//! The ops of `docs/ENGINE.md` 3, and the threads that run them.

use crate::args::Args;
use crate::audio::{Audio, Command, Kind, Retired, Shared, Transport};
use crate::build::{self, number_of, text, whole_of, Converted, Sources};
use crate::export;
use crate::graph::{Index, Param, ROLE_NAMES};
use crate::media::{Failure, HeldCache, MediaPart, Reader, Source, MAX_BLOCK, MAX_SAMPLE};
use crate::record::{self, Take};
use crate::server::{done, fail, Outbox, Reply};
use crate::wav::{WavFormat, WavWriter};
use rtrb::Producer;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use wwav_formats::meta::{Kind as SongKind, Lineage, SongMeta};
use wwav_wire::msg::{self, ErrorBody, HelloResult};
use wwav_wire::shm::{self, TakeFields};
use wwav_wire::{frame, PROTOCOL};

type Args_ = Map<String, Value>;

/// A request waiting its turn on the worker.
enum Job {
    Op {
        conn: u64,
        id: u64,
        op: String,
        args: Args_,
    },
    /// `session.load`, with how many changes had come before it.
    Load {
        conn: u64,
        id: u64,
        args: Args_,
        changes_from: usize,
    },
    /// A `param.set` that had to wait for what was ahead of it.
    Param {
        conn: u64,
        id: u64,
        args: Args_,
        node: String,
    },
    Debug {
        conn: u64,
        id: u64,
        op: String,
        args: Args_,
    },
    /// The engine is going: stop the audio, then say so.
    Quit(Sender<()>),
}

/// What hello reports, kept by the worker, read by the socket thread.
struct Format {
    device: Option<String>,
    rate: u32,
    block: u32,
}

struct Change {
    node: String,
    param: Param,
    value: f32,
}

/// `param.set` is answered on the socket thread and goes straight to the
/// audio thread, so a mute never waits behind a load, a play or a render on
/// the worker (`docs/ENGINE.md` 3.4: heard within one block). This is what
/// it needs from the worker.
struct ParamState {
    /// The names in the graph the audio thread plays.
    live: Option<Arc<Index>>,
    /// `session.load`s received and not yet finished on the worker. A
    /// `param.set` that comes meanwhile changes the playing graph now and is
    /// kept in `changes`, and each pending load replays the ones that came
    /// after it onto its new graph before swapping it in: the load's JSON
    /// was compiled before them.
    loads_pending: u32,
    changes: Vec<Change>,
    /// Nodes with a `param.set` waiting on the worker (one for a node only a
    /// pending load has). Later ones for the same node wait behind it, so a
    /// node's changes land in the order sent.
    deferred: HashMap<String, u32>,
    queue: Producer<Command>,
}

/// The socket file as this engine bound it.
struct Bound {
    path: PathBuf,
    dev: u64,
    ino: u64,
}

pub struct Engine {
    args: Args,
    shared: Arc<Shared>,
    out: Outbox,
    jobs: Mutex<Sender<Job>>,
    param: Mutex<ParamState>,
    format: Mutex<Format>,
    /// A render running on the worker ends at its next block.
    stopping: AtomicBool,
    quitting: AtomicBool,
    connections: AtomicU64,
    bound: Mutex<Option<Bound>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// `hello`'s `features` (`docs/ENGINE.md` 3.1).
const FEATURES: [&str; 4] = ["decode", "resample", "record", "wwav.write"];

fn unknown(op: &str) -> Reply {
    fail("unknown_op", format!("No op named \"{op}\"."))
}

fn hang() -> ! {
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

/// `param.set`'s args checked against the graph, into the command for the
/// audio thread.
fn check_param(index: Option<&Index>, args: &Args_) -> Result<(Kind, Param, f32), ErrorBody> {
    let Some(index) = index else {
        return fail("no_session", "No session is loaded.");
    };
    let node = match args.get("node") {
        Some(Value::String(s)) if !s.contains('\0') => s.as_str(),
        _ => return fail("bad_args", "param.set needs node, a node's id."),
    };
    let name = text(args.get("param"));
    let Some(at) = index.find(node) else {
        return fail("no_such_node", format!("No node {node}."));
    };
    if name == "send.reverb" || name == "send.delay" {
        return fail(
            "unsupported",
            "Sends feed the built-in reverb and delay returns, which come in a later phase.",
        );
    }
    let Some(param) = Param::named(name).filter(|p| index.takes(at, *p)) else {
        let takes = if at == index.master_node() {
            "The master takes gain_db and mute."
        } else {
            "Tracks and buses take gain_db, pan, mute and solo."
        };
        return fail(
            "no_such_param",
            format!("{node} has no param \"{name}\". {takes}"),
        );
    };
    if args.contains_key("at") {
        return fail(
            "unsupported",
            "A change at a set sample comes with automation, in a later phase. Without at, it is heard from the next block.",
        );
    }
    let v = args.get("value");
    let value = match param {
        Param::Mute | Param::Solo => match v.and_then(Value::as_bool) {
            Some(on) => f32::from(u8::from(on)),
            None => return fail("bad_args", format!("{name} is true or false.")),
        },
        Param::GainDb => match number_of(v).filter(|v| *v <= 24.0) {
            Some(v) => v as f32,
            None => return fail("bad_args", "gain_db is a number of dB up to +24."),
        },
        Param::Pan => match number_of(v).filter(|v| (-1.0..=1.0).contains(v)) {
            Some(v) => v as f32,
            None => return fail("bad_args", "pan is a number from -1 to 1."),
        },
    };
    let kind = Kind::SetParam {
        node: at as u32,
        param,
        value,
        gen: index.gen,
    };
    Ok((kind, param, value))
}

impl Engine {
    /// The engine and its worker, which is still to be started.
    pub fn new(args: Args, shm: shm::Shm) -> (Arc<Engine>, Worker) {
        let (audio, queue) = Audio::new(shm, args.rate, args.block);
        let (jobs, queue_of_jobs) = mpsc::channel();
        let engine = Arc::new(Engine {
            shared: audio.shared().clone(),
            out: Outbox::default(),
            jobs: Mutex::new(jobs),
            param: Mutex::new(ParamState {
                live: None,
                loads_pending: 0,
                changes: Vec::new(),
                deferred: HashMap::new(),
                queue,
            }),
            format: Mutex::new(Format {
                device: None,
                rate: args.rate,
                block: args.block,
            }),
            stopping: AtomicBool::new(false),
            quitting: AtomicBool::new(false),
            connections: AtomicU64::new(0),
            bound: Mutex::new(None),
            args,
        });
        let worker = Worker {
            engine: engine.clone(),
            audio,
            reader: Reader::new(),
            held: HeldCache::default(),
            jobs: queue_of_jobs,
            gen: 0,
            loop_from: None,
            take: None,
        };
        (engine, worker)
    }

    /// The socket this engine listens on, so it unlinks only its own.
    pub fn bound(&self, path: &Path) {
        if let Ok(m) = std::fs::symlink_metadata(path) {
            *lock(&self.bound) = Some(Bound {
                path: path.into(),
                dev: m.dev(),
                ino: m.ino(),
            });
        }
    }

    /// An engine that exits leaves the path alone when a replacement has
    /// since bound it.
    fn unlink_socket(&self) {
        if let Some(b) = lock(&self.bound).take() {
            if std::fs::symlink_metadata(&b.path)
                .is_ok_and(|m| (m.dev(), m.ino()) == (b.dev, b.ino))
            {
                let _ = std::fs::remove_file(&b.path);
            }
        }
    }

    fn post(&self, job: Job) {
        // The worker lives as long as the engine.
        let _ = lock(&self.jobs).send(job);
    }

    fn event(&self, ev: &str, fields: Value) {
        self.out.event(ev, fields);
    }

    fn transport_event(&self, t: Transport) {
        let state = if t.playing { "playing" } else { "stopped" };
        self.event("transport", json!({"state": state, "sample": t.pos}));
    }

    /// Stops the audio and exits (stdin closed, `shutdown`, a signal). The
    /// contract gives the engine a second; if stopping hangs (a hung audio
    /// thread, an op that can't be cut short) it leaves anyway, with a
    /// quarter second to spare for a busy machine.
    pub fn quit(self: &Arc<Self>, why: &str) {
        if self.quitting.swap(true, Ordering::SeqCst) {
            return;
        }
        eprintln!("wwav-engine: {why}; stopping");
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(750));
            eprintln!("wwav-engine: stopping took too long; leaving now");
            // SAFETY: ends the process without running anything else.
            unsafe { libc::_exit(0) }
        });
        let engine = self.clone();
        std::thread::spawn(move || {
            engine.stopping.store(true, Ordering::SeqCst);
            engine.out.flush(Duration::from_millis(200));
            let (stopped, wait) = mpsc::channel();
            engine.post(Job::Quit(stopped));
            let _ = wait.recv_timeout(Duration::from_millis(400));
            engine.unlink_socket();
            std::process::exit(0);
        });
    }

    /// One client, until it goes: reads its frames, answers what is quick,
    /// and hands the rest to the worker in the order they came.
    pub fn serve(self: &Arc<Self>, mut stream: UnixStream) {
        let conn = self.connections.fetch_add(1, Ordering::Relaxed) + 1;
        if self.out.connect(conn, &stream).is_err() {
            return;
        }
        let mut greeted = false;
        while let Ok(Some(mut m)) = frame::read(&mut stream) {
            // A request without an id can't be answered: the client is broken.
            let Some(id) = msg::request_id(&m) else { break };
            let args = match m.remove("args") {
                None | Some(Value::Null) => Map::new(),
                Some(Value::Object(args)) => args,
                Some(_) => {
                    self.out.reply(
                        conn,
                        id,
                        fail("bad_args", "A request's args are an object."),
                    );
                    continue;
                }
            };
            let Some(Value::String(op)) = m.remove("op") else {
                self.out
                    .reply(conn, id, fail("bad_request", "A request needs an op."));
                continue;
            };
            if !greeted && op != "hello" {
                self.out
                    .reply(conn, id, fail("hello_first", "Send hello first."));
                continue;
            }
            if !self.request(conn, id, op, args, &mut greeted) {
                // The answer goes out before the connection closes.
                self.out.flush(Duration::from_millis(200));
                break;
            }
        }
        self.out.disconnect(conn);
    }

    /// One request, on the socket thread. False closes the connection.
    fn request(
        self: &Arc<Self>,
        conn: u64,
        id: u64,
        op: String,
        args: Args_,
        greeted: &mut bool,
    ) -> bool {
        let answer = |r: Reply| self.out.reply(conn, id, r);
        match op.as_str() {
            "hello" => {
                let r = self.hello(&args);
                let refused = matches!(&r, Err(e) if e.code == "protocol");
                *greeted = *greeted || r.is_ok();
                answer(r);
                return !refused;
            }
            // Answered here, so a hung message thread shows as an
            // unanswered ping (docs/ENGINE.md 3.1).
            "ping" => answer(done(json!({"t": shm::monotonic_ns()}))),
            "shutdown" => {
                answer(done(json!({})));
                self.quit("asked to shut down");
            }
            "plugin.state" | "plugin.editor.open" | "plugin.editor.close" | "plugin.params" => {
                answer(fail("unsupported", "Plugins are not in this engine."))
            }
            "midi.inputs" | "midi.route" => {
                answer(fail("unsupported", "MIDI comes in a later phase."))
            }
            "debug.crash" | "debug.hang" | "debug.crumb" => {
                if !self.args.test {
                    answer(unknown(&op));
                    return true;
                }
                let place = text(args.get("in"));
                if op != "debug.crumb" && place == "message" {
                    if op == "debug.crash" {
                        // No answer: the connection closing is the answer.
                        std::process::abort();
                    }
                    answer(done(json!({})));
                    self.out.flush(Duration::from_millis(200));
                    hang();
                }
                if op != "debug.crumb" && place != "audio" {
                    answer(fail(
                        "bad_args",
                        "debug.crash and debug.hang take in: \"audio\" or \"message\".",
                    ));
                    return true;
                }
                self.post(Job::Debug { conn, id, op, args });
            }
            "param.set" => self.param_set_now(conn, id, args),
            "session.load" => {
                let changes_from = {
                    let mut p = lock(&self.param);
                    p.loads_pending += 1;
                    p.changes.len()
                };
                self.post(Job::Load {
                    conn,
                    id,
                    args,
                    changes_from,
                });
            }
            "device.list" | "device.open" | "session.unload" | "transport.play"
            | "transport.stop" | "transport.locate" | "transport.loop" | "render"
            | "record.start" | "record.stop" | "wwav.write" => {
                self.post(Job::Op { conn, id, op, args })
            }
            _ => answer(unknown(&op)),
        }
        true
    }

    fn hello(&self, args: &Args_) -> Reply {
        let Some(protocol) = whole_of(args.get("protocol")) else {
            return fail("bad_args", "hello needs the client's protocol number.");
        };
        if protocol != PROTOCOL as i64 {
            return fail(
                "protocol",
                format!("This engine speaks protocol {PROTOCOL}; the app speaks {protocol}."),
            );
        }
        let f = lock(&self.format);
        let hello = HelloResult {
            protocol: PROTOCOL,
            engine: format!("wwav-engine {}", env!("CARGO_PKG_VERSION")),
            pid: std::process::id(),
            sample_rate: f.rate,
            block: f.block,
            device: f.device.clone(),
            shm_layout: shm::LAYOUT,
        };
        let mut hello = serde_json::to_value(hello).unwrap_or_default();
        // What this engine does beyond the first cut of the contract, so a
        // client (and the tests the engines share) can ask instead of guess.
        hello["features"] = json!(FEATURES);
        done(hello)
    }

    /// The socket thread. Checked against the playing graph and queued for
    /// the audio thread at once; heard from the next block.
    fn param_set_now(&self, conn: u64, id: u64, args: Args_) {
        let node = text(args.get("node")).to_string();
        {
            // Not held while the reply is queued: nothing here waits.
            let mut p = lock(&self.param);
            let only_in_a_pending_load =
                p.loads_pending > 0 && p.live.as_ref().is_none_or(|l| l.find(&node).is_none());
            if !p.deferred.contains_key(&node) && !only_in_a_pending_load {
                match check_param(p.live.as_deref(), &args) {
                    Err(e) => {
                        drop(p);
                        return self.out.reply(conn, id, Err(e));
                    }
                    Ok((kind, param, value)) => {
                        if p.queue.push(kind.into()).is_ok() {
                            if p.loads_pending > 0 {
                                p.changes.push(Change { node, param, value });
                            }
                            drop(p);
                            return self.out.reply(conn, id, done(json!({})));
                        }
                        // 1024 changes waiting: the audio thread has stopped
                        // taking them. The worker will try.
                    }
                }
            }
            // A node only a pending load has, or one with a change already
            // waiting: the worker answers it after what is ahead of it.
            *p.deferred.entry(node.clone()).or_insert(0) += 1;
        }
        self.post(Job::Param {
            conn,
            id,
            args,
            node,
        });
    }
}

/// Ops that run one at a time, in order, off the socket thread. Long ones
/// (a load, a render) hold up neither ping nor `param.set`.
pub struct Worker {
    engine: Arc<Engine>,
    audio: Audio,
    reader: Reader,
    held: HeldCache,
    jobs: Receiver<Job>,
    /// The count of `session.load`s that built a graph.
    gen: u64,
    /// The loop last sent, for the streamed clips of every graph to keep ready.
    loop_from: Option<i64>,
    /// The take being recorded.
    take: Option<Take>,
}

impl Worker {
    /// `--device null`, a named device, or the system's default. One that
    /// won't open is said on stderr, and the timer runs so there is a clock;
    /// `device.open` can try again.
    pub fn open_first(&mut self) {
        let (rate, block) = (self.engine.args.rate, self.engine.args.block);
        match self.engine.args.device.clone().as_deref() {
            Some("null") => self.audio.open_timer(rate, block, true),
            name => {
                if let Err((_, message)) = self.audio.open_device(name, rate, block) {
                    eprintln!("wwav-engine: no audio device: {message}");
                }
            }
        }
        self.note_device();
    }

    fn note_device(&self) {
        let d = self.audio.device();
        *lock(&self.engine.format) = Format {
            device: d.name.clone(),
            rate: d.rate,
            block: d.block,
        };
    }

    pub fn run(mut self) {
        while let Ok(job) = self.jobs.recv() {
            let engine = self.engine.clone();
            match job {
                Job::Op { conn, id, op, args } => {
                    let r = self.op(&op, &args);
                    engine.out.reply(conn, id, r);
                }
                Job::Load {
                    conn,
                    id,
                    args,
                    changes_from,
                } => {
                    let r = self.session_load(&args, changes_from);
                    {
                        let mut p = lock(&engine.param);
                        p.loads_pending -= 1;
                        if p.loads_pending == 0 {
                            p.changes.clear();
                        }
                    }
                    engine.out.reply(conn, id, r);
                }
                Job::Param {
                    conn,
                    id,
                    args,
                    node,
                } => {
                    let checked = check_param(self.audio.index().map(|i| &**i), &args);
                    let r = checked.map(|(kind, _, _)| {
                        self.audio.post(kind);
                        Map::new()
                    });
                    {
                        let mut p = lock(&engine.param);
                        if let Some(n) = p.deferred.get_mut(&node) {
                            *n -= 1;
                            if *n == 0 {
                                p.deferred.remove(&node);
                            }
                        }
                    }
                    engine.out.reply(conn, id, r);
                }
                Job::Debug { conn, id, op, args } => {
                    if let Some(r) = self.debug(&op, &args) {
                        engine.out.reply(conn, id, r);
                    }
                }
                Job::Quit(stopped) => {
                    // A take in hand keeps what it has, with a true header.
                    let _ = self.record_stop();
                    self.audio.stop();
                    let _ = stopped.send(());
                }
            }
        }
    }

    fn op(&mut self, op: &str, args: &Args_) -> Reply {
        match op {
            "device.list" => self.device_list(),
            "device.open" => self.device_open(args),
            "session.unload" => self.session_unload(),
            "render" => self.render(args),
            "record.start" => self.record_start(args),
            "record.stop" => self.record_stop(),
            "wwav.write" => self.wwav_write(args),
            _ => self.transport(op, args),
        }
    }

    // ---- devices ----------------------------------------------------------

    fn device_list(&self) -> Reply {
        let mut devices = vec![json!({"name": "null", "inputs": 2, "outputs": 2,
                                      "rates": [44100, 48000, 88200, 96000]})];
        for d in crate::device::list() {
            devices.push(
                json!({"name": d.name, "inputs": d.inputs, "outputs": d.outputs, "rates": d.rates}),
            );
        }
        done(json!({"devices": devices}))
    }

    fn device_open(&mut self, args: &Args_) -> Reply {
        // null: the default, which is the timer when the engine started
        // with --device null.
        let name = match args.get("name") {
            Some(Value::String(name)) => Some(name.clone()),
            None | Some(Value::Null) => self.engine.args.device.clone().filter(|d| d == "null"),
            Some(_) => return fail("bad_args", "device.open's name is a device's name or null."),
        };
        if self.take.is_some() {
            return fail(
                "recording",
                "A take is being recorded. Stop it before changing the device.",
            );
        }
        let now = self.audio.device().clone();
        let rate = match args.get("sample_rate") {
            None => now.rate,
            v => match whole_of(v).filter(|r| (8000..=384_000).contains(r)) {
                Some(r) => r as u32,
                None => return fail("bad_args", "device.open's sample_rate is a rate in Hz."),
            },
        };
        let block = match args.get("block") {
            None => now.block,
            v => match whole_of(v).filter(|b| (16..=MAX_BLOCK as i64).contains(b)) {
                Some(b) => b as u32,
                None => {
                    return fail(
                        "bad_args",
                        format!("device.open's block is from 16 to {MAX_BLOCK} frames."),
                    )
                }
            },
        };
        let session = self.audio.index().map(|i| i.sample_rate);
        if let Some(at) = session.filter(|at| *at != rate) {
            return fail(
                "rate_mismatch",
                format!("The loaded session runs at {at} Hz. Open the device at that rate, or unload the session first."),
            );
        }
        let opened = match name.as_deref() {
            Some("null") => {
                self.audio.open_timer(rate, block, true);
                Ok(())
            }
            name => self.audio.open_device(name, rate, block),
        };
        self.note_device();
        if let Err((code, message)) = opened {
            return fail(code, message);
        }
        let d = self.audio.device().clone();
        // A device that hasn't the rate asked for may open at one it has.
        // With a session loaded that would play it fast or slow: close it.
        if let Some(at) = session.filter(|at| *at != d.rate) {
            self.audio.close_device();
            self.note_device();
            return fail(
                "rate_mismatch",
                format!(
                    "The device \"{}\" opened at {} Hz, not the session's {at} Hz, so it was closed again. Unload the session first, or pick a device that runs at that rate.",
                    d.name.unwrap_or_default(),
                    d.rate
                ),
            );
        }
        done(json!({
            "name": d.name, "sample_rate": d.rate, "block": d.block,
            "output_latency": d.output_latency, "input_latency": d.input_latency,
        }))
    }

    // ---- the session ------------------------------------------------------

    /// A file the engine can't read where it lies, made into one it can.
    fn convert(
        engine: &Engine,
        path: &str,
        source: Source,
        direct: Option<&MediaPart>,
        rate: u32,
    ) -> Result<Converted, Failure> {
        let dir = match &engine.args.cache {
            Some(dir) => dir.clone(),
            None => std::env::temp_dir().join("wwav-engine-cache"),
        };
        let mut tick = Instant::now();
        crate::convert::convert(
            &dir,
            path,
            source,
            direct,
            rate,
            &engine.stopping,
            &mut |done, total| {
                // A few a second: a long file says it is still coming.
                if tick.elapsed() > Duration::from_millis(250) {
                    tick = Instant::now();
                    engine.event(
                        "convert.progress",
                        json!({"path": path, "done": done, "total": total}),
                    );
                }
            },
        )
    }

    fn retire(&self, old: Option<Retired>) {
        if let Some(old) = old {
            for s in old.streams.iter() {
                self.reader.remove(s);
            }
            // The graph is freed here, on the worker: never on the audio thread.
        }
    }

    fn session_load(&mut self, args: &Args_, changes_from: usize) -> Reply {
        let playhead = match args.get("playhead") {
            None => None,
            v => match whole_of(v).filter(|p| (0..=MAX_SAMPLE).contains(p)) {
                Some(p) => Some(p),
                None => return fail("bad_args", "session.load's playhead is a sample from 0."),
            },
        };
        let engine = self.engine.clone();
        let mut sources = Sources {
            held: &mut self.held,
            convert: &mut |path, source, direct, rate| {
                Worker::convert(&engine, path, source, direct, rate)
            },
        };
        let rate = self.audio.device().rate;
        let mut graph = match build::build(args, rate, self.gen + 1, &mut sources) {
            Ok(g) => Box::new(g),
            Err(f) => return fail(f.code, f.message),
        };
        self.gen += 1;
        // Streamed clips start with their window full where the playhead
        // will be, and where a loop jumps back to.
        let streams = graph.streams();
        streams.want_loop(self.loop_from);
        streams.prefill(playhead.unwrap_or_else(|| self.audio.playhead()));
        for s in streams.iter() {
            self.reader.add(s.clone());
        }
        let index = graph.index.clone();
        // The locate is queued before the swap, so the new graph never plays
        // a block at the old playhead: its streamed clips were read ahead at
        // the new one. The block that takes the locate plays either graph at
        // the new place.
        if let Some(p) = playhead {
            self.audio.post(Kind::Locate(p));
        }
        let old = {
            let mut p = lock(&engine.param);
            // param.sets that came after this load: its JSON doesn't have them.
            let mut changed = false;
            for c in p.changes.iter().skip(changes_from) {
                // A node this graph doesn't have is skipped.
                if let Some(node) = index.find(&c.node).filter(|n| index.takes(*n, c.param)) {
                    graph.set(node, c.param, c.value);
                    changed = true;
                }
            }
            if changed {
                // It starts at those levels, as the old graph had ramped to them.
                graph.settle();
            }
            p.live = Some(index.clone());
            self.audio.swap(Some(graph))
        };
        self.retire(old);
        if playhead.is_some() {
            let t = self.audio.wait_applied();
            engine.transport_event(t);
        }
        let slots: Map<String, Value> = index
            .ids()
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), json!(i)))
            .collect();
        done(json!({"nodes": index.nodes(), "latency": {}, "meter_slots": slots}))
    }

    fn session_unload(&mut self) -> Reply {
        let old = {
            let mut p = lock(&self.engine.param);
            p.live = None;
            self.audio.swap(None)
        };
        self.retire(old);
        done(json!({}))
    }

    fn transport(&mut self, op: &str, args: &Args_) -> Reply {
        let sample = |v: Option<&Value>| whole_of(v).filter(|n| (0..=MAX_SAMPLE).contains(n));
        let kind = match op {
            "transport.play" => {
                // Streamed clips need their first quarter second read before
                // play starts.
                if let (Some(streams), Some(index)) = (self.audio.streams(), self.audio.index()) {
                    let (pos, quarter) = (self.audio.playhead(), index.sample_rate as i64 / 4);
                    let until = Instant::now() + Duration::from_millis(300);
                    while !streams.ready(pos, quarter) && Instant::now() < until {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                }
                Kind::Play
            }
            "transport.stop" => Kind::Stop,
            "transport.locate" => match sample(args.get("sample")) {
                Some(n) => Kind::Locate(n),
                None => return fail("bad_args", "transport.locate needs a sample from 0."),
            },
            _ => {
                let Some(on) = args.get("on").and_then(Value::as_bool) else {
                    return fail("bad_args", "transport.loop needs on: true or false.");
                };
                let (mut start, mut end) = (0, 0);
                if on {
                    match (sample(args.get("start")), sample(args.get("end"))) {
                        (Some(a), Some(b)) if b > a => (start, end) = (a, b),
                        _ => {
                            return fail(
                                "bad_args",
                                "A loop needs a start from 0 and an end after it.",
                            )
                        }
                    }
                }
                self.audio.apply(Kind::Loop { on, start, end });
                self.loop_from = on.then_some(start);
                if let Some(streams) = self.audio.streams() {
                    streams.want_loop(self.loop_from);
                }
                return done(json!({}));
            }
        };
        let t = self.audio.apply(kind);
        self.engine.transport_event(t);
        done(json!({"sample": t.pos}))
    }

    // ---- render -----------------------------------------------------------

    fn render(&mut self, args: &Args_) -> Reply {
        let dir = text(args.get("out_dir"));
        if !dir.starts_with('/') {
            return fail("bad_args", "render needs out_dir, an absolute path.");
        }
        let dir = PathBuf::from(dir);
        let sample = |v: Option<&Value>| whole_of(v).filter(|n| (0..=MAX_SAMPLE).contains(n));
        let start = match args.get("start") {
            None => 0,
            v => match sample(v) {
                Some(s) => s,
                None => return fail("bad_args", "render's start is a sample from 0."),
            },
        };
        let Some(len) = sample(args.get("len")).filter(|n| *n > 0) else {
            return fail("bad_args", "render needs len, a number of frames above 0.");
        };
        let mut want = [true, true];
        for (i, key) in ["master", "stems"].iter().enumerate() {
            match args.get(*key) {
                None => {}
                Some(Value::Bool(b)) => want[i] = *b,
                Some(_) => return fail("bad_args", format!("render's {key} is true or false.")),
            }
        }
        if !want[0] && !want[1] {
            return fail("bad_args", "render writes the master, the stems or both.");
        }
        let format = match args.get("format") {
            None => WavFormat::F32,
            v => match text(v) {
                "f32" => WavFormat::F32,
                "s16" => WavFormat::S16,
                _ => return fail("bad_args", "render's format is f32 or s16."),
            },
        };
        let Some(rate) = self.audio.index().map(|i| i.sample_rate) else {
            return fail("no_session", "Load a session before rendering.");
        };
        if std::fs::create_dir_all(&dir).is_err() {
            return fail("render_failed", format!("Can't make {}.", dir.display()));
        }

        // Five files: the master and the stems, in stem order.
        struct Out {
            name: &'static str,
            role: Option<usize>,
            path: PathBuf,
            wav: WavWriter,
        }
        let mut outs = Vec::new();
        let roles = ROLE_NAMES
            .iter()
            .enumerate()
            .map(|(r, name)| (*name, Some(r)));
        for (name, role) in std::iter::once(("master", None)).chain(roles) {
            if (role.is_none() && !want[0]) || (role.is_some() && !want[1]) {
                continue;
            }
            let path = dir.join(format!("{name}.wav"));
            match WavWriter::create(&path, format, rate, len as u64) {
                Ok(wav) => outs.push(Out {
                    name,
                    role,
                    path,
                    wav,
                }),
                Err(message) => return fail("render_failed", message),
            }
        }

        // Live playback stops for the length of a render and comes back stopped.
        if self.audio.transport().playing {
            let t = self.audio.apply(Kind::Stop);
            self.engine.transport_event(t);
        }
        let block = (self.audio.device().block as usize).min(MAX_BLOCK);
        let engine = self.engine.clone();
        let stage = if want[1] { "stems" } else { "master" };
        // The same graph object as live, as fast as the CPU allows, at the
        // session's block size.
        let rendered = self.audio.parked(|g| -> Result<(), String> {
            g.settle();
            let (mut done, mut tick) = (0i64, -1);
            while done < len {
                if engine.stopping.load(Ordering::Relaxed) {
                    return Err("The engine stopped during the render.".into());
                }
                let n = (block as i64).min(len - done) as usize;
                g.begin_block(n);
                g.process(
                    start + done,
                    0,
                    n,
                    true,
                    false,
                    Some(&engine.shared.shm().region().crumb),
                );
                g.end_block(n, None);
                for o in &mut outs {
                    let (l, r) = match o.role {
                        Some(role) => g.bus(role),
                        None => g.master_out(),
                    };
                    o.wav
                        .write(&l[..n], &r[..n])
                        .map_err(|e| format!("Can't write {} ({e}).", o.path.display()))?;
                }
                done += n as i64;
                // About twenty progress events a render.
                if done * 20 / len != tick {
                    tick = done * 20 / len;
                    engine.event(
                        "render.progress",
                        json!({"stage": stage, "done": done, "total": len}),
                    );
                }
            }
            Ok(())
        });
        if let Some(Err(message)) = rendered {
            return fail("render_failed", message);
        }
        let (mut files, mut hashes) = (Map::new(), Map::new());
        for o in outs {
            match o.wav.finish() {
                Ok(sha) => {
                    files.insert(o.name.into(), json!(o.path));
                    hashes.insert(o.name.into(), json!(sha));
                }
                Err(e) => {
                    return fail(
                        "render_failed",
                        format!("Can't write {} ({e}).", o.path.display()),
                    )
                }
            }
        }
        done(json!({"files": files, "frames": len, "sha256": hashes}))
    }

    // ---- writing a .wwav --------------------------------------------------

    fn wwav_write(&mut self, args: &Args_) -> Reply {
        let path = text(args.get("path"));
        if !path.starts_with('/') {
            return fail("bad_args", "wwav.write needs path, an absolute path.");
        }
        let path = PathBuf::from(path);
        let sample = |v: Option<&Value>| whole_of(v).filter(|n| (0..=MAX_SAMPLE).contains(n));
        let start = match args.get("start") {
            None => 0,
            v => match sample(v) {
                Some(s) => s,
                None => return fail("bad_args", "wwav.write's start is a sample from 0."),
            },
        };
        let Some(len) = sample(args.get("len")).filter(|n| *n > 0) else {
            return fail(
                "bad_args",
                "wwav.write needs len, a number of frames above 0.",
            );
        };
        let dither = match args.get("dither") {
            None => true,
            Some(Value::Bool(b)) => *b,
            Some(_) => return fail("bad_args", "wwav.write's dither is true or false."),
        };
        let field = |o: Option<&Value>, key: &str| text(o.and_then(|o| o.get(key))).to_string();
        let m = args.get("meta");
        let kind = match field(m, "type").as_str() {
            "" | "original" => SongKind::Original,
            "split" => SongKind::Split,
            "remix" => SongKind::Remix,
            other => {
                return fail(
                    "bad_args",
                    format!("A song's type is original, split or remix, not \"{other}\"."),
                )
            }
        };
        let mut meta = SongMeta {
            song_id: field(m, "song_id"),
            title: field(m, "title"),
            artist: field(m, "artist"),
            bpm: number_of(m.and_then(|m| m.get("bpm"))).unwrap_or(0.0),
            key: field(m, "key"),
            kind,
            splitter: field(m, "splitter"),
            created: field(m, "created"),
        };
        if meta.song_id.is_empty() {
            meta.song_id = wwav_ids::new_work_id();
        }
        if meta.created.is_empty() {
            meta.created = wwav_formats::text::today();
        }
        let creator = text(args.get("creator"));
        // A parent makes it a child: the parent's root, one generation on.
        let lineage = match args.get("parent").filter(|p| p.is_object()) {
            None => Lineage::original(&meta.song_id, creator),
            Some(p) => {
                let id = text(p.get("song_id"));
                let root = text(p.get("root_id"));
                let generation =
                    whole_of(p.get("generation")).filter(|g| (0..u32::MAX as i64).contains(g));
                let (false, Some(generation)) = (id.is_empty(), generation) else {
                    return fail(
                        "bad_args",
                        "wwav.write's parent needs the parent's song_id and generation.",
                    );
                };
                let parent = Lineage {
                    root_id: if root.is_empty() {
                        id.into()
                    } else {
                        root.into()
                    },
                    generation: generation as u32,
                    ..Lineage::default()
                };
                Lineage::child_of(id, &parent, creator)
            }
        };
        if self.audio.index().is_none() {
            return fail("no_session", "Load a session before writing a .wwav.");
        }
        if self.audio.transport().playing {
            let t = self.audio.apply(Kind::Stop);
            self.engine.transport_event(t);
        }
        let block = (self.audio.device().block as usize).min(MAX_BLOCK);
        let engine = self.engine.clone();
        let mut tick = -1;
        let written = self.audio.parked(|g| {
            export::write_wwav(
                g,
                block,
                start,
                len,
                &path,
                &meta,
                &lineage,
                dither,
                &engine.stopping,
                &mut |done| {
                    if done * 20 / len != tick {
                        tick = done * 20 / len;
                        engine.event(
                            "render.progress",
                            json!({"stage": "wwav", "done": done, "total": len}),
                        );
                    }
                },
            )
        });
        match written {
            Some(Ok(w)) => done(json!({
                "path": path, "song_id": meta.song_id, "frames": w.frames, "bytes": w.bytes,
                "sample_rate": wwav_formats::wwav::RATE,
                "folds": w.folds(),
                "fold_dbfs": if w.fold_dbfs.is_finite() { json!(w.fold_dbfs) } else { Value::Null },
                "peak": w.peak,
            })),
            Some(Err(message)) => fail("write_failed", format!("{message}.").replace("..", ".")),
            None => fail("no_session", "Load a session before writing a .wwav."),
        }
    }

    // ---- recording --------------------------------------------------------

    fn record_start(&mut self, args: &Args_) -> Reply {
        if self.take.is_some() {
            return fail("recording", "A take is already being recorded.");
        }
        let path = text(args.get("path")).to_string();
        if !path.starts_with('/') {
            return fail(
                "bad_args",
                "record.start needs path, an absolute path for the take's WAV.",
            );
        }
        let input = match args.get("input") {
            None | Some(Value::Null) => None,
            Some(Value::String(name)) => Some(name.as_str()),
            Some(_) => {
                return fail(
                    "bad_args",
                    "record.start's input is an input's name or null.",
                )
            }
        };
        let device = self.audio.device().clone();
        let rate = device.rate;
        // The timer has no microphone: its input is its own output, which
        // is also how a take is checked without one.
        let loopback = match input {
            Some(name) => name == "null",
            None => device.name.as_deref() == Some("null"),
        };
        // Said before a card is opened: a take that can't be written.
        if let Err(e) = std::fs::File::create(&path) {
            return fail("record_failed", format!("Can't write {path} ({e})."));
        }
        let shared = self.audio.shared().clone();
        let stream = if loopback {
            shared.capture().begin(2, true);
            None
        } else {
            match crate::device::open_input(input, rate, shared.clone()) {
                Ok(stream) => Some(stream),
                Err((code, message)) => {
                    let _ = std::fs::remove_file(&path);
                    return fail(code, message);
                }
            }
        };
        let (name, channels, latency) = match &stream {
            Some(s) => (s.name.clone(), s.channels as u32, s.latency),
            None => ("null".to_string(), 2, 0),
        };
        // The block that takes this says where the playhead is and when that
        // sample is heard; the block after it has said so in full.
        self.audio.apply(Kind::Record(true));
        self.audio.apply(Kind::Nop);
        let capture = shared.capture();
        let mark = capture.marked();
        let (first, anchor) = match &stream {
            Some(_) => (capture.frames(), capture.anchor()),
            None => (mark.frame, None),
        };
        let at = record::place(mark, rate, first, anchor);
        let fields = TakeFields {
            at,
            sample_rate: rate,
            channels,
            frames_per_peak: record::FRAMES_PER_PEAK,
        };
        let last = Arc::new(AtomicU64::new(u64::MAX));
        let writer = std::thread::Builder::new().name("take".into()).spawn({
            let (shared, last, path) = (shared.clone(), last.clone(), PathBuf::from(&path));
            move || record::write_take(shared.capture(), shared.shm(), &path, fields, first, &last)
        });
        let writer = match writer {
            Ok(writer) => writer,
            Err(e) => {
                self.audio.apply(Kind::Record(false));
                capture.end();
                return fail(
                    "record_failed",
                    format!("Can't start the take's writer ({e})."),
                );
            }
        };
        self.engine.event(
            "record",
            json!({"state": "recording", "path": path, "at": at}),
        );
        self.take = Some(Take {
            path: path.clone(),
            at,
            rate,
            channels,
            input: name.clone(),
            input_latency: latency,
            last,
            writer,
            stream,
        });
        done(json!({
            "path": path, "input": name, "channels": channels, "sample_rate": rate,
            "at": at, "input_latency": latency,
        }))
    }

    fn record_stop(&mut self) -> Reply {
        let Some(take) = self.take.take() else {
            return fail("not_recording", "No take is being recorded.");
        };
        // As at the start: the block that takes this is the take's last word.
        self.audio.apply(Kind::Record(false));
        self.audio.apply(Kind::Nop);
        let shared = self.audio.shared().clone();
        let capture = shared.capture();
        let end = match &take.stream {
            Some(_) => capture.frames(),
            None => capture.marked().frame,
        };
        take.last.store(end, Ordering::Release);
        let written = take.writer.join();
        drop(take.stream);
        capture.end();
        // No block is still pushing into the ring once this one has run.
        self.audio.apply(Kind::Nop);
        let taken = match written {
            Ok(Ok(taken)) => taken,
            Ok(Err(e)) => {
                return fail("record_failed", format!("Can't write {} ({e}).", take.path))
            }
            Err(_) => {
                return fail(
                    "record_failed",
                    format!("The writer of {} stopped.", take.path),
                )
            }
        };
        self.engine.event(
            "record",
            json!({"state": "stopped", "path": take.path, "at": take.at, "frames": taken.frames}),
        );
        done(json!({
            "path": take.path, "frames": taken.frames, "at": take.at, "sample_rate": take.rate,
            "channels": take.channels, "input": take.input, "input_latency": take.input_latency,
            "peak": taken.peak, "dropped": capture.dropped(),
        }))
    }

    // ---- debug (with --test) ----------------------------------------------

    /// `None` when there is no answer: the connection closing is the answer.
    fn debug(&mut self, op: &str, args: &Args_) -> Option<Reply> {
        match op {
            "debug.crumb" => {
                let Some(index) = self.audio.index() else {
                    return Some(fail("no_session", "No session is loaded."));
                };
                let node = text(args.get("node"));
                let Some(at) = index.find(node) else {
                    return Some(fail("no_such_node", format!("No node {node}.")));
                };
                let gen = index.gen;
                self.audio.post(Kind::CrashIn {
                    node: at as u32,
                    gen,
                });
                None
            }
            "debug.crash" => {
                self.audio.post(Kind::Crash);
                None
            }
            _ => {
                self.audio.post(Kind::Hang);
                Some(done(json!({})))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> Index {
        Index::new(7, 44100, vec!["T".into()])
    }

    fn args(v: Value) -> Args_ {
        v.as_object().unwrap().clone()
    }

    fn code(v: Value) -> String {
        check_param(Some(&index()), &args(v))
            .err()
            .map(|e| e.code)
            .unwrap_or_default()
    }

    #[test]
    fn a_change_names_its_node_param_and_graph() {
        let (kind, ..) = check_param(
            Some(&index()),
            &args(json!({"node": "bus:bass", "param": "mute", "value": true})),
        )
        .unwrap();
        assert!(
            matches!(kind, Kind::SetParam { node: 4, param: Param::Mute, gen: 7, value } if value == 1.0)
        );
    }

    #[test]
    fn what_is_wrong_with_a_change_has_its_own_code() {
        assert_eq!(
            code(json!({"node": "X", "param": "mute", "value": true})),
            "no_such_node"
        );
        assert_eq!(
            code(json!({"node": "T", "param": "wobble", "value": 1})),
            "no_such_param"
        );
        assert_eq!(
            code(json!({"node": "master", "param": "solo", "value": true})),
            "no_such_param"
        );
        assert_eq!(
            code(json!({"node": "T", "param": "mute", "value": "yes"})),
            "bad_args"
        );
        assert_eq!(
            code(json!({"node": "T", "param": "pan", "value": 2.0})),
            "bad_args"
        );
        assert_eq!(
            code(json!({"node": "T", "param": "gain_db", "value": 25})),
            "bad_args"
        );
        assert_eq!(
            code(json!({"node": "T", "param": "send.reverb", "value": 0.5})),
            "unsupported"
        );
        assert_eq!(
            code(json!({"node": "T", "param": "gain_db", "value": -3, "at": 48000})),
            "unsupported"
        );
        assert_eq!(
            code(json!({"node": 5, "param": "mute", "value": true})),
            "bad_args"
        );
        assert_eq!(
            code(json!({"node": "x\u{0}", "param": "mute", "value": true})),
            "bad_args"
        );
        let none = check_param(
            None,
            &args(json!({"node": "T", "param": "mute", "value": true})),
        );
        assert_eq!(none.err().unwrap().code, "no_session");
    }
}
