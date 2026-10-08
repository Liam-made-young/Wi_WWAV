//! Space's pages (docs/SPACE.md 11): a link opened in Space is the real
//! page, in a web view of its own laid over the sky where its body is.
//!
//! There is one such web view, `space-page`, a child of the main window. It
//! is in no capability file, so the page in it can call nothing of the
//! app's (docs/SPEC.md 9.8); it is the one web view the fence lets leave the
//! app, and only to http and https. Its cookies are the app's own store on
//! this Mac, which lasts between launches: sign in once and stay signed in.
//!
//! The UI moves it with four `core` cmds the shell answers itself:
//!
//! - `space.page.open {url, x, y, width, height, zoom?}`: go to a link, at a
//!   place in the window (points from its top left).
//! - `space.page.place {x, y, width, height, zoom, shown}`: follow the body
//!   as you fly. `zoom` scales the page with its size, so it shrinks into
//!   the distance instead of folding up.
//! - `space.page.dock {docked}`: you are in the page (it has the keyboard
//!   and its own scrolling), or back out in the sky.
//! - `space.page.close`, `.back`, `.forward`, `.reload`.
//!
//! And one that moves you instead of the page: `space.fly {url, enter?}`
//! tells the sky to fly you to the body that is that link, and into its page
//! when `enter` is true. It is how a search will move you to its results.
//!
//! The page talks back by one road: a script put in every page asks to go to
//! `wwavspace://say/?<json>`, which is never a place; the request is read
//! and refused. What arrives is passed to the UI as the `space.page` event,
//! as the page wrote it, and is never trusted for more than that.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::OnceLock;

use serde_json::{json, Value};
use tauri::webview::{PageLoadEvent, PageLoadPayload, WebviewBuilder};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, Url, Webview, WebviewUrl};

use wi_core::space::{fit, said, youtube_id};

use crate::bridge::{self, CoreError};

/// The web view's label. No capability names it.
pub const LABEL: &str = "space-page";

/// The scheme a page "goes to" in order to say something.
const SAY: &str = "wwavspace";

/// What Safari on this Mac calls itself. WebKit in an app leaves out the
/// last two words, and some sites then serve a lesser page.
const SAFARI: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
                      (KHTML, like Gecko) Version/26.0 Safari/605.1.15";

/// Put in every page before its own scripts. Until you are in the page it is
/// part of the sky: scrolling and pinching over it move you, and a click
/// goes in. Once you are in, the page has its keys and its scrolling, and
/// Esc or a pinch outward asks to leave.
const PAGE_JS: &str = include_str!("space_page.js");

/// The page that holds YouTube's player. The player refuses a page with no
/// web address of its own ("Error 153"), so it is given one on this Mac.
const YOUTUBE_HTML: &str = include_str!("space_youtube.html");

fn bad(message: impl Into<String>) -> CoreError {
    CoreError::new("bad_args", message)
}

fn failed(what: &str, e: impl std::fmt::Display) -> CoreError {
    CoreError::new("internal", format!("Space couldn't {what}: {e}"))
}

/// The four numbers of a place in the window.
fn rect(args: &Value) -> Result<(LogicalPosition<f64>, LogicalSize<f64>), CoreError> {
    let n = |key: &str| {
        args[key]
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| bad(format!("A page's place needs a number for {key}.")))
    };
    let (width, height) = (n("width")?.max(1.0), n("height")?.max(1.0));
    Ok((
        LogicalPosition::new(n("x")?, n("y")?),
        LogicalSize::new(width, height),
    ))
}

/// Starts the page that holds YouTube's player, once, and says its port. It
/// answers this Mac only, and only with that one page.
fn holder() -> Option<u16> {
    static PORT: OnceLock<Option<u16>> = OnceLock::new();
    *PORT.get_or_init(|| {
        let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
        let port = listener.local_addr().ok()?.port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || answer(stream));
            }
        });
        Some(port)
    })
}

fn answer(mut stream: TcpStream) {
    let mut first = String::new();
    {
        let mut reader = BufReader::new(&stream);
        if reader.read_line(&mut first).is_err() {
            return;
        }
        // The rest of the request is read and dropped, so the browser
        // doesn't see the line close on it.
        let mut line = String::new();
        while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
            line.clear();
        }
    }
    let path = first.split_whitespace().nth(1).unwrap_or_default();
    let (status, body) = match held_page(path) {
        Some(page) => ("200 OK", page),
        None => ("404 Not Found", String::from("Not here.")),
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nReferrer-Policy: strict-origin-when-cross-origin\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
}

