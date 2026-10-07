//! Wi_WWAV's MCP server (docs/SPEC.md 3.13, 8.8; docs/HEAT.md).
//!
//! Claude Desktop and Claude Code start `wi-mcp` and speak MCP to it over
//! stdio: one JSON-RPC 2.0 message per line in, one per line out, logs on
//! stderr only. This crate is the protocol and the door: which tools exist,
//! what their arguments may hold, and how an answer is worded. A [`Backend`]
//! does the work, one store transaction per call.

pub mod tools;

use std::io::{BufRead, Write};

use serde_json::{json, Map, Value};

/// The protocol versions this server speaks, newest first. A client asking
/// for one of these gets it; anything else gets the newest.
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub const SERVER_NAME: &str = "wi-wwav";

const INSTRUCTIONS: &str = "Heat, the planner in Wi_WWAV, on this Mac: tasks, grades, focus time and school mail. Claude estimates and drafts; the person decides. Restate only numbers these tools return.";

/// One plain sentence for Claude, returned with the error flag set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolError(pub String);

impl ToolError {
    pub fn new(sentence: impl Into<String>) -> Self {
        ToolError(sentence.into())
    }
}

/// Does the work behind the tools: reads the switches and runs one call as
/// one transaction. `args` has already passed the tool's schema.
pub trait Backend {
    /// Whether the tool is switched on in Settings → Claude. Read on every
    /// list and every call (8.8).
    fn enabled(&mut self, tool: &str) -> bool;
    /// Runs the tool. A write's result carries `undo_label`.
    fn call(&mut self, tool: &str, args: &Map<String, Value>) -> Result<Value, ToolError>;
}

/// Serves one client until stdin ends. Never panics on bad input: every
/// line gets an answer or, for a notification, nothing.
pub fn serve<R: BufRead, W: Write>(input: R, mut output: W, backend: &mut dyn Backend) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle_line(&line, backend) {
            let mut text = serde_json::to_string(&reply).unwrap_or_default();
            text.push('\n');
            output.write_all(text.as_bytes())?;
            output.flush()?;
        }
    }
    Ok(())
}

/// The answer to one line, or None for a notification or a response.
pub fn handle_line(line: &str, backend: &mut dyn Backend) -> Option<Value> {
    let message: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return Some(error(Value::Null, -32700, "Parse error")),
    };
    let Value::Object(message) = message else {
        return Some(error(Value::Null, -32600, "One JSON-RPC message per line; batches aren't supported."));
    };
    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        // A response to a request we never send, or junk: nothing to say.
        return match id {
            Some(id) if !message.contains_key("result") && !message.contains_key("error") => {
                Some(error(id, -32600, "A request needs a method."))
            }
            _ => None,
        };
    };
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let id = id?; // a notification: initialized, cancelled. Nothing comes back.
    if !(id.is_string() || id.is_i64() || id.is_u64()) {
        return Some(error(Value::Null, -32600, "A request id is a string or a whole number."));
    }
    Some(match method {
        "initialize" => result(id, initialize(&params)),
        "ping" => result(id, json!({})),
        "tools/list" => result(id, json!({ "tools": list(backend) })),
        "tools/call" => match call(&params, backend) {
            Ok(v) => result(id, v),
            Err((code, text)) => error(id, code, &text),
        },
        _ => error(id, -32601, &format!("Method not found: {method}")),
    })
}

fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
    let version = PROTOCOL_VERSIONS.iter().find(|v| **v == asked).unwrap_or(&PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": SERVER_NAME, "title": "Wi_WWAV", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS
    })
}

fn list(backend: &mut dyn Backend) -> Vec<Value> {
    tools::NAMES
        .iter()
        .filter(|t| backend.enabled(t))
        .map(|t| tools::definition(t))
        .collect()
}

fn call(params: &Value, backend: &mut dyn Backend) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "tools/call needs a tool name.".to_string()))?;
    if !tools::NAMES.contains(&name) {
        return Err((-32602, format!("Unknown tool: {name}")));
    }
    if !backend.enabled(name) {
        return Ok(tool_error("This tool is switched off in Wi_WWAV."));
    }
    let args = match params.get("arguments") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(m)) => m.clone(),
        Some(_) => return Ok(tool_error("The arguments must be an object.")),
    };
    if let Err(sentence) = check(&tools::input_schema(name), &args) {
        return Ok(tool_error(&sentence));
    }
    Ok(match backend.call(name, &args) {
        Ok(value) => json!({
            "content": [{ "type": "text", "text": serde_json::to_string(&value).unwrap_or_default() }],
            "structuredContent": value,
            "isError": false
        }),
        Err(ToolError(sentence)) => tool_error(&sentence),
    })
}

fn tool_error(sentence: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": sentence }], "isError": true })
}

