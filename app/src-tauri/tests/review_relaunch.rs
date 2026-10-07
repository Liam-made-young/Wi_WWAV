//! Reviewer's test: after a relaunch, the Edit menu names the undo the
//! journal kept (docs/SPEC.md 2.7: "Undo is ⌘Z everywhere ... and always
//! labelled"; docs/PLAN.md F5: a label survives a relaunch).
//!
//! The shell learns labels from the core's `history` event (sent after a
//! change) or from `shell.room` (sent by the UI when it changes views). At
//! launch neither happens, so once the core is open the shell asks for the
//! labels of the view the app opens on, Heat: until it did, the Edit menu
//! read a greyed "Undo" while Heat's journal held "add task".

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::menu::MenuItemKind;
use tauri::test::{mock_builder, MockRuntime};
use tauri::{App, Manager};
use wi_wwav_app::{build_menu, shell, start_core, Bridge};

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

#[test]
fn after_a_relaunch_the_edit_menu_names_the_kept_undo() {
    let library = tempfile::tempdir().unwrap();
    let engine = library.path().join("no-engine");

    // The last session: one change in Heat, then quit.
    {
        let config = wi_core::Config::new(
            &engine,
            "http://127.0.0.1:9",
            Arc::new(wi_core::MemorySecrets::default()),
            Arc::new(|_: &str| Err("no browser".to_string())),
        );
        let core = wi_core::Core::open(library.path(), config).unwrap();
        let put = json!({ "op": "put", "kind": "task", "id": "01JC5Q8V3M2T7R9X4K6W0YHZNB", "value": { "title": "Read chapter 4" } });
        core.invoke(
            "records.mutate",
            json!({ "label": "add task", "room": "heat", "ops": [put] }),
        )
        .unwrap();
        let kept = core
            .invoke("history.get", json!({ "room": "heat" }))
            .unwrap();
        assert_eq!(kept["undo"], "Undo add task");
    }

    // The relaunch, through the shell.
    std::env::set_var("WI_WWAV_LIBRARY", library.path());
    std::env::set_var("WI_WWAV_ENGINE", &engine);
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
    start_core(app.handle());
    let core = app.state::<Bridge>().core().expect("the core opens");
    assert_eq!(
        core.invoke("history.get", json!({ "room": "heat" }))
            .unwrap()["undo"],
        "Undo add task",
        "the journal kept the label"
    );

    // Give the shell time to have asked, had it been going to.
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if edit_item(&app, "history.undo").1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        edit_item(&app, "history.undo"),
        ("Undo add task".into(), true),
        "the Edit menu at launch"
    );
}
