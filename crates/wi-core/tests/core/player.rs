//! The listening player (docs/PLAN.md S0.3, the Linux part; docs/SPEC.md
//! 2.3). What a fail looks like:
//! - the app's verdict for a file differs by a character from
//!   `wwav_pack.py info`'s (or `swav_pack.py info`'s for a film);
//! - a mute doesn't reach the audio within one block of the command
//!   returning, measured in the engine's meters;
//! - a second click (mute off, solo on) doesn't leave that stem alone;
//! - the clock event doesn't come about ten times a second while playing;
//! - pressing play in the Console doesn't pause the player with "console",
//!   or stopping the Console resumes it.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::common::*;
use serde_json::{json, Value};
use wi_core::{Core, PausedFor};

const T: Duration = Duration::from_secs(5);

/// The reference tool's verdict line, or None when it refuses the file.
fn reference(path: &Path) -> Option<(String, Vec<String>)> {
    let (tool, label) = match path.extension().and_then(|e| e.to_str()) {
        Some("swav") | Some("mp4") => ("formats/formats/swav/swav_pack.py", "  reads as: "),
        _ => ("formats/prana/tools/wwav_pack.py", "  on PRANA: "),
    };
    let out = Command::new("python3")
        .arg(workspace().join(tool))
        .arg("info")
        .arg(path)
        .output()
        .expect("python3 runs the reference tool");
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<String> = text.lines().map(String::from).collect();
    let verdict = lines.last()?.strip_prefix(label)?.to_string();
    Some((verdict, lines[1..lines.len() - 1].to_vec()))
}

