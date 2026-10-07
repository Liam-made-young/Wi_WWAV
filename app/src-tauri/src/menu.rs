//! The menu bar (docs/SPEC.md 2.1, 2.7, 2.13).
//!
//! Most items do what a key or a button in the UI does, so their clicks go
//! to the main window as the event `menu` `{action}` and the UI's one input
//! router acts on them as on its own keys. The shell keeps three for itself:
//! Open… (a native picker), Settings… (a window) and the macOS items.
//!
//! The Edit menu names the current view's undo: "Undo move clip", or
//! "Can't undo a message." greyed out. It follows the core's `history`
//! event, and the UI says which view is showing with `shell.room` (the
//! views are Heat, Space and Console, on ⌘1–⌘3; the library drawer over any
//! of them is `library`).

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu,
};
use tauri::{AppHandle, Manager, Runtime};

use crate::bridge::{self, Bridge, CoreError};

/// One item the shell adds: its id (the action the UI receives), its title
/// and its keys.
pub struct Item {
    pub id: &'static str,
    pub title: &'static str,
    pub keys: Option<&'static str>,
}

const fn item(id: &'static str, title: &'static str, keys: Option<&'static str>) -> Item {
    Item { id, title, keys }
}

pub const NEW_SESSION: Item = item("session.new", "New session", Some("CmdOrCtrl+N"));
pub const OPEN: Item = item("file.open", "Open…", Some("CmdOrCtrl+O"));
pub const EXPORT: Item = item(
    "export.everything",
    "Export everything…",
    Some("CmdOrCtrl+Shift+E"),
);
pub const UNDO: Item = item("history.undo", "Undo", Some("CmdOrCtrl+Z"));
pub const REDO: Item = item("history.redo", "Redo", Some("CmdOrCtrl+Shift+Z"));
pub const LIBRARY: Item = item("library.toggle", "Library", Some("CmdOrCtrl+L"));
pub const SETTINGS: Item = item("settings", "Settings…", Some("CmdOrCtrl+,"));

/// The view segments, in the switcher's order (2.1): Heat, the profile
/// view; Space, the social view; the Console, the creation view.
pub const ROOMS: [Item; 3] = [
    item("room.heat", "Heat", Some("CmdOrCtrl+1")),
    item("room.space", "Space", Some("CmdOrCtrl+2")),
    item("room.console", "Console", Some("CmdOrCtrl+3")),
];

/// What the journal scopes ⌘Z by (9.6): the three views, and the library
/// drawer over any of them.
const HISTORY_ROOMS: [&str; 4] = ["heat", "space", "console", "library"];

/// What a click on an item does.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// The UI does it: sent as the event `menu` `{action}`.
    Ui(&'static str),
    /// A native picker, then the files go where opened files go.
    OpenFiles,
    /// The settings window opens (2.13).
    Settings,
}

pub fn action(id: &str) -> Option<Action> {
    match id {
        "file.open" => Some(Action::OpenFiles),
        "settings" => Some(Action::Settings),
        _ => [&NEW_SESSION, &EXPORT, &UNDO, &REDO, &LIBRARY]
            .into_iter()
            .chain(&ROOMS)
            .find(|i| i.id == id)
            .map(|i| Action::Ui(i.id)),
    }
}

/// A view's undo and redo labels, as `history.get` and the `history` event
/// give them (docs/COMMANDS.md): ready ("Undo move clip"), or held by work
/// that left the machine ("Can't undo a message.").
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct History {
    pub undo: Option<String>,
    pub redo: Option<String>,
    pub cant: Option<String>,
    #[serde(rename = "cantRedo")]
    pub cant_redo: Option<String>,
}

/// An item's title and whether it can be chosen.
pub type Title = (String, bool);

impl History {
    /// The Undo and Redo items' titles. A held one wins: what ⌘Z would
    /// reach is work that left the machine, and it can't be undone.
    pub fn titles(&self) -> (Title, Title) {
        fn title(held: &Option<String>, ready: &Option<String>, plain: &str) -> Title {
            match (held, ready) {
                (Some(held), _) => (held.clone(), false),
                (None, Some(ready)) => (ready.clone(), true),
                (None, None) => (plain.to_string(), false),
            }
        }
        (
            title(&self.cant, &self.undo, UNDO.title),
            title(&self.cant_redo, &self.redo, REDO.title),
        )
    }
}

