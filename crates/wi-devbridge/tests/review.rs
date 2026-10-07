//! An independent reviewer's adversarial test of the dev bridge. It exposed a
//! finding, ran ignored until it was fixed, and runs now.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use tungstenite::client::IntoClientRequest;
use tungstenite::Message;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Finding: the bridge accepts a WebSocket from any web page. Browsers
/// don't apply the same-origin policy to WebSockets, so any site open in
/// the developer's browser can reach ws://127.0.0.1:8790 and run every
/// command of the core: import any file it names (`library.import
/// {paths: ["/home/me/.ssh"]}`), drop it on a system so the upload queue
/// sends it to the signed-in account, export the library to any path, or
/// sign out. The handshake's Origin is never checked.
#[test]
fn review_a_page_from_another_origin_cant_drive_the_core() {
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

    let mut request = url.as_str().into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "https://evil.example".parse().unwrap());
    let outcome = match tungstenite::connect(request) {
        Err(e) => Err(e.to_string()),
        Ok((mut ws, _)) => {
            let secret = dir.path().join("passwords.txt");
            std::fs::write(&secret, "bank: hunter2\n").unwrap();
            ws.send(Message::text(
                json!({"id": 1, "cmd": "library.import", "args": {"paths": [secret], "label": "import"}})
                    .to_string(),
            ))
            .unwrap();
            loop {
                if let Message::Text(t) = ws.read().unwrap() {
                    let v: Value = serde_json::from_str(&t).unwrap();
                    if v["id"] == 1 {
                        break Ok(v);
                    }
                }
            }
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    match outcome {
        Err(refused) => assert!(
            refused.contains("403"),
            "refused, but not with a 403: {refused}"
        ),
        Ok(answer) => {
            panic!("a page on https://evil.example imported a file through the bridge: {answer}")
        }
    }
}
