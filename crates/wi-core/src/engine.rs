//! The engine supervisor (docs/ENGINE.md §1, §3.1 and §5; docs/SPEC.md 9.3).
//!
//! The session lives in the app; the engine only renders. The supervisor
//! starts `wwav-engine` (or `mock-engine`) through wwav-wire, greets it,
//! opens the device and loads whatever session is current. A start that
//! fails is tried again a few times, since a busy device or a crash in the
//! first second is often over by the next try. Then it watches three things:
//! a ping every second, which must answer within one; the shared-memory
//! clock, which must keep moving while it says playing; and, after a play
//! the engine acknowledged, that the audio thread runs a block at all, since
//! a hung audio thread never writes "playing".
//!
//! When the engine dies (its socket closes) or hangs (the supervisor kills
//! it), the supervisor reads the crumb to name the device that was running,
//! starts a new engine on the same shared memory, reloads the session as it
//! stands with that device off and each plugin's last reported state, and
//! locates to where the dead engine's clock last stood. The transport comes
//! back stopped: nothing resumes on its own.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use wwav_wire::client::CallError;
use wwav_wire::graph::{Graph, PluginState};
use wwav_wire::msg::Event as EngineEvent;
use wwav_wire::process::{EngineConfig, EngineProcess};
use wwav_wire::shm::{self, crumb_node, ClockFields, MeterFrame};

use crate::bus::{lock, Bus};
use crate::CoreError;

/// The app pings once a second while connected; no answer within a second
/// means the engine has hung (§3.1).
pub const PING_EVERY: Duration = Duration::from_secs(1);
pub const PING_TIMEOUT: Duration = Duration::from_secs(1);
/// A clock that stands still this long while it says playing is a hang.
pub const STALL: Duration = Duration::from_millis(500);
/// Three crashes in ten minutes from one plugin and it stays off (9.3).
const REPEAT_WINDOW: Duration = Duration::from_secs(600);
const REPEAT_LIMIT: usize = 3;
/// How long a command waits for an engine that is starting or restarting.
const START_WAIT: Duration = Duration::from_secs(5);
const CALL_TIMEOUT: Duration = Duration::from_secs(2);
const LOAD_TIMEOUT: Duration = Duration::from_secs(10);
/// The UI's frame, for meters, and the clock event's period (10 a second).
const FRAME: Duration = Duration::from_millis(16);
const CLOCK_EVERY: Duration = Duration::from_millis(100);

/// A session as the engine receives it, with the names a sentence needs.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineSession {
    pub graph: Graph,
    /// Display names by node id: tracks ("Keys") and devices ("Tape Echo").
    pub names: BTreeMap<String, String>,
}

impl EngineSession {
    fn device_ids(&self) -> impl Iterator<Item = &str> {
        let g = &self.graph;
        g.tracks
            .iter()
            .flat_map(|t| &t.devices)
            .chain(&g.master.devices)
            .map(|d| d.id.as_str())
    }

    /// "the track 'Keys'", or "the master", for the device `id`.
    fn place_of(&self, id: &str) -> String {
        match self
            .graph
            .tracks
            .iter()
            .find(|t| t.devices.iter().any(|d| d.id == id))
        {
            Some(t) => format!("the track '{}'", self.name(&t.id)),
            None => "the master".to_string(),
        }
    }

