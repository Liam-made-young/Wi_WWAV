//! The mock engine's state and its answer to every op in docs/ENGINE.md §3.
//! It speaks the contract exactly; only the audio is fake (`model`).

use crate::model::{self, Mix, Strip};
use crate::wav::{SampleFormat, WavWriter};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use wwav_wire::frame;
use wwav_wire::graph::{Device, Format, Graph, PluginState, Role};
use wwav_wire::msg::{self, ErrorBody, Event, HelloResult, Request, Response};
use wwav_wire::shm::{self, ClockFields, HeaderFields, Shm, METER_SLOTS};
use wwav_wire::PROTOCOL;

pub type Reply = Result<Map<String, Value>, ErrorBody>;

fn fail(code: &str, message: impl Into<String>) -> Reply {
    Err(ErrorBody::new(code, message))
}

fn done(v: Value) -> Reply {
    match v {
        Value::Object(m) => Ok(m),
        _ => Ok(Map::new()),
    }
}

/// The devices the mock pretends to have. `null` is the timer (§1).
pub struct FakeDevice {
    pub name: &'static str,
    inputs: u32,
    outputs: u32,
    rates: &'static [u32],
    output_latency: i64,
    input_latency: i64,
}

pub const DEVICES: [FakeDevice; 2] = [
    FakeDevice {
        name: "null",
        inputs: 0,
        outputs: 2,
        rates: &[44100, 48000, 88200, 96000],
        output_latency: 0,
        input_latency: 0,
    },
    FakeDevice {
        name: "Mock Interface",
        inputs: 2,
        outputs: 2,
        rates: &[44100, 48000, 96000],
        output_latency: 312,
        input_latency: 296,
    },
];

pub fn device_named(name: &str) -> Option<&'static FakeDevice> {
    DEVICES.iter().find(|d| d.name == name)
}

pub fn valid_format(device: &FakeDevice, rate: u32, block: u32) -> Result<(), String> {
    if !device.rates.contains(&rate) {
        return Err(format!(
            "{} runs at {:?} Hz, not {rate}.",
            device.name, device.rates
        ));
    }
    if !(16..=4096).contains(&block) {
        return Err(format!("A block is from 16 to 4096 frames, not {block}."));
    }
    Ok(())
}

const MIDI_INPUT: (&str, &str) = ("mock-keys", "Mock Keys");
/// States over this go to a file, never inline (§3.7).
const INLINE_STATE_MAX: usize = 256 * 1024;
/// The mock plugin's parameters, p0 to p3; a parameter never set reads 0.
/// Lookahead adds latency, like a real limiter's.
const PLUGIN_PARAMS: [&str; 4] = ["Mix", "Time", "Feedback", "Lookahead"];
const PLUGIN_STATE_EVERY: Duration = Duration::from_secs(60);
const RENDER_CHUNK: i64 = 65_536;

/// What the audio thread does on its next block, from the `debug.*` ops.
#[derive(Debug, Clone, Copy)]
pub enum Fault {
    Crash,
    Hang,
    Crumb(u64),
}

/// What the message thread does after it answers.
pub enum After {
    Nothing,
    Close,
    Exit,
    Abort,
    Hang,
    Fault(Fault),
}

pub struct State {
    device: &'static FakeDevice,
    pub rate: u32,
    pub block: u32,
    playing: bool,
    pos: i64,
    looping: Option<(i64, i64)>,
    session: Option<Session>,
    dropouts: u32,
    callbacks: u64,
    fault: Option<Fault>,
    plugins_dirty: bool,
    plugins_sent: Instant,
}

struct Session {
    graph: Graph,
    off: HashSet<String>,
    buses: [Strip; 4],
    master: Strip,
    /// Node ids in meter-slot order.
    slots: Vec<String>,
    /// States loaded from the app that aren't the mock plugin's own: kept
    /// and returned as they came until a parameter changes.
    opaque: HashMap<String, Vec<u8>>,
    /// `param.set` changes with an `at` still ahead of the playhead.
    pending: Vec<(i64, String, Change)>,
    mix: Mix,
    /// The crumb of every device the graph calls, in the order it calls them.
    crumbs: Vec<u64>,
    /// The longest path's plugin delay, which the clock subtracts.
    delay: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Node {
    Track(usize),
    Bus(usize),
    Master,
    /// A device on a track (`Some(track)`) or on the master (`None`).
    Device(Option<usize>, usize),
}

#[derive(Debug, Clone)]
enum Change {
    Gain(f64),
    Pan(f64),
    Mute(bool),
    Solo(bool),
    Reverb(f64),
    Delay(f64),
    Param(String, Value),
}

/// The mock's plugin delay rule: built-ins have none; a third-party device
/// reports 0, 64, 128 or 192 samples, fixed by its id, plus 2048 × its
/// Lookahead (p3), so a test can change it and see `plugin.latency`.
fn latency(d: &Device) -> i64 {
    match d.format {
        Format::Builtin => 0,
        Format::Vst3 | Format::Au => {
            let lookahead = d.params.get("p3").and_then(Value::as_f64).unwrap_or(0.0);
            64 * (shm::crumb_hash(&d.id) % 4) as i64 + (lookahead * 2048.0).round() as i64
        }
    }
}

impl Session {
    fn new(graph: Graph, off: HashSet<String>) -> Result<Session, ErrorBody> {
        let mut slots: Vec<String> = graph.tracks.iter().map(|t| t.id.clone()).collect();
        slots.extend(Role::ALL.iter().map(|r| r.bus()));
        slots.push("master".into());
        let master = Strip {
            gain_db: graph.master.gain_db,
            ..Strip::default()
        };
        let mut s = Session {
            graph,
            off,
            buses: [Strip::default(); 4],
            master,
            slots,
            opaque: HashMap::new(),
            pending: Vec::new(),
            mix: Mix {
                tracks: vec![],
                buses: [[0.0; 2]; 4],
                master: [0.0; 2],
                master_gain: 1.0,
            },
            crumbs: Vec::new(),
            delay: 0,
        };
        s.restore_states()?;
        s.crumbs = s
            .devices()
            .filter(|d| s.runs(d))
            .map(|d| shm::crumb_hash(&d.id))
            .collect();
        s.delay = s.delay();
        s.remix();
        Ok(s)
    }