/// The view showing and the last labels heard for each.
pub struct EditState {
    room: String,
    history: HashMap<String, History>,
}

impl Default for EditState {
    fn default() -> Self {
        // The app opens on Heat (2.14).
        Self {
            room: "heat".into(),
            history: HashMap::new(),
        }
    }
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let add = |i: &Item| MenuItem::with_id(app, i.id, i.title, true, i.keys);
    let sep = || PredefinedMenuItem::separator(app);

    let mut file: Vec<Entry<R>> = vec![
        add(&NEW_SESSION)?.boxed(),
        add(&OPEN)?.boxed(),
        sep()?.boxed(),
        add(&EXPORT)?.boxed(),
    ];
    // Elsewhere there is no app menu, so Settings… and Quit end File.
    if !cfg!(target_os = "macos") {
        file.extend([
            sep()?.boxed(),
            add(&SETTINGS)?.boxed(),
            sep()?.boxed(),
            PredefinedMenuItem::quit(app, None)?.boxed(),
        ]);
    }
    let file = Submenu::with_items(app, "File", true, &refs(&file))?;

    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &MenuItem::with_id(app, UNDO.id, UNDO.title, false, UNDO.keys)?,
            &MenuItem::with_id(app, REDO.id, REDO.title, false, REDO.keys)?,
            &sep()?,
            // The web view's own text editing needs these on macOS.
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let mut view: Vec<Entry<R>> = Vec::new();
    for (n, room) in ROOMS.iter().enumerate() {
        view.push(
            CheckMenuItem::with_id(app, room.id, room.title, true, n == 0, room.keys)?.boxed(),
        );
    }
    view.extend([
        sep()?.boxed(),
        add(&LIBRARY)?.boxed(),
        sep()?.boxed(),
        PredefinedMenuItem::fullscreen(app, None)?.boxed(),
    ]);
    let view = Submenu::with_items(app, "View", true, &refs(&view))?;

    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &sep()?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    // On macOS the Help menu holds the system's menu search.
    let help = Submenu::new(app, "Help", true)?;

    #[cfg(target_os = "macos")]
    {
        window.set_as_windows_menu_for_nsapp()?;
        help.set_as_help_menu_for_nsapp()?;
    }

    let mut bar: Vec<Entry<R>> = Vec::new();
    if cfg!(target_os = "macos") {
        bar.push(app_menu(app)?.boxed());
    }
    bar.extend([
        file.boxed(),
        edit.boxed(),
        view.boxed(),
        window.boxed(),
        help.boxed(),
    ]);
    Menu::with_items(app, &refs(&bar))
}

/// The macOS app menu.
fn app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let info = app.package_info();
    let about = tauri::menu::AboutMetadata {
        name: Some(info.name.clone()),
        version: Some(info.version.to_string()),
        // 9.10: these are credited in About.
        credits: Some(CREDITS.into()),
        ..Default::default()
    };
    let sep = || PredefinedMenuItem::separator(app);
    Submenu::with_items(
        app,
        &info.name,
        true,
        &[
            &PredefinedMenuItem::about(app, None, Some(about))?,
            &sep()?,
            &MenuItem::with_id(app, SETTINGS.id, SETTINGS.title, true, SETTINGS.keys)?,
            &sep()?,
            &PredefinedMenuItem::services(app, None)?,
            &sep()?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &sep()?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )
}

/// The licences 9.10 says About credits.
pub const CREDITS: &str =
    "Built with Tauri, wgpu and Rust crates under the MIT or Apache-2.0 licences; \
JUCE 8 under its commercial licence; the VST3 SDK under the MIT licence; \
FFmpeg under the LGPL 2.1 or later, as shared libraries you may replace and re-sign.";

/// A menu entry of any kind, so a submenu's items can be gathered in a list.
type Entry<R> = Box<dyn IsMenuItem<R>>;

trait Boxed<R: Runtime> {
    fn boxed(self) -> Entry<R>;
}

impl<R: Runtime, T: IsMenuItem<R> + 'static> Boxed<R> for T {
    fn boxed(self) -> Entry<R> {
        Box::new(self)
    }
}