/// The holder page for a request's path, or nothing for any other path.
fn held_page(path: &str) -> Option<String> {
    let url = Url::parse(&format!("http://127.0.0.1{path}")).ok()?;
    if url.path() != "/youtube" {
        return None;
    }
    let watch = format!("https://www.youtube.com/watch?{}", url.query()?);
    let id = youtube_id(&Url::parse(&watch).ok()?)?;
    Some(YOUTUBE_HTML.replace("__ID__", &id))
}

fn page<R: Runtime>(app: &AppHandle<R>) -> Result<Webview<R>, CoreError> {
    app.get_webview(LABEL)
        .ok_or_else(|| CoreError::new("no_page", "No page is open in Space."))
}

fn open<R: Runtime>(app: &AppHandle<R>, args: &Value) -> Result<Value, CoreError> {
    let link = args["url"]
        .as_str()
        .and_then(|s| Url::parse(s).ok())
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .ok_or_else(|| bad("Space opens web pages: an http or https address."))?;
    let load = fit(&link, holder());
    let (at, size) = rect(args)?;
    let zoom = args["zoom"].as_f64().unwrap_or(1.0).clamp(0.05, 4.0);
    match app.get_webview(LABEL) {
        Some(view) => {
            view.navigate(load.clone())
                .map_err(|e| failed("go to that page", e))?;
            let _ = view.set_position(at);
            let _ = view.set_size(size);
            let _ = view.set_zoom(zoom);
            let _ = view.show();
        }
        None => {
            let window = app
                .get_window("main")
                .ok_or_else(|| CoreError::new("closing", "Wi_WWAV is closing."))?;
            let builder = WebviewBuilder::new(LABEL, WebviewUrl::External(load.clone()))
                .initialization_script(PAGE_JS)
                .user_agent(SAFARI);
            let view = window
                .add_child(builder, at, size)
                .map_err(|e| failed("open a page", e))?;
            let _ = view.set_zoom(zoom);
        }
    }
    Ok(json!({ "loading": load.as_str() }))
}

fn place<R: Runtime>(app: &AppHandle<R>, args: &Value) -> Result<Value, CoreError> {
    let view = page(app)?;
    if args["shown"].as_bool() == Some(false) {
        let _ = view.hide();
        return Ok(json!({}));
    }
    let (at, size) = rect(args)?;
    let _ = view.set_position(at);
    let _ = view.set_size(size);
    if let Some(zoom) = args["zoom"].as_f64() {
        let _ = view.set_zoom(zoom.clamp(0.05, 4.0));
    }
    let _ = view.show();
    Ok(json!({}))
}

fn dock<R: Runtime>(app: &AppHandle<R>, args: &Value) -> Result<Value, CoreError> {
    let view = page(app)?;
    let docked = args["docked"].as_bool().unwrap_or(false);
    let _ = view.eval(format!(
        "window.__wwavSpace && window.__wwavSpace.dock({docked})"
    ));
    // The keyboard goes with you: into the page, or back to the sky.
    if docked {
        let _ = view.set_focus();
    } else if let Some(main) = app.get_webview("main") {
        let _ = main.set_focus();
    }
    Ok(json!({}))
}

fn close<R: Runtime>(app: &AppHandle<R>) -> Result<Value, CoreError> {
    if let Some(view) = app.get_webview(LABEL) {
        let _ = view.hide();
        // A page left behind stops playing.
        let _ = view.navigate(Url::parse("about:blank").expect("a fixed address"));
        if let Some(main) = app.get_webview("main") {
            let _ = main.set_focus();
        }
    }
    Ok(json!({}))
}

fn eval<R: Runtime>(app: &AppHandle<R>, script: &str) -> Result<Value, CoreError> {
    page(app)?
        .eval(script)
        .map_err(|e| failed("reach the page", e))?;
    Ok(json!({}))
}

