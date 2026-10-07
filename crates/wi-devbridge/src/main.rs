//! wi-devbridge: the app's core over a WebSocket, so the web UI and
//! Playwright drive the real core, with mock-engine and tools/mock-server
//! behind it, from a browser (docs/COMMANDS.md).
//!
//! ```text
//! wi-devbridge --library DIR [--engine PATH] [--server URL] [--port N]
//! ```
//!
//! - A text frame `{id, cmd, args}` is one command; its answer is
//!   `{id, ok: true, result}` or `{id, ok: false, error: {code, message}}`.
//!   Commands run side by side, so a sign-in waiting on the browser holds
//!   up nothing else, and answers may come in any order.
//! - Events go out as text frames `{event, payload}`.
//! - Meters go out as binary frames: the newest meter entry, at most once
//!   a frame.
//!
//! `--engine` defaults to the workspace's mock-engine beside this binary,
//! on its null device; `--server` to the mock server's default address. It
//! listens on 127.0.0.1 only and prints `wi-devbridge listening ws://…`
//! once it is ready. Secrets are held in memory: nothing here touches the
//! keychain.

use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tungstenite::{Error as WsError, Message};
use wi_core::{Config, Core, MemorySecrets};

const USAGE: &str = "usage: wi-devbridge --library DIR [--engine PATH] [--server URL] [--port N]";

struct Args {
    library: PathBuf,
    engine: PathBuf,
    server: String,
    port: u16,
}

fn parse(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut library = None;
    let mut engine = None;
    let mut server = "http://127.0.0.1:8787".to_string();
    let mut port = 8790;
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--library" => library = Some(PathBuf::from(value()?)),
            "--engine" => engine = Some(PathBuf::from(value()?)),
            "--server" => server = value()?,
            "--port" => port = value()?.parse().map_err(|_| "--port is a port number")?,
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let engine = match engine {
        Some(e) => e,
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("mock-engine"),
    };
    Ok(Args {
        library: library.ok_or("--library is required")?,
        engine,
        server,
        port,
    })
}

/// Opens a URL in the system browser, for sign-in; says where it is too,
/// for a machine without one.
fn open_browser(url: &str) -> Result<(), String> {
    eprintln!("wi-devbridge: sign in at {url}");
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn main() {
    let args = parse(std::env::args().skip(1)).unwrap_or_else(|e| {
        eprintln!("wi-devbridge: {e}\n{USAGE}");
        std::process::exit(2)
    });
    let mut config = Config::new(
        &args.engine,
        &args.server,
        Arc::new(MemorySecrets::default()),
        Arc::new(open_browser),
    );
    config.device = Some("null".into());
    let core = Core::open(&args.library, config).unwrap_or_else(|e| {
        eprintln!("wi-devbridge: {e}");
        std::process::exit(1)
    });
    let core = Arc::new(core);
    let listener = TcpListener::bind(("127.0.0.1", args.port)).unwrap_or_else(|e| {
        eprintln!("wi-devbridge: can't listen on port {} ({e})", args.port);
        std::process::exit(1)
    });
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(args.port);
    println!("wi-devbridge listening ws://127.0.0.1:{port}");
    for stream in listener.incoming().flatten() {
        let core = core.clone();
        std::thread::spawn(move || {
            if let Err(e) = serve(&core, stream) {
                eprintln!("wi-devbridge: a connection ended: {e}");
            }
        });
    }
}

/// The answer to one text frame.
fn answer(core: &Core, text: &str) -> Value {
    let req: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let Some(cmd) = req.get("cmd").and_then(Value::as_str) else {
        let error = json!({"code": "bad_request", "message": "A request is {id, cmd, args}."});
        return json!({"id": id, "ok": false, "error": error});
    };
    let args = req.get("args").cloned().unwrap_or_else(|| json!({}));
    match core.invoke(cmd, args) {
        Ok(result) => json!({"id": id, "ok": true, "result": result}),
        Err(e) => json!({"id": id, "ok": false, "error": e}),
    }
}

/// One browser tab: commands in, answers, events and meters out. The
/// socket is read with a short timeout so the same thread can write
/// whatever is waiting in between.
fn serve(core: &Arc<Core>, stream: TcpStream) -> Result<(), Box<WsError>> {
    let mut ws = tungstenite::accept(stream).map_err(|e| match e {
        tungstenite::HandshakeError::Failure(e) => Box::new(e),
        tungstenite::HandshakeError::Interrupted(_) => Box::new(WsError::ConnectionClosed),
    })?;
    ws.get_mut()
        .set_read_timeout(Some(Duration::from_millis(5)))
        .map_err(|e| Box::new(WsError::Io(e)))?;
    let events = core.events();
    let meters = core.meters();
    let (out, answers) = mpsc::channel::<String>();
    loop {
        // Answers taken first, events second, and the events sent first: an
        // answer's own events were emitted before it, so they arrive before it.
        let ready: Vec<String> = answers.try_iter().collect();
        for e in events.try_iter() {
            ws.send(Message::text(serde_json::to_string(&e).unwrap_or_default()))?;
        }
        for text in ready {
            ws.send(Message::text(text))?;
        }
        if let Ok(bytes) = meters.try_recv() {
            ws.send(Message::binary(bytes))?;
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                let (core, out) = (core.clone(), out.clone());
                let text = text.to_string();
                std::thread::spawn(move || {
                    let _ = out.send(answer(&core, &text).to_string());
                });
            }
            Ok(Message::Close(_)) => return Ok(()),
            Ok(_) => {}
            Err(WsError::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => return Ok(()),
            Err(e) => return Err(Box::new(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Result<Args, String> {
        parse(s.split_whitespace().map(String::from))
    }

    #[test]
    fn flags_parse_with_their_defaults() {
        let a = args("--library /tmp/lib").unwrap();
        assert_eq!(a.library, PathBuf::from("/tmp/lib"));
        assert_eq!((a.server.as_str(), a.port), ("http://127.0.0.1:8787", 8790));
        assert_eq!(a.engine.file_name().unwrap(), "mock-engine");
        let a = args("--library /l --engine /e --server http://x:1 --port 0").unwrap();
        assert_eq!(
            (a.engine, a.server, a.port),
            (PathBuf::from("/e"), "http://x:1".into(), 0)
        );
        assert!(args("--engine /e").is_err());
        assert!(args("--library /l --port many").is_err());
        assert!(args("--library /l --colour red").is_err());
    }
}
