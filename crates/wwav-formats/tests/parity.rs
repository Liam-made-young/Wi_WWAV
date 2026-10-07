//! F3, one verdict: tools/parity/check.py reads every corpus file with
//! wwav_pack.py or swav_pack.py, Wi's JavaScript, PRANA's C++ and this
//! crate's `wwav`.
//!
//! Fails if: any reader says something else about any file, other than the
//! differences between the reference readers that manifest.json names (each
//! must still be exactly as written there); the Rust reader may never
//! differ. The known differences themselves still fail F3: run the script
//! without --allow-known to see them (docs/PLAN.md keeps F3 failed until
//! the readers are reconciled).

mod common;

use common::*;

#[test]
fn the_four_readers_give_one_verdict() {
    if !have_references("the_four_readers_give_one_verdict") {
        return;
    }
    for tool in ["node", "c++"] {
        if !has(tool) {
            eprintln!("the_four_readers_give_one_verdict: skipped, it needs {tool}");
            return;
        }
    }
    let check = root().join("tools/parity/check.py");
    let o = run(
        "python3",
        &[
            s(&check),
            "--wwav",
            env!("CARGO_BIN_EXE_wwav"),
            "--allow-known",
        ],
    );
    let out = String::from_utf8_lossy(&o.stdout);
    println!("{out}");
    assert!(
        o.status.success(),
        "{out}{}",
        String::from_utf8_lossy(&o.stderr)
    );
}
