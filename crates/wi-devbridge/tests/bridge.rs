//! The dev bridge end to end: the binary, a real core behind it, and
//! mock-engine behind that, driven over a WebSocket as the web UI drives
//! it. What a fail looks like: a command's answer doesn't carry its id, an
//! error isn't `{code, message}`, events don't arrive as `{event,
//! payload}`, or meters don't arrive as binary frames while a song plays.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::client::IntoClientRequest;
use tungstenite::{stream::MaybeTlsStream, Message, WebSocket};

type Socket = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

struct Bridge {
    child: Child,
    url: String,
    _dir: tempfile::TempDir,
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start() -> Bridge {
    // The bridge's default engine is the mock-engine beside it.
    let built = Command::new(env!("CARGO"))
        .args(["build", "-q", "-p", "mock-engine", "--bin", "mock-engine"])
        .current_dir(workspace())
        .status()
        .unwrap();
    assert!(built.success());
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_wi-devbridge"))
        .arg("--library")
        .arg(dir.path().join("Wi_WWAV"))
        .args(["--port", "0", "--server", "http://127.0.0.1:9"])
        .env("TMPDIR", dir.path())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let url = line
        .trim()
        .strip_prefix("wi-devbridge listening ")
        .unwrap()
        .to_string();
    Bridge {
        child,
        url,
        _dir: dir,
    }
}

fn send(ws: &mut Socket, id: u64, cmd: &str, args: Value) {
    ws.send(Message::text(
        json!({"id": id, "cmd": cmd, "args": args}).to_string(),
    ))
    .unwrap();
}

/// Reads frames until the answer to `id`, keeping events and meters.
fn answer(ws: &mut Socket, id: u64, seen: &mut Vec<Value>, meters: &mut usize) -> Value {
    loop {
        match ws.read().unwrap() {
            Message::Text(t) => {
                let v: Value = serde_json::from_str(&t).unwrap();
                if v["id"] == id {
                    return v;
                }
                seen.push(v);
            }
            Message::Binary(_) => *meters += 1,
            _ => {}
        }
    }
}

#[test]
fn commands_events_and_meters_cross_the_socket() {
    let bridge = start();
    let (mut ws, _) = tungstenite::connect(&bridge.url).unwrap();
    let (mut seen, mut meters) = (Vec::new(), 0);

    send(&mut ws, 1, "app.hello", json!({}));
    let hello = answer(&mut ws, 1, &mut seen, &mut meters);
    assert_eq!(hello["ok"], true);
    assert_eq!(hello["result"]["signedIn"], false);

    send(&mut ws, 2, "no.such", json!({}));
    let refused = answer(&mut ws, 2, &mut seen, &mut meters);
    assert_eq!(
        refused,
        json!({"id": 2, "ok": false, "error": {"code": "unknown_command", "message": "There is no command called 'no.such'."}})
    );

    let song = workspace().join("tests/corpus/original.wwav");
    send(
        &mut ws,
        3,
        "library.import",
        json!({"paths": [song], "label": "import"}),
    );
    let imported = answer(&mut ws, 3, &mut seen, &mut meters);
    let clip = imported["result"]["clips"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        seen.iter()
            .any(|e| e["event"] == "library.import" && e["payload"]["total"] == 1),
        "{seen:?}"
    );
    assert!(seen
        .iter()
        .any(|e| e["event"] == "history" && e["payload"]["undo"] == "Undo import"));

    send(&mut ws, 4, "player.load", json!({"clip": clip}));
    assert_eq!(answer(&mut ws, 4, &mut seen, &mut meters)["ok"], true);
    send(&mut ws, 5, "player.play", json!({}));
    assert_eq!(
        answer(&mut ws, 5, &mut seen, &mut meters)["result"]["state"]["playing"],
        true
    );

    // Meters as raw bytes, the newest entry: 32 bytes, then 16 a slot.
    let began = Instant::now();
    let mut frame = None;
    while began.elapsed() < Duration::from_secs(5) && frame.is_none() {
        if let Message::Binary(b) = ws.read().unwrap() {
            frame = Some(b);
        }
    }
    let frame = frame.expect("no meters while playing");
    let slots = u32::from_le_bytes(frame[16..20].try_into().unwrap()) as usize;
    assert_eq!(slots, 4 + 4 + 1);
    assert_eq!(frame.len(), 32 + slots * 16);

    // A clock event, ten times a second, while it plays.
    let began = Instant::now();
    let mut clock = None;
    while began.elapsed() < Duration::from_secs(5) && clock.is_none() {
        if let Message::Text(t) = ws.read().unwrap() {
            let v: Value = serde_json::from_str(&t).unwrap();
            if v["event"] == "clock" {
                clock = Some(v["payload"].clone());
            }
        }
    }
    assert_eq!(clock.expect("a clock event")["state"], "playing");
}

/// Browsers don't hold a WebSocket to the same-origin policy, so the bridge
/// checks the handshake's Origin itself: a page from this machine (the Vite
/// server, Playwright) connects, a page from anywhere else is refused with a
/// 403 before it is a socket, and a client with no Origin at all (Node, a
/// terminal) isn't a web page.
#[test]
fn only_pages_from_this_machine_are_let_in() {
    let bridge = start();
    let connect = |origin: Option<&str>| {
        let mut request = bridge.url.as_str().into_client_request().unwrap();
        if let Some(o) = origin {
            request.headers_mut().insert("Origin", o.parse().unwrap());
        }
        tungstenite::connect(request).map_err(Box::new)
    };
    for local in [
        None,
        Some("http://localhost:5173"),
        Some("http://127.0.0.1:5173"),
    ] {
        let (mut ws, _) = connect(local).unwrap_or_else(|e| panic!("{local:?} was refused: {e}"));
        send(&mut ws, 1, "app.hello", json!({}));
        let hello = answer(&mut ws, 1, &mut Vec::new(), &mut 0);
        assert_eq!(hello["ok"], true, "{local:?}");
    }
    for other in [
        "https://evil.example",
        "http://localhost.evil.example",
        "null",
    ] {
        let refused = connect(Some(other)).err().unwrap_or_else(|| {
            panic!("a page from {other} connected");
        });
        assert!(
            matches!(&*refused, tungstenite::Error::Http(r) if r.status() == 403),
            "{other}: {refused}"
        );
    }
}