    fn runs(&self, d: &Device) -> bool {
        d.on && !self.off.contains(&d.id)
    }

    /// The longest track's plugin delay plus the master chain's.
    fn delay(&self) -> i64 {
        let path = |ds: &[Device]| ds.iter().filter(|d| self.runs(d)).map(latency).sum::<i64>();
        let longest = self
            .graph
            .tracks
            .iter()
            .map(|t| path(&t.devices))
            .max()
            .unwrap_or(0);
        longest + path(&self.graph.master.devices)
    }

    fn remix(&mut self) {
        self.mix = model::mix(&self.graph, &self.buses, &self.master);
    }

    fn devices(&self) -> impl Iterator<Item = &Device> {
        self.graph
            .tracks
            .iter()
            .flat_map(|t| &t.devices)
            .chain(&self.graph.master.devices)
    }

    fn find(&self, id: &str) -> Option<Node> {
        if id == "master" {
            return Some(Node::Master);
        }
        if let Some(r) = Role::ALL.iter().position(|r| r.bus() == id) {
            return Some(Node::Bus(r));
        }
        for (t, track) in self.graph.tracks.iter().enumerate() {
            if track.id == id {
                return Some(Node::Track(t));
            }
            if let Some(d) = track.devices.iter().position(|d| d.id == id) {
                return Some(Node::Device(Some(t), d));
            }
        }
        let d = self.graph.master.devices.iter().position(|d| d.id == id)?;
        Some(Node::Device(None, d))
    }

    fn device(&self, id: &str) -> Option<&Device> {
        match self.find(id)? {
            Node::Device(Some(t), d) => Some(&self.graph.tracks[t].devices[d]),
            Node::Device(None, d) => Some(&self.graph.master.devices[d]),
            _ => None,
        }
    }

    fn device_mut(&mut self, id: &str) -> Option<&mut Device> {
        match self.find(id)? {
            Node::Device(Some(t), d) => Some(&mut self.graph.tracks[t].devices[d]),
            Node::Device(None, d) => Some(&mut self.graph.master.devices[d]),
            _ => None,
        }
    }

    /// Loads each device's saved state: the mock plugin's own state gives
    /// back its parameters; anything else is kept as it came.
    fn restore_states(&mut self) -> Result<(), ErrorBody> {
        let mut opaque = HashMap::new();
        let devices = self
            .graph
            .tracks
            .iter_mut()
            .flat_map(|t| &mut t.devices)
            .chain(&mut self.graph.master.devices);
        for d in devices {
            let bytes = match &d.state {
                None => continue,
                Some(PluginState::Inline(b64)) => BASE64.decode(b64).map_err(|e| e.to_string()),
                Some(PluginState::File(path)) => {
                    std::fs::read(path).map_err(|e| format!("{path}: {e}"))
                }
            }
            .map_err(|e| {
                ErrorBody::new(
                    "bad_session",
                    format!("Device {}'s state doesn't read ({e}).", d.id),
                )
            })?;
            match mock_params(&bytes, &d.uid) {
                Some(params) => d.params = params,
                None => {
                    opaque.insert(d.id.clone(), bytes);
                }
            }
        }
        self.opaque = opaque;
        Ok(())
    }

    fn state_blob(&self, d: &Device) -> Vec<u8> {
        match self.opaque.get(&d.id) {
            Some(bytes) => bytes.clone(),
            None => mock_state(d),
        }
    }

    /// Applies one change. A device whose latency it changes comes back
    /// with its new latency, for `plugin.latency`.
    fn apply(&mut self, node: Node, change: Change) -> Option<(String, i64)> {
        let strip = |s: &mut Strip, c: &Change| match *c {
            Change::Gain(v) => s.gain_db = v,
            Change::Pan(v) => s.pan = v,
            Change::Mute(v) => s.mute = v,
            Change::Solo(v) => s.solo = v,
            _ => {}
        };
        match node {
            Node::Track(t) => {
                let track = &mut self.graph.tracks[t];
                match change {
                    Change::Gain(v) => track.gain_db = v,
                    Change::Pan(v) => track.pan = v,
                    Change::Mute(v) => track.mute = v,
                    Change::Solo(v) => track.solo = v,
                    Change::Reverb(v) => track.sends.reverb = v,
                    Change::Delay(v) => track.sends.delay = v,
                    Change::Param(..) => {}
                }
            }
            Node::Bus(r) => strip(&mut self.buses[r], &change),
            Node::Master => strip(&mut self.master, &change),
            Node::Device(..) => {
                if let Change::Param(name, value) = change {
                    let id = self.node_id(node);
                    self.opaque.remove(&id);
                    let d = self.device_mut(&id)?;
                    let before = latency(d);
                    d.params.insert(name, value);
                    let after = latency(d);
                    if after != before {
                        // Delay compensation is recomputed before anyone hears of it.
                        self.delay = self.delay();
                        return Some((id, after));
                    }
                }
            }
        }
        self.remix();
        None
    }

