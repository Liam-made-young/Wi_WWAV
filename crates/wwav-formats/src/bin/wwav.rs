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

use wwav_formats::{json, pack, swav, text, wwav::Wwav, Error};

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

// The rest of this file down to `without` is argparse (python 3.13) as the
// tools use it: a top-level parser with -h and a command, and each
// command's parser with -h, options taking one value each, and one
// positional. It follows `_parse_optional`, `_get_option_tuples`,
// `consume_optional` and `consume_positionals`, so the same command lines
// print the usage (exit 0), are refused (exit 2) or run.

const HELP: [&str; 2] = ["-h", "--help"];

/// argparse's `^-\d+$|^-\d*\.\d+$`, with any script's digits as `\d` and
/// `$` matching before a final line break.
fn negative_number(arg: &str) -> bool {
    let arg = arg.strip_suffix('\n').unwrap_or(arg);
    let digits = |s: &str| s.chars().all(|c| text::decimal(c).is_some());
    match arg.strip_prefix('-').map(|n| n.split_once('.')) {
        Some(None) => arg.len() > 1 && digits(&arg[1..]),
        Some(Some((whole, fraction))) => digits(whole) && !fraction.is_empty() && digits(fraction),
        None => false,
    }
}

/// How a value came in the option's own argument: after "=" ("--out=x",
/// "-o=x") or glued to a one-letter option ("-ox").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sep {
    Equals,
    Glued,
}

/// One reading of an option argument: the option string it names (None:
/// this command has no such option) and the value it carries, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Found {
    option: Option<&'static str>,
    explicit: Option<(Sep, String)>,
}

/// `_parse_optional`: None for a positional, else every option the
/// argument could name (more than one: ambiguous, said when it's reached).
fn classify(arg: &str, options: &[&'static str]) -> Option<Vec<Found>> {
    if !arg.starts_with('-') {
        return None;
    }
    if let Some(&o) = options.iter().find(|&&o| o == arg) {
        return Some(vec![Found {
            option: Some(o),
            explicit: None,
        }]);
    }
    if arg.chars().count() == 1 {
        return None;
    }
    let split = arg.split_once('=');
    if let Some((head, value)) = split {
        if let Some(&o) = options.iter().find(|&&o| o == head) {
            return Some(vec![Found {
                option: Some(o),
                explicit: Some((Sep::Equals, value.into())),
            }]);
        }
    }
    // `_get_option_tuples`: a prefix of a long option, up to any "="; or a
    // one-letter option with its value glued on
    let (prefix, explicit) = match split {
        Some((head, value)) => (head, Some((Sep::Equals, value.to_string()))),
        None => (arg, None),
    };
    let mut found = Vec::new();
    if arg[1..].starts_with('-') {
        for &o in options.iter().filter(|o| o.starts_with(prefix)) {
            found.push(Found {
                option: Some(o),
                explicit: explicit.clone(),
            });
        }
    } else {
        let short_len = arg.char_indices().nth(2).map_or(arg.len(), |(i, _)| i);
        let (short, glued) = arg.split_at(short_len);
        for &o in options {
            if o == short {
                found.push(Found {
                    option: Some(o),
                    explicit: Some((Sep::Glued, glued.into())),
                });
            } else if o.starts_with(prefix) {
                found.push(Found {
                    option: Some(o),
                    explicit: explicit.clone(),
                });
            }
        }
    }
    if !found.is_empty() {
        return Some(found);
    }
    if negative_number(arg) || arg.contains(' ') {
        return None;
    }
    Some(vec![Found {
        option: None,
        explicit: None,
    }])
}

/// How argparse names an option in its errors.
fn action_name(option: &str) -> &str {
    match option {
        "-h" | "--help" => "-h/--help",
        "-o" | "--out" => "-o/--out",
        o => o,
    }
}

/// python's repr() of a string, as argparse's `%r` shows a value.
fn repr(s: &str) -> String {
    let list = json::py_str(&json::Value::List(vec![json::Value::Str(s.into())]));
    list[1..list.len() - 1].to_string()
}

/// One argument as argparse sees it in the whole command line: after the
/// first "--", everything is a positional.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
    Positional,
    DoubleDash,
    Option(Vec<Found>),
}

