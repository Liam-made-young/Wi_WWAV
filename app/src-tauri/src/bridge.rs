//! The bridge between the web UI and the core (docs/COMMANDS.md).
//!
//! There is one Tauri command, `core`, taking `{cmd, args}`. Each window's
//! capability lists the `cmd`s it may send (docs/SPEC.md 9.8), so the
//! settings window can't publish and a window in no capability file, such
//! as one that shows someone else's page, can't call anything. The core's
//! events go to the main window as one Tauri event, `core`, carrying
//! `{event, payload}`, the shape the dev bridge pushes over its WebSocket.
//! Meters go to a channel as raw bytes.
//!
//! Two `cmd`s are the shell's own rather than the core's:
//!
//! - `meters.listen {channel}` names the channel meters go to.
//! - `shell.room {room}` says which view is showing (`heat`, `space`,
//!   `console`, or `library` for the drawer over them), so the Edit menu can
//!   name that view's undo (docs/SPEC.md 2.7).

use std::sync::{Arc, Condvar, Mutex};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::ipc::{Channel, CommandScope, InvokeResponseBody, JavaScriptChannelId};
use tauri::{AppHandle, Emitter, Manager, Runtime, Webview};

use crate::menu;

/// What the shell asks of the core. `wi_core::Core` answers it in the app;
/// the tests give the shell a stand-in, so the fencing can be checked
/// without starting an engine.
pub trait Core: Send + Sync + 'static {
    fn invoke(&self, cmd: &str, args: Value) -> Result<Value, CoreError>;
}

/// An error as the UI sees it: a word to branch on and a sentence to show.
pub use wi_core::CoreError;

/// One entry of a window's allowlist in its capability file: a `cmd`, an
/// area (`app.settings.*`) or everything (`*`).
#[derive(Debug, Deserialize)]
pub struct Allow {
    pub cmd: String,
}

fn matches(pattern: &str, cmd: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some("") => true,
        Some(area) => area.ends_with('.') && cmd.starts_with(area),
        None => pattern == cmd,
    }
}

/// Whether a window may send `cmd`, given what its capability allows and
/// denies. Nothing listed means nothing allowed.
pub fn permitted(allow: &[Arc<Allow>], deny: &[Arc<Allow>], cmd: &str) -> bool {
    allow.iter().any(|a| matches(&a.cmd, cmd)) && !deny.iter().any(|d| matches(&d.cmd, cmd))
}

/// The shell's hold on the core. The core opens on a thread of its own so
/// the window can draw first (the 1.5 s cold-launch budget, 9.13); until it
/// is open, commands wait for it.
pub struct Bridge {
    core: Mutex<Option<Result<Arc<dyn Core>, CoreError>>>,
    opened: Condvar,
    meters: Mutex<Option<Channel<InvokeResponseBody>>>,
}

impl Bridge {
    pub fn new() -> Self {
        Self {
            core: Mutex::new(None),
            opened: Condvar::new(),
            meters: Mutex::new(None),
        }
    }

    /// Hands over the core once it is open, or why it couldn't open.
    pub fn set_core(&self, core: Result<Arc<dyn Core>, CoreError>) {
        *self.core.lock().unwrap() = Some(core);
        self.opened.notify_all();
    }

    /// Lets the core go as the app quits, so it stops the engine and its
    /// workers finish; a command still arriving is told the app is closing.
    pub fn close(&self) {
        let closing = CoreError::new("closing", "Wi_WWAV is closing.");
        let core = self.core.lock().unwrap().replace(Err(closing));
        self.opened.notify_all();
        drop(core);
    }

    /// The core, waiting for it to open.
    pub fn core(&self) -> Result<Arc<dyn Core>, CoreError> {
        let guard = self
            .opened
            .wait_while(self.core.lock().unwrap(), |c| c.is_none())
            .unwrap();
        guard.clone().expect("waited until set")
    }

    /// Runs `cmd` on a blocking thread, so a slow command never holds up the
    /// event loop.
    pub async fn invoke<R: Runtime>(
        app: &AppHandle<R>,
        cmd: String,
        args: Value,
    ) -> Result<Value, CoreError> {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            app.state::<Bridge>().core()?.invoke(&cmd, args)
        })
        .await
        .map_err(|e| {
            CoreError::new(
                "internal",
                format!("The command stopped before it finished: {e}"),
            )
        })?
    }

    fn listen_meters<R: Runtime>(
        &self,
        webview: &Webview<R>,
        args: &Value,
    ) -> Result<Value, CoreError> {
        let id = JavaScriptChannelId::deserialize(&args["channel"])
            .map_err(|_| CoreError::new("bad_args", "meters.listen needs a channel."))?;
        *self.meters.lock().unwrap() = Some(id.channel_on(webview.clone()));
        Ok(json!({}))
    }
}

