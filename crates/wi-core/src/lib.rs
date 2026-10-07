//! wi-core: the app's Rust core, with no Tauri in it (docs/SPEC.md 9.1).
//!
//! [`Core::open`] opens a library folder and starts the audio engine in the
//! background. [`Core::invoke`] answers every command in docs/COMMANDS.md
//! with JSON, or a [`CoreError`] whose `code` the UI can branch on and whose
//! `message` it can show as is. [`Core::events`] is the event stream
//! (`{event, payload}`), and [`Core::meters`] the meters, raw bytes at most
//! once a frame. The Tauri app and `wi-devbridge` are two hosts of the same
//! core.
//!
//! - `library`: wi-store with wwav-formats as the file reader (2.5, 2.14).
//! - `history` and `records`: labelled undo per room over the journal (2.7).
//! - `player`: the one listening player (2.3).
//! - `engine`: the supervisor (docs/ENGINE.md §1, §3.1, §5).
//! - `account`, `upload`, `heat`: mi-wwav.com (2.4, 2.8, 9.7).
//! - `heat_cmd`, `calendars`, `claude`, `watch`: Heat's `heat.*` commands,
//!   its iCal calendars, Settings → Claude, and noticing the MCP helper
//!   (docs/HEAT.md).
//! - `export`: Export everything (2.9).

mod account;
mod args;
mod bus;
mod calendars;
mod claude;
pub mod engine;
mod export;
mod heat;
mod heat_cmd;
mod history;
mod kv;
mod library;
mod net;
mod player;
mod settings;
mod upload;
mod watch;

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use wi_store::Store;
use wwav_wire::process::EngineConfig;

use args::Args;
pub use bus::Event;
use bus::{lock, Bus};
pub use engine::{Engine, EngineSession, Phase};
pub use library::Formats;
pub use player::PausedFor;

/// A refusal or a failure: `code` is a snake_case word the UI can branch on,
/// `message` a sentence it can show as is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CoreError {
    pub code: String,
    pub message: String,
}

impl CoreError {
    pub fn new(code: &str, message: impl Into<String>) -> CoreError {
        CoreError {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CoreError {}

impl From<wi_store::Error> for CoreError {
    fn from(e: wi_store::Error) -> CoreError {
        match e {
            wi_store::Error::Refused(why) => CoreError::new("refused", why),
            other => CoreError::new("library", other.to_string()),
        }
    }
}

impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> CoreError {
        CoreError::new("library", format!("library.sqlite: {e}"))
    }
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> CoreError {
        CoreError::new("io", e.to_string())
    }
}

/// Where secrets live: the account's tokens, and nothing else of the
/// person's, ever (9.8). The app passes the operating system's keychain; tests
/// pass [`MemorySecrets`].
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    fn delete(&self, key: &str) -> Result<(), String>;
}

/// Secrets held in memory, for tests and the dev bridge.
#[derive(Default)]
pub struct MemorySecrets {
    map: Mutex<HashMap<String, String>>,
}

impl SecretStore for MemorySecrets {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        Ok(lock(&self.map).get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        lock(&self.map).insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        lock(&self.map).remove(key);
        Ok(())
    }
}

/// Opens a URL in the system browser (sign-in, 9.7).
pub type Opener = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

pub struct Config {
    /// `wwav-engine`, or `mock-engine` in tests and the dev bridge.
    pub engine_path: PathBuf,
    /// mi-wwav.com, or `tools/mock-server`'s address.
    pub server_url: String,
    pub secrets: Arc<dyn SecretStore>,
    pub opener: Opener,
    /// The engine's `--device`: None for the default output, "null" for the
    /// timer that stands in for hardware.
    pub device: Option<String>,
    /// The engine's `--test`: its `debug.*` ops.
    pub engine_test: bool,
    /// Where the engine's private socket directory goes: `$TMPDIR`.
    pub tmp_dir: PathBuf,
    /// Where `wi-mcp` is, for Settings → Claude's lines. None: `WI_WWAV_MCP`,
    /// else beside the app (docs/HEAT.md).
    pub helper: Option<PathBuf>,
    /// A fixed "now" in milliseconds since 1970, for tests of what the time
    /// decides. None: the clock.
    pub now: Option<f64>,
}

impl Config {
    pub fn new(
        engine_path: impl Into<PathBuf>,
        server_url: &str,
        secrets: Arc<dyn SecretStore>,
        opener: Opener,
    ) -> Config {
        Config {
            engine_path: engine_path.into(),
            server_url: server_url.trim_end_matches('/').to_string(),
            secrets,
            opener,
            device: None,
            engine_test: false,
            tmp_dir: std::env::temp_dir(),
            helper: None,
            now: None,
        }
    }
}

pub(crate) struct Inner {
    root: PathBuf,
    store: Mutex<Store>,
    kv: kv::Kv,
    bus: Arc<Bus>,
    engine: Engine,
    player: Mutex<player::Player>,
    net: net::Net,
    /// The Keychain, for the calendars' addresses as well as the account.
    secrets: Arc<dyn SecretStore>,
    helper: Option<PathBuf>,
    /// The journal entries this process made, so the watcher tells the views
    /// only about other processes' (the MCP helper's).
    own: Mutex<BTreeSet<String>>,
    /// A fixed "now" for tests (milliseconds), instead of the clock.
    fixed_now: Mutex<Option<f64>>,
    uploads: Mutex<upload::Status>,
    opener: Opener,
    /// One sign-in at a time: the browser may only be asked once.
    signing_in: Mutex<()>,
    closing: AtomicBool,
    /// Wakes the background workers: something to upload or sync, or closing.
    wake: (Mutex<u64>, Condvar),
}

impl Inner {
    fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        lock(&self.store)
    }