fn kinds(args: &[String], options: &[&'static str]) -> Vec<Kind> {
    let mut out = Vec::with_capacity(args.len());
    let mut after = false;
    for a in args {
        out.push(if after {
            Kind::Positional
        } else if a == "--" {
            after = true;
            Kind::DoubleDash
        } else {
            classify(a, options).map_or(Kind::Positional, Kind::Option)
        });
    }
    out
}

/// What reading the arguments came to.
enum Parsed<T> {
    /// -h or --help was reached: print the usage and exit 0.
    Help,
    /// What was read, and any arguments set aside as unrecognized, which
    /// argparse refuses only once everything else has been read.
    Read(T, Vec<String>),
}

/// `consume_optional` at `i`: the option (and any options glued after a
/// one-letter one that takes no value), its value, and where the next
/// argument starts.
fn consume_optional(
    args: &[String],
    kinds: &[Kind],
    i: usize,
    options: &[&'static str],
    values: &mut Vec<(&'static str, String)>,
    extras: &mut Vec<String>,
) -> Result<Option<usize>, String> {
    let Kind::Option(found) = &kinds[i] else {
        return Ok(Some(i + 1));
    };
    if found.len() > 1 {
        let names: Vec<&str> = found.iter().filter_map(|f| f.option).collect();
        return Err(format!(
            "ambiguous option: {} could match {}",
            args[i],
            names.join(", ")
        ));
    }
    let Some(mut option) = found[0].option else {
        extras.push(args[i].clone());
        return Ok(Some(i + 1));
    };
    let mut explicit = found[0].explicit.clone();
    let mut taken: Vec<(&'static str, Option<String>)> = Vec::new();
    let takes_value = |o: &str| !HELP.contains(&o);
    let stop = loop {
        match explicit {
            Some((sep, value)) => {
                if !takes_value(option) && !option.starts_with("--") && !value.is_empty() {
                    // "-hx": -h, then -x if there is one
                    if sep == Sep::Equals || value.starts_with('-') {
                        return Err(format!(
                            "argument {}: ignored explicit argument {}",
                            action_name(option),
                            repr(&value)
                        ));
                    }
                    taken.push((option, None));
                    let c = value.chars().next().unwrap_or('?');
                    let next = format!("-{c}");
                    let Some(&o) = options.iter().find(|&&o| o == next) else {
                        extras.push(format!("-{value}"));
                        break i + 1;
                    };
                    option = o;
                    let rest = &value[c.len_utf8()..];
                    explicit = if rest.is_empty() {
                        None
                    } else if let Some(v) = rest.strip_prefix('=') {
                        Some((Sep::Equals, v.to_string()))
                    } else {
                        Some((Sep::Glued, rest.to_string()))
                    };
                } else if takes_value(option) {
                    taken.push((option, Some(value)));
                    break i + 1;
                } else {
                    return Err(format!(
                        "argument {}: ignored explicit argument {}",
                        action_name(option),
                        repr(&value)
                    ));
                }
            }
            None if !takes_value(option) => {
                taken.push((option, None));
                break i + 1;
            }
            None => match (kinds.get(i + 1), args.get(i + 1)) {
                (Some(Kind::Positional), Some(v)) => {
                    taken.push((option, Some(v.clone())));
                    break i + 2;
                }
                _ => {
                    return Err(format!(
                        "argument {}: expected one argument",
                        action_name(option)
                    ))
                }
            },
        }
    };
    for (o, value) in taken {
        match value {
            None => return Ok(None), // help: print it and exit
            Some(v) => values.push((if o == "-o" { "--out" } else { o }, v)),
        }
    }
    Ok(Some(stop))
}

/// A command's arguments: its options and the one positional, `name`.
fn parse(args: &[String], options: &[&'static str], name: &str) -> Result<Parsed<Args>, String> {
    let kinds = kinds(args, options);
    let is_option = |i: usize| matches!(kinds.get(i), Some(Kind::Option(_)));
    let last_option = (0..args.len()).rev().find(|&i| is_option(i));
    let (mut target, mut values, mut extras) = (None, Vec::new(), Vec::new());
    // `consume_positionals`: the positional takes "-*A-*" (one argument and
    // the "--" before or after it, which it drops), once
    let positional = |at: usize, target: &mut Option<String>| -> usize {
        if target.is_some() {
            return at;
        }
        let k = |j: usize| kinds.get(at + j);
        let n = match (k(0), k(1)) {
            (Some(Kind::Positional), Some(Kind::DoubleDash)) => 2,
            (Some(Kind::Positional), _) => 1,
            (Some(Kind::DoubleDash), Some(Kind::Positional)) => 2,
            _ => return at,
        };
        // the first "--" goes; in "-- --" the second is the positional
        let mut taken = args[at..at + n].to_vec();
        if n == 2 {
            let dash = usize::from(kinds[at] != Kind::DoubleDash);
            taken.remove(dash);
        }
        *target = taken.pop();
        at + n
    };
    let mut at = 0;
    while last_option.is_some_and(|last| at <= last) {
        let next = (at..args.len())
            .find(|&i| is_option(i))
            .unwrap_or(args.len());
        if at != next {
            let end = positional(at, &mut target);
            if end > at {
                at = end;
                continue;
            }
        }
        if !is_option(at) {
            extras.extend(args[at..next].iter().cloned());
            at = next;
        }
        match consume_optional(args, &kinds, at, options, &mut values, &mut extras)? {
            None => return Ok(Parsed::Help),
            Some(stop) => at = stop,
        }
    }
    let stop = positional(at, &mut target);
    extras.extend(args[stop..].iter().cloned());
    match target {
        Some(target) => Ok(Parsed::Read(
            Args {
                target,
                options: values,
            },
            extras,
        )),
        None => Err(format!("the following arguments are required: {name}")),
    }
}

/// The top-level parser: -h, then the command, which takes every argument
/// after it ("A..."). Returns the command and its arguments.
fn command(args: &[String], commands: &[&str]) -> Result<Parsed<(String, Vec<String>)>, String> {
    let kinds = kinds(args, &HELP);
    let mut extras = Vec::new();
    let mut at = 0;
    while at < args.len() {
        let start = match kinds[at] {
            Kind::Positional => at,
            Kind::DoubleDash if at + 1 < args.len() => at + 1,
            Kind::DoubleDash => break,
            Kind::Option(_) => {
                match consume_optional(args, &kinds, at, &HELP, &mut Vec::new(), &mut extras)? {
                    None => return Ok(Parsed::Help),
                    Some(stop) => at = stop,
                }
                continue;
            }
        };
        let cmd = &args[start];
        if !commands.contains(&cmd.as_str()) {
            let choices: Vec<String> = commands.iter().map(|c| repr(c)).collect();
            return Err(format!(
                "argument cmd: invalid choice: {} (choose from {})",
                repr(cmd),
                choices.join(", ")
            ));
        }
        return Ok(Parsed::Read(
            (cmd.clone(), args[start + 1..].to_vec()),
            extras,
        ));
    }
    Err("the following arguments are required: cmd".into())
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
    let usage_error = |e: &str| {
        eprintln!("{USAGE}\nwwav: error: {e}");
        ExitCode::from(2)
    };
    let help = || {
        println!("{USAGE}");
        ExitCode::SUCCESS
    };
    let (cmd, rest, mut extras) = match command(rest, &["pack", "info", "unpack"]) {
        Err(e) => return usage_error(&e),
        Ok(Parsed::Help) => return help(),
        Ok(Parsed::Read((cmd, rest), extras)) => (cmd, rest, extras),
    };
    let (options, name): (&[&'static str], &str) = match (film, cmd.as_str()) {
        (false, "pack") => (
            &["-h", "--help", "-o", "--out", "--splitter", "--creator"],
            "folder",
        ),
        (true, "pack") => (
            &[
                "-h",
                "--help",
                "-o",
                "--out",
                "--title",
                "--artist",
                "--creator",
            ],
            "film",
        ),
        (_, "unpack") => (&["-h", "--help", "-o", "--out"], "file"),
        _ => (&HELP, "file"),
    };
    let a = match parse(&rest, options, name) {
        Err(e) => return usage_error(&e),
        Ok(Parsed::Help) => return help(),
        Ok(Parsed::Read(a, more)) => {
            extras.extend(more);
            a
        }
    };
    if !extras.is_empty() {
        return usage_error(&format!("unrecognized arguments: {}", extras.join(" ")));
    }
    let (tool, said) = if film {
        ("swav_pack", swav_command(&cmd, &a))
    } else {
        ("wwav_pack", wwav_command(&cmd, &a))
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