/// Answers the `space.page.*` cmds.
pub fn command<R: Runtime>(
    app: &AppHandle<R>,
    cmd: &str,
    args: &Value,
) -> Result<Value, CoreError> {
    match cmd {
        "space.page.open" => open(app, args),
        "space.page.place" => place(app, args),
        "space.page.dock" => dock(app, args),
        "space.page.close" => close(app),
        "space.page.back" => eval(app, "history.back()"),
        "space.page.forward" => eval(app, "history.forward()"),
        "space.page.reload" => eval(app, "location.reload()"),
        "space.fly" => {
            let url = args["url"]
                .as_str()
                .ok_or_else(|| bad("space.fly needs the link to fly to."))?;
            let enter = args["enter"].as_bool().unwrap_or(false);
            bridge::emit(app, "space.fly", json!({ "url": url, "enter": enter }));
            Ok(json!({}))
        }
        _ => Err(bad(format!("Space has no command called {cmd}."))),
    }
}

/// Whether the page web view may go to `url`. The fence asks this for
/// `space-page` in place of its own rule. What a page says is heard here and
/// refused, so it never leaves the page it was on.
pub fn may_go<R: Runtime>(webview: &Webview<R>, url: &Url) -> bool {
    match url.scheme() {
        "http" | "https" => true,
        "about" => url.as_str() == "about:blank",
        SAY => {
            for line in said(url) {
                proof::heard(&line);
                bridge::emit(
                    webview.app_handle(),
                    "space.page",
                    json!({ "what": "said", "said": line }),
                );
            }
            false
        }
        // A page can't start another app (mailto:, itms:) or read a file.
        _ => false,
    }
}

/// Tells the UI a page began or finished loading.
pub fn page_load<R: Runtime>(webview: &Webview<R>, load: &PageLoadPayload<'_>) {
    let what = match load.event() {
        PageLoadEvent::Started => "loading",
        PageLoadEvent::Finished => "loaded",
    };
    let url = load.url().as_str();
    proof::heard(&json!({ "load": what, "url": url }));
    bridge::emit(
        webview.app_handle(),
        "space.page",
        json!({ "what": what, "url": url }),
    );
}

/// The proof that pages work in this app (docs/SPACE.md 10, step 1). With
/// `WI_WWAV_SPACE_PROOF` naming a file, the app opens each kind of link in
/// turn, asks the page what happened, writes the answers there and quits.
/// Nobody's sign-in is used: a cookie of the proof's own shows whether one
/// would last between launches.
pub mod proof {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use serde_json::{json, Value};
    use tauri::{AppHandle, Runtime};

    static HEARD: Mutex<Option<Vec<Value>>> = Mutex::new(None);

    /// Asks a page, once it has loaded, to press its play button and say
    /// what is playing, what it holds, and whether it remembers the last
    /// launch.
    const PROBE_JS: &str = include_str!("space_probe.js");

    /// A name, a link, and whether it is a player whose play button is pressed.
    const CASES: [(&str, &str, bool); 6] = [
        ("youtube", "https://www.youtube.com/watch?v=jNQXAC9IVRw", true),
        (
            "spotify",
            "https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT",
            true,
        ),
        (
            "apple-music",
            "https://music.apple.com/us/album/never-gonna-give-you-up-escape-to-new-york-mix/1612648318?i=1612648440",
            true,
        ),
        ("wikipedia", "https://en.wikipedia.org/wiki/Saturn", false),
        ("x", "https://x.com/NASA", false),
        ("duckduckgo", "https://duckduckgo.com/?q=saturn", false),
    ];

    pub fn heard(line: &Value) {
        if let Some(lines) = HEARD.lock().unwrap().as_mut() {
            lines.push(line.clone());
        }
    }