impl Default for Bridge {
    fn default() -> Self {
        Self::new()
    }
}

/// The one command the web UI calls.
#[tauri::command]
pub async fn core<R: Runtime>(
    app: AppHandle<R>,
    webview: Webview<R>,
    scope: CommandScope<Allow>,
    cmd: String,
    args: Option<Value>,
) -> Result<Value, CoreError> {
    if !permitted(scope.allows(), scope.denies(), &cmd) {
        return Err(CoreError::new(
            "not_allowed",
            format!("This window can't use {cmd}."),
        ));
    }
    let args = args.unwrap_or_else(|| json!({}));
    match cmd.as_str() {
        "meters.listen" => app.state::<Bridge>().listen_meters(&webview, &args),
        "shell.room" => menu::show_room(&app, &args).await,
        _ => Bridge::invoke(&app, cmd, args).await,
    }
}

/// Passes one of the core's events to the main window, after letting the
/// Edit menu see a `history` change.
pub fn forward_event<R: Runtime>(app: &AppHandle<R>, event: &str, payload: Value) {
    if event == "history" {
        menu::history_changed(app, &payload);
    }
    emit(app, event, payload);
}

/// Sends the main window an event shaped like the core's.
pub fn emit<R: Runtime>(app: &AppHandle<R>, event: &str, payload: Value) {
    // Nothing to do if the main window is gone: the app is quitting.
    let _ = app.emit_to(
        "main",
        "core",
        json!({ "event": event, "payload": payload }),
    );
}

/// Passes the newest meter entry to the channel the UI named, if any.
pub fn forward_meters<R: Runtime>(app: &AppHandle<R>, bytes: Vec<u8>) {
    let bridge = app.state::<Bridge>();
    let mut meters = bridge.meters.lock().unwrap();
    if let Some(channel) = meters.as_ref() {
        if channel.send(InvokeResponseBody::Raw(bytes)).is_err() {
            // The page that asked has gone (a reload); it asks again.
            *meters = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(cmds: &[&str]) -> Vec<Arc<Allow>> {
        cmds.iter()
            .map(|c| Arc::new(Allow { cmd: c.to_string() }))
            .collect()
    }

    #[test]
    fn nothing_listed_allows_nothing() {
        assert!(!permitted(&[], &[], "app.hello"));
    }

    #[test]
    fn an_area_covers_its_commands_and_no_others() {
        let allow = rules(&["app.settings.*"]);
        assert!(permitted(&allow, &[], "app.settings.get"));
        assert!(permitted(&allow, &[], "app.settings.set"));
        assert!(!permitted(&allow, &[], "app.settingsx"));
        assert!(!permitted(&allow, &[], "app.settings"));
        assert!(!permitted(&allow, &[], "app.hello"));
    }

    #[test]
    fn an_exact_cmd_covers_only_itself() {
        let allow = rules(&["engine.status"]);
        assert!(permitted(&allow, &[], "engine.status"));
        assert!(!permitted(&allow, &[], "engine.status.more"));
        assert!(!permitted(&allow, &[], "engine.plugin.keepOff"));
    }

    #[test]
    fn a_star_covers_everything_a_deny_doesnt_name() {
        let allow = rules(&["*"]);
        assert!(permitted(&allow, &[], "publish.drop"));
        assert!(!permitted(&allow, &rules(&["publish.*"]), "publish.drop"));
    }

    #[test]
    fn a_star_inside_a_word_is_not_a_wildcard() {
        assert!(!permitted(&rules(&["app*"]), &[], "app.hello"));
    }

    #[test]
    fn commands_wait_for_the_core_to_open() {
        struct Echo;
        impl Core for Echo {
            fn invoke(&self, cmd: &str, _: Value) -> Result<Value, CoreError> {
                Ok(json!(cmd))
            }
        }
        let bridge = Arc::new(Bridge::new());
        let waiter = {
            let bridge = bridge.clone();
            std::thread::spawn(move || bridge.core().unwrap().invoke("app.hello", json!({})))
        };
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(!waiter.is_finished());
        bridge.set_core(Ok(Arc::new(Echo)));
        assert_eq!(waiter.join().unwrap(), Ok(json!("app.hello")));
    }

    #[test]
    fn a_core_that_failed_to_open_answers_every_command_with_why() {
        let bridge = Bridge::new();
        let why = CoreError::new(
            "library_unwritable",
            "Wi_WWAV can't write to ~/Music/Wi_WWAV.",
        );
        bridge.set_core(Err(why.clone()));
        assert_eq!(bridge.core().err(), Some(why));
    }
}