    fn node_id(&self, node: Node) -> String {
        match node {
            Node::Track(t) => self.graph.tracks[t].id.clone(),
            Node::Bus(r) => Role::ALL[r].bus(),
            Node::Master => "master".into(),
            Node::Device(Some(t), d) => self.graph.tracks[t].devices[d].id.clone(),
            Node::Device(None, d) => self.graph.master.devices[d].id.clone(),
        }
    }
}

/// The mock plugin's state: its uid and parameters as JSON with sorted keys,
/// padded with spaces to `mock.state_bytes` when a test wants a big one.
fn mock_state(d: &Device) -> Vec<u8> {
    let params: BTreeMap<&String, &Value> = d.params.iter().collect();
    let mut bytes = serde_json::to_vec(&json!({"mock_plugin": 1, "uid": d.uid, "params": params}))
        .expect("json");
    if let Some(n) = d.params.get("mock.state_bytes").and_then(Value::as_u64) {
        bytes.resize(bytes.len().max(n as usize), b' ');
    }
    bytes
}

fn mock_params(bytes: &[u8], uid: &str) -> Option<Map<String, Value>> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if v["mock_plugin"] != 1 || v["uid"] != uid {
        return None;
    }
    v["params"].as_object().cloned()
}

/// Which change `param` with `value` is on `node`, or why it is none.
fn change(
    s: &Session,
    node: Node,
    id: &str,
    param: &str,
    value: &Value,
) -> Result<Change, ErrorBody> {
    let number = |lo: f64, hi: f64, what: &str| {
        value
            .as_f64()
            .filter(|v| v.is_finite() && (lo..=hi).contains(v))
            .ok_or_else(|| ErrorBody::new("bad_args", what))
    };
    let flag = || {
        value
            .as_bool()
            .ok_or_else(|| ErrorBody::new("bad_args", format!("{param} is true or false.")))
    };
    let gain = || number(f64::MIN, 24.0, "gain_db is a number of dB up to +24.").map(Change::Gain);
    let pan = || number(-1.0, 1.0, "pan is a number from -1 to 1.").map(Change::Pan);
    let send = |what| number(0.0, 1.0, what);
    let no_param = |takes: &str| {
        Err(ErrorBody::new(
            "no_such_param",
            format!("{id} has no param \"{param}\". {takes}"),
        ))
    };
    match (node, param) {
        (Node::Track(_) | Node::Bus(_) | Node::Master, "gain_db") => gain(),
        (Node::Track(_) | Node::Bus(_), "pan") => pan(),
        (Node::Track(_) | Node::Bus(_) | Node::Master, "mute") => flag().map(Change::Mute),
        (Node::Track(_) | Node::Bus(_), "solo") => flag().map(Change::Solo),
        (Node::Track(_), "send.reverb") => {
            send("A send is a number from 0 to 1.").map(Change::Reverb)
        }
        (Node::Track(_), "send.delay") => {
            send("A send is a number from 0 to 1.").map(Change::Delay)
        }
        (Node::Track(_), _) => {
            no_param("Tracks take gain_db, pan, mute, solo, send.reverb and send.delay.")
        }
        (Node::Bus(_), _) => no_param("Buses take gain_db, pan, mute and solo."),
        (Node::Master, _) => no_param("The master takes gain_db and mute."),
        (Node::Device(..), _) => {
            let d = s.device(id).expect("found");
            if d.format != Format::Builtin {
                let index = param
                    .strip_prefix('p')
                    .and_then(|i| i.parse::<usize>().ok());
                if !index.is_some_and(|i| i < PLUGIN_PARAMS.len()) {
                    return no_param("This plugin's params are p0 to p3.");
                }
                number(0.0, 1.0, "A plugin param is a number from 0 to 1.")?;
            } else if !(value.is_number() || value.is_boolean()) {
                return Err(ErrorBody::new(
                    "bad_args",
                    format!("{param} is a number or true or false."),
                ));
            }
            Ok(Change::Param(param.into(), value.clone()))
        }
    }
}

fn whole(v: Option<&Value>) -> Option<i64> {
    let v = v?;
    v.as_i64().or_else(|| {
        v.as_f64()
            .filter(|f| f.fract() == 0.0 && f.abs() < 9e15)
            .map(|f| f as i64)
    })
}

pub struct Engine {
    shm: Shm,
    test: bool,
    socket: PathBuf,
    state: Mutex<State>,
    /// The connected client, for replies and events.
    out: Mutex<Option<UnixStream>>,
}

