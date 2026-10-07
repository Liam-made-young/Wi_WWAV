//! The app's side of starting the engine (`docs/ENGINE.md` §1): the private
//! directory and the shared memory, which outlive the engine; the engine
//! itself, with its flags and a pipe on its stdin; its listening line; and a
//! connected client. wi-core's supervisor and `wwav-engine-cli` both start
//! engines through this.

use crate::client::{CallError, Client};
use crate::msg::{Event, HelloResult};
use crate::shm::{Shm, LAYOUT};
use crate::PROTOCOL;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// The engine binary: `wwav-engine`, or `mock-engine` in tests.
    pub binary: PathBuf,
    /// Where the private directory `wwav-<app pid>/` goes: `$TMPDIR`.
    pub tmp_dir: PathBuf,
    /// `--device`: a device name, or `null` for the timer instead of hardware.
    pub device: Option<String>,
    pub rate: Option<u32>,
    pub block: Option<u32>,
    /// `--test`: the `debug.*` ops.
    pub test: bool,
    /// How long the engine has to say it is listening.
    pub start_timeout: Duration,
}

impl EngineConfig {
    pub fn new(binary: impl Into<PathBuf>) -> EngineConfig {
        EngineConfig {
            binary: binary.into(),
            tmp_dir: std::env::temp_dir(),
            device: None,
            rate: None,
            block: None,
            test: false,
            start_timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("The engine exited before it was listening ({0}).")]
    Exited(ExitStatus),
    #[error("The engine didn't say it was listening within {0:?}.")]
    NoListen(Duration),
    #[error("The engine printed {0:?} instead of its listening line.")]
    Stdout(String),
    #[error("hello failed: {0}")]
    Hello(CallError),
    #[error("{0}")]
    Mismatch(String),
}

/// A running engine and what the app made for it.
pub struct EngineProcess {
    config: EngineConfig,
    socket: PathBuf,
    shm: Shm,
    child: Child,
    stdin: Option<ChildStdin>,
    client: Client,
    /// Dropped after `drop` has stopped the engine and removed its socket.
    _dir: PrivateDir,
}

/// How long `hello` may take.
const HELLO_TIMEOUT: Duration = Duration::from_secs(2);

impl EngineProcess {
    /// Makes the private directory and the region, starts the engine and
    /// connects to it. It doesn't send `hello`: `hello` does, so a terminal
    /// can send its own.
    pub fn spawn(config: EngineConfig) -> Result<(EngineProcess, Receiver<Event>), StartError> {
        let shm = Shm::create_for_app()?;
        let dir = PrivateDir::enter(&config.tmp_dir)?;
        // One socket per engine, named for its region (`/wwav-<pid>-<n>`
        // listens on `engine-<n>.sock`), so engines this process starts
        // never share a path.
        let n = shm.name().rsplit('-').next().unwrap_or_default();
        let socket = dir.path.join(format!("engine-{n}.sock"));
        let (child, stdin, client, events) = launch(&config, &socket, shm.name())?;
        Ok((
            EngineProcess {
                config,
                socket,
                shm,
                child,
                stdin: Some(stdin),
                client,
                _dir: dir,
            },
            events,
        ))
    }

    /// After a crash or a kill: a new engine on the same region and
    /// directory, connected but not yet greeted. The old one is killed if it
    /// is still running.
    pub fn respawn(&mut self) -> Result<Receiver<Event>, StartError> {
        self.kill()?;
        self.child.wait()?;
        // The old engine may have died halfway through a clock write (§4.2).
        self.shm.region().clock.repair();
        let (child, stdin, client, events) = launch(&self.config, &self.socket, self.shm.name())?;
        self.child = child;
        self.stdin = Some(stdin);
        self.client = client;
        Ok(events)
    }

