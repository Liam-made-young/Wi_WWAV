//! What the core's tests share: a core on a fresh library with mock-engine
//! behind it, the mock server, and waiting for events.
#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use wi_core::{Config, Core, Event, MemorySecrets, Opener, SecretStore};

pub fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// target/<profile>/, where this test binary's own deps/ folder sits.
fn target_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent().unwrap().parent().unwrap().to_path_buf()
}

static BUILD: Once = Once::new();

/// The workspace's mock-engine, built once per test run.
pub fn mock_engine() -> PathBuf {
    BUILD.call_once(|| {
        let ok = Command::new(env!("CARGO"))
            .args(["build", "-q", "-p", "mock-engine", "--bin", "mock-engine"])
            .current_dir(workspace())
            .status()
            .expect("cargo runs")
            .success();
        assert!(ok, "mock-engine didn't build");
    });
    let path = target_dir().join("mock-engine");
    assert!(path.exists(), "no mock-engine at {}", path.display());
    path
}

pub fn corpus(name: &str) -> PathBuf {
    workspace().join("tests/corpus").join(name)
}

pub struct Setup {
    pub dir: tempfile::TempDir,
    pub secrets: Arc<MemorySecrets>,
}

impl Setup {
    pub fn new() -> Setup {
        Setup {
            dir: tempfile::tempdir().unwrap(),
            secrets: Arc::new(MemorySecrets::default()),
        }
    }

    pub fn library(&self) -> PathBuf {
        self.dir.path().join("Wi_WWAV")
    }

    pub fn config(&self, server: &str, opener: Opener) -> Config {
        self.config_with(&mock_engine(), server, opener)
    }

    pub fn config_with(&self, engine: &Path, server: &str, opener: Opener) -> Config {
        let tmp = self.dir.path().join("t");
        std::fs::create_dir_all(&tmp).unwrap();
        let secrets: Arc<dyn SecretStore> = self.secrets.clone();
        let mut c = Config::new(engine, server, secrets, opener);
        c.device = Some("null".into());
        c.engine_test = true;
        c.tmp_dir = tmp;
        c
    }

    /// A core on this setup's library, with no server behind it.
    pub fn core(&self) -> Core {
        Core::open(&self.library(), self.config(NO_SERVER, no_browser())).unwrap()
    }

    pub fn core_on(&self, server: &str) -> Core {
        Core::open(&self.library(), self.config(server, no_browser())).unwrap()
    }
}

/// An address nothing listens on: the core is offline.
pub const NO_SERVER: &str = "http://127.0.0.1:9";

pub fn no_browser() -> Opener {
    Arc::new(|_url: &str| Err("no browser in tests".to_string()))
}

/// Waits for the next event called `name`, skipping others.
pub fn wait_event(rx: &Receiver<Event>, name: &str, timeout: Duration) -> Event {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(e) if e.event == name => return e,
            Ok(_) => {}
            Err(_) => panic!("no '{name}' event within {timeout:?}"),
        }
    }
}

/// Waits until `f` holds, polling.
pub fn eventually(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    f()
}