impl Engine {
    pub fn new(
        shm: Shm,
        socket: PathBuf,
        test: bool,
        device: &'static FakeDevice,
        rate: u32,
        block: u32,
    ) -> Engine {
        shm.region().header.write(&HeaderFields {
            sample_rate: rate,
            block_size: block,
            engine_pid: std::process::id() as u64,
            engine_start_ns: shm::monotonic_ns(),
        });
        let state = State {
            device,
            rate,
            block,
            playing: false,
            pos: 0,
            looping: None,
            session: None,
            dropouts: 0,
            callbacks: 0,
            fault: None,
            plugins_dirty: false,
            plugins_sent: Instant::now(),
        };
        Engine {
            shm,
            test,
            socket,
            state: Mutex::new(state),
            out: Mutex::new(None),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn send<T: Serialize>(&self, msg: &T) {
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(stream) = out.as_mut() {
            // A client that has gone is noticed by the read side.
            let _ = frame::write(stream, msg);
        }
    }

    fn emit(&self, ev: &str, fields: Value) {
        let Value::Object(fields) = fields else {
            return;
        };
        self.send(&Event {
            ev: ev.into(),
            fields,
        });
    }

    /// The engine has gone: stdin closed, or `shutdown`.
    pub fn exit(&self) -> ! {
        let _ = std::fs::remove_file(&self.socket);
        std::process::exit(0)
    }

    /// One client, until it goes. Requests are answered in order.
    pub fn serve(&self, mut stream: UnixStream) {
        *self.out.lock().unwrap_or_else(|e| e.into_inner()) = stream.try_clone().ok();
        let mut greeted = false;
        while let Ok(Some(m)) = frame::read(&mut stream) {
            // A request without an id can't be answered: the client is broken.
            let Some(id) = msg::request_id(&m) else { break };
            let (reply, after) = if m
                .get("args")
                .is_some_and(|a| !a.is_object() && !a.is_null())
            {
                (
                    Some(fail("bad_args", "A request's args are an object.")),
                    After::Nothing,
                )
            } else {
                match Request::from_object(m) {
                    Err(_) => (
                        Some(fail("bad_request", "A request needs an op.")),
                        After::Nothing,
                    ),
                    Ok(req) if !greeted && req.op != "hello" => (
                        Some(fail("hello_first", "Send hello first.")),
                        After::Nothing,
                    ),
                    Ok(req) => self.handle(&req.op, &req.args(), &mut greeted),
                }
            };
            if let Some(outcome) = reply {
                self.send(&Response { id, outcome });
            }
            match after {
                After::Nothing => {}
                After::Close => break,
                After::Exit => self.exit(),
                After::Abort => std::process::abort(),
                After::Hang => hang(),
                // Set after the answer is out, so the answer always arrives.
                After::Fault(f) => self.lock().fault = Some(f),
            }
        }
        let _ = stream.shutdown(std::net::Shutdown::Both);
        *self.out.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn handle(
        &self,
        op: &str,
        args: &Map<String, Value>,
        greeted: &mut bool,
    ) -> (Option<Reply>, After) {
        let reply = match op {
            "hello" => return self.hello(args, greeted),
            "ping" => done(json!({"t": shm::monotonic_ns()})),
            "shutdown" => return (Some(done(json!({}))), After::Exit),
            "device.list" => self.device_list(),
            "device.open" => self.device_open(args),
            "session.load" => self.session_load(args),
            "session.unload" => {
                self.lock().session = None;
                done(json!({}))
            }
            "param.set" => self.param_set(args),
            "transport.play" | "transport.stop" | "transport.locate" | "transport.loop" => {
                self.transport(op, args)
            }
            "render" => self.render(args),
            "plugin.state" | "plugin.editor.open" | "plugin.editor.close" | "plugin.params" => {
                self.plugin(op, args)
            }
            "midi.inputs" => done(json!({"inputs": [{"id": MIDI_INPUT.0, "name": MIDI_INPUT.1}]})),
            "midi.route" => self.midi_route(args),
            "debug.crash" | "debug.hang" | "debug.crumb" if self.test => {
                return self.debug(op, args)
            }
            _ => fail("unknown_op", format!("No op named \"{op}\".")),
        };
        (Some(reply), After::Nothing)
    }

    fn hello(&self, args: &Map<String, Value>, greeted: &mut bool) -> (Option<Reply>, After) {
        let Some(protocol) = args.get("protocol").and_then(Value::as_u64) else {
            return (
                Some(fail(
                    "bad_args",
                    "hello needs the client's protocol number.",
                )),
                After::Nothing,
            );
        };
        if protocol != PROTOCOL as u64 {
            let message =
                format!("This engine speaks protocol {PROTOCOL}; the app speaks {protocol}.");
            return (Some(fail("protocol", message)), After::Close);
        }
        *greeted = true;
        let st = self.lock();
        let hello = HelloResult {
            protocol: PROTOCOL,
            engine: format!("mock-engine {}", env!("CARGO_PKG_VERSION")),
            pid: std::process::id(),
            sample_rate: st.rate,
            block: st.block,
            device: Some(st.device.name.into()),
            shm_layout: shm::LAYOUT,
        };
        (
            Some(done(serde_json::to_value(hello).expect("json"))),
            After::Nothing,
        )
    }

    fn device_list(&self) -> Reply {
        let devices: Vec<Value> = DEVICES
            .iter()
            .map(|d| json!({"name": d.name, "inputs": d.inputs, "outputs": d.outputs, "rates": d.rates}))
            .collect();
        done(json!({"devices": devices}))
    }

    fn device_open(&self, args: &Map<String, Value>) -> Reply {
        let device = match args.get("name") {
            None | Some(Value::Null) => &DEVICES[0],
            Some(Value::String(name)) => match device_named(name) {
                Some(d) => d,
                None => return fail("no_such_device", format!("No device named \"{name}\".")),
            },
            Some(_) => return fail("bad_args", "device.open's name is a device's name or null."),
        };
        let mut st = self.lock();
        let rate = match args.get("sample_rate") {
            None => st.rate,
            v => match whole(v) {
                Some(r) if r > 0 && r <= u32::MAX as i64 => r as u32,
                _ => return fail("bad_args", "device.open's sample_rate is a rate in Hz."),
            },
        };
        let block = match args.get("block") {
            None => st.block,
            v => match whole(v) {
                Some(b) if b > 0 && b <= u32::MAX as i64 => b as u32,
                _ => return fail("bad_args", "device.open's block is a number of frames."),
            },
        };
        if let Err(e) = valid_format(device, rate, block) {
            return fail("bad_args", e);
        }
        if let Some(s) = &st.session {
            if s.graph.sample_rate != rate {
                return fail(
                    "rate_mismatch",
                    format!(
                        "The loaded session runs at {} Hz. Open the device at that rate, or unload the session first.",
                        s.graph.sample_rate
                    ),
                );
            }
        }
        (st.device, st.rate, st.block) = (device, rate, block);
        self.shm.region().header.set_format(rate, block);
        done(json!({
            "name": device.name, "sample_rate": rate, "block": block,
            "output_latency": device.output_latency, "input_latency": device.input_latency,
        }))
    }

    fn session_load(&self, args: &Map<String, Value>) -> Reply {
        let graph: Graph = match args.get("graph") {
            None => return fail("bad_session", "session.load needs a graph."),
            Some(g) => match serde_json::from_value(g.clone()) {
                Ok(g) => g,
                Err(e) => return fail("bad_session", format!("The graph doesn't read: {e}.")),
            },
        };
        let off: HashSet<String> = match args.get("off") {
            None | Some(Value::Null) => HashSet::new(),
            Some(Value::Array(ids)) if ids.iter().all(Value::is_string) => ids
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            Some(_) => return fail("bad_args", "session.load's off is a list of device ids."),
        };
        let playhead = match args.get("playhead") {
            None => None,
            v => match whole(v) {
                Some(p) if p >= 0 => Some(p),
                _ => return fail("bad_args", "session.load's playhead is a sample from 0."),
            },
        };
        if let Err(message) = check_graph(&graph) {
            return fail("bad_session", message);
        }
        let mut st = self.lock();
        if graph.sample_rate != st.rate {
            return fail(
                "rate_mismatch",
                format!(
                    "This session runs at {} Hz and the device at {} Hz. Open the device at {} Hz first.",
                    graph.sample_rate, st.rate, graph.sample_rate
                ),
            );
        }
        let session = Session::new(graph, off)?;
        let latency: Map<String, Value> = session
            .devices()
            .map(|d| (d.id.clone(), json!(latency(d))))
            .collect();
        let slots: Map<String, Value> = session
            .slots
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), json!(i)))
            .collect();
        let nodes = session.slots.len() + session.devices().count();
        st.session = Some(session);
        st.plugins_dirty = false;
        if let Some(p) = playhead {
            st.pos = p;
            self.transport_event(&st);
        }
        done(json!({"nodes": nodes, "latency": latency, "meter_slots": slots}))
    }

