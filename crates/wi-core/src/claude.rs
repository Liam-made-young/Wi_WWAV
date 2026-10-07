//! Settings → Claude (docs/SPEC.md 2.13, 3.13): the lines that add Wi_WWAV to
//! Claude Desktop and Claude Code, with the helper's real path; a switch for
//! each of the eight tools; and Claude's recent changes. The helper is
//! `wi-mcp`, a program of its own that Claude starts: the app never runs it.

use std::path::PathBuf;

use serde_json::{json, Value};
use wi_heat_store::snapshot;

use crate::args::Args;
use crate::heat_cmd::{core_error, wrote_outside};
use crate::{CoreError, Inner};

/// What to say when the helper isn't where the lines point.
pub const BUILD_HINT: &str = "Build the helper first: cargo build -p wi-mcp";

/// Where the helper is: `WI_WWAV_MCP` if set; else `Contents/Helpers/wi-mcp`
/// inside a bundle; else `wi-mcp` beside the app's own binary, which is
/// `target/debug/wi-mcp` for `cargo tauri dev` after `cargo build -p wi-mcp`.
pub(crate) fn helper_path(i: &Inner) -> PathBuf {
    if let Some(path) = &i.helper {
        return path.clone();
    }
    if let Some(path) = std::env::var_os("WI_WWAV_MCP") {
        return path.into();
    }
    let exe = std::env::current_exe().unwrap_or_default();
    helper_beside(&exe, std::env::consts::OS)
}

/// The helper for an app binary at `exe`.
pub(crate) fn helper_beside(exe: &std::path::Path, os: &str) -> PathBuf {
    let dir = exe.parent().unwrap_or(std::path::Path::new("."));
    let in_bundle = dir.file_name().is_some_and(|n| n == "MacOS") && dir.parent().and_then(|c| c.file_name()).is_some_and(|n| n == "Contents");
    if in_bundle {
        return dir.parent().unwrap_or(dir).join("Helpers/wi-mcp");
    }
    dir.join(if os == "windows" { "wi-mcp.exe" } else { "wi-mcp" })
}

/// A path as one word of a shell line: plain when it can be, else in quotes.
fn shell_word(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// `heat.claude.get`
pub(crate) fn get(i: &Inner) -> Result<Value, CoreError> {
    let path = helper_path(i);
    let shown = path.display().to_string();
    let data = snapshot::claude_data(&i.store()).map_err(core_error)?;
    let quoted = serde_json::to_string(&shown).unwrap_or_default();
    let mut out = json!({
        "helper": shown,
        "desktop": format!("{{ \"mcpServers\": {{ \"wi-wwav\": {{ \"command\": {quoted} }} }} }}"),
        "code": format!("claude mcp add --scope user wi-wwav -- {}", shell_word(&shown)),
        "tools": data["tools"],
        "recent": data["recent"],
    });
    if !path.exists() {
        out["note"] = json!(BUILD_HINT);
    }
    Ok(out)
}

/// `heat.claude.setTool {name, on}`: a switch in Settings → Claude. The helper
/// reads it on every list and every call.
pub(crate) fn set_tool(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let name = a.str("name")?;
    let on = a.opt_bool("on")?.ok_or_else(|| CoreError::new("bad_args", "heat.claude.setTool needs on, true or false."))?;
    wi_heat_store::set_tool(&mut i.store(), name, on).map_err(core_error)?;
    wrote_outside(i, &["heatSetting"]);
    Ok(json!({}))
}