fn refs<R: Runtime>(items: &[Entry<R>]) -> Vec<&dyn IsMenuItem<R>> {
    items.iter().map(|i| i.as_ref()).collect()
}

/// Where a click goes.
pub fn on_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let id = event.id().as_ref();
    match action(id) {
        Some(Action::Ui(action)) => {
            bridge::emit(app, "menu", json!({ "action": action }));
            // The View items are check items, which muda flips as one is
            // chosen: ⌘1 in Heat would untick Heat, and ⌘2 would tick two.
            // The tick follows the view the UI says is showing, never the
            // click, so it goes back at once; `shell.room` moves it.
            if ROOMS.iter().any(|r| r.id == id) {
                refresh(app);
            }
        }
        Some(Action::OpenFiles) => crate::open::pick(app),
        Some(Action::Settings) => {
            if let Err(e) = crate::open_settings(app) {
                eprintln!("wi-wwav: the settings window didn't open: {e}");
            }
        }
        None => {}
    }
}

/// `shell.room {room}`: the UI changed views (or opened the library drawer).
/// The View menu ticks it, and the Edit menu shows its undo, asking the core
/// if it hasn't said yet.
pub async fn show_room<R: Runtime>(app: &AppHandle<R>, args: &Value) -> Result<Value, CoreError> {
    let room = args["room"].as_str().unwrap_or_default().to_string();
    if !HISTORY_ROOMS.contains(&room.as_str()) {
        return Err(CoreError::new(
            "bad_args",
            format!("There is no view called '{room}'."),
        ));
    }
    let known = {
        let state = app.state::<Mutex<EditState>>();
        let mut state = state.lock().unwrap();
        state.room = room.clone();
        state.history.contains_key(&room)
    };
    if !known {
        let got = Bridge::invoke(app, "history.get".into(), json!({ "room": room })).await?;
        let history = History::deserialize(&got)
            .map_err(|e| CoreError::new("internal", format!("history.get answered oddly: {e}")))?;
        let state = app.state::<Mutex<EditState>>();
        state
            .lock()
            .unwrap()
            .history
            .entry(room.clone())
            .or_insert(history);
    }
    refresh(app);
    Ok(json!({}))
}

/// At launch, once the core is open: the labels the journal kept for the view
/// the app opens on, so the Edit menu names ⌘Z before anything changes (a
/// label survives a relaunch, 2.7). A `history` event that came first wins.
pub fn load_history<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<Mutex<EditState>>();
    let room = state.lock().unwrap().room.clone();
    let Ok(core) = app.state::<Bridge>().core() else {
        return;
    };
    let Ok(got) = core.invoke("history.get", json!({ "room": room })) else {
        return;
    };
    let Ok(history) = History::deserialize(&got) else {
        return;
    };
    state.lock().unwrap().history.entry(room).or_insert(history);
    refresh(app);
}

/// The core's `history` event: `{room, undo, redo, cant, cantRedo}`.
pub fn history_changed<R: Runtime>(app: &AppHandle<R>, payload: &Value) {
    let (Some(room), Ok(history)) = (payload["room"].as_str(), History::deserialize(payload))
    else {
        return;
    };
    let state = app.state::<Mutex<EditState>>();
    state
        .lock()
        .unwrap()
        .history
        .insert(room.to_string(), history);
    refresh(app);
}

/// Sets Undo, Redo and the View ticks from the state.
fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<Mutex<EditState>>();
    // Held while the menu changes, so two refreshes can't interleave.
    let state = state.lock().unwrap();
    let Some(menu) = app.menu() else { return };
    let history = state.history.get(&state.room).cloned().unwrap_or_default();
    let ((undo, can_undo), (redo, can_redo)) = history.titles();
    for (id, text, enabled) in [(UNDO.id, undo, can_undo), (REDO.id, redo, can_redo)] {
        if let Some(item) = find(&menu, id).as_ref().and_then(MenuItemKind::as_menuitem) {
            let _ = item.set_text(text);
            let _ = item.set_enabled(enabled);
        }
    }
    // The library drawer opens over a view, so the tick stays on that view.
    if state.room == "library" {
        return;
    }
    for segment in &ROOMS {
        if let Some(item) = find(&menu, segment.id)
            .as_ref()
            .and_then(MenuItemKind::as_check_menuitem)
        {
            let _ = item.set_checked(segment.id == format!("room.{}", state.room));
        }
    }
}