    fn param_set(&self, args: &Map<String, Value>) -> Reply {
        let mut st = self.lock();
        let pos = st.pos;
        let Some(s) = st.session.as_mut() else {
            return fail("no_session", "No session is loaded.");
        };
        let (Some(id), Some(param), Some(value)) = (
            args.get("node").and_then(Value::as_str),
            args.get("param").and_then(Value::as_str),
            args.get("value"),
        ) else {
            return fail("bad_args", "param.set needs a node, a param and a value.");
        };
        let Some(node) = s.find(id) else {
            return fail("no_such_node", format!("No node {id}."));
        };
        let c = change(s, node, id, param, value)?;
        let at = match args.get("at") {
            None => None,
            v => match whole(v) {
                Some(at) if at >= 0 => Some(at),
                _ => return fail("bad_args", "param.set's at is a sample from 0."),
            },
        };
        let is_param = matches!(c, Change::Param(..));
        let moved = match at {
            Some(at) if at > pos => {
                s.pending.push((at, id.into(), c));
                None
            }
            _ => s.apply(node, c),
        };
        if is_param {
            st.plugins_dirty = true;
        }
        if let Some((node, samples)) = moved {
            self.emit("plugin.latency", json!({"node": node, "samples": samples}));
        }
        done(json!({}))
    }

    fn transport_event(&self, st: &State) {
        let state = if st.playing { "playing" } else { "stopped" };
        self.emit("transport", json!({"state": state, "sample": st.pos}));
    }

    fn stop(&self, st: &mut State) {
        st.playing = false;
        self.transport_event(st);
        // Every plugin's state on every stop (§3.7).
        self.send_plugin_states(st);
    }

    fn send_plugin_states(&self, st: &mut State) {
        if let Some(s) = &st.session {
            for d in s
                .devices()
                .filter(|d| d.format != Format::Builtin && d.on && !s.off.contains(&d.id))
            {
                if let Ok(state) = self.state_value(s, d) {
                    self.emit(
                        "plugin.state",
                        json!({"node": d.id, "state": state["state"]}),
                    );
                }
            }
        }
        st.plugins_dirty = false;
        st.plugins_sent = Instant::now();
    }

    /// Every 60 s while a plugin's parameters have changed.
    pub fn tick(&self) {
        let mut st = self.lock();
        if st.plugins_dirty && st.plugins_sent.elapsed() >= PLUGIN_STATE_EVERY {
            self.send_plugin_states(&mut st);
        }
    }

