//! `wi-mcp`: started by Claude Desktop or Claude Code, it speaks MCP on
//! stdin and stdout until stdin ends (docs/SPEC.md 8.8).
//!
//! `wi-mcp [--library <dir>]`. Without `--library` it opens the library the
//! app uses: `WI_WWAV_LIBRARY`, or `~/Music/Wi_WWAV`.

use std::io::{self, BufReader};
use std::path::PathBuf;

use wi_mcp::library::{default_root, Library};
use wi_mcp::serve;

fn main() {
    let mut root: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--library" => match args.next() {
                Some(dir) => root = Some(dir.into()),
                None => {
                    eprintln!("wi-mcp: --library needs a folder");
                    std::process::exit(2);
                }
            },
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
    let mut library = Library::open(&root.unwrap_or_else(default_root));
    if let Err(e) = serve(stdin, stdout, &mut library) {
        eprintln!("wi-mcp: {e}");
        std::process::exit(1);
    }
}
