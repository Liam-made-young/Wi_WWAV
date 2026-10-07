//! The web view is fenced (docs/SPEC.md 9.8): each window may call only the
//! commands its capability lists, and a window in no capability file, one
//! that shows someone else's page, may call none.
//!
//! Fails if: a window in no capability file reaches the core at all; the settings
//! window reaches a command its capability doesn't list (it must never
//! publish, buy or change clips); the main window can't reach every core
//! command; or any window can drive the updater, which only quitting may
//! install (9.9).

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use wi_wwav_app::{shell, Bridge, Core, CoreError};

/// Records what reached it.
#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl Core for Recorder {
    fn invoke(&self, cmd: &str, _args: Value) -> Result<Value, CoreError> {
        self.0.lock().unwrap().push(cmd.to_string());
        Ok(json!({ "ran": cmd }))
    }
}

fn app() -> (App<MockRuntime>, Arc<Recorder>) {
    let app = shell(mock_builder())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(tauri::generate_context!())
        .expect("the shell builds on the mock runtime");
    let core = Arc::new(Recorder::default());
    app.state::<Bridge>().set_core(Ok(core.clone()));
    (app, core)
}

/// Opens a window as tauri.conf.json describes it (the mock runtime opens
/// none by itself).
fn window(app: &App<MockRuntime>, label: &str) -> WebviewWindow<MockRuntime> {
    let config = app.config().app.windows.iter().find(|w| w.label == label);
    let config = config.unwrap_or_else(|| panic!("tauri.conf.json describes the {label} window"));
    WebviewWindowBuilder::from_config(app.handle(), config)
        .unwrap()
        .build()
        .unwrap()
}

const LOCAL: &str = if cfg!(windows) {
    "http://tauri.localhost"
} else {
    "tauri://localhost"
};

fn call(
    window: &WebviewWindow<MockRuntime>,
    url: &str,
    tauri_cmd: &str,
    body: Value,
) -> Result<Value, Value> {
    let request = InvokeRequest {
        cmd: tauri_cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: url.parse().unwrap(),
        body: InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    get_ipc_response(window, request).map(|b| b.deserialize::<Value>().unwrap())
}

/// Tauri itself refused the call, before any command ran.
fn refused_by_acl(answer: Result<Value, Value>) {
    let why = answer.expect_err("the call was refused").to_string();
    assert!(why.contains("not allowed"), "{why}");
}

fn core_call(window: &WebviewWindow<MockRuntime>, cmd: &str) -> Result<Value, Value> {
    call(window, LOCAL, "core", json!({ "cmd": cmd, "args": {} }))
}

#[test]
fn the_main_window_reaches_every_core_command() {
    let (app, core) = app();
    let main = window(&app, "main");
    for cmd in [
        "app.hello",
        "library.delete",
        "publish.drop",
        "history.undo",
        "account.signIn",
    ] {
        assert_eq!(core_call(&main, cmd), Ok(json!({ "ran": cmd })));
    }
    assert_eq!(core.0.lock().unwrap().len(), 5);
}

#[test]
fn the_settings_window_reaches_only_what_settings_need() {
    let (app, core) = app();
    wi_wwav_app::open_settings(app.handle()).unwrap();
    let settings = app
        .get_webview_window("settings")
        .expect("Settings… opens the settings window");
    for cmd in [
        "app.settings.get",
        "app.settings.set",
        "account.signOut",
        "engine.status",
        "library.trash.empty",
    ] {
        assert_eq!(
            core_call(&settings, cmd),
            Ok(json!({ "ran": cmd })),
            "{cmd}"
        );
    }
    for cmd in [
        "library.delete",
        "publish.drop",
        "records.mutate",
        "history.undo",
        "player.play",
        "export.everything",
    ] {
        let refused = core_call(&settings, cmd).expect_err(cmd);
        assert_eq!(refused["code"], "not_allowed", "{cmd}");
    }
    assert_eq!(
        core.0.lock().unwrap().len(),
        5,
        "a refused command never reaches the core"
    );
}

#[test]
fn a_window_in_no_capability_file_reaches_nothing() {
    let (app, core) = app();
    let elsewhere = "https://example.org/someone-elses-page";
    let remote = WebviewWindowBuilder::new(
        &app,
        "remote",
        WebviewUrl::External(elsewhere.parse().unwrap()),
    )
    .build()
    .unwrap();
    for url in [elsewhere, LOCAL] {
        for cmd in ["app.hello", "account.status", "publish.drop"] {
            refused_by_acl(call(
                &remote,
                url,
                "core",
                json!({ "cmd": cmd, "args": {} }),
            ));
        }
        let listen = json!({ "event": "core", "target": { "kind": "Any" }, "handler": 1 });
        refused_by_acl(call(&remote, url, "plugin:event|listen", listen));
    }
    assert!(core.0.lock().unwrap().is_empty());
}

#[test]
fn no_window_can_drive_the_updater() {
    let (app, _) = app();
    let main = window(&app, "main");
    for cmd in [
        "plugin:updater|check",
        "plugin:updater|download_and_install",
        "plugin:updater|install",
    ] {
        refused_by_acl(call(&main, LOCAL, cmd, json!({})));
    }
}

#[test]
fn only_the_main_window_takes_the_meters() {
    let (app, _) = app();
    let main = window(&app, "main");
    let args = json!({ "cmd": "meters.listen", "args": { "channel": "__CHANNEL__:7" } });
    assert_eq!(call(&main, LOCAL, "core", args.clone()), Ok(json!({})));
    wi_wwav_app::open_settings(app.handle()).unwrap();
    let settings = app.get_webview_window("settings").unwrap();
    assert_eq!(
        call(&settings, LOCAL, "core", args).unwrap_err()["code"],
        "not_allowed"
    );
}
