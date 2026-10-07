//! A reviewer's tests of the View menu's ticks. They exposed findings in the
//! shell (muda flips a check item as it is chosen, so ⌘1 in Heat unticked
//! Heat and ⌘2 ticked two views), and ran ignored until the shell kept the
//! tick to the view the UI says is showing. They run now.

use std::sync::Arc;

use serde_json::{json, Value};
use tauri::menu::{MenuEvent, MenuId, MenuItemKind};
use tauri::test::{mock_builder, MockRuntime};
use tauri::{App, Manager};

use crate::bridge::{Bridge, Core, CoreError};
use crate::menu::{self, ROOMS};

struct NoHistory;

impl Core for NoHistory {
    fn invoke(&self, _cmd: &str, _args: Value) -> Result<Value, CoreError> {
        Ok(json!({ "undo": null, "redo": null, "cant": null }))
    }
}

fn app() -> App<MockRuntime> {
    let app = crate::shell(mock_builder())
        .build(tauri::generate_context!())
        .unwrap();
    app.set_menu(menu::build(app.handle()).unwrap()).unwrap();
    app.state::<Bridge>().set_core(Ok(Arc::new(NoHistory)));
    app
}

fn check_item(app: &App<MockRuntime>, id: &str) -> tauri::menu::CheckMenuItem<MockRuntime> {
    let menus = app.menu().unwrap().items().unwrap();
    let found = menus
        .iter()
        .filter_map(MenuItemKind::as_submenu)
        .find_map(|m| m.get(id));
    match found {
        Some(MenuItemKind::Check(c)) => c,
        _ => panic!("{id} is missing or not a check item"),
    }
}

fn ticked(app: &App<MockRuntime>) -> Vec<&'static str> {
    ROOMS
        .iter()
        .filter(|r| check_item(app, r.id).is_checked().unwrap())
        .map(|r| r.title)
        .collect()
}

/// What choosing a View item does in the real app: muda flips the item's
/// own tick first (macOS: `NsMenuItem::action`; GTK: `CheckMenuItem`'s
/// toggle, muda 0.20), then the shell hears the click.
fn choose(app: &App<MockRuntime>, id: &str) {
    let item = check_item(app, id);
    item.set_checked(!item.is_checked().unwrap()).unwrap();
    menu::on_event(
        app.handle(),
        MenuEvent {
            id: MenuId::new(id),
        },
    );
}

/// ⌘1 in Heat (the UI has nothing to change, so it never sends
/// `shell.room`): the View menu should still tick Heat.
#[test]
fn choosing_the_room_already_showing_keeps_its_tick() {
    let app = app();
    assert_eq!(ticked(&app), ["Heat"]);
    choose(&app, "room.heat");
    assert_eq!(ticked(&app), ["Heat"], "⌘1 in Heat took Heat's tick away");
}

/// ⌘2 from Heat, before (or without) the UI answering with `shell.room`
/// (a sheet that keeps the room, a UI still loading): one room at most is
/// ticked, never two.
#[test]
fn choosing_another_room_never_ticks_two() {
    let app = app();
    choose(&app, "room.space");
    assert!(ticked(&app).len() <= 1, "ticked: {:?}", ticked(&app));
}