    fn transport(&self, op: &str, args: &Map<String, Value>) -> Reply {
        let mut st = self.lock();
        match op {
            "transport.play" => {
                st.playing = true;
                self.transport_event(&st);
            }
            "transport.stop" => self.stop(&mut st),
            "transport.locate" => match whole(args.get("sample")) {
                Some(n) if n >= 0 => {
                    st.pos = n;
                    self.transport_event(&st);
                }
                _ => return fail("bad_args", "transport.locate needs a sample from 0."),
            },
            _ => {
                let Some(on) = args.get("on").and_then(Value::as_bool) else {
                    return fail("bad_args", "transport.loop needs on: true or false.");
                };
                st.looping = None;
                if on {
                    match (whole(args.get("start")), whole(args.get("end"))) {
                        (Some(a), Some(b)) if a >= 0 && b > a => st.looping = Some((a, b)),
                        _ => {
                            return fail(
                                "bad_args",
                                "A loop needs a start from 0 and an end after it.",
                            )
                        }
                    }
                }
                return done(json!({}));
            }
        }
        done(json!({"sample": st.pos}))
    }

    fn render(&self, args: &Map<String, Value>) -> Reply {
        let dir = match args.get("out_dir").and_then(Value::as_str) {
            Some(d) if Path::new(d).is_absolute() => PathBuf::from(d),
            _ => return fail("bad_args", "render needs out_dir, an absolute path."),
        };
        let start = match args.get("start") {
            None => 0,
            v => match whole(v) {
                Some(s) if s >= 0 => s,
                _ => return fail("bad_args", "render's start is a sample from 0."),
            },
        };
        let len = match whole(args.get("len")) {
            Some(n) if n > 0 => n,
            _ => return fail("bad_args", "render needs len, a number of frames above 0."),
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
        let format = match args.get("format").and_then(Value::as_str) {
            None | Some("f32") => SampleFormat::F32,
            Some("s16") => SampleFormat::S16,
            Some(_) => return fail("bad_args", "render's format is f32 or s16."),
        };
        let (mix, rate) = {
            let mut st = self.lock();
            let Some(s) = &st.session else {
                return fail("no_session", "Load a session before rendering.");
            };
            let found = (s.mix.clone(), s.graph.sample_rate);
            // Live playback stops for the render and comes back stopped.
            if st.playing {
                self.stop(&mut st);
            }
            found
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return fail(
                "render_failed",
                format!("Can't make {} ({e}).", dir.display()),
            );
        }
        let rendered = self.write_render(&dir, &mix, rate, start, len, want, format);
        let (files, hashes) = rendered
            .map_err(|e| ErrorBody::new("render_failed", format!("The render failed: {e}.")))?;
        done(json!({"files": files, "frames": len, "sha256": hashes}))
    }

    /// Stems first, then the master (§3.6), each pass in chunks with a
    /// progress event after each.
    #[allow(clippy::too_many_arguments)]
    fn write_render(
        &self,
        dir: &Path,
        mix: &Mix,
        rate: u32,
        start: i64,
        len: i64,
        want: [bool; 2],
        format: SampleFormat,
    ) -> std::io::Result<(Map<String, Value>, Map<String, Value>)> {
        let mut files = BTreeMap::new();
        let mut hashes = BTreeMap::new();
        let progress = |stage: &str, done: i64| {
            self.emit(
                "render.progress",
                json!({"stage": stage, "done": done, "total": len}),
            )
        };
        if want[1] {
            let mut writers = Role::ALL
                .iter()
                .map(|r| {
                    WavWriter::create(
                        &dir.join(format!("{}.wav", r.name())),
                        rate,
                        format,
                        len as u64,
                    )
                })
                .collect::<std::io::Result<Vec<_>>>()?;
            for chunk in (0..len).step_by(RENDER_CHUNK as usize) {
                let n = RENDER_CHUNK.min(len - chunk);
                let frames: Vec<[[f32; 2]; 4]> = (0..n)
                    .map(|i| model::stem_frames(mix, start + chunk + i, rate))
                    .collect();
                for (r, w) in writers.iter_mut().enumerate() {
                    w.write(&frames.iter().map(|f| f[r]).collect::<Vec<_>>())?;
                }
                progress("stems", chunk + n);
            }
            for (role, w) in Role::ALL.iter().zip(writers) {
                files.insert(role.name(), dir.join(format!("{}.wav", role.name())));
                hashes.insert(role.name(), w.finish()?);
            }
        }
        if want[0] {
            let path = dir.join("master.wav");
            let mut w = WavWriter::create(&path, rate, format, len as u64)?;
            for chunk in (0..len).step_by(RENDER_CHUNK as usize) {
                let n = RENDER_CHUNK.min(len - chunk);
                let frames: Vec<[f32; 2]> = (0..n)
                    .map(|i| {
                        model::master_frame(mix, &model::stem_frames(mix, start + chunk + i, rate))
                    })
                    .collect();
                w.write(&frames)?;
                progress("master", chunk + n);
            }
            files.insert("master", path);
            hashes.insert("master", w.finish()?);
        }
        // The contract's order: the master, then the stems in role order.
        let order = ["master", "vocals", "drums", "other", "bass"];
        let pick = |m: &BTreeMap<&str, Value>| -> Map<String, Value> {
            order
                .iter()
                .filter_map(|k| m.get(k).map(|v| (k.to_string(), v.clone())))
                .collect()
        };
        let files: BTreeMap<&str, Value> = files.into_iter().map(|(k, p)| (k, json!(p))).collect();
        let hashes: BTreeMap<&str, Value> =
            hashes.into_iter().map(|(k, h)| (k, json!(h))).collect();
        Ok((pick(&files), pick(&hashes)))
    }

    /// `plugin.state`'s result for one device.
    fn state_value(&self, s: &Session, d: &Device) -> Reply {
        let bytes = s.state_blob(d);
        let sha = hex::encode(Sha256::digest(&bytes));
        let state = if bytes.len() > INLINE_STATE_MAX {
            let Some(dir) = &s.graph.state_dir else {
                return fail(
                    "no_state_dir",
                    "This state is over 256 KB and the graph names no state_dir for it.",
                );
            };
            let path = Path::new(dir).join(format!("{}.state", d.id));
            std::fs::create_dir_all(dir)
                .and_then(|_| std::fs::write(&path, &bytes))
                .map_err(|e| {
                    ErrorBody::new(
                        "state_failed",
                        format!("Can't write {} ({e}).", path.display()),
                    )
                })?;
            json!({"file": path})
        } else {
            json!({"inline": BASE64.encode(&bytes)})
        };
        done(json!({"state": state, "bytes": bytes.len(), "sha256": sha}))
    }

    fn plugin(&self, op: &str, args: &Map<String, Value>) -> Reply {
        let st = self.lock();
        let Some(s) = &st.session else {
            return fail("no_session", "No session is loaded.");
        };
        let id = args.get("node").and_then(Value::as_str).unwrap_or_default();
        let Some(d) = s.device(id) else {
            return fail("no_such_node", format!("No plugin {id}."));
        };
        let third_party = d.format != Format::Builtin;
        match op {
            "plugin.state" => self.state_value(s, d),
            "plugin.editor.open" if third_party => {
                done(json!({"window": format!("mock-window:{id}")}))
            }
            "plugin.editor.open" => fail(
                "no_editor",
                "Built-in devices have no plugin window; the app draws them.",
            ),
            "plugin.editor.close" => done(json!({})),
            _ if third_party => {
                let params: Vec<Value> = PLUGIN_PARAMS
                    .iter()
                    .enumerate()
                    .map(|(i, name)| {
                        let value = d
                            .params
                            .get(&format!("p{i}"))
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0);
                        json!({"index": i, "name": name, "value": value, "automatable": true})
                    })
                    .collect();
                done(json!({"params": params}))
            }
            _ => {
                let sorted: BTreeMap<&String, &Value> = d.params.iter().collect();
                let params: Vec<Value> = sorted
                    .iter()
                    .enumerate()
                    .map(|(i, (name, value))| json!({"index": i, "name": name, "value": value, "automatable": true}))
                    .collect();
                done(json!({"params": params}))
            }
        }
    }