    fn closing(&self) -> bool {
        self.closing.load(Ordering::Relaxed)
    }

    /// Notes a journal entry this process made.
    fn note_own(&self, id: &str) {
        let mut own = lock(&self.own);
        if own.len() > 10_000 {
            own.clear();
        }
        own.insert(id.to_string());
    }

    /// Whether `id` is an entry this process made (and forgets it was).
    fn take_own(&self, id: &str) -> bool {
        lock(&self.own).remove(id)
    }

    /// Now and the person's time zone: Settings → Heat's, else the system's.
    fn clock(&self) -> wi_heat_store::Clock {
        let now =
            lock(&self.fixed_now).unwrap_or_else(|| jiff::Timestamp::now().as_millisecond() as f64);
        let zone = settings::read(self)
            .ok()
            .and_then(|s| {
                s["heat"]["timeZone"]
                    .as_str()
                    .and_then(|name| jiff::tz::TimeZone::get(name).ok())
            })
            .unwrap_or_else(jiff::tz::TimeZone::system);
        wi_heat_store::Clock::at(now, zone)
    }

    /// Wakes the workers now.
    fn poke(&self) {
        *lock(&self.wake.0) += 1;
        self.wake.1.notify_all();
    }

    /// Sleeps up to `d`, or until poked or closing. True if poked.
    fn nap(&self, d: Duration) -> bool {
        let mut n = lock(&self.wake.0);
        let before = *n;
        let deadline = std::time::Instant::now() + d;
        while *n == before && !self.closing() {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return false;
            }
            n = self
                .wake
                .1
                .wait_timeout(n, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        true
    }
}

/// The app's core: one library, one engine, one account.
pub struct Core {
    inner: Arc<Inner>,
    workers: Vec<JoinHandle<()>>,
}

impl Core {
    /// Opens the library at `library` (made if missing) and starts the
    /// engine in the background, so nothing waits on it.
    pub fn open(library: &Path, config: Config) -> Result<Core, CoreError> {
        let store = Store::open(library)?;
        let kv = kv::Kv::open(library)?;
        let bus = Arc::new(Bus::default());
        let engine = Engine::start(
            EngineConfig {
                binary: config.engine_path.clone(),
                tmp_dir: config.tmp_dir.clone(),
                device: config.device.clone(),
                rate: None,
                block: None,
                test: config.engine_test,
                start_timeout: Duration::from_secs(5),
            },
            bus.clone(),
        );
        let net = net::Net::new(&config.server_url, config.secrets.clone());
        let secrets = config.secrets.clone();
        let inner = Arc::new(Inner {
            root: library.to_path_buf(),
            store: Mutex::new(store),
            kv,
            bus,
            engine,
            player: Mutex::new(player::Player::default()),
            net,
            secrets,
            helper: config.helper.clone(),
            own: Mutex::new(BTreeSet::new()),
            fixed_now: Mutex::new(config.now),
            uploads: Mutex::new(upload::Status::default()),
            opener: config.opener,
            signing_in: Mutex::new(()),
            closing: AtomicBool::new(false),
            wake: (Mutex::new(0), Condvar::new()),
        });
        player::listen(&inner);
        let workers = vec![
            upload::start(&inner),
            heat::start(&inner),
            watch::start(&inner),
            calendars::start(&inner),
        ];
        Ok(Core { inner, workers })
    }