    fn name(&self, id: &str) -> String {
        self.names
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Starting,
    Running,
    Restarting,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hang {
    Ping,
    Clock,
}

enum Notice {
    Died(u64),
    Hung(u64, Hang),
    Shutdown,
}

struct State {
    phase: Phase,
    /// Counts engines started; events from an older one are ignored.
    generation: u64,
    why: Option<String>,
    device: Option<String>,
    rate: u32,
    block: u32,
    session: Option<EngineSession>,
    /// Devices that load switched off: kept off by the person, or crashed and
    /// not turned on again.
    off: BTreeSet<String>,
    /// Each plugin's last reported state, and when it came.
    states: BTreeMap<String, (PluginState, Instant)>,
    loaded_at: Instant,
    crashes: BTreeMap<String, Vec<Instant>>,
    slots: BTreeMap<String, usize>,
    last_clock: Option<ClockFields>,
    hung: Option<Hang>,
    /// A play the engine acknowledged: which engine, how many blocks its
    /// audio thread had run, and when. A thread that has run none since, a
    /// moment later, is hung, though the clock never said playing.
    awaiting: Option<(u64, u64, Instant)>,
}

type Listener = Arc<dyn Fn(&EngineEvent) + Send + Sync>;

struct Shared {
    config: EngineConfig,
    bus: Arc<Bus>,
    process: RwLock<Option<EngineProcess>>,
    pid: AtomicU32,
    st: Mutex<State>,
    changed: Condvar,
    notices: Mutex<Sender<Notice>>,
    closing: AtomicBool,
    listener: Mutex<Option<Listener>>,
}

/// The engine as the rest of the core sees it.
pub struct Engine {
    shared: Arc<Shared>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

fn restarting() -> CoreError {
    CoreError::new(
        "engine_restarting",
        "The audio engine stopped. It is restarting.",
    )
}

fn call_error(e: CallError) -> CoreError {
    match e {
        CallError::Engine(body) => CoreError::new(&body.code, body.message),
        CallError::Closed => restarting(),
        CallError::Timeout(t) => CoreError::new(
            "engine_timeout",
            format!(
                "The audio engine didn't answer within {} s.",
                t.as_secs_f32()
            ),
        ),
        other => CoreError::new("engine", other.to_string()),
    }
}

impl Engine {
    /// Starts the supervisor, which starts the engine in the background: the
    /// app never waits on the engine to draw (S0.2).
    pub(crate) fn start(config: EngineConfig, bus: Arc<Bus>) -> Engine {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared {
            config,
            bus,
            process: RwLock::new(None),
            pid: AtomicU32::new(0),
            st: Mutex::new(State {
                phase: Phase::Starting,
                generation: 0,
                why: None,
                device: None,
                rate: 0,
                block: 0,
                session: None,
                off: BTreeSet::new(),
                states: BTreeMap::new(),
                loaded_at: Instant::now(),
                crashes: BTreeMap::new(),
                slots: BTreeMap::new(),
                last_clock: None,
                hung: None,
                awaiting: None,
            }),
            changed: Condvar::new(),
            notices: Mutex::new(tx),
            closing: AtomicBool::new(false),
            listener: Mutex::new(None),
        });
        let threads = [
            (
                "engine supervisor",
                Box::new({
                    let s = shared.clone();
                    move || s.supervise(rx)
                }) as Box<dyn FnOnce() + Send>,
            ),
            (
                "engine monitor",
                Box::new({
                    let s = shared.clone();
                    move || s.monitor()
                }),
            ),
            (
                "engine ping",
                Box::new({
                    let s = shared.clone();
                    move || s.pinger()
                }),
            ),
        ]
        .into_iter()
        .filter_map(|(name, f)| thread::Builder::new().name(name.into()).spawn(f).ok())
        .collect();
        Engine {
            shared,
            threads: Mutex::new(threads),
        }
    }

    /// Hears the engine's events (transport, render progress, keys).
    pub(crate) fn set_listener(&self, f: Listener) {
        *lock(&self.shared.listener) = Some(f);
    }

    pub fn phase(&self) -> Phase {
        lock(&self.shared.st).phase
    }

    /// `engine.status`.
    pub fn status(&self) -> Value {
        let st = lock(&self.shared.st);
        let known = st.phase != Phase::Stopped && st.rate > 0;
        json!({
            "state": st.phase,
            "device": if known { json!(st.device) } else { Value::Null },
            "sampleRate": if known { json!(st.rate) } else { Value::Null },
            "block": if known { json!(st.block) } else { Value::Null },
            "why": st.why,
            "off": st.off,
        })
    }

    /// The device's sample rate, once the engine is running.
    pub fn rate(&self) -> Result<u32, CoreError> {
        self.wait_running(START_WAIT)?;
        Ok(lock(&self.shared.st).rate)
    }

    /// The running engine's process id.
    pub fn pid(&self) -> Option<u32> {
        match self.shared.pid.load(Ordering::Relaxed) {
            0 => None,
            pid => Some(pid),
        }
    }

    /// Waits for a starting or restarting engine; refuses a stopped one.
    pub fn wait_running(&self, timeout: Duration) -> Result<(), CoreError> {
        let deadline = Instant::now() + timeout;
        let mut st = lock(&self.shared.st);
        loop {
            match st.phase {
                Phase::Running => return Ok(()),
                Phase::Stopped => {
                    let why = st.why.clone().unwrap_or_default();
                    return Err(CoreError::new(
                        "engine_stopped",
                        format!("The audio engine isn't running. {why}").trim_end(),
                    ));
                }
                Phase::Starting | Phase::Restarting => {}
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(CoreError::new(
                    "engine_starting",
                    "The audio engine is starting. Try again in a moment.",
                ));
            }
            st = self
                .shared
                .changed
                .wait_timeout(st, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    /// Sends one op (`docs/ENGINE.md` §3) and waits for its result.
    pub fn call(
        &self,
        op: &str,
        args: Value,
        timeout: Duration,
    ) -> Result<Map<String, Value>, CoreError> {
        self.wait_running(START_WAIT)?;
        let result = self.shared.call(op, args, timeout);
        if op == "transport.play" && result.is_ok() {
            self.shared.expect_blocks();
        }
        result
    }

    /// Makes `session` the one the engine plays, at `playhead`, and returns
    /// its meter slots by node id. While the engine is restarting, the
    /// session is kept and the restart loads it.
    pub fn load(
        &self,
        session: EngineSession,
        playhead: i64,
    ) -> Result<BTreeMap<String, usize>, CoreError> {
        {
            let mut st = lock(&self.shared.st);
            let ids: BTreeSet<String> = session.device_ids().map(String::from).collect();
            st.states.retain(|id, _| ids.contains(id));
            st.off.retain(|id| ids.contains(id));
            st.session = Some(session);
            st.loaded_at = Instant::now();
        }
        self.wait_running(START_WAIT)?;
        match self.shared.load_now(playhead) {
            Ok(slots) => Ok(slots),
            Err(e) if e.code == "engine_restarting" => {
                self.wait_running(START_WAIT)?;
                Ok(lock(&self.shared.st).slots.clone())
            }
            Err(e) => Err(e),
        }
    }

    /// `param.set`, kept in the session too, so a restart reloads it.
    pub fn param(&self, node: &str, param: &str, value: Value) -> Result<(), CoreError> {
        {
            let mut st = lock(&self.shared.st);
            if let Some(s) = st.session.as_mut() {
                set_in_graph(&mut s.graph, node, param, &value);
            }
        }
        let args = json!({"node": node, "param": param, "value": value});
        match self.call("param.set", args, CALL_TIMEOUT) {
            // The session holds the change; the restart loads it.
            Err(e) if e.code == "engine_restarting" => Ok(()),
            other => other.map(|_| ()),
        }
    }

    /// The shared-memory clock, as the engine last wrote it.
    pub fn clock(&self) -> Option<ClockFields> {
        let p = self
            .shared
            .process
            .read()
            .unwrap_or_else(|e| e.into_inner());
        p.as_ref().and_then(|p| p.shm().region().clock.read())
    }

    /// Waits for the engine to write its next block (a few ms), so the clock
    /// shows what a transport command just did.
    pub fn next_block(&self) {
        self.shared.settle();
    }

    /// The newest meter entry.
    pub fn meters(&self) -> Option<MeterFrame> {
        let p = self
            .shared
            .process
            .read()
            .unwrap_or_else(|e| e.into_inner());
        p.as_ref().and_then(|p| p.shm().region().newest_meters())
    }

    pub fn meter_slots(&self) -> BTreeMap<String, usize> {
        lock(&self.shared.st).slots.clone()
    }

    /// The session as it stands, hashed: what a crash must never change.
    pub fn session_hash(&self) -> Option<String> {
        let st = lock(&self.shared.st);
        st.session.as_ref().map(|s| {
            let graph = with_states(&s.graph, &st.states);
            hex::encode(Sha256::digest(
                serde_json::to_vec(&graph).unwrap_or_default(),
            ))
        })
    }

    /// The devices that load switched off.
    pub fn off(&self) -> Vec<String> {
        lock(&self.shared.st).off.iter().cloned().collect()
    }

    /// **Keep it off** (5.7).
    pub fn keep_off(&self, device: &str) -> Result<(), CoreError> {
        let mut st = lock(&self.shared.st);
        known_device(&st, device)?;
        st.off.insert(device.to_string());
        Ok(())
    }

    /// **Try it again**: the device loads on, unless it crashed three times
    /// in ten minutes.
    pub fn try_again(&self, device: &str) -> Result<(), CoreError> {
        {
            let mut st = lock(&self.shared.st);
            let name = known_device(&st, device)?;
            if recent_crashes(&mut st, device) >= REPEAT_LIMIT {
                return Err(CoreError::new(
                    "crashed_3_times",
                    format!("'{name}' crashed 3 times in ten minutes, so it stays off."),
                ));
            }
            st.off.remove(device);
        }
        let playhead = self.clock().map_or(0, |c| c.sample_pos.max(0));
        self.wait_running(START_WAIT)?;
        self.shared.load_now(playhead).map(|_| ())
    }

    pub(crate) fn shutdown(&self) {
        self.shared.closing.store(true, Ordering::Relaxed);
        let _ = lock(&self.shared.notices).send(Notice::Shutdown);
        for t in lock(&self.threads).drain(..) {
            let _ = t.join();
        }
        // Dropping the process closes the engine's stdin, so it exits (§1).
        let gone = self
            .shared
            .process
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        drop(gone);
        self.shared
            .set_phase(Phase::Stopped, Some("The app closed it.".into()));
    }
}

fn known_device(st: &State, device: &str) -> Result<String, CoreError> {
    st.session
        .as_ref()
        .filter(|s| s.device_ids().any(|d| d == device))
        .map(|s| s.name(device))
        .ok_or_else(|| CoreError::new("no_such_device", "That plugin isn't in this session."))
}

fn recent_crashes(st: &mut State, device: &str) -> usize {
    let list = st.crashes.entry(device.to_string()).or_default();
    list.retain(|t| t.elapsed() < REPEAT_WINDOW);
    list.len()
}

/// The graph with each plugin's last reported state in place.
fn with_states(graph: &Graph, states: &BTreeMap<String, (PluginState, Instant)>) -> Graph {
    let mut g = graph.clone();
    for d in g
        .tracks
        .iter_mut()
        .flat_map(|t| t.devices.iter_mut())
        .chain(g.master.devices.iter_mut())
    {
        if let Some((state, _)) = states.get(&d.id) {
            d.state = Some(state.clone());
        }
    }
    g
}

/// Keeps a `param.set` in the app's copy of the graph.
fn set_in_graph(graph: &mut Graph, node: &str, param: &str, value: &Value) {
    if let Some(t) = graph.tracks.iter_mut().find(|t| t.id == node) {
        match (param, value.as_f64(), value.as_bool()) {
            ("gain_db", Some(v), _) => t.gain_db = v,
            ("pan", Some(v), _) => t.pan = v,
            ("mute", _, Some(b)) => t.mute = b,
            ("solo", _, Some(b)) => t.solo = b,
            ("send.reverb", Some(v), _) => t.sends.reverb = v,
            ("send.delay", Some(v), _) => t.sends.delay = v,
            _ => {}
        }
        return;
    }
    if node == "master" && param == "gain_db" {
        if let Some(v) = value.as_f64() {
            graph.master.gain_db = v;
        }
        return;
    }
    for d in graph
        .tracks
        .iter_mut()
        .flat_map(|t| t.devices.iter_mut())
        .chain(graph.master.devices.iter_mut())
        .filter(|d| d.id == node)
    {
        d.params.insert(param.to_string(), value.clone());
    }
}

/// The newest meter entry as raw bytes, laid out as in shared memory
/// (docs/ENGINE.md §4.4) but holding only the slots in use.
pub fn meter_bytes(m: &MeterFrame) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + m.slots.len() * 16);
    out.extend_from_slice(&m.callback.to_le_bytes());
    out.extend_from_slice(&m.dsp_load.to_le_bytes());
    out.extend_from_slice(&m.dropouts.to_le_bytes());
    out.extend_from_slice(&(m.slots.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 12]);
    for s in &m.slots {
        for v in s {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

fn state_name(state: u32) -> &'static str {
    match state {
        shm::STATE_PLAYING => "playing",
        shm::STATE_RECORDING => "recording",
        _ => "stopped",
    }
}

/// `{sample, hostTimeNs, rate, state}`: the shared-memory clock, for the UI
/// to extrapolate the cursor.
pub fn clock_payload(c: &ClockFields) -> Value {
    json!({
        "sample": c.sample_pos,
        "hostTimeNs": c.host_time_ns,
        "rate": c.rate,
        "state": state_name(c.state),
    })
}

impl Shared {
    fn closing(&self) -> bool {
        self.closing.load(Ordering::Relaxed)
    }

    fn notify(&self, n: Notice) {
        let _ = lock(&self.notices).send(n);
    }

    fn set_phase(&self, phase: Phase, why: Option<String>) {
        let status = {
            let mut st = lock(&self.st);
            st.phase = phase;
            st.why = why;
            json!({"state": phase, "device": st.device, "sampleRate": st.rate, "block": st.block, "why": st.why})
        };
        self.changed.notify_all();
        self.bus.emit("engine", status);
    }

    fn call(
        &self,
        op: &str,
        args: Value,
        timeout: Duration,
    ) -> Result<Map<String, Value>, CoreError> {
        let p = self.process.read().unwrap_or_else(|e| e.into_inner());
        let p = p.as_ref().ok_or_else(restarting)?;
        p.client().call(op, args, timeout).map_err(call_error)
    }

    /// A play was acknowledged: from now the audio thread must run a block
    /// within [`STALL`], whatever the clock says. (The clock only says
    /// playing once the audio thread writes it, so a thread that hung while
    /// stopped would never show it.)
    fn expect_blocks(&self) {
        let blocks = {
            let p = self.process.read().unwrap_or_else(|e| e.into_inner());
            p.as_ref()
                .and_then(|p| p.shm().region().clock.read())
                .map(|c| c.callbacks)
        };
        let mut st = lock(&self.st);
        st.awaiting = blocks.map(|b| (st.generation, b, Instant::now()));
    }

    /// Brings an engine up, trying again after each failure up to `tries`
    /// times, waiting `wait(n)` after the nth.
    fn bring_up_again(
        self: &Arc<Self>,
        playhead: i64,
        tries: u32,
        wait: impl Fn(u32) -> Duration,
    ) -> Result<(), String> {
        let mut n = 0;
        loop {
            n += 1;
            match self.bring_up(playhead) {
                Ok(()) => return Ok(()),
                Err(e) if n >= tries || self.closing() => return Err(e),
                Err(_) => thread::sleep(wait(n)),
            }
        }
    }

    /// The device runs at the session's rate. The engine refuses a session at
    /// any other rate than its device's, and a clip at any other than its
    /// session's, so a song at 44.1 kHz needs the device opened at 44.1 kHz
    /// first (docs/SPEC.md 8.4: "48 kHz, or 44.1 kHz when opened from a
    /// .wwav"). `device.open` refuses while a session at the old rate is
    /// loaded, so it is unloaded first; the load that follows puts the new
    /// one in. If the device can't run at the new rate, it is put back at
    /// the old one, so the next song can still play.
    fn follow_rate(&self, rate: u32) -> Result<(), CoreError> {
        let (device, had, block) = {
            let st = lock(&self.st);
            (st.device.clone(), st.rate, st.block)
        };
        if had == 0 || had == rate {
            return Ok(());
        }
        self.call("session.unload", Value::Null, CALL_TIMEOUT)?;
        let open = |rate: u32| {
            self.call(
                "device.open",
                json!({"name": device, "sample_rate": rate, "block": block}),
                CALL_TIMEOUT,
            )
        };
        match open(rate) {
            Ok(opened) => {
                let mut st = lock(&self.st);
                st.device = opened.get("name").and_then(Value::as_str).map(String::from);
                st.rate = opened
                    .get("sample_rate")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32;
                st.block = opened.get("block").and_then(Value::as_u64).unwrap_or(0) as u32;
                Ok(())
            }
            Err(e) => {
                let _ = open(had);
                Err(e)
            }
        }
    }

    /// `session.load` of the session as it stands.
    fn load_now(&self, playhead: i64) -> Result<BTreeMap<String, usize>, CoreError> {
        let (args, rate) = {
            let st = lock(&self.st);
            let Some(s) = &st.session else {
                return Ok(BTreeMap::new());
            };
            let args = json!({
                "graph": with_states(&s.graph, &st.states),
                "playhead": playhead.max(0),
                "off": st.off,
            });
            (args, s.graph.sample_rate)
        };
        self.follow_rate(rate)?;
        let result = self.call("session.load", args, LOAD_TIMEOUT)?;
        let slots: BTreeMap<String, usize> = result
            .get("meter_slots")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| Some((k.clone(), v.as_u64()? as usize)))
                    .collect()
            })
            .unwrap_or_default();
        lock(&self.st).slots = slots.clone();
        Ok(slots)
    }

    /// Starts an engine (or a new one after a death), greets it, opens the
    /// device, reloads the session and locates. `playhead` is where the
    /// transport comes back, stopped.
    fn bring_up(self: &Arc<Self>, playhead: i64) -> Result<(), String> {
        let events = {
            let mut p = self.process.write().unwrap_or_else(|e| e.into_inner());
            let events = match p.as_mut() {
                Some(proc) => proc.respawn().map_err(|e| e.to_string())?,
                None => {
                    let (proc, events) =
                        EngineProcess::spawn(self.config.clone()).map_err(|e| e.to_string())?;
                    *p = Some(proc);
                    events
                }
            };
            let proc = p.as_ref().expect("just set");
            self.pid.store(proc.pid(), Ordering::Relaxed);
            proc.hello(&format!("wi_wwav {}", env!("CARGO_PKG_VERSION")))
                .map_err(|e| e.to_string())?;
            events
        };
        let device = {
            let st = lock(&self.st);
            match &st.device {
                // The last device, at the last rate and block (§5).
                Some(name) => json!({"name": name, "sample_rate": st.rate, "block": st.block}),
                None => json!({"name": self.config.device}),
            }
        };
        let opened = self
            .call("device.open", device, CALL_TIMEOUT)
            .map_err(|e| e.message)?;
        {
            let mut st = lock(&self.st);
            st.device = opened.get("name").and_then(Value::as_str).map(String::from);
            st.rate = opened
                .get("sample_rate")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;
            st.block = opened.get("block").and_then(Value::as_u64).unwrap_or(0) as u32;
            st.hung = None;
        }
        self.load_now(playhead).map_err(|e| e.message)?;
        if playhead > 0 {
            self.call(
                "transport.locate",
                json!({"sample": playhead}),
                CALL_TIMEOUT,
            )
            .map_err(|e| e.message)?;
        }
        self.settle();
        let generation = {
            let mut st = lock(&self.st);
            st.generation += 1;
            st.last_clock = None;
            st.awaiting = None;
            st.generation
        };
        let pump = self.clone();
        thread::Builder::new()
            .name("engine events".into())
            .spawn(move || pump.pump(events, generation))
            .map_err(|e| e.to_string())?;
        self.set_phase(Phase::Running, None);
        Ok(())
    }

    /// Waits (briefly) for the new engine to write a block after the load
    /// and the locate, so the clock shows where the transport stands before
    /// anyone is told the engine is back.
    fn settle(&self) {
        let p = self.process.read().unwrap_or_else(|e| e.into_inner());
        let Some(region) = p.as_ref().map(|p| p.shm().region()) else {
            return;
        };
        let before = region.clock.read().map(|c| c.callbacks);
        let deadline = Instant::now() + Duration::from_millis(250);
        while Instant::now() < deadline && region.clock.read().map(|c| c.callbacks) == before {
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// One engine's events, until its connection ends: then it has died.
    fn pump(&self, events: Receiver<EngineEvent>, generation: u64) {
        for ev in events {
            if ev.ev == "plugin.state" {
                let node = ev.fields.get("node").and_then(Value::as_str);
                let state = ev
                    .fields
                    .get("state")
                    .and_then(|s| serde_json::from_value::<PluginState>(s.clone()).ok());
                if let (Some(node), Some(state)) = (node, state) {
                    lock(&self.st)
                        .states
                        .insert(node.to_string(), (state, Instant::now()));
                }
            }
            let listener = lock(&self.listener).clone();
            if let Some(f) = listener {
                f(&ev);
            }
        }
        self.notify(Notice::Died(generation));
    }

    fn supervise(self: Arc<Self>, notices: Receiver<Notice>) {
        // A busy device, or a crash in the first second, is often over by the
        // next try: 0.2, 0.5, 1 and 2 s apart, five tries in all.
        let waits = [200, 500, 1000, 2000];
        let first = self.bring_up_again(0, waits.len() as u32 + 1, |n| {
            Duration::from_millis(waits[(n as usize - 1).min(waits.len() - 1)])
        });
        if let Err(why) = first {
            self.set_phase(Phase::Stopped, Some(format!("It couldn't start: {why}")));
        }
        for notice in notices {
            if self.closing() {
                break;
            }
            let current = {
                let st = lock(&self.st);
                (st.phase == Phase::Running).then_some(st.generation)
            };
            match notice {
                Notice::Shutdown => break,
                Notice::Hung(g, how) if Some(g) == current => {
                    lock(&self.st).hung = Some(how);
                    // SIGKILL: the socket closes, and Died follows.
                    let pid = self.pid.load(Ordering::Relaxed);
                    if pid != 0 {
                        // SAFETY: kill has no memory preconditions; pid is our child's.
                        unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
                    }
                }
                Notice::Died(g) if Some(g) == current => self.recover(),
                _ => {}
            }
        }
    }

    /// §5: name the device, restart, reload, locate, stopped.
    fn recover(self: &Arc<Self>) {
        let began = Instant::now();
        let last = lock(&self.st).last_clock;
        let (crumb, playhead) = {
            let p = self.process.read().unwrap_or_else(|e| e.into_inner());
            match p.as_ref() {
                Some(p) => {
                    let region = p.shm().region();
                    let crumb = region.crumb.get();
                    // The region outlives the engine; clear the crumb so a
                    // later death isn't blamed on this one's device.
                    region.crumb.clear();
                    let clock = region.clock.read().or(last);
                    let playhead = clock.map_or(0, |c| c.sample_pos.max(0));
                    // The engine is dead, so the transport is stopped where
                    // it was: say so now, until the new engine writes.
                    region.clock.write(&ClockFields {
                        sample_pos: playhead,
                        host_time_ns: shm::monotonic_ns(),
                        rate: 0.0,
                        state: shm::STATE_STOPPED,
                        ..clock.unwrap_or_default()
                    });
                    (crumb, playhead)
                }
                None => (0, 0),
            }
        };
        let (culprit, plugin, place, hung) = {
            let mut st = lock(&self.st);
            st.phase = Phase::Restarting;
            let hung = st.hung.take();
            let found = st.session.as_ref().and_then(|s| {
                let id = crumb_node(crumb, s.device_ids())?.to_string();
                Some((s.name(&id), s.place_of(&id), id))
            });
            match found {
                Some((plugin, place, id)) => {
                    st.crashes
                        .entry(id.clone())
                        .or_default()
                        .push(Instant::now());
                    st.off.insert(id.clone());
                    (Some(id), Some(plugin), Some(place), hung)
                }
                None => (None, None, None, hung),
            }
        };
        self.changed.notify_all();
        let sentence = match (&plugin, &place, hung) {
            (Some(plugin), Some(place), _) => format!(
                "The audio engine stopped. '{plugin}' on {place} was running when it did. Restarting…"
            ),
            (_, _, Some(_)) => "The audio engine stopped answering. Restarting…".to_string(),
            _ => "The audio engine stopped. Restarting…".to_string(),
        };
        let track = place
            .as_ref()
            .and_then(|p| p.strip_prefix("the track '"))
            .and_then(|p| p.strip_suffix('\''));
        self.bus.emit(
            "engine.stopped",
            json!({"plugin": plugin, "device": culprit, "track": track, "sentence": sentence}),
        );
        self.set_phase(Phase::Restarting, None);

        let up = self.bring_up_again(playhead, 3, |_| Duration::from_millis(200));
        if let Err(why) = up {
            self.set_phase(
                Phase::Stopped,
                Some(format!("It didn't start again: {why}")),
            );
            self.bus.status(
                "engine",
                "The audio engine didn't start again. Quit and reopen Wi_WWAV.",
            );
            return;
        }
        let back = {
            let mut st = lock(&self.st);
            match (&culprit, &plugin) {
                (Some(id), Some(plugin)) if recent_crashes(&mut st, id) >= REPEAT_LIMIT => {
                    format!("Back. '{plugin}' crashed 3 times in ten minutes, so it stays off.")
                }
                (Some(id), Some(plugin)) => {
                    let since = st.states.get(id).map_or(st.loaded_at, |(_, at)| *at);
                    format!(
                        "Back. '{plugin}' is off until you turn it on. Changes made inside its own window in the last {} s may be lost.",
                        since.elapsed().as_secs()
                    )
                }
                _ => "Back.".to_string(),
            }
        };
        let try_again = {
            let mut st = lock(&self.st);
            culprit
                .as_deref()
                .is_some_and(|id| recent_crashes(&mut st, id) < REPEAT_LIMIT)
        };
        self.bus.emit(
            "engine.back",
            json!({
                "plugin": plugin, "device": culprit, "track": track, "sentence": back,
                "playhead": playhead, "tryAgain": try_again,
                "ms": began.elapsed().as_millis() as u64,
            }),
        );
    }

    /// Meters once a frame, and the clock ten times a second, from shared
    /// memory; and the stall check: a clock that says playing but hasn't
    /// moved for 500 ms is a hang.
    fn monitor(&self) {
        let mut last_meter = None;
        let mut last_clock_at = Instant::now();
        let mut last_state = None;
        let mut stall: Option<(u64, u64, Instant)> = None;
        while !self.closing() {
            thread::sleep(FRAME);
            let generation = {
                let st = lock(&self.st);
                if st.phase != Phase::Running {
                    stall = None;
                    continue;
                }
                st.generation
            };
            let p = self.process.read().unwrap_or_else(|e| e.into_inner());
            let Some(region) = p.as_ref().map(|p| p.shm().region()) else {
                continue;
            };
            if self.bus.wants_meters() {
                if let Some(m) = region.newest_meters() {
                    if last_meter != Some(m.callback) {
                        last_meter = Some(m.callback);
                        self.bus.send_meters(&meter_bytes(&m));
                    }
                }
            }
            // After a play the engine acknowledged, its audio thread must
            // run a block. One that hung while stopped never will.
            let awaited = lock(&self.st).awaiting;
            if let Some((g, blocks, since)) = awaited {
                let alive = region.clock.read().map(|c| c.callbacks) != Some(blocks);
                if g != generation || alive {
                    lock(&self.st).awaiting = None;
                } else if since.elapsed() >= STALL {
                    lock(&self.st).awaiting = None;
                    self.notify(Notice::Hung(generation, Hang::Clock));
                }
            }
            if last_clock_at.elapsed() < CLOCK_EVERY {
                continue;
            }
            last_clock_at = Instant::now();
            let Some(c) = region.clock.read() else {
                continue;
            };
            drop(p);
            lock(&self.st).last_clock = Some(c);
            if c.state != shm::STATE_STOPPED || last_state != Some(c.state) {
                self.bus.emit("clock", clock_payload(&c));
            }
            last_state = Some(c.state);
            if c.state == shm::STATE_STOPPED {
                stall = None;
                continue;
            }
            match stall {
                Some((g, callbacks, since)) if g == generation && callbacks == c.callbacks => {
                    if since.elapsed() >= STALL {
                        self.notify(Notice::Hung(generation, Hang::Clock));
                        stall = None;
                    }
                }
                _ => stall = Some((generation, c.callbacks, Instant::now())),
            }
        }
    }

    fn pinger(&self) {
        let mut next = Instant::now() + PING_EVERY;
        while !self.closing() {
            thread::sleep(FRAME);
            if Instant::now() < next {
                continue;
            }
            next = Instant::now() + PING_EVERY;
            let generation = {
                let st = lock(&self.st);
                if st.phase != Phase::Running {
                    continue;
                }
                st.generation
            };
            if let Err(e) = self.call("ping", Value::Null, PING_TIMEOUT) {
                if e.code == "engine_timeout" {
                    self.notify(Notice::Hung(generation, Hang::Ping));
                }
            }
        }
    }
}
