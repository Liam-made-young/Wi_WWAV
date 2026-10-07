//! The `wwav` command line reads its arguments as argparse reads the
//! tools': the same runs succeed, with the same output, and the same runs
//! are refused.
//!
//! Fails if: for any argument list here, `wwav` and the Python tool exit
//! differently, or, both succeeding, print other lines or write other
//! files. (A refusal's own words are the program's usage text, which
//! differs, so only its exit code is compared.)

mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::*;

const SONG_TXT: &str =
    "title = Song\nsong_id = 0123456789abcdef0123456789abcdef\ncreated = 2026-10-03\n";
const FILM_TXT: &str =
    "title = Film\nfilm_id = 00112233445566778899aabbccddeeff\ncreated = 2026-10-03\n";

fn in_dir(dir: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

/// Every file under `dir`, with its bytes, in name order.
fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let name = p.strip_prefix(dir).unwrap().display().to_string();
                out.push((name, std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// Runs `args` in two identical folders, one through the Python tool and
/// one through `wwav`, and compares what happened.
fn same_as_tool(n: usize, tool: &Path, ours: &[&str], args: &[&str]) {
    let dir = tmp(&format!("cli-{n}"));
    let (py, rs) = (dir.join("py"), dir.join("rs"));
    for d in [&py, &rs] {
        let song = d.join("01 Song");
        std::fs::create_dir_all(&song).unwrap();
        for (i, name) in NAMED.iter().enumerate() {
            std::fs::write(song.join(name), wav(10, i as u32)).unwrap();
        }
        std::fs::write(song.join("song.txt"), SONG_TXT).unwrap();
        std::fs::copy(corpus().join("plain.mp4"), d.join("film.mp4")).unwrap();
        std::fs::write(d.join("film.txt"), FILM_TXT).unwrap();
        std::fs::copy(corpus().join("original.wwav"), d.join("song.wwav")).unwrap();
    }
    let mut py_args = vec![tool.to_str().unwrap()];
    py_args.extend(args);
    let want = in_dir(&py, "python3", &py_args);
    let got = in_dir(&rs, env!("CARGO_BIN_EXE_wwav"), &[ours, args].concat());
    let what = format!("{args:?}");
    assert_eq!(
        got.status.code(),
        want.status.code(),
        "{what}: wwav said {}, the tool {}",
        String::from_utf8_lossy(&got.stderr),
        String::from_utf8_lossy(&want.stderr)
    );
    let help = args
        .iter()
        .any(|a| a.starts_with("-h") || a.starts_with("--h"));
    if want.status.success() && !help {
        assert_eq!(said(&got), said(&want), "{what}");
        let (a, b) = (files(&rs), files(&py));
        assert_eq!(
            a.iter().map(|f| &f.0).collect::<Vec<_>>(),
            b.iter().map(|f| &f.0).collect::<Vec<_>>(),
            "{what}"
        );
        assert!(a == b, "{what}: the files written differ");
    }
}

#[test]
fn arguments_read_as_argparse_reads_them() {
    if !have_references("arguments_read_as_argparse_reads_them") {
        return;
    }
    let wwav_cases: &[&[&str]] = &[
        &["pack", "01 Song"],
        &["pack", "01 Song", "--creator", "-x"],
        &["pack", "01 Song", "--creator", "-1"],
        &["pack", "01 Song", "--creator", "-1.5"],
        &["pack", "01 Song", "--creator", "-.5"],
        &["pack", "01 Song", "--creator", "-1.wwav"],
        &["pack", "01 Song", "--creator", "-٣"],
        &["pack", "01 Song", "--creator", "-a b"],
        &["pack", "01 Song", "--creator", "-"],
        &["pack", "01 Song", "--creator", ""],
        &["pack", "01 Song", "--creator=-x"],
        &["pack", "01 Song", "--creator"],
        &["pack", "01 Song", "--cr", "me", "--s", "demucs"],
        &["pack", "01 Song", "--c=me"],
        &["pack", "01 Song", "-o", ""],
        &["pack", "01 Song", "-o", "x.wwav"],
        &["pack", "01 Song", "-o=x.wwav"],
        &["pack", "01 Song", "-ox.wwav"],
        &["pack", "01 Song", "-o-x.wwav"],
        &["pack", "01 Song", "--out", "-x.wwav"],
        &["pack", "01 Song", "--o", "x.wwav"],
        &["pack", "01 Song", "--=x"],
        &["pack", "01 Song", "--bogus"],
        &["pack", "01 Song", "-x"],
        &["pack", "01 Song", "01 Song"],
        &["pack", "--", "01 Song"],
        &["pack", "--creator", "a", "01 Song"],
        &["pack"],
        &["pack", "01 Song", "--he"],
        &["pack", "-h"],
        &["info", "song.wwav"],
        &["info", "song.wwav", "-o", "x"],
        &["unpack", "song.wwav"],
        &["unpack", "song.wwav", "--o", "back"],
        &["unpack", "song.wwav", "-o", "-back"],
        &["-h"],
        &["--help"],
        &[],
        &["frobnicate"],
        // -h takes no value: "=" or a glued "-" is a usage error, a glued
        // letter is another option (-ho needs -o's value), and options it
        // doesn't know before it are refused only after the help
        &["info", "--help=x"],
        &["info", "-h=x"],
        &["info", "--help="],
        &["info", "-h="],
        &["info", "--he=x"],
        &["info", "-h-x"],
        &["info", "-h-"],
        &["info", "-hh=x"],
        &["info", "-hx"],
        &["info", "-hhx"],
        &["info", "--hel"],
        &["pack", "-ho"],
        &["pack", "-ho", "x"],
        &["pack", "-hox"],
        &["info", "--bogus", "-h"],
        &["info", "-x", "-h"],
        &["info", "a", "b", "-h"],
        &["pack", "01 Song", "--bogus", "-h"],
        &["pack", "-o", "-h"],
        &["pack", "--c", "-h"],
        &["--help=x"],
        &["-h=x"],
        &["-hx"],
        &["-h-x"],
        &["--bogus", "-h"],
        &["--bogus", "info", "song.wwav"],
        &["frobnicate", "-h"],
        &["--", "-h"],
        &["--"],
        &["--", "info", "song.wwav"],
        &["info", "--", "song.wwav"],
        &["info", "song.wwav", "--"],
        &["info", "--", "--"],
    ];
    for (i, args) in wwav_cases.iter().enumerate() {
        same_as_tool(i, &wwav_pack(), &[], args);
    }
    let swav_cases: &[&[&str]] = &[
        &["pack", "film.mp4"],
        &["pack", "film.mp4", "--title", "-x"],
        &["pack", "film.mp4", "--ti", "T", "--a", "A", "--cr", "C"],
        &["pack", "film.mp4", "--title", "-a b", "--artist="],
        &["pack", "film.mp4", "-o", "", "--title", ""],
        &["pack", "film.mp4", "--splitter", "x"],
        &["info", "film.mp4"],
        &["unpack", "film.mp4"],
        &["-h"],
        &["pack", "film.mp4", "--title=x", "-h="],
        &["info", "film.mp4", "--t", "-h"],
    ];
    for (i, args) in swav_cases.iter().enumerate() {
        same_as_tool(100 + i, &swav_pack(), &["swav"], args);
    }
}
