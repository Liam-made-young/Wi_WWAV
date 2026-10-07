//! `wi-mcp`: started by Claude Desktop or Claude Code, it speaks MCP on
//! stdin and stdout until stdin ends (docs/SPEC.md 8.8).
//!
//! `wi-mcp [--library <dir>]`. Without `--library` it opens the library the
//! app uses, as the app's settings name it.

use std::io::{self, BufReader};

use serde_json::{Map, Value};
use wi_mcp::{serve, Backend, ToolError};

/// Answers while there is no library to open: every tool is listed, and
/// every call says what to do.
struct NoLibrary;

impl Backend for NoLibrary {
    fn enabled(&mut self, _tool: &str) -> bool {
        true
    }
    fn call(&mut self, _tool: &str, _args: &Map<String, Value>) -> Result<Value, ToolError> {
        Err(ToolError::new("No Wi_WWAV library yet. Open the app once."))
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--library" => {
                args.next();
            }
            "--version" => {
                println!("wi-mcp {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            other => {
                eprintln!("wi-mcp: unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let stdin = BufReader::new(io::stdin().lock());
    let stdout = io::stdout().lock();
    if let Err(e) = serve(stdin, stdout, &mut NoLibrary) {
        eprintln!("wi-mcp: {e}");
        std::process::exit(1);
    }
}
