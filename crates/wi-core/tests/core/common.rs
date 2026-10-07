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
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
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
    let r = ok(core, "library.import", json!({"paths": paths, "label": "import"}));
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
}

impl MockServer {
    pub fn start() -> MockServer {
        let mut child = Command::new("node")
            .arg(workspace().join("tools/mock-server/server.js"))
            .args(["--port", "0"])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("node runs the mock server");
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let url = line
            .trim()
            .rsplit(' ')
            .next()
            .expect("the mock prints its address")
            .to_string();
        assert!(url.starts_with("http://"), "unexpected line: {line}");
        MockServer { child, url }
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
        (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
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
                let page = agent.get(&url).call().map_err(|e| e.to_string())?.into_string().unwrap();
                let request = page
                    .split("name=\"request\" value=\"")
                    .nth(1)
                    .and_then(|r| r.split('"').next())
                    .ok_or("no request field")?
                    .to_string();
                let answer = match agent
                    .post(&format!("{base}/oauth/desktop/login"))
                    .send_form(&[("request", &request), ("email", email), ("password", password)])
                {
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
        setup.config(&server.url, server.browser("lmy@mi-wwav.com", "WeWave-lmy1")),
    )
    .unwrap();
    ok(&core, "account.signIn", json!({}));
    core
}