    fn midi_route(&self, args: &Map<String, Value>) -> Reply {
        let input = args
            .get("input")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if input != MIDI_INPUT.0 {
            return fail("no_such_input", format!("No MIDI input {input}."));
        }
        let st = self.lock();
        let Some(s) = &st.session else {
            return fail("no_session", "No session is loaded.");
        };
        let track = args
            .get("track")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !matches!(s.find(track), Some(Node::Track(_))) {
            return fail("no_such_node", format!("No track {track}."));
        }
        match args.get("channel") {
            None | Some(Value::Null) => {}
            Some(c) if c.as_u64().is_some_and(|c| (1..=16).contains(&c)) => {}
            Some(_) => {
                return fail(
                    "bad_args",
                    "midi.route's channel is 1 to 16, or null for all.",
                )
            }
        }
        done(json!({}))
    }

    fn debug(&self, op: &str, args: &Map<String, Value>) -> (Option<Reply>, After) {
        if op == "debug.crumb" {
            let st = self.lock();
            let Some(s) = &st.session else {
                return (
                    Some(fail("no_session", "No session is loaded.")),
                    After::Nothing,
                );
            };
            let id = args.get("node").and_then(Value::as_str).unwrap_or_default();
            if s.find(id).is_none() {
                return (
                    Some(fail("no_such_node", format!("No node {id}."))),
                    After::Nothing,
                );
            }
            return (
                Some(done(json!({}))),
                After::Fault(Fault::Crumb(shm::crumb_hash(id))),
            );
        }
        let crash = op == "debug.crash";
        match args.get("in").and_then(Value::as_str) {
            // A crash on the message thread has no answer: the connection closing is the answer.
            Some("message") if crash => (None, After::Abort),
            Some("message") => (Some(done(json!({}))), After::Hang),
            Some("audio") => (
                Some(done(json!({}))),
                After::Fault(if crash { Fault::Crash } else { Fault::Hang }),
            ),
            _ => (
                Some(fail(
                    "bad_args",
                    format!("{op} takes in: \"audio\" or \"message\"."),
                )),
                After::Nothing,
            ),
        }
    }

    pub fn format(&self) -> (u32, u32) {
        let st = self.lock();
        (st.rate, st.block)
    }

    /// The timer fell far behind: what a device reports as a dropout.
    pub fn dropout(&self) {
        self.lock().dropouts += 1;
    }