    fn take() -> Vec<Value> {
        HEARD
            .lock()
            .unwrap()
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    fn wait_for(seconds: u64, done: impl Fn(&[Value]) -> bool) -> bool {
        let until = Instant::now() + Duration::from_secs(seconds);
        while Instant::now() < until {
            let lines = HEARD.lock().unwrap();
            if done(lines.as_deref().unwrap_or_default()) {
                return true;
            }
            drop(lines);
            std::thread::sleep(Duration::from_millis(150));
        }
        false
    }

    /// Runs once, after the main window's page has loaded.
    pub fn start<R: Runtime>(app: &AppHandle<R>) {
        let Some(out) = std::env::var_os("WI_WWAV_SPACE_PROOF") else {
            return;
        };
        {
            let mut heard = HEARD.lock().unwrap();
            if heard.is_some() {
                return;
            }
            *heard = Some(Vec::new());
        }
        let app = app.clone();
        std::thread::spawn(move || {
            let mut results = Vec::new();
            for (name, link, press) in CASES {
                take();
                let place = json!({ "url": link, "x": 160, "y": 110, "width": 960, "height": 600 });
                let opened = super::open(&app, &place);
                let loaded = wait_for(25, |lines| lines.iter().any(|l| l["load"] == "loaded"));
                let asked = json!({ "name": name, "press": press });
                let _ = super::eval(&app, &format!("window.__wwavProof = {asked}; {PROBE_JS}"));
                let finished = wait_for(30, |lines| {
                    lines
                        .iter()
                        .any(|l| l["proof"]["at"] == "end" && l["proof"]["name"] == name)
                });
                results.push(json!({
                    "case": name,
                    "link": link,
                    "opened": opened.map_err(|e| e.message).unwrap_or_else(Value::String),
                    "loaded": loaded,
                    "finished": finished,
                    "heard": take(),
                }));
            }
            let _ = super::close(&app);
            let text = serde_json::to_string_pretty(&results).unwrap_or_default();
            if let Err(e) = std::fs::write(&out, text) {
                eprintln!("wi-wwav: the Space proof couldn't be written: {e}");
            }
            app.exit(0);
        });
    }

    /// The same for the eye. With `WI_WWAV_SPACE_TOUR` naming a folder, the
    /// app shows Space, flies to a video, goes into it, comes out and goes
    /// into an article, taking a picture of the screen at each stop, and
    /// quits. The pictures show the whole screen, so they are for the person
    /// at this Mac and nobody else.
    pub fn tour<R: Runtime>(app: &AppHandle<R>) {
        let Some(folder) = std::env::var_os("WI_WWAV_SPACE_TOUR") else {
            return;
        };
        static STARTED: Mutex<bool> = Mutex::new(false);
        if std::mem::replace(&mut *STARTED.lock().unwrap(), true) {
            return;
        }
        let app = app.clone();
        let folder = std::path::PathBuf::from(folder);
        std::thread::spawn(move || {
            let pause = |seconds: u64| std::thread::sleep(Duration::from_secs(seconds));
            let picture = |name: &str| {
                let _ = std::process::Command::new("screencapture")
                    .args(["-x", "-t", "jpg"])
                    .arg(folder.join(format!("{name}.jpg")))
                    .status();
            };
            let fly = |url: &str, enter: bool| {
                let _ = super::command(&app, "space.fly", &json!({ "url": url, "enter": enter }));
            };
            let video = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
            let article = "https://en.wikipedia.org/wiki/Saturn";
            pause(3);
            if let Some(window) = tauri::Manager::get_window(&app, "main") {
                let _ = window.set_focus();
            }
            // A library nobody has opened before greets you first; the tour has seen it.
            if let Some(main) = tauri::Manager::get_webview(&app, "main") {
                let _ = main.eval(
                    "if (!localStorage.getItem('wi.firstLaunch')) { localStorage.setItem('wi.firstLaunch', 'done'); location.reload(); }",
                );
            }
            pause(4);
            crate::bridge::emit(&app, "menu", json!({ "action": "room.space" }));
            pause(3);
            picture("1-the-sky");
            fly(video, false);
            pause(5);
            picture("2-near-a-video");
            fly(video, true);
            pause(5);
            picture("3-in-the-video");
            fly(article, false);
            pause(6);
            picture("4-near-an-article");
            fly(article, true);
            pause(6);
            picture("5-in-the-article");
            app.exit(0);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_holder_serves_one_page_and_only_for_a_plain_id() {
        let page = held_page("/youtube?v=jNQXAC9IVRw").unwrap();
        assert!(page.contains("jNQXAC9IVRw") && !page.contains("__ID__"));
        for path in [
            "/",
            "/youtube",
            "/youtube?v=",
            "/youtube?v=%22%3E%3Cscript%3E",
            "/other?v=jNQXAC9IVRw",
            "/../etc/passwd",
        ] {
            assert!(held_page(path).is_none(), "{path}");
        }
    }
}