/// An item in one of the menu bar's menus (Menu::get looks only at the bar).
fn find<R: Runtime>(menu: &Menu<R>, id: &str) -> Option<MenuItemKind<R>> {
    let menus = menu.items().ok()?;
    menus
        .iter()
        .filter_map(MenuItemKind::as_submenu)
        .find_map(|m| m.get(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(undo: Option<&str>, redo: Option<&str>, cant: Option<&str>) -> History {
        History {
            undo: undo.map(Into::into),
            redo: redo.map(Into::into),
            cant: cant.map(Into::into),
            cant_redo: None,
        }
    }

    #[test]
    fn undo_is_named_after_the_act() {
        let ((undo, on), (redo, redo_on)) =
            h(Some("Undo move clip"), Some("Redo tag clip"), None).titles();
        assert_eq!((undo.as_str(), on), ("Undo move clip", true));
        assert_eq!((redo.as_str(), redo_on), ("Redo tag clip", true));
    }

    #[test]
    fn work_that_left_the_machine_greys_undo_out_and_says_so() {
        let ((undo, on), _) =
            h(Some("Undo move clip"), None, Some("Can't undo a purchase.")).titles();
        assert_eq!((undo.as_str(), on), ("Can't undo a purchase.", false));
    }

    #[test]
    fn a_held_redo_is_grey_and_says_why() {
        let held = History {
            cant_redo: Some("Can't redo a publish.".into()),
            ..History::default()
        };
        let (_, (redo, on)) = held.titles();
        assert_eq!((redo.as_str(), on), ("Can't redo a publish.", false));
    }

    #[test]
    fn with_nothing_to_undo_both_are_plain_and_grey() {
        let ((undo, on), (redo, redo_on)) = History::default().titles();
        assert_eq!(
            (undo.as_str(), on, redo.as_str(), redo_on),
            ("Undo", false, "Redo", false)
        );
    }

    #[test]
    fn the_views_are_on_cmd_1_to_3_in_the_switchers_order() {
        let got: Vec<_> = ROOMS.iter().map(|r| (r.title, r.keys.unwrap())).collect();
        assert_eq!(
            got,
            [
                ("Heat", "CmdOrCtrl+1"),
                ("Space", "CmdOrCtrl+2"),
                ("Console", "CmdOrCtrl+3"),
            ]
        );
        assert!(
            !HISTORY_ROOMS.contains(&"unquantized") && action("room.unquantized").is_none(),
            "there is no fourth view, and no Cmd+4"
        );
    }

    #[test]
    fn the_keys_are_the_specs() {
        let keys = |i: &Item| (i.title, i.keys);
        assert_eq!(keys(&NEW_SESSION), ("New session", Some("CmdOrCtrl+N")));
        assert_eq!(keys(&OPEN), ("Open…", Some("CmdOrCtrl+O")));
        assert_eq!(
            keys(&EXPORT),
            ("Export everything…", Some("CmdOrCtrl+Shift+E"))
        );
        assert_eq!(keys(&UNDO), ("Undo", Some("CmdOrCtrl+Z")));
        assert_eq!(keys(&REDO), ("Redo", Some("CmdOrCtrl+Shift+Z")));
        assert_eq!(keys(&LIBRARY), ("Library", Some("CmdOrCtrl+L")));
        assert_eq!(keys(&SETTINGS), ("Settings…", Some("CmdOrCtrl+,")));
    }

    /// The Edit menu and the View ticks on Tauri's mock runtime, with a core
    /// that knows only the Console's history.
    mod live {
        use std::sync::Arc;

        use serde_json::{json, Value};
        use tauri::menu::MenuItemKind;
        use tauri::test::{mock_builder, MockRuntime};
        use tauri::{App, Manager};

        use super::super::*;
        use crate::bridge::Core;

        struct ConsoleHistory;

        impl Core for ConsoleHistory {
            fn invoke(&self, cmd: &str, args: Value) -> Result<Value, CoreError> {
                assert_eq!((cmd, &args), ("history.get", &json!({ "room": "console" })));
                Ok(json!({ "undo": null, "redo": "Redo cut at playhead", "cant": null }))
            }
        }

        fn app() -> App<MockRuntime> {
            let app = crate::shell(mock_builder())
                .build(tauri::generate_context!())
                .unwrap();
            app.set_menu(build(app.handle()).unwrap()).unwrap();
            app.state::<Bridge>().set_core(Ok(Arc::new(ConsoleHistory)));
            app
        }

        fn item(app: &App<MockRuntime>, id: &str) -> (String, bool) {
            match find(&app.menu().unwrap(), id) {
                Some(MenuItemKind::MenuItem(i)) => (i.text().unwrap(), i.is_enabled().unwrap()),
                _ => panic!("{id} is missing or of another kind"),
            }
        }

        fn ticked(app: &App<MockRuntime>) -> Vec<&'static str> {
            let menu = app.menu().unwrap();
            let checked = |id| match find(&menu, id) {
                Some(MenuItemKind::Check(c)) => c.is_checked().unwrap(),
                _ => panic!("{id} is missing or of another kind"),
            };
            ROOMS
                .iter()
                .filter(|r| checked(r.id))
                .map(|r| r.title)
                .collect()
        }

        fn show(app: &App<MockRuntime>, room: &str) -> Result<Value, CoreError> {
            tauri::async_runtime::block_on(show_room(app.handle(), &json!({ "room": room })))
        }

        #[test]
        fn the_edit_menu_names_the_current_rooms_undo() {
            let app = app();
            assert_eq!(item(&app, UNDO.id), ("Undo".into(), false));
            assert_eq!(ticked(&app), ["Heat"]);

            let heat =
                json!({ "room": "heat", "undo": "Undo mark done", "redo": null, "cant": null });
            let space = json!({ "room": "space", "undo": null, "redo": null, "cant": "Can't undo a publish. Unpublish 'World Ending'…" });
            crate::bridge::forward_event(app.handle(), "history", heat);
            crate::bridge::forward_event(app.handle(), "history", space);
            assert_eq!(
                item(&app, UNDO.id),
                ("Undo mark done".into(), true),
                "Space's history doesn't show in Heat"
            );
            assert_eq!(item(&app, REDO.id), ("Redo".into(), false));

            show(&app, "space").unwrap();
            assert_eq!(
                item(&app, UNDO.id),
                (
                    "Can't undo a publish. Unpublish 'World Ending'…".into(),
                    false
                )
            );
            assert_eq!(ticked(&app), ["Space"]);

            // A room the core hasn't spoken about yet is asked.
            show(&app, "console").unwrap();
            assert_eq!(item(&app, UNDO.id), ("Undo".into(), false));
            assert_eq!(item(&app, REDO.id), ("Redo cut at playhead".into(), true));
            assert_eq!(ticked(&app), ["Console"]);
        }

        #[test]
        fn the_library_drawer_keeps_the_rooms_tick() {
            let app = app();
            crate::bridge::forward_event(
                app.handle(),
                "history",
                json!({ "room": "library", "undo": "Undo tag clip" }),
            );
            show(&app, "library").unwrap();
            assert_eq!(item(&app, UNDO.id), ("Undo tag clip".into(), true));
            assert_eq!(ticked(&app), ["Heat"]);
        }

        #[test]
        fn an_unknown_room_is_refused() {
            let app = app();
            assert_eq!(show(&app, "kitchen").unwrap_err().code, "bad_args");
        }
    }

    #[test]
    fn every_item_has_an_action_and_only_the_shells_stay_in_the_shell() {
        for room in &ROOMS {
            assert_eq!(action(room.id), Some(Action::Ui(room.id)));
        }
        for i in [&NEW_SESSION, &EXPORT, &UNDO, &REDO, &LIBRARY] {
            assert_eq!(action(i.id), Some(Action::Ui(i.id)), "{}", i.id);
        }
        assert_eq!(action(OPEN.id), Some(Action::OpenFiles));
        assert_eq!(action(SETTINGS.id), Some(Action::Settings));
        assert_eq!(action("about"), None);
    }
}