pub fn ok(core: &Core, cmd: &str, args: Value) -> Value {
    core.invoke(cmd, args)
        .unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

/// Imports `files` into the library and returns their clip ids.
pub fn import(core: &Core, files: &[&Path]) -> Vec<String> {
    let paths: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
    let r = ok(
        core,
        "library.import",
        json!({"paths": paths, "label": "import"}),
    );
    r["clips"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect()
}

/// tools/mock-server on a free port, for as long as this lives.
pub struct MockServer {
    child: Child,
    pub url: String,
    /// The side door of `mock_counts.mjs`, when started with counts.
    pub counts: Option<String>,
}

fn address(line: &str) -> String {
    let url = line.trim().rsplit(' ').next().unwrap_or("").to_string();
    assert!(url.starts_with("http://"), "unexpected line: {line}");
    url
}

impl MockServer {
    pub fn start() -> MockServer {
        MockServer::spawn(
            &workspace().join("tools/mock-server/server.js"),
            &["--port", "0"],
            false,
        )
    }

    /// The mock with a side door that counts each part's sends.
    pub fn start_counting() -> MockServer {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/core/mock_counts.mjs");
        MockServer::spawn(&script, &[], true)
    }

    fn spawn(script: &Path, args: &[&str], counts: bool) -> MockServer {
        let mut child = Command::new("node")
            .arg(script)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("node runs the mock server");
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let url = address(&lines.next().unwrap().unwrap());
        let counts = counts.then(|| address(&lines.next().unwrap().unwrap()));
        MockServer { child, url, counts }
    }

    pub fn counted(&self) -> Value {
        let url = self.counts.as_ref().expect("started with counts");
        serde_json::from_str(&ureq::get(url).call().unwrap().into_string().unwrap()).unwrap()
    }

    pub fn call(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let req = ureq::request(method, &format!("{}{path}", self.url));
        let r = match body {
            Some(b) => req.send_json(b),
            None => req.call(),
        };
        let resp = match r {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => panic!("the mock server didn't answer: {e}"),
        };
        let status = resp.status();
        let text = resp.into_string().unwrap_or_default();
        (
            status,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
    }

    pub fn state(&self) -> Value {
        self.call("GET", "/__mock/state", None).1
    }

    /// The browser's half of desktop sign-in: the authorize page, the form,
    /// and the redirect back to the app's loopback address.
    pub fn browser(&self, email: &'static str, password: &'static str) -> Opener {
        let base = self.url.clone();
        Arc::new(move |url: &str| {
            let url = url.to_string();
            let base = base.clone();
            std::thread::spawn(move || {
                let agent = ureq::AgentBuilder::new().redirects(0).build();
                let page = agent
                    .get(&url)
                    .call()
                    .map_err(|e| e.to_string())?
                    .into_string()
                    .unwrap();
                let request = page
                    .split("name=\"request\" value=\"")
                    .nth(1)
                    .and_then(|r| r.split('"').next())
                    .ok_or("no request field")?
                    .to_string();
                let answer = match agent
                    .post(&format!("{base}/oauth/desktop/login"))
                    .send_form(&[
                        ("request", &request),
                        ("email", email),
                        ("password", password),
                    ]) {
                    Ok(r) => r,
                    Err(ureq::Error::Status(_, r)) => r,
                    Err(e) => return Err(e.to_string()),
                };
                let location = answer.header("location").ok_or("no redirect")?.to_string();
                agent.get(&location).call().map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            });
            Ok(())
        })
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Signs `core` in through the mock's browser pages as LMY.
pub fn sign_in(setup: &Setup, server: &MockServer) -> Core {
    let core = Core::open(
        &setup.library(),
        setup.config(
            &server.url,
            server.browser("lmy@mi-wwav.com", "WeWave-lmy1"),
        ),
    )
    .unwrap();
    ok(&core, "account.signIn", json!({}));
    core
}

// ----- Heat -----

/// "2026-10-07 09:00" in New York as epoch milliseconds: the zone 3.11 says
/// URI's students get from the system.
pub fn ny(text: &str) -> f64 {
    let (date, time) = text.split_once(' ').unwrap_or((text, "00:00"));
    let d: Vec<i16> = date.split('-').map(|p| p.parse().unwrap()).collect();
    let t: Vec<i8> = time.split(':').map(|p| p.parse().unwrap()).collect();
    jiff::civil::date(d[0], d[1] as i8, d[2] as i8)
        .at(t[0], t[1], 0, 0)
        .in_tz("America/New_York")
        .unwrap()
        .timestamp()
        .as_millisecond() as f64
}

/// A core with no server and no engine worth waiting for, on New York's
/// clock, standing at `now` ("2026-10-07 09:00").
pub fn heat_core(setup: &Setup, now: &str) -> Core {
    let core = setup.core();
    pin(&core, now);
    core
}

/// Puts a core's clock at `now` in New York.
pub fn pin(core: &Core, now: &str) {
    ok(
        core,
        "app.settings.set",
        json!({"patch": {"heat": {"timeZone": "America/New_York"}}}),
    );
    core.set_now(Some(ny(now)));
}

/// The snapshot for `date`.
pub fn snap(core: &Core, date: &str) -> Value {
    ok(core, "heat.snapshot", json!({"date": date}))
}

/// Records of one kind in a snapshot.
pub fn records(snapshot: &Value, kind: &str) -> Vec<Value> {
    snapshot["records"][kind]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// A command refused: its code and sentence.
pub fn refused(core: &Core, cmd: &str, args: Value) -> (String, String) {
    let e = core.invoke(cmd, args).expect_err(cmd);
    (e.code, e.message)
}

/// A task the way a view makes one.
pub fn add_task(core: &Core, space: &str, title: &str, extra: Value) -> Value {
    let mut record = json!({"spaceId": space, "title": title});
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        record[k] = v;
    }
    ok(core, "heat.put", json!({"kind": "task", "record": record}))["record"].clone()
}

/// Events of one name received so far, without waiting.
pub fn drain(rx: &Receiver<Event>, name: &str) -> Vec<Event> {
    rx.try_iter().filter(|e| e.event == name).collect()
}

/// A core standing at `now` (New York's clock) from the moment it opens, so
/// the workers that start with it see the same time the test does.
pub fn heat_core_on(setup: &Setup, now: &str, server: &str) -> Core {
    let mut config = setup.config(server, no_browser());
    config.now = Some(ny(now));
    let core = Core::open(&setup.library(), config).unwrap();
    ok(
        &core,
        "app.settings.set",
        json!({"patch": {"heat": {"timeZone": "America/New_York"}}}),
    );
    core
}

/// A calendar feed on a free port: answers every request with what it is
/// told to, and counts them. For the core's iCal fetch.
pub struct FeedServer {
    pub port: u16,
    state: Arc<std::sync::Mutex<(u16, Vec<u8>)>>,
    hits: Arc<std::sync::atomic::AtomicUsize>,
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl FeedServer {
    pub fn start(body: &[u8]) -> FeedServer {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::Ordering;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(std::sync::Mutex::new((200u16, body.to_vec())));
        let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (s, h, st) = (state.clone(), hits.clone(), stop.clone());
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                if st.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut conn) = conn else { continue };
                h.fetch_add(1, Ordering::SeqCst);
                let (status, body) = s.lock().unwrap().clone();
                std::thread::spawn(move || {
                    let mut seen = Vec::new();
                    let mut buf = [0u8; 1024];
                    while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                        match conn.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => seen.extend_from_slice(&buf[..n]),
                        }
                    }
                    let head = format!(
                        "HTTP/1.1 {status} X\r\nContent-Type: text/calendar\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = conn.write_all(head.as_bytes());
                    let _ = conn.write_all(&body);
                });
            }
        });
        FeedServer {
            port,
            state,
            hits,
            stop,
        }
    }

    /// What the next requests are answered with.
    pub fn serve(&self, status: u16, body: &[u8]) {
        *self.state.lock().unwrap() = (status, body.to_vec());
    }

    pub fn hits(&self) -> usize {
        self.hits.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The private address of the feed, with a token in it as D2L's carry.
    pub fn address(&self, token: &str) -> String {
        format!(
            "http://127.0.0.1:{}/d2l/le/calendar/feed/user/feed.ics?token={token}",
            self.port
        )
    }
}

impl Drop for FeedServer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
    }
}