    /// The handshake: refuses an engine of another protocol or another
    /// shared-memory layout, or one that hasn't written the region.
    pub fn hello(&self, client_name: &str) -> Result<HelloResult, StartError> {
        let args = json!({"protocol": PROTOCOL, "client": client_name});
        let result = self
            .client
            .call("hello", args, HELLO_TIMEOUT)
            .map_err(StartError::Hello)?;
        let hello: HelloResult = serde_json::from_value(Value::Object(result))
            .map_err(|e| StartError::Mismatch(format!("hello's result doesn't read: {e}.")))?;
        if hello.protocol != PROTOCOL || hello.shm_layout != LAYOUT {
            return Err(StartError::Mismatch(format!(
                "The engine speaks protocol {} with shared-memory layout {}; the app speaks {PROTOCOL} with {LAYOUT}.",
                hello.protocol, hello.shm_layout
            )));
        }
        let header = self
            .shm
            .region()
            .header
            .read()
            .map_err(StartError::Mismatch)?;
        if header.engine_pid != hello.pid as u64 {
            return Err(StartError::Mismatch(format!(
                "The shared memory was written by pid {}, not the engine (pid {}).",
                header.engine_pid, hello.pid
            )));
        }
        Ok(hello)
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn shm(&self) -> &Shm {
        &self.shm
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// SIGKILL.
    pub fn kill(&mut self) -> io::Result<()> {
        if self.is_alive() {
            self.child.kill()?;
        }
        Ok(())
    }

    /// What the engine sees when the app quits or dies: end-of-file on stdin.
    pub fn close_stdin(&mut self) {
        self.stdin.take();
    }

    /// Waits up to `timeout` for the engine to exit.
    pub fn wait(&mut self, timeout: Duration) -> io::Result<Option<ExitStatus>> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Ok(Some(status));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for EngineProcess {
    fn drop(&mut self) {
        // The polite way first: the engine exits within 1 s of losing stdin.
        self.client.close();
        self.close_stdin();
        if !matches!(self.wait(Duration::from_millis(1500)), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let _ = fs::remove_file(&self.socket);
    }
}

/// How many `EngineProcess`es use each private directory. Every engine this
/// process starts under one `tmp_dir` shares `wwav-<app pid>/`, and the last
/// one to go removes it, so no one's respawn finds it gone.
static DIR_USERS: Mutex<BTreeMap<PathBuf, usize>> = Mutex::new(BTreeMap::new());

fn dir_users() -> MutexGuard<'static, BTreeMap<PathBuf, usize>> {
    DIR_USERS.lock().unwrap_or_else(|e| e.into_inner())
}

/// One engine's use of the private directory.
struct PrivateDir {
    path: PathBuf,
}

impl PrivateDir {
    fn enter(tmp: &Path) -> io::Result<PrivateDir> {
        let mut users = dir_users();
        let path = private_dir(tmp)?;
        *users.entry(path.clone()).or_insert(0) += 1;
        Ok(PrivateDir { path })
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let mut users = dir_users();
        let Some(n) = users.get_mut(&self.path) else {
            return;
        };
        *n -= 1;
        if *n == 0 {
            users.remove(&self.path);
            let _ = fs::remove_dir(&self.path); // only if nothing else is in it
        }
    }
}

/// `$TMPDIR/wwav-<app pid>/`, mode 0700. One that already exists is used only
/// if it is a directory of ours that no one else can enter.
fn private_dir(tmp: &Path) -> io::Result<PathBuf> {
    let dir = tmp.join(format!("wwav-{}", std::process::id()));
    match fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => Ok(dir),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            let m = fs::symlink_metadata(&dir)?;
            // SAFETY: geteuid has no preconditions.
            let me = unsafe { libc::geteuid() };
            if m.file_type().is_dir() && m.uid() == me && m.mode() & 0o077 == 0 {
                Ok(dir)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "{} isn't a private directory of this user's.",
                        dir.display()
                    ),
                ))
            }
        }
        Err(e) => Err(e),
    }
}

/// Starts one engine and connects to it once it says it is listening.
fn launch(
    config: &EngineConfig,
    socket: &Path,
    shm: &str,
) -> Result<(Child, ChildStdin, Client, Receiver<Event>), StartError> {
    // A socket file left by an engine that died; the new one binds afresh.
    let _ = fs::remove_file(socket);
    let mut cmd = Command::new(&config.binary);
    cmd.arg("--socket").arg(socket).arg("--shm").arg(shm);
    if let Some(device) = &config.device {
        cmd.arg("--device").arg(device);
    }
    if let Some(rate) = config.rate {
        cmd.arg("--rate").arg(rate.to_string());
    }
    if let Some(block) = config.block {
        cmd.arg("--block").arg(block.to_string());
    }
    if config.test {
        cmd.arg("--test");
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let stdin = child.stdin.take().expect("piped");
    let stdout = child.stdout.take().expect("piped");

    // The first line is the listening line, read as bytes (it may not be
    // UTF-8) and cut at 4 KiB (it may not end). The rest of stdout is
    // drained for the engine's life, so a stray print can never block it on
    // a full pipe.
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("engine stdout".into())
        .spawn(move || {
            let mut out = BufReader::new(stdout);
            let mut line = Vec::new();
            if matches!((&mut out).take(4096).read_until(b'\n', &mut line), Ok(n) if n > 0) {
                let _ = tx.send(line);
                let _ = io::copy(&mut out, &mut io::sink());
            }
        })?;
    let deadline = Instant::now() + config.start_timeout;
    let line = match rx.recv_timeout(config.start_timeout) {
        Ok(line) => line,
        Err(RecvTimeoutError::Timeout) => {
            return Err(give_up(child, StartError::NoListen(config.start_timeout)))
        }
        // Stdout ended without a line. Usually the engine is exiting; one
        // that closed stdout and kept running can never say it is
        // listening, and gets no longer than one that stays quiet.
        Err(RecvTimeoutError::Disconnected) => loop {
            if let Some(status) = child.try_wait()? {
                return Err(StartError::Exited(status));
            }
            if Instant::now() >= deadline {
                return Err(give_up(child, StartError::NoListen(config.start_timeout)));
            }
            thread::sleep(Duration::from_millis(2));
        },
    };
    let mut want = b"wwav-engine listening ".to_vec();
    want.extend_from_slice(socket.as_os_str().as_bytes());
    let line = line.strip_suffix(b"\n").unwrap_or(&line);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    if line != want {
        let printed = String::from_utf8_lossy(line).into_owned();
        return Err(give_up(child, StartError::Stdout(printed)));
    }
    match Client::connect(socket) {
        Ok((client, events)) => Ok((child, stdin, client, events)),
        Err(e) => Err(give_up(child, e.into())),
    }
}

/// Kills an engine that didn't start right and returns why.
fn give_up(mut child: Child, why: StartError) -> StartError {
    let _ = child.kill();
    let _ = child.wait();
    why
}
