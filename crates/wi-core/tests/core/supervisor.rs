//! The engine supervisor against mock-engine (docs/PLAN.md S0.2 and S3.4's
//! Linux run; docs/ENGINE.md §1, §3.1, §5). What a fail looks like:
//! - the app waits on the engine to draw (Core::open blocks on it);
//! - after kill -9 during playback the engine isn't back within 2 s, or the
//!   transport isn't stopped, or it isn't at the playhead the dead engine's
//!   clock last showed;
//! - of 200 kills at random moments in playback and render, any takes over
//!   2 s, changes the session hash, leaves the transport running, or takes
//!   the app core down;
//! - a hang (the audio thread or the message thread) isn't noticed;
//! - the crumb doesn't name the device that was running, or a plugin that
//!   crashed three times in ten minutes can be turned on again.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::common::*;
use rand::Rng;
use serde_json::{json, Value};
use wi_core::{Core, EngineSession, Phase};
use wwav_wire::graph::Graph;
use wwav_wire::shm::STATE_STOPPED;

const T: Duration = Duration::from_secs(5);

fn kill_9(pid: u32) {
    // SAFETY: kill has no memory preconditions.
    assert_eq!(unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) }, 0);
}

fn running(core: &Core) -> u32 {
    core.engine().wait_running(T).unwrap();
    core.engine().pid().unwrap()
}

