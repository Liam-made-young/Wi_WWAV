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

/// One positional argument and the options it was given.
struct Args {
    target: String,
    options: Vec<(&'static str, String)>,
}

impl Args {
    /// An option's last value; None when it wasn't given or is "", which
    /// the tools treat alike (`a.out or ...`).
    fn get(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .rev()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    }
}

/// argparse's `^-\d+$|^-\d*\.\d+$`, with any script's digits as `\d`.
fn negative_number(arg: &str) -> bool {
    let digits = |s: &str| s.chars().all(|c| text::decimal(c).is_some());
    match arg.strip_prefix('-').map(|n| n.split_once('.')) {
        Some(None) => arg.len() > 1 && digits(&arg[1..]),
        Some(Some((whole, fraction))) => digits(whole) && !fraction.is_empty() && digits(fraction),
        None => false,
    }
}

/// How argparse reads one argument (`_parse_optional`): Ok(None) for a
/// positional, or an option by its full name with any value given in the
/// same argument ("--out=x", "-ox"). A long option may be shortened to any
/// prefix that names one ("--cr"). Err: it looks like an option this
/// command doesn't have.
fn option(
    arg: &str,
    options: &[&'static str],
) -> Result<Option<(&'static str, Option<String>)>, String> {
    if !arg.starts_with('-') || arg == "-" {
        return Ok(None);
    }
    let (head, value) = match arg.split_once('=') {
        Some((h, v)) => (h, Some(v.to_string())),
        None => (arg, None),
    };
    if let Some(&o) = options.iter().find(|&&o| o == arg || o == head) {
        return Ok(Some((o, if o == arg { None } else { value })));
    }
    let found: Vec<_> = if arg.starts_with("--") {
        options.iter().filter(|o| o.starts_with(head)).collect()
    } else {
        options.iter().filter(|o| arg.starts_with(**o)).collect()
    };
    match found[..] {
        [&o] if o.len() == 2 => Ok(Some((o, Some(arg[2..].to_string())))),
        [&o] => Ok(Some((o, value))),
        [] if negative_number(arg) || arg.contains(' ') => Ok(None),
        [] => Err(format!("unrecognized arguments: {arg}")),
        _ => Err(format!("ambiguous option: {arg}")),
    }
}

/// The arguments after the command: `--` ends the options, an option's
/// value can't look like an option, and -h or --help asks for the usage
/// (Ok(None)).
fn parse(args: &[String], options: &[&'static str]) -> Result<Option<Args>, String> {
    let mut positional = Vec::new();
    let mut found = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--" {
            positional.extend(rest.by_ref().cloned());
            break;
        }
        let Some((name, inline)) = option(arg, options)? else {
            positional.push(arg.clone());
            continue;
        };
        if name == "--help" || name == "-h" {
            return Ok(None);
        }
        let value = match inline {
            Some(v) => v,
            None => match rest.next() {
                Some(v) if v != "--" && option(v, options) == Ok(None) => v.clone(),
                _ => return Err(format!("argument {name}: expected one argument")),
            },
        };
        found.push((if name == "-o" { "--out" } else { name }, value));
    }
    match <[String; 1]>::try_from(positional) {
        Ok([target]) => Ok(Some(Args {
            target,
            options: found,
        })),
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
    let Ok(args) = std::env::args_os()
        .skip(1)
        .map(|a| a.into_string())
        .collect::<Result<Vec<String>, _>>()
    else {
        eprintln!("{USAGE}\nwwav: error: an argument isn't UTF-8");
        return ExitCode::from(2);
    };
    let (film, rest) = match args.first().map(String::as_str) {
        Some("swav") => (true, &args[1..]),
        _ => (false, &args[..]),
    };
    let cmd = rest.first().map_or("", String::as_str);
    const HELP: [&str; 2] = ["-h", "--help"];
    let options: &[&'static str] = match (film, cmd) {
        (false, "pack") => &["-h", "--help", "-o", "--out", "--splitter", "--creator"],
        (true, "pack") => &[
            "-h",
            "--help",
            "-o",
            "--out",
            "--title",
            "--artist",
            "--creator",
        ],
        (_, "unpack") => &["-h", "--help", "-o", "--out"],
        (_, "info") => &HELP,
        _ if matches!(option(cmd, &HELP), Ok(Some(_))) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let a = match parse(&rest[1..], options) {
        Ok(Some(a)) => a,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
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
