//! F1 and F2 on files and command lines nobody wrote by hand:
//! tools/parity/fuzz.py runs random variations of the corpus, random song
//! folders and film.txt files, and random command lines through the Python
//! tools and this crate's `wwav`, over several seeds, each in parallel.
//!
//! Fails if: for any case, `wwav` and the tool exit differently, print
//! other lines, or write other files; or the harness itself fails on a
//! seed (97 once broke it).

mod common;

use std::process::{Command, Stdio};

use common::*;

#[test]
fn wwav_does_what_the_tools_do_on_random_cases() {
    if !have_references("wwav_does_what_the_tools_do_on_random_cases") {
        return;
    }
    let fuzz = root().join("tools/parity/fuzz.py");
    let runs: Vec<_> = [1, 97, 2026]
        .iter()
        .map(|seed| {
            Command::new("python3")
                .args([s(&fuzz), "--cases", "150", "--seed", &seed.to_string()])
                .args(["--wwav", env!("CARGO_BIN_EXE_wwav")])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for run in runs {
        let o = run.wait_with_output().unwrap();
        let out = String::from_utf8_lossy(&o.stdout);
        println!("{out}");
        assert!(
            o.status.success(),
            "{out}{}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
}