    /// One block on the audio thread, as a device callback would run it.
    /// `host_ns` is the block's nominal time, on `shm::monotonic_ns`.
    pub fn process_block(&self, host_ns: u64) {
        let mut st = self.lock();
        if let Some(fault) = st.fault.take() {
            drop(st);
            match fault {
                Fault::Crash => std::process::abort(),
                Fault::Hang => hang(),
                Fault::Crumb(hash) => {
                    self.shm.region().crumb.set(hash);
                    std::process::abort()
                }
            }
        }
        let region = self.shm.region();
        let st = &mut *st;
        let block = st.block as i64;
        let (playing, pos) = (st.playing, st.pos);
        if let Some(s) = st.session.as_mut() {
            for &hash in &s.crumbs {
                region.crumb.set(hash);
                region.crumb.clear();
            }
            if playing && s.pending.iter().any(|p| p.0 < pos + block) {
                let (due, later): (Vec<_>, Vec<_>) =
                    s.pending.drain(..).partition(|p| p.0 < pos + block);
                s.pending = later;
                for (_, id, c) in due {
                    if let Some((node, samples)) = s.find(&id).and_then(|node| s.apply(node, c)) {
                        self.emit("plugin.latency", json!({"node": node, "samples": samples}));
                    }
                }
            }
        }
        st.callbacks += 1;
        let delay = st.session.as_ref().map_or(0, |s| s.delay);
        let latency = if playing {
            st.device.output_latency + delay
        } else {
            0
        };
        region.clock.write(&ClockFields {
            sample_pos: pos - latency,
            host_time_ns: host_ns,
            rate: if playing { st.rate as f64 } else { 0.0 },
            state: if playing {
                shm::STATE_PLAYING
            } else {
                shm::STATE_STOPPED
            },
            dropouts: st.dropouts,
            callbacks: st.callbacks,
        });
        let (slots, load) = match &st.session {
            Some(s) if playing => (model::meter_slots(&s.mix), dsp_load(s)),
            Some(s) => (vec![[0.0; 4]; s.slots.len()], dsp_load(s)),
            None => (Vec::new(), 0.01),
        };
        region.write_meters(st.callbacks, load, st.dropouts, &slots);
        if playing {
            st.pos += block;
            if let Some((a, b)) = st.looping {
                if st.pos >= b {
                    st.pos = a + (st.pos - b);
                }
            }
        }
    }
}

/// A made-up but steady DSP load that grows with the session.
fn dsp_load(s: &Session) -> f32 {
    (0.02 + 0.01 * s.graph.tracks.len() as f32 + 0.005 * s.devices().count() as f32).min(0.95)
}

fn check_graph(g: &Graph) -> Result<(), String> {
    let max = METER_SLOTS - Role::ALL.len() - 1;
    if g.tracks.len() > max {
        return Err(format!("A session holds {max} tracks at most."));
    }
    let mut ids = HashSet::new();
    let devices = g
        .tracks
        .iter()
        .flat_map(|t| &t.devices)
        .chain(&g.master.devices);
    for id in g.tracks.iter().map(|t| &t.id).chain(devices.map(|d| &d.id)) {
        if id.is_empty() || id == "master" || id.starts_with("bus:") {
            return Err(format!(
                "A node can't be called \"{id}\"; the buses and the master are."
            ));
        }
        if !ids.insert(id) {
            return Err(format!("Two nodes have the id {id}."));
        }
    }
    for t in &g.tracks {
        if !(t.gain_db.is_finite() && t.gain_db <= 24.0) {
            return Err(format!(
                "Track {}'s gain_db is a number of dB up to +24.",
                t.id
            ));
        }
        if !(-1.0..=1.0).contains(&t.pan) {
            return Err(format!("Track {}'s pan is a number from -1 to 1.", t.id));
        }
        if !(0.0..=1.0).contains(&t.sends.reverb) || !(0.0..=1.0).contains(&t.sends.delay) {
            return Err(format!("Track {}'s sends are numbers from 0 to 1.", t.id));
        }
    }
    Ok(())
}

fn hang() -> ! {
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(params: Value) -> Device {
        serde_json::from_value(json!({"id": "d", "format": "vst3", "uid": "UID", "params": params}))
            .unwrap()
    }

    #[test]
    fn the_mock_plugins_state_is_its_params_and_reads_back() {
        let d = device(json!({"p1": 0.75, "p0": 0.25}));
        let bytes = mock_state(&d);
        assert_eq!(
            bytes,
            br#"{"mock_plugin":1,"uid":"UID","params":{"p0":0.25,"p1":0.75}}"#
        );
        let params = mock_params(&bytes, "UID").unwrap();
        assert_eq!(params["p1"], 0.75);
        assert!(
            mock_params(&bytes, "OTHER").is_none(),
            "another plugin's state"
        );
        assert!(mock_params(b"\x00\x01opaque", "UID").is_none());
    }

    #[test]
    fn a_big_state_pads_and_still_reads() {
        let d = device(json!({"p0": 0.5, "mock.state_bytes": 1000}));
        let bytes = mock_state(&d);
        assert_eq!(bytes.len(), 1000);
        assert_eq!(mock_params(&bytes, "UID").unwrap()["p0"], 0.5);
    }

    #[test]
    fn whole_numbers_may_come_as_floats() {
        assert_eq!(whole(Some(&json!(48000))), Some(48000));
        assert_eq!(whole(Some(&json!(48000.0))), Some(48000));
        assert_eq!(whole(Some(&json!(0.5))), None);
        assert_eq!(whole(Some(&json!("1"))), None);
        assert_eq!(whole(None), None);
    }
}
