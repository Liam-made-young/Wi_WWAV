//! F1 and F2, reading: on every file in tests/corpus/, the Rust verdict is
//! the reference tool's sentence, and `wwav info` prints what `info` prints,
//! line for line.
//!
//! Fails if: any verdict differs by a character from manifest.json's (which
//! the corpus generator took from wwav_pack.py and swav_pack.py); any
//! `wwav info` or `wwav swav info` output, error or exit code differs from
//! the Python tool's for the same path.

mod common;

use common::*;
use wwav_formats::{swav::Swav, wwav::Wwav};

#[test]
fn every_verdict_is_the_references() {
    let files = manifest();
    assert!(files.len() >= 40, "the corpus has {} files", files.len());
    for e in files {
        let path = corpus().join(&e.file);
        let verdict = if is_film(&e.file) {
            Swav::open(&path).unwrap().verdict().to_string()
        } else {
            Wwav::open(&path).unwrap().verdict().to_string()
        };
        assert_eq!(verdict, e.verdict, "{}", e.file);
    }
}

#[test]
fn info_prints_what_the_tools_print() {
    if !have_references("info_prints_what_the_tools_print") {
        return;
    }
    for e in manifest() {
        let path = corpus().join(&e.file);
        let (reference, ours) = if is_film(&e.file) {
            (
                python(&swav_pack(), &["info", s(&path)]),
                wwav(&["swav", "info", s(&path)]),
            )
        } else {
            (
                python(&wwav_pack(), &["info", s(&path)]),
                wwav(&["info", s(&path)]),
            )
        };
        same_run(said(&ours), said(&reference), &e.file);
    }
}

#[test]
fn info_refuses_what_the_tools_refuse() {
    if !have_references("info_refuses_what_the_tools_refuse") {
        return;
    }
    let dir = tmp("info-refuses");
    let mp4 = std::fs::read(corpus().join("plain.mp4")).unwrap();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("text.wwav", b"not a wav at all".to_vec()),
        ("short.wwav", b"RIFF".to_vec()),
        ("rifx.wwav", b"RIFX\x04\x00\x00\x00WAVE".to_vec()),
        ("text.swav", b"just some text, long enough".to_vec()),
        ("cut.swav", mp4[..mp4.len() - 100].to_vec()),
        ("stray.swav", [&mp4[..], b"abc"].concat()),
        ("ftyp-only.swav", mp4[..32].to_vec()),
        ("empty.swav", vec![]),
    ];
    for (name, bytes) in cases {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        let (reference, ours) = if is_film(name) {
            (
                python(&swav_pack(), &["info", s(&path)]),
                wwav(&["swav", "info", s(&path)]),
            )
        } else {
            (
                python(&wwav_pack(), &["info", s(&path)]),
                wwav(&["info", s(&path)]),
            )
        };
        same_run(said(&ours), said(&reference), name);
    }
}
