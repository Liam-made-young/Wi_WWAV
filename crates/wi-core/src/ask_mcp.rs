//! Where Claude reaches the prompt box's tools: an MCP server inside the
//! core, on a loopback port, for the length of one question (docs/ASK.md).
//!
//! The run is the person's own Claude Code (`claude -p`), told of exactly one
//! MCP server, this one, with every built-in tool off. It speaks MCP's
//! streamable HTTP in its plainest form: one JSON-RPC message per POST,
//! answered as JSON. A tool call runs the same core command a view would.
//!
//! Who may call: only a request that carries the token of a run that is
//! still asking. The port is bound to 127.0.0.1; a request with an `Origin`
//! (a web page) or a `Host` that isn't this address is refused before it is
//! read, so nothing in a browser can reach it; and a run's token stops
//! working the moment its answer is in.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Weak};
use std::time::Duration;

use serde_json::{json, Value};

use crate::ask::State;
use crate::ask_tools::{self, Run};
use crate::Inner;

const MAX_BODY: usize = 8 << 20;
/// A tool's answer longer than this is cut, and says so.
const MAX_ANSWER: usize = 60_000;

/// Starts listening, if it isn't yet, and returns the port.
pub(crate) fn port(inner: &Arc<Inner>) -> Result<u16, String> {
    let state: &State = &inner.ask;
    let mut have = crate::bus::lock(&state.port);
    if let Some(p) = *have {
        return Ok(p);
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let p = listener.local_addr().map_err(|e| e.to_string())?.port();
    let weak = Arc::downgrade(inner);
    std::thread::Builder::new()
        .name("ask tools".into())
        .spawn(move || serve(listener, p, weak))
        .map_err(|e| e.to_string())?;
    *have = Some(p);
    Ok(p)
}

fn serve(listener: TcpListener, port: u16, inner: Weak<Inner>) {
    loop {
        // The core is gone or going: stop listening.
        match inner.upgrade() {
            Some(i) if !i.closing() => {}
            _ => return,
        }
        match listener.accept() {
            Ok((conn, _)) => {
                let inner = inner.clone();
                let _ = std::thread::Builder::new()
                    .name("ask tool call".into())
                    .spawn(move || {
                        let _ = handle(conn, port, &inner);
                    });
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

fn read_request(conn: &mut TcpStream) -> Option<Request> {
    conn.set_nonblocking(false).ok()?;
    conn.set_read_timeout(Some(Duration::from_secs(15))).ok()?;
    let mut seen = Vec::new();
    let mut buf = [0u8; 8192];
    let head_end = loop {
        if let Some(at) = seen.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        if seen.len() > 64 * 1024 {
            return None;
        }
        match conn.read(&mut buf) {
            Ok(0) | Err(_) => return None,
            Ok(n) => seen.extend_from_slice(&buf[..n]),
        }
    };
    let head = String::from_utf8_lossy(&seen[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let (method, path) = (first.next()?.to_string(), first.next()?.to_string());
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    if length > MAX_BODY {
        return None;
    }
    let mut body = seen[head_end..].to_vec();
    while body.len() < length {
        match conn.read(&mut buf) {
            Ok(0) | Err(_) => return None,
            Ok(n) => body.extend_from_slice(&buf[..n]),
        }
    }
    body.truncate(length);
    Some(Request {
        method,
        path,
        headers,
        body,
    })
}

fn respond(
    conn: &mut TcpStream,
    status: u16,
    reason: &str,
    body: Option<&Value>,
) -> std::io::Result<()> {
    let text = body.map(Value::to_string).unwrap_or_default();
    let kind = if body.is_some() {
        "Content-Type: application/json\r\n"
    } else {
        ""
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n{kind}Content-Length: {}\r\nConnection: close\r\n\r\n",
        text.len()
    );
    conn.write_all(head.as_bytes())?;
    conn.write_all(text.as_bytes())?;
    conn.flush()
}

fn handle(mut conn: TcpStream, port: u16, inner: &Weak<Inner>) -> std::io::Result<()> {
    let Some(req) = read_request(&mut conn) else {
        return respond(&mut conn, 400, "Bad Request", None);
    };
    // A web page sends an Origin; nothing that should be here does.
    if req.header("origin").is_some() {
        return respond(&mut conn, 403, "Forbidden", None);
    }
    let host = req.header("host").unwrap_or("");
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        return respond(&mut conn, 403, "Forbidden", None);
    }
    let Some(inner) = inner.upgrade() else {
        return respond(&mut conn, 503, "Closing", None);
    };
    let token = req
        .header("authorization")
        .and_then(|a| a.strip_prefix("Bearer "))
        .unwrap_or("");
    let Some(run) = inner.ask.run_by_token(token) else {
        return respond(&mut conn, 401, "Unauthorized", None);
    };
    if req.path.split('?').next() != Some("/mcp") {
        return respond(&mut conn, 404, "Not Found", None);
    }
    match req.method.as_str() {
        "POST" => {}
        // No stream of its own to offer, and nothing to end.
        "GET" => return respond(&mut conn, 405, "Method Not Allowed", None),
        "DELETE" => return respond(&mut conn, 200, "OK", None),
        _ => return respond(&mut conn, 405, "Method Not Allowed", None),
    }
    let Ok(message) = serde_json::from_slice::<Value>(&req.body) else {
        return respond(
            &mut conn,
            400,
            "Bad Request",
            Some(
                &json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "That isn't JSON."}}),
            ),
        );
    };
    // A notification has no id and gets no answer.
    if message.get("id").is_none() {
        return respond(&mut conn, 202, "Accepted", None);
    }
    let answer = answer(&inner, &run, &message);
    respond(&mut conn, 200, "OK", Some(&answer))
}

/// One JSON-RPC request answered.
pub(crate) fn answer(inner: &Arc<Inner>, run: &Arc<Run>, message: &Value) -> Value {
    let id = message["id"].clone();
    let result = match message["method"].as_str().unwrap_or("") {
        "initialize" => Ok(json!({
            "protocolVersion": message["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "learn", "title": "Learn", "version": env!("CARGO_PKG_VERSION")},
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": ask_tools::list()})),
        "tools/call" => {
            let name = message["params"]["name"].as_str().unwrap_or("");
            let empty = json!({});
            let args = match &message["params"]["arguments"] {
                Value::Null => &empty,
                other => other,
            };
            if let Some(t) = ask_tools::tool(name) {
                inner
                    .bus
                    .emit("ask", json!({"id": run.id, "tool": name, "doing": t.doing}));
            }
            let (text, failed) = match ask_tools::call(inner, run, name, args) {
                Ok(v) => (v.to_string(), false),
                Err(why) => (why, true),
            };
            let text = if text.chars().count() > MAX_ANSWER {
                let cut: String = text.chars().take(MAX_ANSWER).collect();
                format!("{cut}\n[Cut here: the answer was longer. Ask for fewer rows or columns.]")
            } else {
                text
            };
            Ok(json!({"content": [{"type": "text", "text": text}], "isError": failed}))
        }
        other => {
            Err(json!({"code": -32601, "message": format!("There is no method called '{other}'.")}))
        }
    };
    match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    }
}