fn result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Checks arguments against one of [`tools::input_schema`]'s schemas: the
/// subset they use (object, string, integer, enum, required, lengths,
/// minimum and maximum, pattern, and closed properties). Says what is wrong
/// in one sentence.
pub fn check(schema: &Value, args: &Map<String, Value>) -> Result<(), String> {
    let props = schema.get("properties").and_then(Value::as_object).cloned().unwrap_or_default();
    for key in args.keys() {
        if !props.contains_key(key) {
            return Err(format!("This tool takes no \"{key}\" argument."));
        }
    }
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for r in required.iter().filter_map(Value::as_str) {
            if args.get(r).map_or(true, Value::is_null) {
                return Err(format!("\"{r}\" is required."));
            }
        }
    }
    for (key, value) in args {
        if value.is_null() {
            continue; // the same as leaving it out
        }
        check_value(key, &props[key], value)?;
    }
    Ok(())
}

fn check_value(key: &str, schema: &Value, value: &Value) -> Result<(), String> {
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => {
            let Some(s) = value.as_str() else {
                return Err(format!("\"{key}\" must be text."));
            };
            let chars = s.chars().count() as u64;
            if let Some(min) = schema.get("minLength").and_then(Value::as_u64) {
                if chars < min || s.trim().is_empty() && min > 0 {
                    return Err(format!("\"{key}\" can't be empty."));
                }
            }
            if let Some(max) = schema.get("maxLength").and_then(Value::as_u64) {
                if chars > max {
                    return Err(format!("\"{key}\" is longer than {max} characters."));
                }
            }
            if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
                if !allowed.iter().any(|a| a.as_str() == Some(s)) {
                    let names: Vec<_> = allowed.iter().filter_map(Value::as_str).collect();
                    return Err(format!("\"{key}\" must be one of: {}.", names.join(", ")));
                }
            }
            if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
                if !matches_pattern(pattern, s) {
                    return Err(format!("\"{key}\" isn't in the form the tool asks for."));
                }
            }
            if schema.get("format").and_then(Value::as_str) == Some("date-time") && !is_date_time(s) {
                return Err(format!("\"{key}\" must be ISO 8601 with an offset, such as 2026-10-07T23:59:00-04:00."));
            }
            Ok(())
        }
        Some("integer") => {
            let Some(n) = value.as_i64() else {
                return Err(format!("\"{key}\" must be a whole number."));
            };
            if let Some(min) = schema.get("minimum").and_then(Value::as_i64) {
                if n < min {
                    return Err(format!("\"{key}\" must be at least {min}."));
                }
            }
            if let Some(max) = schema.get("maximum").and_then(Value::as_i64) {
                if n > max {
                    return Err(format!("\"{key}\" must be at most {max}."));
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The two patterns the schemas use, checked by hand so the helper needs no
/// regex engine: a date and a 24-hour time.
fn matches_pattern(pattern: &str, s: &str) -> bool {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| r.into_iter().all(|i| b[i].is_ascii_digit());
    if pattern.contains("\\d{4}-") {
        return b.len() == 10 && digits(0..4) && b[4] == b'-' && digits(5..7) && b[7] == b'-' && digits(8..10);
    }
    if pattern.contains(":[0-5]") {
        return b.len() == 5
            && digits(0..2)
            && b[2] == b':'
            && digits(3..5)
            && (b[0] - b'0') * 10 + (b[1] - b'0') < 24
            && b[3] - b'0' < 6;
    }
    true
}

/// `YYYY-MM-DDTHH:MM(:SS(.fff)?)?` followed by `Z` or `±HH:MM`.
pub fn is_date_time(s: &str) -> bool {
    if !s.is_ascii() {
        return false;
    }
    let b = s.as_bytes();
    if b.len() < 17 || !matches_pattern("\\d{4}-", &s[..10.min(s.len())]) || b[10] != b'T' {
        return false;
    }
    let time_end = b[11..]
        .iter()
        .position(|c| *c == b'Z' || *c == b'+' || *c == b'-')
        .map(|p| p + 11);
    let Some(end) = time_end else { return false };
    let time = &s[11..end];
    let ok_time = match time.len() {
        5 => matches_pattern(":[0-5]", time),
        8.. => {
            matches_pattern(":[0-5]", &time[..5])
                && time.as_bytes()[5] == b':'
                && time[6..8].bytes().all(|c| c.is_ascii_digit())
                && (time.len() == 8 || time.as_bytes()[8] == b'.' && time.len() > 9 && time[9..].bytes().all(|c| c.is_ascii_digit()))
        }
        _ => false,
    };
    let zone = &s[end..];
    ok_time && (zone == "Z" || zone.len() == 6 && matches_pattern(":[0-5]", &zone[1..]))
}
