//! wwav-engine-cli: the engine's socket from a terminal. "Anything provable
//! in a terminal is true in the app" (`docs/SPEC.md` 9.2): every op in
//! `docs/ENGINE.md` can be sent from here, through the same `wwav-wire`
//! client and the same start-up the app uses.
//!
//! ```text
//! wwav-engine-cli --spawn <engine> [--test] [--device <name|null>] [--rate N] [--block N]
//!                 [--tmp <dir>] [--timeout <time>] [script.txt | -]
//! wwav-engine-cli --connect <socket> [--shm <name>] [--timeout <time>] [script.txt | -]
//! ```
//!
//! A script line is `<op> <json args>`, or one of these:
//!
//! - `sleep 200ms` (or `2s`): wait, printing events as they come.
//! - `clock`: the shared-memory clock, with `now_ns`, when it was read.
//! - `meters`: the newest meter entry, its slots named from `session.load`.
//! - `kill9`: SIGKILL the engine, then wait (2 s at most) to see what the app
//!   sees: the socket closing and the child exiting.
//! - `respawn`: a new engine on the same region and directory, as the app
//!   starts after a crash (`--spawn` only). It isn't greeted: send `hello`.
//!
//! Blank lines and lines starting with `#` are skipped. With `-`, or no
//! script, lines come from stdin as they are typed.
//!
//! Every response and event is printed as the JSON object it was on the
//! wire, one per line, in the order they arrived. What the CLI saw itself is
//! a one-key object: `spawned`, `connected`, `clock`, `meters`, `kill9`,
//! `respawned`, `closed`, `exited` or `failed`. `after_ms` in `closed`,
//! `exited` and `respawned` counts from the CLI's last act: a request sent,
//! a `kill9`, a `respawn`, or at the end, closing the engine's stdin.
//!
//! At the end of the script the CLI closes the engine's stdin, as the app
//! quitting would, and reports the exit. The exit status is 0 when every
//! line read, 1 when one didn't or the engine couldn't be started, and 2
//! for a bad command line.

use serde_json::{json, Map, Value};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};
use wwav_wire::client::{CallError, Client};
use wwav_wire::msg::Event;
use wwav_wire::process::{EngineConfig, EngineProcess};
use wwav_wire::shm::{self, Shm};

pub const USAGE: &str = "\
usage: wwav-engine-cli --spawn <engine> [--test] [--device <name|null>] [--rate N] [--block N]
                       [--tmp <dir>] [--timeout <time>] [script.txt | -]
       wwav-engine-cli --connect <socket> [--shm <name>] [--timeout <time>] [script.txt | -]";

/// How often the CLI looks for events, a closed socket and an exit while it
/// waits, which is also how fine `after_ms` is.
const POLL: Duration = Duration::from_millis(5);
/// How long `kill9` and the end of a script wait to see the engine go.
const GONE_WITHIN: Duration = Duration::from_secs(2);

enum Target {
    Spawn(EngineConfig),
    Connect {
        socket: PathBuf,
        shm: Option<String>,
    },
}

struct Options {
    target: Target,
    /// How long a request waits for its answer.
    timeout: Duration,
    /// `None` reads the script from stdin.
    script: Option<PathBuf>,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut it = args.iter();
    let (mut spawn, mut connect, mut shm, mut script) = (None, None, None, None);
    let (mut test, mut device, mut rate, mut block, mut tmp) = (false, None, None, None, None);
    let mut timeout = Duration::from_secs(10);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} needs a value."));
        match arg.as_str() {
            "--spawn" => spawn = Some(PathBuf::from(value()?)),
            "--connect" => connect = Some(PathBuf::from(value()?)),
            "--shm" => shm = Some(value()?.clone()),
            "--test" => test = true,
            "--device" => device = Some(value()?.clone()),
            "--rate" => rate = Some(value()?.parse().map_err(|_| "--rate is a number of Hz.")?),
            "--block" => {
                block = Some(
                    value()?
                        .parse()
                        .map_err(|_| "--block is a number of frames.")?,
                )
            }
            "--tmp" => tmp = Some(PathBuf::from(value()?)),
            "--timeout" => timeout = parse_time(value()?)?,
            "-" if script.is_none() => script = Some(None),
            s if !s.starts_with('-') && script.is_none() => script = Some(Some(PathBuf::from(s))),
            _ => return Err(format!("{arg} isn't a flag this takes.")),
        }
    }
    let spawn_only = test || device.is_some() || rate.is_some() || block.is_some() || tmp.is_some();
    let target = match (spawn, connect) {
        (Some(binary), None) if shm.is_none() => {
            let mut c = EngineConfig::new(binary);
            if let Some(tmp) = tmp {
                c.tmp_dir = tmp;
            }
            (c.test, c.device, c.rate, c.block) = (test, device, rate, block);
            Target::Spawn(c)
        }
        (Some(_), None) => return Err("--shm goes with --connect; --spawn makes its own.".into()),
        (None, Some(socket)) if !spawn_only => Target::Connect { socket, shm },
        (None, Some(_)) => return Err(
            "--test, --device, --rate, --block and --tmp start an engine: use them with --spawn."
                .into(),
        ),
        _ => return Err("Give one of --spawn or --connect.".into()),
    };
    Ok(Options {
        target,
        timeout,
        script: script.flatten(),
    })
}