/// A session with a plugin on a track, so crumbs have something to name.
fn keys_session(rate: u32) -> EngineSession {
    let graph: Graph = serde_json::from_value(json!({
        "sample_rate": rate,
        "tracks": [
            {"id": "01JC5R00000000000000000VOX", "kind": "stem", "role": "vocals"},
            {"id": "01JC5R0000000000000000KEYS", "kind": "audio", "role": "other",
             "devices": [{"id": "01JC5T0000000000000000TAPE", "format": "vst3", "uid": "TAPEECHO",
                          "params": {"p0": 0.25}}]},
            {"id": "01JC5R0000000000000000BASS", "kind": "audio", "role": "bass", "gain_db": -3.0}
        ]
    }))
    .unwrap();
    let names: BTreeMap<String, String> = [
        ("01JC5R00000000000000000VOX", "Vocals"),
        ("01JC5R0000000000000000KEYS", "Keys"),
        ("01JC5T0000000000000000TAPE", "Tape Echo"),
        ("01JC5R0000000000000000BASS", "Bass"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    EngineSession { graph, names }
}

const TAPE: &str = "01JC5T0000000000000000TAPE";

fn play_song(core: &Core) {
    let ids = import(core, &[&corpus("original.wwav")]);
    ok(core, "player.load", json!({"clip": ids[0]}));
    ok(core, "player.play", json!({}));
}

#[test]
fn the_app_does_not_wait_on_the_engine() {
    // An engine that takes a second to say it is listening.
    let setup = Setup::new();
    let slow = setup.dir.path().join("slow-engine");
    std::fs::write(
        &slow,
        format!(
            "#!/bin/sh\nsleep 1\nexec '{}' \"$@\"\n",
            mock_engine().display()
        ),
    )
    .unwrap();
    let mut p = std::fs::metadata(&slow).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut p, 0o755);
    std::fs::set_permissions(&slow, p).unwrap();

    let began = Instant::now();
    let core = Core::open(
        &setup.library(),
        setup.config_with(&slow, NO_SERVER, no_browser()),
    )
    .unwrap();
    let opened = began.elapsed();
    assert!(
        opened < Duration::from_millis(500),
        "Core::open took {opened:?}"
    );
    assert_eq!(
        core.invoke("engine.status", json!({})).unwrap()["state"],
        "starting"
    );
    // The library answers while the engine starts.
    import(&core, &[&corpus("original.wwav")]);
    core.engine().wait_running(T).unwrap();
    assert_eq!(
        core.invoke("engine.status", json!({})).unwrap()["state"],
        "running"
    );
}

#[test]
fn kill_9_mid_playback_is_back_stopped_at_the_same_playhead() {
    let setup = Setup::new();
    let core = setup.core();
    let events = core.events();
    let pid = running(&core);
    play_song(&core);
    std::thread::sleep(Duration::from_millis(300));
    let before = core.engine().clock().unwrap();
    assert_eq!(before.state, 1, "playing");
    let killed = Instant::now();
    kill_9(pid);

    let back = wait_event(&events, "engine.back", T);
    let took = killed.elapsed();
    assert!(took < Duration::from_secs(2), "back after {took:?}");
    assert_eq!(back.payload["sentence"], "Back.");
    let new_pid = core.engine().pid().unwrap();
    assert_ne!(new_pid, pid);

    // Stopped, at the playhead the dead engine's clock last showed.
    let playhead = back.payload["playhead"].as_i64().unwrap();
    assert!(eventually(T, || core
        .engine()
        .clock()
        .is_some_and(|c| c.callbacks > 2)));
    let after = core.engine().clock().unwrap();
    assert_eq!(after.state, STATE_STOPPED);
    assert_eq!(after.sample_pos, playhead);
    let rate = 48_000.0;
    let slack = (killed.elapsed().as_secs_f64().min(0.05) + 0.02) * rate;
    assert!(
        playhead >= before.sample_pos && (playhead - before.sample_pos) as f64 <= slack,
        "the playhead moved from {} to {playhead}",
        before.sample_pos
    );
    // It stays where it is: nothing resumes on its own.
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(core.engine().clock().unwrap().sample_pos, playhead);
    let state = ok(&core, "player.pause", json!({}))["state"].clone();
    assert_eq!(state["playing"], false);
    // And plays again from there when asked.
    ok(&core, "player.play", json!({}));
    assert!(eventually(T, || core.engine().clock().unwrap().sample_pos > playhead));
}

#[test]
fn two_hundred_kills_in_playback_and_render() {
    let setup = Setup::new();
    let core = setup.core();
    let events = core.events();
    running(&core);
    let rate = core.engine().rate().unwrap();
    core.engine().load(keys_session(rate), 0).unwrap();
    // Let the plugin report its state once, as it does on every stop.
    core.engine()
        .call("transport.play", Value::Null, T)
        .unwrap();
    core.engine()
        .call("transport.stop", Value::Null, T)
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let hash = core.engine().session_hash().unwrap();
    let render_dir = setup.dir.path().join("render");

    let mut rng = rand::thread_rng();
    let mut took = Vec::new();
    let mut in_render = 0;
    let render = json!({"out_dir": render_dir, "start": 0, "len": rate * 20, "format": "s16"});
    for n in 0..200 {
        let pid = running(&core);
        let wait = Duration::from_millis(rng.gen_range(0..150));
        // Back is timed from the kill to the event, as it arrives.
        let kill_and_wait = |pid: u32| {
            let killed = Instant::now();
            kill_9(pid);
            let back = wait_event(&events, "engine.back", T);
            (back, killed.elapsed())
        };
        let (back, t) = if n % 2 == 1 {
            // A render waits on the engine; the kill lands somewhere in it.
            std::thread::scope(|s| {
                let call = s.spawn(|| {
                    core.engine()
                        .call("render", render.clone(), Duration::from_secs(60))
                });
                std::thread::sleep(wait);
                let back = kill_and_wait(pid);
                if call.join().unwrap().is_err() {
                    in_render += 1;
                }
                back
            })
        } else {
            core.engine()
                .call("transport.play", Value::Null, T)
                .unwrap();
            std::thread::sleep(wait);
            kill_and_wait(pid)
        };
        took.push(t);
        assert!(t < Duration::from_secs(2), "kill {n} back after {t:?}");
        assert!(back.payload["playhead"].as_i64().unwrap() >= 0);
        assert!(eventually(T, || core
            .engine()
            .clock()
            .is_some_and(|c| c.callbacks > 1)));
        assert_eq!(
            core.engine().clock().unwrap().state,
            STATE_STOPPED,
            "kill {n} left it running"
        );
        assert_eq!(
            core.engine().session_hash().unwrap(),
            hash,
            "kill {n} changed the session"
        );
        assert!(
            core.invoke("app.hello", json!({})).is_ok(),
            "the core went down at kill {n}"
        );
    }
    took.sort();
    eprintln!(
        "200 kills ({} mid-render): back after max {:?}, p50 {:?}, p99 {:?}",
        in_render, took[199], took[100], took[197]
    );
    assert!(in_render > 50, "only {in_render} kills landed mid-render");
    // The session the engine renders after all that is the one before it.
    let r = core
        .engine()
        .call("render", json!({"out_dir": render_dir, "len": 4800}), T)
        .unwrap();
    assert!(r["sha256"]["master"].is_string());
    assert_eq!(core.engine().session_hash().unwrap(), hash);
}

#[test]
fn a_hang_in_the_audio_thread_is_seen_by_the_clock() {
    let setup = Setup::new();
    let core = setup.core();
    let events = core.events();
    let pid = running(&core);
    play_song(&core);
    std::thread::sleep(Duration::from_millis(100));
    let hung = Instant::now();
    core.engine()
        .call("debug.hang", json!({"in": "audio"}), T)
        .unwrap();
    let stopped = wait_event(&events, "engine.stopped", T);
    assert_eq!(
        stopped.payload["sentence"],
        "The audio engine stopped answering. Restarting…"
    );
    let found = hung.elapsed();
    // 500 ms of stalled clock, checked ten times a second.
    assert!(
        found >= Duration::from_millis(450) && found < Duration::from_millis(900),
        "{found:?}"
    );
    wait_event(&events, "engine.back", T);
    assert_ne!(core.engine().pid().unwrap(), pid);
    assert_eq!(core.engine().phase(), Phase::Running);
}

#[test]
fn a_hang_in_the_message_thread_is_seen_by_the_ping() {
    let setup = Setup::new();
    let core = setup.core();
    let events = core.events();
    let pid = running(&core);
    let hung = Instant::now();
    core.engine()
        .call("debug.hang", json!({"in": "message"}), T)
        .unwrap();
    wait_event(&events, "engine.stopped", T);
    let found = hung.elapsed();
    // The next ping (within 1 s) goes unanswered for 1 s.
    assert!(
        found >= Duration::from_millis(900) && found < Duration::from_millis(2300),
        "{found:?}"
    );
    wait_event(&events, "engine.back", T);
    assert_ne!(core.engine().pid().unwrap(), pid);
}

#[test]
fn the_crumb_names_the_device_and_it_comes_back_off() {
    let setup = Setup::new();
    let core = setup.core();
    let events = core.events();
    running(&core);
    let rate = core.engine().rate().unwrap();
    core.engine().load(keys_session(rate), 0).unwrap();
    core.engine()
        .call("transport.play", Value::Null, T)
        .unwrap();
    core.engine()
        .call("debug.crumb", json!({"node": TAPE}), T)
        .unwrap();
    let stopped = wait_event(&events, "engine.stopped", T);
    assert_eq!(
        stopped.payload["sentence"],
        "The audio engine stopped. 'Tape Echo' on the track 'Keys' was running when it did. Restarting…"
    );
    assert_eq!(stopped.payload["plugin"], "Tape Echo");
    assert_eq!(stopped.payload["track"], "Keys");
    let back = wait_event(&events, "engine.back", T);
    let sentence = back.payload["sentence"].as_str().unwrap();
    assert!(
        sentence.starts_with("Back. 'Tape Echo' is off until you turn it on. Changes made inside its own window in the last ")
            && sentence.ends_with(" s may be lost."),
        "{sentence}"
    );
    assert_eq!(back.payload["tryAgain"], true);
    assert_eq!(core.engine().off(), vec![TAPE.to_string()]);
    assert_eq!(
        core.invoke("engine.status", json!({})).unwrap()["off"],
        json!([TAPE])
    );

    // Try it again: it loads on.
    ok(&core, "engine.plugin.tryAgain", json!({"device": TAPE}));
    assert!(core.engine().off().is_empty());
    // Keep it off: it stays off through the next restart.
    ok(&core, "engine.plugin.keepOff", json!({"device": TAPE}));
    kill_9(core.engine().pid().unwrap());
    wait_event(&events, "engine.back", T);
    assert_eq!(core.engine().off(), vec![TAPE.to_string()]);
}

#[test]
fn three_crashes_in_ten_minutes_keep_a_plugin_off() {
    let setup = Setup::new();
    let core = setup.core();
    let events = core.events();
    running(&core);
    let rate = core.engine().rate().unwrap();
    core.engine().load(keys_session(rate), 0).unwrap();
    for n in 1..=3 {
        if n > 1 {
            ok(&core, "engine.plugin.tryAgain", json!({"device": TAPE}));
        }
        core.engine()
            .call("debug.crumb", json!({"node": TAPE}), T)
            .unwrap();
        wait_event(&events, "engine.stopped", T);
        let back = wait_event(&events, "engine.back", T);
        if n == 3 {
            assert_eq!(
                back.payload["sentence"],
                "Back. 'Tape Echo' crashed 3 times in ten minutes, so it stays off."
            );
            assert_eq!(back.payload["tryAgain"], false);
        }
    }
    let refused = core
        .invoke("engine.plugin.tryAgain", json!({"device": TAPE}))
        .unwrap_err();
    assert_eq!(refused.code, "crashed_3_times");
    assert_eq!(
        refused.message,
        "'Tape Echo' crashed 3 times in ten minutes, so it stays off."
    );
    assert_eq!(core.engine().off(), vec![TAPE.to_string()]);
}

#[test]
fn the_engine_leaves_when_the_core_closes() {
    let setup = Setup::new();
    let core = setup.core();
    let pid = running(&core);
    drop(core);
    // SAFETY: signal 0 only asks whether the process exists.
    let gone = eventually(Duration::from_millis(1500), || unsafe {
        libc::kill(pid as libc::pid_t, 0) != 0
    });
    assert!(gone, "the engine outlived the core");
}

#[test]
fn a_missing_engine_says_so_and_the_library_still_works() {
    let setup = Setup::new();
    let core = Core::open(
        &setup.library(),
        setup.config_with(
            &setup.dir.path().join("no-such-engine"),
            NO_SERVER,
            no_browser(),
        ),
    )
    .unwrap();
    let err = core.engine().wait_running(T).unwrap_err();
    assert_eq!(err.code, "engine_stopped");
    assert_eq!(
        core.invoke("engine.status", json!({})).unwrap()["state"],
        "stopped"
    );
    import(&core, &[&corpus("original.wwav")]);
}