// ----- the MCP helper, as a second process -----

static BUILD_MCP: Once = Once::new();

/// The workspace's `wi-mcp`, built once per test run.
pub fn wi_mcp() -> PathBuf {
    BUILD_MCP.call_once(|| {
        let ok = Command::new(env!("CARGO"))
            .args(["build", "-q", "-p", "wi-mcp", "--bin", "wi-mcp"])
            .current_dir(workspace())
            .status()
            .expect("cargo runs")
            .success();
        assert!(ok, "wi-mcp didn't build");
    });
    let path = target_dir().join("wi-mcp");
    assert!(path.exists(), "no wi-mcp at {}", path.display());
    path
}

/// `wi-mcp` running on a library, spoken to over stdio as Claude would.
pub struct McpHelper {
    child: Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next: i64,
}

impl McpHelper {
    pub fn start(library: &Path) -> McpHelper {
        use std::io::Write;
        let mut child = Command::new(wi_mcp())
            .arg("--library")
            .arg(library)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("wi-mcp starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut h = McpHelper {
            child,
            stdin,
            stdout,
            next: 1,
        };
        h.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}));
        writeln!(
            h.stdin,
            "{}",
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .unwrap();
        h
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        use std::io::Write;
        let id = self.next;
        self.next += 1;
        writeln!(
            self.stdin,
            "{}",
            json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
        )
        .unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
        assert_eq!(reply["id"], id);
        reply
    }

    /// A tool's answer: Ok(structuredContent), or Err(the sentence).
    pub fn call(&mut self, tool: &str, args: Value) -> Result<Value, String> {
        let reply = self.request("tools/call", json!({"name": tool, "arguments": args}));
        let result = &reply["result"];
        if result["isError"] == true {
            Err(result["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string())
        } else {
            Ok(result["structuredContent"].clone())
        }
    }

    pub fn tools(&mut self) -> Vec<String> {
        let reply = self.request("tools/list", json!({}));
        reply["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }
}

impl Drop for McpHelper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