#[test]
fn every_verdict_is_the_reference_tools_word_for_word() {
    let setup = Setup::new();
    let core = setup.core();
    let mut checked = 0;
    let mut files: Vec<_> = std::fs::read_dir(workspace().join("tests/corpus"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("wwav" | "swav")
            )
        })
        .collect();
    files.sort();
    for f in files {
        let theirs = reference(&f);
        let r = core.invoke(
            "library.import",
            json!({"paths": [f.display().to_string()], "label": "import"}),
        );
        match (theirs, r) {
            (None, Ok(r)) => assert!(
                r["clips"].as_array().unwrap().is_empty(),
                "{} came in though the reference refuses it",
                f.display()
            ),
            (Some(_), Err(e)) => panic!("{} was refused: {e}", f.display()),
            (None, Err(_)) => {}
            (Some((verdict, lines)), Ok(r)) => {
                let id = r["clips"][0]["id"].as_str().unwrap();
                let info = ok(&core, "library.get", json!({"id": id}));
                assert_eq!(info["verdict"], verdict, "{}", f.display());
                assert_eq!(info["clip"]["verdict"], verdict, "{}", f.display());
                // Get Info's chunks say what the tool says of each.
                let chunks = info["chunks"].as_array().unwrap();
                assert_eq!(chunks.len(), lines.len(), "{}", f.display());
                for (c, line) in chunks.iter().zip(&lines) {
                    let detail = c["detail"].as_str().unwrap();
                    let at = format!("at {}, {} bytes", c["at"], c["size"]);
                    assert!(line.contains(&at), "{}: {line} vs {c}", f.display());
                    assert!(
                        line.ends_with(detail),
                        "{}: {line} vs {detail}",
                        f.display()
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(checked >= 20, "only {checked} corpus files compared");
}

fn load(core: &Core) -> Value {
    let ids = import(core, &[&corpus("original.wwav")]);
    ok(core, "player.load", json!({"clip": ids[0]}))["state"].clone()
}

/// The vocals' meter, as the engine writes it.
fn vocals_peak(core: &Core, slot: usize) -> Option<(u64, f32)> {
    core.engine()
        .meters()
        .map(|m| (m.callback, m.slots.get(slot).map_or(0.0, |s| s[0])))
}

#[test]
fn a_song_loads_its_four_stems_and_plays_from_the_clock() {
    let setup = Setup::new();
    let core = setup.core();
    let events = core.events();
    let state = load(&core);
    let stems: Vec<&str> = state["stems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["stem"].as_str().unwrap())
        .collect();
    assert_eq!(stems, ["vocals", "drums", "other", "bass"]);
    assert_eq!(state["title"], "Tést \"Song\"");
    assert_eq!(state["key"], "A minor");
    assert_eq!(state["playing"], false);
    assert!((state["duration"].as_f64().unwrap() - 600.0 / 44100.0).abs() < 1e-9);

    ok(&core, "player.play", json!({}));
    // Ten clock events a second while playing.
    let began = Instant::now();
    let mut clocks = Vec::new();
    while began.elapsed() < Duration::from_secs(1) {
        if let Ok(e) = events.recv_timeout(Duration::from_millis(200)) {
            if e.event == "clock" {
                clocks.push(e.payload);
            }
        }
    }
    assert!(
        (8..=12).contains(&clocks.len()),
        "{} clock events in a second",
        clocks.len()
    );
    assert!(clocks
        .iter()
        .all(|c| c["state"] == "playing" && c["rate"] == 48000.0));
    let samples: Vec<i64> = clocks
        .iter()
        .map(|c| c["sample"].as_i64().unwrap())
        .collect();
    assert!(samples.windows(2).all(|w| w[1] > w[0]), "{samples:?}");
    let playing = ok(&core, "player.pause", json!({}))["state"].clone();
    assert_eq!(playing["playing"], false);

    let state = ok(&core, "player.seek", json!({"seconds": 0.005}))["state"].clone();
    assert!(eventually(T, || core.engine().clock().unwrap().sample_pos == 240));
    assert!((state["position"].as_f64().unwrap() - 0.005).abs() < 1e-3);
}

#[test]
fn a_mute_is_in_the_next_block_and_a_second_click_solos() {
    let setup = Setup::new();
    let core = setup.core();
    let state = load(&core);
    let slot = state["stems"][0]["slot"].as_u64().unwrap() as usize;
    let drums = state["stems"][1]["slot"].as_u64().unwrap() as usize;
    ok(&core, "player.play", json!({}));
    assert!(eventually(T, || vocals_peak(&core, slot).is_some_and(|(_, p)| p > 0.2)));

    // The click: mute. Every block the engine writes after the command
    // returns is silent in the vocals.
    let asked = Instant::now();
    ok(
        &core,
        "player.stem",
        json!({"stem": "vocals", "mute": true}),
    );
    let answered = asked.elapsed();
    let (after, _) = vocals_peak(&core, slot).unwrap();
    let mut heard = None;
    assert!(eventually(T, || {
        let (cb, peak) = vocals_peak(&core, slot).unwrap();
        if cb > after {
            heard = Some((cb, peak, asked.elapsed()));
            true
        } else {
            false
        }
    }));
    let (cb, peak, took) = heard.unwrap();
    assert_eq!(peak, 0.0, "block {cb} still had vocals");
    eprintln!("mute: answered in {answered:?}, silent in the meters after {took:?}");
    assert!(
        answered < Duration::from_millis(10),
        "the mute took {answered:?} to answer"
    );

    // The second click within 250 ms: the mute reverts and the stem solos.
    let state = ok(
        &core,
        "player.stem",
        json!({"stem": "vocals", "mute": false, "solo": true}),
    )["state"]
        .clone();
    assert_eq!(state["stems"][0]["mute"], false);
    assert_eq!(state["stems"][0]["solo"], true);
    let (after, _) = vocals_peak(&core, slot).unwrap();
    assert!(eventually(T, || {
        let m = core.engine().meters().unwrap();
        m.callback > after && m.slots[slot][0] > 0.2 && m.slots[drums][0] == 0.0
    }));

    // A level is the fader: half is -6 dB.
    ok(
        &core,
        "player.stem",
        json!({"stem": "vocals", "level": 0.5}),
    );
    let (after, _) = vocals_peak(&core, slot).unwrap();
    assert!(eventually(T, || {
        let (cb, peak) = vocals_peak(&core, slot).unwrap();
        cb > after && (peak - 0.125).abs() < 1e-4
    }));
    let bad = core
        .invoke("player.stem", json!({"stem": "vocals", "level": 2}))
        .unwrap_err();
    assert_eq!(bad.code, "bad_args");
}

#[test]
fn the_console_pauses_the_player_and_nothing_resumes_it() {
    let setup = Setup::new();
    let core = setup.core();
    load(&core);
    ok(&core, "player.play", json!({}));
    std::thread::sleep(Duration::from_millis(100));
    let state = core.pause_player(PausedFor::Console).unwrap()["state"].clone();
    assert_eq!(state["pausedFor"], "console");
    assert_eq!(state["playing"], false);
    let kept = state["position"].as_f64().unwrap();
    assert!(kept > 0.0);

    // The Console plays its own session, then stops.
    let rate = core.engine().rate().unwrap();
    let console = wi_core::EngineSession {
        graph: serde_json::from_value(json!({"sample_rate": rate, "tracks": [{"id": "keys", "kind": "audio", "role": "other"}]})).unwrap(),
        names: Default::default(),
    };
    core.engine().load(console, 0).unwrap();
    core.engine()
        .call("transport.play", Value::Null, T)
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    core.engine()
        .call("transport.stop", Value::Null, T)
        .unwrap();
    std::thread::sleep(Duration::from_millis(150));
    let state = ok(&core, "player.pause", json!({}))["state"].clone();
    assert_eq!(
        state["pausedFor"], "console",
        "stopping the Console resumed nothing"
    );
    assert_eq!(state["playing"], false);

    // Play: the song comes back where it was, and the strip clears.
    let state = ok(&core, "player.play", json!({}))["state"].clone();
    assert_eq!(state["pausedFor"], Value::Null);
    assert!(eventually(T, || {
        let c = core.engine().clock().unwrap();
        c.state == 1 && c.sample_pos as f64 / rate as f64 >= kept
    }));
    assert_eq!(core.engine().meter_slots().len(), 4 + 4 + 1);
}

#[test]
fn plain_audio_plays_as_the_master_only() {
    let setup = Setup::new();
    let core = setup.core();
    let ids = import(&core, &[&corpus("mono.wav")]);
    let clip = ok(&core, "library.get", json!({"id": ids[0]}));
    assert_eq!(clip["verdict"], "Plain audio comes in as master only.");
    let state = ok(&core, "player.load", json!({"clip": ids[0]}))["state"].clone();
    assert_eq!(state["stems"], json!([]));
    let refused = core
        .invoke("player.stem", json!({"stem": "vocals", "mute": true}))
        .unwrap_err();
    assert_eq!(
        refused.message,
        "This song has no stems: it plays as the master only."
    );
    ok(&core, "player.play", json!({}));
    assert!(eventually(T, || core.engine().clock().unwrap().state == 1));
}

#[test]
fn nothing_loaded_says_what_to_do() {
    let setup = Setup::new();
    let core = setup.core();
    let e = core.invoke("player.play", json!({})).unwrap_err();
    assert_eq!(e.code, "nothing_loaded");
    assert_eq!(
        e.message,
        "Nothing is loaded. Select a song and press Space."
    );
}
