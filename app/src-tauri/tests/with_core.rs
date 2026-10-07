//! The shell around the real core (crates/wi-core), on Tauri's mock runtime.
//!
//! Fails if: the core doesn't open on the library the shell chose; a command
//! from the main window doesn't reach it; a change's `history` event doesn't
//! reach the main window as the event `core` `{event, payload}`; or the Edit
//! menu doesn't then name that room's undo, and after an undo its redo.

use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::menu::MenuItemKind;
use tauri::test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, WebviewWindow, WebviewWindowBuilder};
use wi_wwav_app::{build_menu, shell, start_core};

fn call(window: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    let request = InvokeRequest {
        cmd: "core".into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: if cfg!(windows) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        }
        .parse()
        .unwrap(),
        body: InvokeBody::Json(json!({ "cmd": cmd, "args": args })),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    get_ipc_response(window, request).map(|b| b.deserialize::<Value>().unwrap())
}

/// The Edit menu's item `id`: its title and whether it can be chosen.
fn edit_item(app: &App<MockRuntime>, id: &str) -> (String, bool) {
    let menus = app.menu().unwrap().items().unwrap();
    let edit = menus
        .iter()
        .filter_map(MenuItemKind::as_submenu)
        .find(|m| m.text().unwrap() == "Edit")
        .unwrap();
    let item = edit.get(id).unwrap();
    let item = item.as_menuitem().unwrap();
    (item.text().unwrap(), item.is_enabled().unwrap())
}

/// Waits for the core to send the main window a `history` event for `room`.
fn history_for(events: &mpsc::Receiver<Value>, room: &str) -> Value {
    loop {
        let e = events
            .recv_timeout(Duration::from_secs(5))
            .expect("a history event within 5 s");
        if e["event"] == "history" && e["payload"]["room"] == room {
            return e["payload"].clone();
        }
    }
}

#[test]
fn commands_reach_the_core_and_its_history_reaches_the_window_and_menu() {
    let library = tempfile::tempdir().unwrap();
    std::env::set_var("WI_WWAV_LIBRARY", library.path());
    // No engine and no server: this is about the bridge, and the core opens
    // without either. No keyring either, which reads as signed out.
    std::env::set_var("WI_WWAV_ENGINE", library.path().join("no-engine"));
    std::env::set_var("WI_WWAV_SERVER", "http://127.0.0.1:9");
    std::env::set_var(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={}", library.path().join("no-bus").display()),
    );

    let app = shell(mock_builder())
        .plugin(tauri_plugin_opener::init())
        .build(tauri::generate_context!())
        .unwrap();
    app.set_menu(build_menu(app.handle()).unwrap()).unwrap();
    let (tx, events) = mpsc::channel();
    app.listen_any("core", move |e| {
        let _ = tx.send(serde_json::from_str::<Value>(e.payload()).unwrap());
    });
    start_core(app.handle());
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .unwrap()
        .clone();
    let main = WebviewWindowBuilder::from_config(app.handle(), &config)
        .unwrap()
        .build()
        .unwrap();

    let hello = call(&main, "app.hello", json!({})).unwrap();
    assert_eq!(hello["library"], json!(library.path()));
    assert_eq!(hello["signedIn"], json!(false));

    call(&main, "shell.room", json!({ "room": "space" })).unwrap();
    assert_eq!(edit_item(&app, "history.undo"), ("Undo".into(), false));

    let put = json!({ "op": "put", "kind": "save", "id": "01JC5Q8V3M2T7R9X4K6W0YHZNB", "value": { "title": "World Ending" } });
    call(
        &main,
        "records.mutate",
        json!({ "label": "add world", "room": "space", "ops": [put] }),
    )
    .unwrap();
    assert_eq!(history_for(&events, "space")["undo"], "Undo add world");
    assert_eq!(
        edit_item(&app, "history.undo"),
        ("Undo add world".into(), true)
    );

    call(&main, "history.undo", json!({ "room": "space" })).unwrap();
    assert_eq!(history_for(&events, "space")["redo"], "Redo add world");
    assert_eq!(edit_item(&app, "history.undo"), ("Undo".into(), false));
    assert_eq!(
        edit_item(&app, "history.redo"),
        ("Redo add world".into(), true)
    );
}