/// `200ms`, `2s` or `1.5s`.
fn parse_time(s: &str) -> Result<Duration, String> {
    let bad = || format!("{s:?} isn't a time: say 200ms or 2s.");
    let (number, unit) = match s.strip_suffix("ms") {
        Some(n) => (n, 1e-3),
        None => (s.strip_suffix('s').ok_or_else(bad)?, 1.0),
    };
    let n: f64 = number.parse().map_err(|_| bad())?;
    Duration::try_from_secs_f64(n * unit).map_err(|_| bad())
}

/// One line of a script.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Op(String, Value),
    Sleep(Duration),
    Clock,
    Meters,
    Kill9,
    Respawn,
}

/// `None` for a blank line or a comment.
fn parse_line(line: &str) -> Result<Option<Step>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    let bare = |step: Step| match rest {
        "" => Ok(Some(step)),
        _ => Err(format!("{word} takes nothing after it.")),
    };
    match word {
        "sleep" => Ok(Some(Step::Sleep(parse_time(rest)?))),
        "clock" => bare(Step::Clock),
        "meters" => bare(Step::Meters),
        "kill9" => bare(Step::Kill9),
        "respawn" => bare(Step::Respawn),
        op => {
            let args = match rest {
                "" => Value::Null,
                json => match serde_json::from_str(json) {
                    Ok(Value::Object(m)) => Value::Object(m),
                    Ok(_) => return Err(format!("{op}'s args are a JSON object.")),
                    Err(e) => return Err(format!("{op}'s args aren't JSON: {e}.")),
                },
            };
            Ok(Some(Step::Op(op.into(), args)))
        }
    }
}

/// Runs the CLI with `args` (without the program name), printing to `out`.
/// Returns the exit status.
pub fn run(args: &[String], out: &mut dyn Write) -> i32 {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        let _ = writeln!(out, "{USAGE}");
        return 0;
    }
    let opts = match parse_args(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("wwav-engine-cli: {e}\n{USAGE}");
            return 2;
        }
    };
    let script: Box<dyn Read + Send> = match &opts.script {
        None => Box::new(std::io::stdin()),
        Some(path) => match File::open(path) {
            Ok(f) => Box::new(f),
            Err(e) => {
                eprintln!("wwav-engine-cli: can't read {} ({e})", path.display());
                return 1;
            }
        },
    };
    let mut session = match Session::start(opts, out) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("wwav-engine-cli: {e}");
            return 1;
        }
    };
    // Lines come in on a thread, so events still print while a person at
    // the terminal is typing the next one.
    let (tx, lines) = mpsc::channel();
    thread::spawn(move || {
        for (n, line) in BufReader::new(script).lines().enumerate() {
            let Ok(line) = line else { break };
            if tx.send((n + 1, line)).is_err() {
                break;
            }
        }
    });
    loop {
        match lines.recv_timeout(POLL) {
            Ok((n, line)) => session.line(n, &line),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        session.events();
        session.notice();
    }
    session.finish()
}

enum Engine {
    Spawned(EngineProcess),
    Connected { client: Client, shm: Option<Shm> },
}

struct Session<'a> {
    out: &'a mut dyn Write,
    engine: Engine,
    events: Receiver<Event>,
    timeout: Duration,
    /// The engine's pid: the child's, or `hello`'s answer over `--connect`.
    pid: Option<u32>,
    /// Meter slot names by slot, from the last `session.load`.
    slots: Vec<String>,
    /// The CLI's last act, which `after_ms` counts from.
    since: Instant,
    told_closed: bool,
    told_exited: bool,
    bad_lines: usize,
}