    /// Answers one command of docs/COMMANDS.md.
    pub fn invoke(&self, cmd: &str, args: Value) -> Result<Value, CoreError> {
        let a = Args::new(cmd, &args);
        let i = &self.inner;
        match cmd {
            "app.hello" => app_hello(i),
            "app.settings.get" => settings::get(i),
            "app.settings.set" => settings::set(i, &a),

            "history.get" => history::get(i, &a),
            "history.undo" => history::undo(i, &a),
            "history.redo" => history::redo(i, &a),
            "history.undoEntry" => history::undo_entry(i, &a),

            "records.list" => history::records_list(i, &a),
            "records.get" => history::records_get(i, &a),
            "records.mutate" => history::records_mutate(i, &a),

            heat if heat.starts_with("heat.") => heat_cmd::invoke(i, heat, &a),

            "library.list" => library::list(i, &a),
            "library.search" => library::search(i, &a),
            "library.get" => library::get(i, &a),
            "library.inspect" => library::inspect(&a),
            "library.import" => library::import(i, &a),
            "library.tag" => library::tag(i, &a),
            "library.rename" => library::rename(i, &a),
            "library.colour" => library::colour(i, &a),
            "library.pin" => library::pin(i, &a),
            "library.smart.save" => library::smart_save(i, &a),
            "library.delete" => library::delete(i, &a),
            "library.cleanup.preview" => library::cleanup_preview(i),
            "library.cleanup.run" => library::cleanup_run(i),
            "library.trash.empty" => library::trash_empty(i),

            "player.load" => player::load(i, &a),
            "player.play" => player::play(i),
            "player.pause" => player::pause(i),
            "player.seek" => player::seek(i, &a),
            "player.stem" => player::stem(i, &a),

            "engine.status" => Ok(i.engine.status()),
            "engine.plugin.keepOff" => i.engine.keep_off(a.str("device")?).map(|_| json!({})),
            "engine.plugin.tryAgain" => i.engine.try_again(a.str("device")?).map(|_| json!({})),

            "account.status" => account::status(i),
            "account.signIn" => account::sign_in(i),
            "account.signOut" => account::sign_out(i),

            "publish.drop" => upload::drop_on(i, &a),
            "publish.queue" => upload::queue(i),

            "export.everything" => export::everything(i, &a),

            _ => Err(CoreError::new(
                "unknown_command",
                format!("There is no command called '{cmd}'."),
            )),
        }
    }

    /// The event stream: every event from now on.
    pub fn events(&self) -> Receiver<Event> {
        self.inner.bus.subscribe()
    }

    /// The meters: the newest meter entry (docs/ENGINE.md §4.4) as raw
    /// bytes, at most once a frame, only while the engine runs.
    pub fn meters(&self) -> Receiver<Vec<u8>> {
        self.inner.bus.meters()
    }

    /// The engine, for the Console and for tests.
    pub fn engine(&self) -> &Engine {
        &self.inner.engine
    }

    /// The library folder.
    pub fn library(&self) -> &Path {
        &self.inner.root
    }

    /// Pauses the listening player because the Console pressed play or a
    /// film opened (2.3). Nothing resumes on its own.
    pub fn pause_player(&self, why: PausedFor) -> Result<Value, CoreError> {
        player::pause_for(&self.inner, why)
    }

    /// Pushes and pulls Heat's records now, rather than at the next round.
    pub fn sync_heat(&self) -> Result<Value, CoreError> {
        heat::sync_now(&self.inner)
    }

    /// Makes "now" a fixed time (milliseconds since 1970), or the clock again
    /// for None. For tests of what the time decides: heat, Plan my day, the
    /// timer, what a Now making line shows.
    pub fn set_now(&self, ms: Option<f64>) {
        *lock(&self.inner.fixed_now) = ms;
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        self.inner.closing.store(true, Ordering::Relaxed);
        self.inner.poke();
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
        self.inner.engine.shutdown();
    }
}

fn app_hello(i: &Inner) -> Result<Value, CoreError> {
    let settings = settings::read(i)?;
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "library": i.root,
        "signedIn": i.net.signed_in(),
        "platform": std::env::consts::OS,
        "reduceMotion": settings["reduceMotion"],
    }))
}
