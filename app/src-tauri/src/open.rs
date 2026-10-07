//! Files opened from outside: a double-click in Finder or a file manager, a
//! drop on the Dock icon, a path on the command line, or File → Open….
//!
//! Songs and films come into the library through the core's
//! `library.import`, and the main window hears `open` `{clips}` so it can
//! show them. A `.wwavsession` is the Console's to open, so the main window
//! hears `open` `{sessions}` with its path. When the import fails, `open`
//! carries `{error: {code, message}}` to show as is. A file that opens the
//! app arrives before the page can hear, so it waits for the page to load.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::DialogExt;

use crate::bridge::{self, Bridge};

/// The kinds Wi_WWAV opens, which its file associations declare.
pub const EXTENSIONS: [&str; 3] = ["wwav", "swav", "wwavsession"];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Opened {
    pub media: Vec<PathBuf>,
    pub sessions: Vec<PathBuf>,
}

/// Sorts paths into media and sessions; anything else is ignored, since the
/// system only hands over the kinds the app declared.
pub fn sort(paths: impl IntoIterator<Item = PathBuf>) -> Opened {
    let mut opened = Opened::default();
    for path in paths {
        match extension(&path).as_deref() {
            Some("wwav" | "swav") => opened.media.push(path),
            Some("wwavsession") => opened.sessions.push(path),
            _ => {}
        }
    }
    opened
}

fn extension(path: &Path) -> Option<String> {
    Some(path.extension()?.to_str()?.to_ascii_lowercase())
}

/// The undo label for an import: "Undo open World Ending.wwav".
pub fn label(media: &[PathBuf]) -> String {
    match media {
        [one] => format!(
            "open {}",
            one.file_name().unwrap_or_default().to_string_lossy()
        ),
        many => format!("open {} files", many.len()),
    }
}

/// What was opened before the main window's page loaded; `None` once it has.
pub struct Waiting(Mutex<Option<Vec<Value>>>);

impl Default for Waiting {
    fn default() -> Self {
        Self(Mutex::new(Some(Vec::new())))
    }
}

/// Tells the main window something opened, or keeps it until the page loads.
fn tell<R: Runtime>(app: &AppHandle<R>, payload: Value) {
    let waiting = app.state::<Waiting>();
    let mut waiting = waiting.0.lock().unwrap();
    match waiting.as_mut() {
        Some(queue) => queue.push(payload),
        None => bridge::emit(app, "open", payload),
    }
}

/// The main window's page has loaded: it hears what waited.
pub fn page_loaded<R: Runtime>(app: &AppHandle<R>) {
    let waited = app.state::<Waiting>().0.lock().unwrap().take();
    for payload in waited.unwrap_or_default() {
        bridge::emit(app, "open", payload);
    }
}

pub fn route<R: Runtime>(app: &AppHandle<R>, paths: impl IntoIterator<Item = PathBuf>) {
    let Opened { media, sessions } = sort(paths);
    if !sessions.is_empty() {
        tell(app, json!({ "sessions": sessions }));
    }
    if media.is_empty() {
        return;
    }
    focus_main(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let args = json!({ "paths": media, "label": label(&media) });
        let payload = match Bridge::invoke(&app, "library.import".into(), args).await {
            Ok(result) => json!({ "clips": result.get("clips").cloned().unwrap_or(Value::Null) }),
            Err(error) => json!({ "error": error }),
        };
        tell(&app, payload);
    });
}

/// File → Open…: the system's picker, limited to the kinds the app opens.
pub fn pick<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    app.dialog()
        .file()
        .add_filter("Songs, films and sessions", &EXTENSIONS)
        .pick_files(move |files| {
            let paths = files
                .unwrap_or_default()
                .into_iter()
                .filter_map(|f| f.into_path().ok());
            route(&handle, paths);
        });
}

/// Paths given on the command line, as Linux and Windows open a file with
/// the app (on macOS they arrive as an event instead).
pub fn from_args(args: impl IntoIterator<Item = String>) -> Vec<PathBuf> {
    args.into_iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .map(PathBuf::from)
        .collect()
}

fn focus_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn songs_and_films_are_media_and_sessions_are_the_consoles() {
        let opened = sort(
            [
                "/a/World Ending.wwav",
                "/a/clip.SWAV",
                "/a/Night.wwavsession",
                "/a/notes.txt",
                "/a/x",
            ]
            .map(PathBuf::from),
        );
        assert_eq!(
            opened.media,
            ["/a/World Ending.wwav", "/a/clip.SWAV"].map(PathBuf::from)
        );
        assert_eq!(opened.sessions, [PathBuf::from("/a/Night.wwavsession")]);
    }

    /// Fails if a file opened before the page has loaded (a launch by
    /// double-click) is never heard of, or one opened after waits.
    #[test]
    fn what_opens_before_the_page_loads_waits_for_it() {
        use std::sync::mpsc;
        use tauri::test::mock_builder;
        use tauri::Listener;

        let app = crate::shell(mock_builder())
            .build(tauri::generate_context!())
            .unwrap();
        let (tx, heard) = mpsc::channel();
        app.listen_any("core", move |e| {
            let _ = tx.send(e.payload().to_string());
        });
        tell(
            app.handle(),
            json!({ "sessions": ["/a/Night.wwavsession"] }),
        );
        assert!(
            heard.try_recv().is_err(),
            "nothing is said before the page can hear it"
        );
        page_loaded(app.handle());
        let said: Value = serde_json::from_str(&heard.try_recv().unwrap()).unwrap();
        assert_eq!(
            said,
            json!({ "event": "open", "payload": { "sessions": ["/a/Night.wwavsession"] } })
        );
        tell(app.handle(), json!({ "clips": [] }));
        assert!(
            heard.try_recv().is_ok(),
            "once loaded, the page hears at once"
        );
    }

    #[test]
    fn the_label_names_one_file_or_counts_several() {
        assert_eq!(
            label(&[PathBuf::from("/a/World Ending.wwav")]),
            "open World Ending.wwav"
        );
        assert_eq!(
            label(&[PathBuf::from("/a/1.wwav"), PathBuf::from("/a/2.swav")]),
            "open 2 files"
        );
    }

    #[test]
    fn the_command_line_gives_paths_but_not_the_program_or_flags() {
        let args = ["wi-wwav", "--flag", "/a/World Ending.wwav"].map(String::from);
        assert_eq!(from_args(args), [PathBuf::from("/a/World Ending.wwav")]);
    }
}