impl<'a> Session<'a> {
    fn start(opts: Options, out: &'a mut dyn Write) -> Result<Session<'a>, String> {
        let (engine, events, pid, hello) = match opts.target {
            Target::Spawn(config) => {
                let binary = config.binary.display().to_string();
                let (p, events) = EngineProcess::spawn(config)
                    .map_err(|e| format!("can't start {binary}: {e}"))?;
                let hello = json!({"spawned": {"pid": p.pid(), "socket": p.socket(), "shm": p.shm().name()}});
                let pid = p.pid();
                (Engine::Spawned(p), events, Some(pid), hello)
            }
            Target::Connect { socket, shm } => {
                let (client, events) = Client::connect(&socket)
                    .map_err(|e| format!("can't connect to {} ({e})", socket.display()))?;
                let shm = match shm {
                    Some(name) => {
                        Some(Shm::open(&name).map_err(|e| format!("can't map {name} ({e})"))?)
                    }
                    None => None,
                };
                let hello = json!({"connected": {"socket": socket}});
                (Engine::Connected { client, shm }, events, None, hello)
            }
        };
        let mut s = Session {
            out,
            engine,
            events,
            timeout: opts.timeout,
            pid,
            slots: Vec::new(),
            since: Instant::now(),
            told_closed: false,
            told_exited: false,
            bad_lines: 0,
        };
        s.say(hello);
        Ok(s)
    }

    fn client(&self) -> &Client {
        match &self.engine {
            Engine::Spawned(p) => p.client(),
            Engine::Connected { client, .. } => client,
        }
    }

    fn shm(&self) -> Option<&Shm> {
        match &self.engine {
            Engine::Spawned(p) => Some(p.shm()),
            Engine::Connected { shm, .. } => shm.as_ref(),
        }
    }

    fn say(&mut self, v: Value) {
        // A closed stdout is no reason to leave an engine running: carry on.
        let _ = writeln!(self.out, "{v}");
        let _ = self.out.flush();
    }

    fn failed(&mut self, line: usize, what: &str, error: impl Into<String>) {
        let error = error.into();
        self.say(json!({"failed": {"line": line, "op": what, "error": error}}));
    }

    fn line(&mut self, n: usize, text: &str) {
        match parse_line(text) {
            Ok(None) => {}
            Ok(Some(Step::Op(op, args))) => self.call(n, &op, args),
            Ok(Some(Step::Sleep(d))) => self.wait_until(Instant::now() + d, |_| false),
            Ok(Some(Step::Clock)) => self.clock(n),
            Ok(Some(Step::Meters)) => self.meters(n),
            Ok(Some(Step::Kill9)) => self.kill9(n),
            Ok(Some(Step::Respawn)) => self.respawn(n),
            Err(e) => {
                self.bad_lines += 1;
                self.say(json!({"failed": {"line": n, "error": e}}));
            }
        }
    }

    fn call(&mut self, n: usize, op: &str, args: Value) {
        self.since = Instant::now();
        let answer = self.client().request(op, args, self.timeout);
        // Events that came before the answer were queued before it: print
        // them first, so the output keeps the wire's order.
        self.events();
        match answer {
            Ok(response) => {
                if let Ok(result) = &response.outcome {
                    self.learn(op, result);
                }
                self.say(serde_json::to_value(&response).expect("a response is JSON"));
            }
            Err(CallError::Timeout(_)) => self.failed(n, op, "timeout"),
            Err(CallError::Closed) => self.failed(n, op, "closed"),
            Err(e) => self.failed(n, op, e.to_string()),
        }
    }

    /// What later lines need from an answer: the engine's pid, and the
    /// names of the meter slots.
    fn learn(&mut self, op: &str, result: &Map<String, Value>) {
        match op {
            "hello" => {
                if let Some(pid) = result.get("pid").and_then(Value::as_u64) {
                    self.pid = u32::try_from(pid).ok();
                }
            }
            "session.load" => {
                let Some(slots) = result.get("meter_slots").and_then(Value::as_object) else {
                    return;
                };
                let mut names = vec![String::new(); slots.len()];
                for (node, slot) in slots {
                    if let Some(name) = slot.as_u64().and_then(|s| names.get_mut(s as usize)) {
                        *name = node.clone();
                    }
                }
                self.slots = names;
            }
            _ => {}
        }
    }

    fn clock(&mut self, n: usize) {
        let Some(shm) = self.shm() else {
            return self.failed(n, "clock", "No shared memory: give --shm with --connect.");
        };
        let Some(c) = shm.region().clock.read() else {
            return self.failed(
                n,
                "clock",
                "The clock's seq stayed odd: its writer died mid-write.",
            );
        };
        let now_ns = shm::monotonic_ns();
        self.say(json!({"clock": {
            "sample_pos": c.sample_pos, "host_time_ns": c.host_time_ns, "rate": c.rate,
            "state": c.state, "dropouts": c.dropouts, "callbacks": c.callbacks, "now_ns": now_ns,
        }}));
    }

    fn meters(&mut self, n: usize) {
        let Some(shm) = self.shm() else {
            return self.failed(n, "meters", "No shared memory: give --shm with --connect.");
        };
        let Some(m) = shm.region().newest_meters() else {
            return self.say(json!({"meters": null}));
        };
        // Slots this CLI has no name for (it didn't send session.load) go
        // by their number.
        let slots: Map<String, Value> = m
            .slots
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let name = self.slots.get(i).filter(|n| !n.is_empty()).cloned();
                (name.unwrap_or_else(|| i.to_string()), json!(s))
            })
            .collect();
        self.say(json!({"meters": {
            "callback": m.callback, "dsp_load": m.dsp_load, "dropouts": m.dropouts, "slots": slots,
        }}));
    }

    fn kill9(&mut self, n: usize) {
        let Some(pid) = self.pid else {
            return self.failed(
                n,
                "kill9",
                "The engine's pid isn't known yet: send hello first.",
            );
        };
        self.since = Instant::now();
        let killed = match &mut self.engine {
            Engine::Spawned(p) => p.kill(),
            // SAFETY: kill has no memory preconditions.
            Engine::Connected { .. } => {
                match unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) } {
                    0 => Ok(()),
                    _ => Err(std::io::Error::last_os_error()),
                }
            }
        };
        if let Err(e) = killed {
            return self.failed(n, "kill9", e.to_string());
        }
        self.say(json!({"kill9": {"pid": pid}}));
        let child = matches!(self.engine, Engine::Spawned(_));
        self.wait_until(Instant::now() + GONE_WITHIN, |s| {
            s.told_closed && (s.told_exited || !child)
        });
    }

    fn respawn(&mut self, n: usize) {
        self.events();
        let Engine::Spawned(p) = &mut self.engine else {
            return self.failed(
                n,
                "respawn",
                "respawn needs an engine this CLI started (--spawn).",
            );
        };
        match p.respawn() {
            Ok(events) => {
                let pid = p.pid();
                self.events = events;
                self.pid = Some(pid);
                self.slots.clear();
                (self.told_closed, self.told_exited) = (false, false);
                let after_ms = self.since.elapsed().as_millis() as u64;
                self.since = Instant::now();
                self.say(json!({"respawned": {"pid": pid, "after_ms": after_ms}}));
            }
            Err(e) => self.failed(n, "respawn", e.to_string()),
        }
    }

    /// Prints events, the socket closing and the engine exiting until
    /// `done` says so or `deadline` passes.
    fn wait_until(&mut self, deadline: Instant, done: impl Fn(&Session) -> bool) {
        while !done(self) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match self.events.recv_timeout(left.min(POLL)) {
                Ok(e) => self.say(json!(e)),
                // The connection has gone; keep watching for the exit.
                Err(RecvTimeoutError::Disconnected) => thread::sleep(left.min(POLL)),
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.events();
            self.notice();
        }
    }

    /// Prints every event that has arrived.
    fn events(&mut self) {
        while let Ok(e) = self.events.try_recv() {
            self.say(json!(e));
        }
    }

    /// Says once when the socket closes and once when the child exits.
    fn notice(&mut self) {
        let after_ms = self.since.elapsed().as_millis() as u64;
        if !self.told_closed && self.client().is_closed() {
            self.told_closed = true;
            let reason = self.client().closed_reason();
            self.say(json!({"closed": {"after_ms": after_ms, "reason": reason}}));
        }
        let exited = match &mut self.engine {
            Engine::Spawned(p) if !self.told_exited => p.wait(Duration::ZERO).ok().flatten(),
            _ => None,
        };
        if let Some(status) = exited {
            use std::os::unix::process::ExitStatusExt;
            self.told_exited = true;
            self.say(json!({"exited": {"code": status.code(), "signal": status.signal(), "after_ms": after_ms}}));
        }
    }

    /// The end of the script: the engine's stdin closes, as when the app
    /// quits, and the engine has 1 s by the contract (2 s here) to go.
    fn finish(mut self) -> i32 {
        self.events();
        self.notice();
        if let Engine::Spawned(p) = &mut self.engine {
            if !self.told_exited {
                p.close_stdin();
                self.since = Instant::now();
                self.wait_until(self.since + GONE_WITHIN, |s| s.told_exited);
                if !self.told_exited {
                    self.failed(
                        0,
                        "exit",
                        "The engine was still running 2 s after its stdin closed.",
                    );
                }
            }
        }
        if self.bad_lines > 0 {
            1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Result<Options, String> {
        parse_args(&s.split_whitespace().map(String::from).collect::<Vec<_>>())
    }

    #[test]
    fn spawn_flags_reach_the_engines_config() {
        let o = args(
            "--spawn /bin/engine --test --device null --rate 44100 --block 256 --tmp /t script.txt",
        )
        .unwrap();
        let Target::Spawn(c) = o.target else { panic!() };
        assert_eq!(c.binary, PathBuf::from("/bin/engine"));
        assert_eq!(
            (c.test, c.device.as_deref(), c.rate, c.block),
            (true, Some("null"), Some(44100), Some(256))
        );
        assert_eq!(c.tmp_dir, PathBuf::from("/t"));
        assert_eq!(o.script, Some(PathBuf::from("script.txt")));
        assert_eq!(o.timeout, Duration::from_secs(10));
    }

    #[test]
    fn connect_takes_a_socket_and_maybe_a_region() {
        let o = args("--connect /tmp/wwav-1/engine.sock --shm /wwav-1-0 --timeout 30s -").unwrap();
        let Target::Connect { socket, shm } = o.target else {
            panic!()
        };
        assert_eq!(socket, PathBuf::from("/tmp/wwav-1/engine.sock"));
        assert_eq!(shm.as_deref(), Some("/wwav-1-0"));
        assert_eq!(o.timeout, Duration::from_secs(30));
        assert_eq!(o.script, None, "- is stdin");
        assert_eq!(
            args("--connect /s").unwrap().script,
            None,
            "and so is no script"
        );
    }

    #[test]
    fn bad_command_lines_are_named() {
        for bad in [
            "",
            "--spawn /e --connect /s",
            "--spawn /e --shm /wwav-1-0",
            "--connect /s --test",
            "--connect /s --rate 48000",
            "--spawn /e --rate fast",
            "--spawn /e --timeout 5",
            "--spawn /e a.txt b.txt",
            "--spawn /e --colour blue",
            "--spawn",
        ] {
            assert!(args(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn times_are_ms_or_s() {
        assert_eq!(parse_time("200ms"), Ok(Duration::from_millis(200)));
        assert_eq!(parse_time("2s"), Ok(Duration::from_secs(2)));
        assert_eq!(parse_time("1.5s"), Ok(Duration::from_millis(1500)));
        for bad in ["200", "ms", "-1s", "soon", "2 s"] {
            assert!(parse_time(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn script_lines_are_ops_with_json_args_or_directives() {
        assert_eq!(parse_line("  # a comment"), Ok(None));
        assert_eq!(parse_line(""), Ok(None));
        assert_eq!(
            parse_line("ping"),
            Ok(Some(Step::Op("ping".into(), Value::Null)))
        );
        assert_eq!(
            parse_line(r#"param.set {"node": "vox", "param": "mute", "value": true}"#),
            Ok(Some(Step::Op(
                "param.set".into(),
                json!({"node": "vox", "param": "mute", "value": true})
            )))
        );
        assert_eq!(
            parse_line("sleep 200ms"),
            Ok(Some(Step::Sleep(Duration::from_millis(200))))
        );
        assert_eq!(parse_line("clock"), Ok(Some(Step::Clock)));
        assert_eq!(parse_line("meters"), Ok(Some(Step::Meters)));
        assert_eq!(parse_line("kill9"), Ok(Some(Step::Kill9)));
        assert_eq!(parse_line("respawn"), Ok(Some(Step::Respawn)));
        for bad in [
            "ping [1]",
            "ping {",
            "ping 3",
            "sleep",
            "sleep later",
            "clock now",
        ] {
            assert!(parse_line(bad).is_err(), "{bad:?}");
        }
    }
}
