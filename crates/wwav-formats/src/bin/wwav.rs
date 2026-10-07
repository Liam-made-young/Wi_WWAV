//! `wwav`: the reference tools' command lines, in Rust.
//!
//!   wwav pack FOLDER [-o OUT] [--splitter S] [--creator NAME]   wwav_pack.py pack
//!   wwav info FILE                                              wwav_pack.py info
//!   wwav unpack FILE [-o DIR]                                   wwav_pack.py unpack
//!   wwav swav pack FILM [-o OUT] [--title T] [--artist A] [--creator NAME]
//!   wwav swav info FILE
//!   wwav swav unpack FILE [-o OUT]                              swav_pack.py ...
//!
//! Each prints what the Python tool prints, and refuses in its words, with
//! its name in front ("wwav_pack: ..." or "swav_pack: ..."), so either can
//! stand in for the other. A wrong command line exits 2, as argparse does.

use std::path::Path;
use std::process::ExitCode;

use wwav_formats::{pack, swav, text, wwav::Wwav, Error};

const USAGE: &str = "usage: wwav pack FOLDER [-o OUT] [--splitter S] [--creator NAME]
       wwav info FILE
       wwav unpack FILE [-o DIR]
       wwav swav pack FILM [-o OUT] [--title T] [--artist A] [--creator NAME]
       wwav swav info FILE
       wwav swav unpack FILE [-o OUT]";

/// One positional argument and the options it was given, argparse-style:
/// `-o X`, `-oX`, `--out X` and `--out=X`, before or after it.
struct Args {
    target: String,
    options: Vec<(String, String)>,
}

impl Args {
    /// An option's last value, "" when it wasn't given (which the tools
    /// treat alike).
    fn get(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

fn parse(args: &[String], options: &[&str]) -> Result<Args, String> {
    let mut positional = Vec::new();
    let mut found = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--" {
            positional.extend(rest.by_ref().cloned());
            break;
        }
        if !arg.starts_with('-') || arg == "-" {
            positional.push(arg.clone());
            continue;
        }
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ if !arg.starts_with("--") && arg.len() > 2 => {
                (arg[..2].to_string(), Some(arg[2..].to_string()))
            }
            _ => (arg.clone(), None),
        };
        let name = if name == "-o" {
            "--out".to_string()
        } else {
            name
        };
        if !options.contains(&name.as_str()) {
            return Err(format!("unrecognized arguments: {arg}"));
        }
        let value = match inline.or_else(|| rest.next().cloned()) {
            Some(v) => v,
            None => return Err(format!("argument {name}: expected one argument")),
        };
        found.push((name, value));
    }
    match <[String; 1]>::try_from(positional) {
        Ok([target]) => Ok(Args {
            target,
            options: found,
        }),
        Err(_) => Err("expected one file or folder".into()),
    }
}

/// The file's name without a final `ext` in any case (`re.sub(r"\.wwav$", ...)`).
fn without(file: &str, ext: &str) -> String {
    let cut = file.len().saturating_sub(ext.len());
    match file.get(cut..) {
        Some(end) if end.eq_ignore_ascii_case(ext) => file[..cut].to_string(),
        _ => file.to_string(),
    }
}

/// What a command prints: its stdout line, and a warning for stderr.
type Said = Result<(String, Option<String>), Error>;

fn wwav_command(cmd: &str, a: &Args) -> Said {
    let target = a.target.as_str();
    let line = match cmd {
        "pack" => {
            let out = a
                .get("--out")
                .map_or_else(|| text::normpath(target) + ".wwav", String::from);
            let p = pack::pack(
                target,
                &out,
                a.get("--splitter").unwrap_or(""),
                a.get("--creator").unwrap_or(""),
            )?;
            p.line(&out)
        }
        "info" => Wwav::open(Path::new(target))?.info(target),
        _ => {
            let out = a
                .get("--out")
                .map_or_else(|| without(target, ".wwav"), String::from);
            format!("{out}/: {}", pack::unpack(target, &out)?.join(", "))
        }
    };
    Ok((line, None))
}

fn swav_command(cmd: &str, a: &Args) -> Said {
    let target = a.target.as_str();
    Ok(match cmd {
        "pack" => {
            let out = a.get("--out").map_or_else(
                || format!("{}.swav", text::splitext(target).0),
                String::from,
            );
            let given = |k| a.get(k).unwrap_or("");
            let (meta, total) = swav::pack_original(
                target,
                &out,
                given("--title"),
                given("--artist"),
                given("--creator"),
            )?;
            (
                format!(
                    "{out}: {}, {total} bytes, film_id {}",
                    meta.title, meta.film_id
                ),
                None,
            )
        }
        "info" => (swav::Swav::open(Path::new(target))?.info(target), None),
        _ => {
            let out = a
                .get("--out")
                .map_or_else(|| without(target, ".swav") + ".mp4", String::from);
            let u = swav::unpack(target, &out)?;
            (
                format!("{out}, {}", u.txt),
                u.warning.map(|w| format!("swav_pack: {w}")),
            )
        }
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (film, rest) = match args.first().map(String::as_str) {
        Some("swav") => (true, &args[1..]),
        _ => (false, &args[..]),
    };
    let cmd = rest.first().map_or("", String::as_str);
    let options: &[&str] = match (film, cmd) {
        (false, "pack") => &["--out", "--splitter", "--creator"],
        (true, "pack") => &["--out", "--title", "--artist", "--creator"],
        (_, "unpack") => &["--out"],
        (_, "info") => &[],
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let a = match parse(&rest[1..], options) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{USAGE}\nwwav: error: {e}");
            return ExitCode::from(2);
        }
    };
    let (tool, said) = if film {
        ("swav_pack", swav_command(cmd, &a))
    } else {
        ("wwav_pack", wwav_command(cmd, &a))
    };
    match said {
        Ok((line, warning)) => {
            println!("{}", line.trim_end_matches('\n'));
            if let Some(w) = warning {
                eprintln!("{w}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{tool}: {e}");
            ExitCode::FAILURE
        }
    }
}
