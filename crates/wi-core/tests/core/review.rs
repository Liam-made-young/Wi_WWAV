//! An independent reviewer's adversarial tests. Each test here exposes a
//! defect the builder's own tests missed; each is `#[ignore]`d until the
//! finding it names is fixed, so `cargo test -p wi-core -- --ignored review`
//! shows what is still open.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

const T: Duration = Duration::from_secs(20);

fn executable(path: &Path, script: &str) {
    std::fs::write(path, script).unwrap();
    let mut p = std::fs::metadata(path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut p, 0o755);
    std::fs::set_permissions(path, p).unwrap();
}

/// Collects events until `name` arrives or `timeout` passes.
fn saw(rx: &std::sync::mpsc::Receiver<wi_core::Event>, name: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(e) = rx.recv_timeout(Duration::from_millis(50)) {
            if e.event == name {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------- engine

/// Finding: a hang in the audio thread while the transport is stopped is
/// never noticed. The stall check only runs while the shared-memory clock
/// says playing, and a hung audio thread never writes "playing" again, so
/// pressing play afterwards does nothing, forever, and the engine is never
/// restarted (docs/ENGINE.md §3.1, §5; S0.2, S3.4).
#[test]
#[ignore = "review finding: an audio-thread hang while stopped is never detected"]
fn review_an_audio_hang_while_stopped_is_noticed_once_play_is_pressed() {
    let setup = Setup::new();
    let core = setup.core();
    let events = core.events();
    core.engine().wait_running(T).unwrap();
    let ids = import(&core, &[&corpus("original.wwav")]);
    ok(&core, "player.load", json!({"clip": ids[0]}));
    core.engine()
        .call("debug.hang", json!({"in": "audio"}), T)
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    // The person presses play: the message thread answers, the audio
    // thread never moves.
    let pressed = core.invoke("player.play", json!({}));
    eprintln!("player.play after the hang: {pressed:?}");
    assert!(
        saw(&events, "engine.stopped", Duration::from_secs(4)),
        "4 s after play on a hung audio thread the engine still hasn't been restarted (phase {:?}, clock {:?})",
        core.engine().phase(),
        core.engine().clock()
    );
}

/// Finding: an engine that fails its first start is never started again.
/// The supervisor tries once, says "It couldn't start", and the app has no
/// audio until it is relaunched; there is no command to start it again.
/// Here the engine's first launch exits at once (a busy device, a crash
/// in the first second); the second would have worked.
#[test]
#[ignore = "review finding: one failed first start leaves the engine stopped for the session"]
fn review_an_engine_that_fails_its_first_start_is_tried_again() {
    let setup = Setup::new();
    let flaky = setup.dir.path().join("flaky-engine");
    let once = setup.dir.path().join("failed-once");
    executable(
        &flaky,
        &format!(
            "#!/bin/sh\nif [ ! -e '{once}' ]; then touch '{once}'; exit 1; fi\nexec '{engine}' \"$@\"\n",
            once = once.display(),
            engine = mock_engine().display()
        ),
    );
    let core = Core::open(
        &setup.library(),
        setup.config_with(&flaky, NO_SERVER, no_browser()),
    )
    .unwrap();
    let up = core.engine().wait_running(Duration::from_secs(10));
    assert!(
        up.is_ok(),
        "the engine stayed down after one failed start: {up:?}; status {}",
        core.invoke("engine.status", json!({})).unwrap()
    );
}

/// Finding: the listening player always builds its session at the
/// device's rate (48 kHz by default) and never at 44.1 kHz for a .wwav
/// (docs/SPEC.md 9.4: "Sessions run at 48 kHz, or 44.1 kHz when opened
/// from a .wwav"). mock-engine accepts any clip rate, so the builder's
/// tests pass; the real wwav-engine refuses a 44.1 kHz clip in a 48 kHz
/// session ("Clips at another rate are resampled once the export
/// resampler (F9) is built"), so no .wwav plays. Run with
/// WWAV_ENGINE=/path/to/wwav-engine.
#[test]
#[ignore = "review finding: no .wwav loads in the player on the real engine (48 kHz session)"]
fn review_a_wwav_plays_on_the_real_engine() {
    let Some(engine) = std::env::var_os("WWAV_ENGINE") else {
        eprintln!("WWAV_ENGINE isn't set: nothing to run against");
        return;
    };
    let setup = Setup::new();
    let core = Core::open(
        &setup.library(),
        setup.config_with(Path::new(&engine), NO_SERVER, no_browser()),
    )
    .unwrap();
    core.engine().wait_running(T).unwrap();
    let ids = import(&core, &[&corpus("original.wwav")]);
    let loaded = core.invoke("player.load", json!({"clip": ids[0]}));
    assert!(
        loaded.is_ok(),
        "the real engine refused the song: {:?}",
        loaded.err()
    );
    ok(&core, "player.play", json!({}));
    assert!(eventually(T, || core
        .engine()
        .clock()
        .is_some_and(|c| c.state == 1 && c.sample_pos > 0)));
}

// ---------------------------------------------------------------- player, library

/// Finding: the player divides a plain WAV's frames by 44,100 whatever the
/// file's rate, so a 48 kHz WAV's duration is 8.8 % long (and seek clamps
/// to that), while the library's own clip says the right length.
#[test]
#[ignore = "review finding: the player's duration ignores a plain WAV's sample rate"]
fn review_a_48k_wav_has_the_same_duration_in_the_player_as_in_the_library() {
    let setup = Setup::new();
    let core = setup.core();
    let id = import(&core, &[&corpus("48k.wav")])[0].clone();
    let library = ok(&core, "library.get", json!({"id": id}))["clip"]["duration"]
        .as_f64()
        .unwrap();
    let player = ok(&core, "player.load", json!({"clip": id}))["state"]["duration"]
        .as_f64()
        .unwrap();
    // 600 frames at 48 kHz.
    assert!((library - 0.0125).abs() < 1e-3, "library says {library}");
    assert!(
        (player - 0.0125).abs() < 1e-6,
        "the player says {player} s for 600 frames at 48 kHz"
    );
}

/// `wwav_pack.py info`'s last line, "  on PRANA: <verdict>".
fn reference_verdict(path: &Path) -> String {
    let out = std::process::Command::new("python3")
        .arg("-I")
        .arg(workspace().join("formats/prana/tools/wwav_pack.py"))
        .arg("info")
        .arg(path)
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    text.lines()
        .find_map(|l| l.trim().strip_prefix("on PRANA: "))
        .unwrap_or_else(|| panic!("no verdict from wwav_pack.py for {}", path.display()))
        .to_string()
}

/// Finding: Get Info's verdict for a plain WAV is 2.5's sentence ("Plain
/// audio comes in as master only.") instead of what `wwav_pack.py info`
/// says of the same file, so S1.7's "the verdict in Get Info differs from
/// wwav_pack.py info" fails for every .wav, and Get Info hides that PRANA
/// won't list a 48 kHz or 24-bit WAV at all.
#[test]
#[ignore = "review finding: Get Info's verdict for a .wav isn't wwav_pack.py's"]
fn review_get_info_says_what_wwav_pack_says_of_a_plain_wav() {
    let setup = Setup::new();
    let core = setup.core();
    let mut differ = Vec::new();
    for name in ["plain.wav", "48k.wav", "24bit.wav", "mono.wav"] {
        let f = corpus(name);
        let id = import(&core, &[&f])[0].clone();
        let ours = ok(&core, "library.get", json!({"id": id}))["verdict"]
            .as_str()
            .unwrap()
            .to_string();
        let theirs = reference_verdict(&f);
        if ours != theirs {
            differ.push(format!("{name}: {ours:?} vs wwav_pack.py {theirs:?}"));
        }
    }
    assert!(differ.is_empty(), "{differ:#?}");
}

// ---------------------------------------------------------------- upload queue

const COVERS: i64 = 2;
const WORLD_ENDING: i64 = 1;

fn drop_on(core: &Core, clip: &str, system: i64) -> Value {
    ok(
        core,
        "publish.drop",
        json!({"clip": clip, "target": {"kind": "system", "id": system}, "label": "publish"}),
    )
}

fn upload_sentences(
    events: &std::sync::mpsc::Receiver<wi_core::Event>,
    until: &str,
    timeout: Duration,
) -> (bool, Vec<String>) {
    let mut said = Vec::new();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(e) = events.recv_timeout(Duration::from_millis(100)) {
            if e.event == "status" && e.payload["area"] == "upload" {
                let s = e.payload["sentence"].as_str().unwrap().to_string();
                said.push(s.clone());
                if s == until {
                    return (true, said);
                }
            }
        }
    }
    (false, said)
}

fn planets_of(server: &MockServer, clip: &str) -> Vec<Value> {
    let state = server.state();
    let tracks: Vec<Value> = state["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| {
            t["versions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["clipId"] == clip)
        })
        .map(|t| t["trackId"].clone())
        .collect();
    state["planets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| tracks.contains(&p["trackId"]))
        .cloned()
        .collect()
}

/// Finding: once `/api/publish` has answered, the clip leaves the queue
/// (`remote_id` is set) before it is placed in its system. A transient
/// failure placing it (a 503, a dropped connection) is recorded as a
/// retry that never happens, because only queued clips are retried: the
/// work is up but in no system, the drop's whole point is lost, and the
/// status bar never says "Up.".
#[test]
#[ignore = "review finding: a failed placement after publish is never retried"]
fn review_a_placement_that_fails_once_is_retried() {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = sign_in(&setup, &server);
    let events = core.events();
    let (status, _) = server.call(
        "POST",
        "/__mock/fail",
        Some(json!({"method": "POST", "path": format!("/api/v2/systems/{COVERS}/planets"), "status": 503})),
    );
    assert_eq!(status, 200);
    let id = import(&core, &[&corpus("original.wwav")])[0].clone();
    drop_on(&core, &id, COVERS);
    let (up, said) = upload_sentences(
        &events,
        "Up. Tést \"Song\" is in your galaxy.",
        Duration::from_secs(12),
    );
    let clip = ok(&core, "library.get", json!({"id": id}))["clip"].clone();
    assert!(
        up && planets_of(&server, &id).len() == 1,
        "published: {}, planets: {:?}, the status said {said:?}",
        clip["published"],
        planets_of(&server, &id)
    );
}

/// Finding: dropping a song that is already up onto a second system
/// answers `{queued: true}` and then nothing happens: `publish()` keeps
/// the first `published_at`, `remote_id` is set, so the queue never holds
/// it again and the new system's tag is never placed (2.5 "Dropping a clip
/// on one publishes it there").
#[test]
#[ignore = "review finding: a second drop of a song that is up does nothing"]
fn review_a_second_drop_of_a_song_that_is_up_lands_in_the_second_system() {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = sign_in(&setup, &server);
    let events = core.events();
    let id = import(&core, &[&corpus("original.wwav")])[0].clone();
    drop_on(&core, &id, COVERS);
    let (up, said) = upload_sentences(
        &events,
        "Up. Tést \"Song\" is in your galaxy.",
        Duration::from_secs(12),
    );
    assert!(up, "{said:?}");
    assert_eq!(planets_of(&server, &id).len(), 1);

    assert_eq!(drop_on(&core, &id, WORLD_ENDING), json!({"queued": true}));
    let landed = eventually(Duration::from_secs(10), || {
        planets_of(&server, &id).len() == 2
    });
    assert!(
        landed,
        "the second drop answered queued but the song is only in {:?}; the queue holds {}",
        planets_of(&server, &id),
        ok(&core, "publish.queue", json!({}))
    );
}

/// Finding: any 400/403/404/409/413/422 marks the upload `refused` in
/// core_kv for good. Nothing ever clears it: not ⌘Z, not dropping it
/// again, not a relaunch, so a work refused once can never go up from this
/// library. The server's own temporary 403 ("Daily upload limit reached.
/// Try again tomorrow.", server.md) is one such refusal.
#[test]
#[ignore = "review finding: one refusal blocks a clip's upload forever, even after ⌘Z and a new drop"]
fn review_a_refusal_ends_when_the_work_is_dropped_again() {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = sign_in(&setup, &server);
    let events = core.events();
    let (status, _) = server.call(
        "POST",
        "/__mock/fail",
        Some(json!({"method": "GET", "path": "/api/upload/sign", "status": 403})),
    );
    assert_eq!(status, 200);
    let id = import(&core, &[&corpus("original.wwav")])[0].clone();
    drop_on(&core, &id, COVERS);
    let (_, said) = upload_sentences(
        &events,
        "'Tést \"Song\"' didn't go up: Injected failure",
        Duration::from_secs(10),
    );
    eprintln!("first try: {said:?}");

    // Take the drop back and drop it again: a new publish.
    assert_eq!(
        ok(&core, "history.undo", json!({"room": "space"}))["label"],
        "publish"
    );
    drop_on(&core, &id, COVERS);
    let (up, said) = upload_sentences(
        &events,
        "Up. Tést \"Song\" is in your galaxy.",
        Duration::from_secs(10),
    );
    assert!(
        up,
        "dropped again, it never went up: {said:?}; publish.queue: {}",
        ok(&core, "publish.queue", json!({}))
    );
}

/// Finding: when the server has forgotten a multipart upload (404 "No
/// upload there") the worker clears `upload_id` to "start again under the
/// same trackId", as its comment says, but the work loop counts 404 and
/// 409 as permanent refusals, so it never starts again: a 300 MB song
/// stops at the part it reached, for good.
#[test]
#[ignore = "review finding: a forgotten multipart upload is refused instead of started again"]
fn review_a_forgotten_multipart_upload_starts_again() {
    let server = MockServer::start_counting();
    let setup = Setup::new();
    let big = setup.dir.path().join("Long Take.wav");
    std::fs::File::create(&big)
        .unwrap()
        .set_len(251 * 1024 * 1024)
        .unwrap();
    let core = sign_in(&setup, &server);
    let events = core.events();
    let id = import(&core, &[&big])[0].clone();
    std::fs::remove_file(&big).unwrap();
    drop_on(&core, &id, COVERS);
    let (going, said) = upload_sentences(
        &events,
        "Uploading Long Take · part 2 of 32",
        Duration::from_secs(20),
    );
    assert!(going, "{said:?}");
    // The server forgets the upload (an expired multipart upload): every
    // later part's signature is a 404.
    let upload = server.counted()["uploads"][0]["uploadId"]
        .as_str()
        .unwrap()
        .to_string();
    for n in 3..=32 {
        server.call(
            "POST",
            "/__mock/fail",
            Some(json!({"method": "GET", "path": format!("/api/upload/parts/{upload}/{n}"), "status": 404})),
        );
    }
    let (up, said) = upload_sentences(
        &events,
        "Up. Long Take is in your galaxy.",
        Duration::from_secs(40),
    );
    assert!(
        up,
        "it never started again: {:?}; publish.queue: {}",
        said.iter().rev().take(3).collect::<Vec<_>>(),
        ok(&core, "publish.queue", json!({}))
    );
}

// ---------------------------------------------------------------- account

/// Finding: any 4xx from /api/auth/refresh deletes the tokens, a 429 (rate
/// limited, the server's own `uploadSign`-style limiter) included: a
/// moment of rate limiting signs the person out: the upload queue and Heat sync stop
/// until they sign in again in the browser.
#[test]
#[ignore = "review finding: a 429 on refresh signs the person out"]
fn review_a_rate_limited_refresh_keeps_the_account() {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = sign_in(&setup, &server);
    // The access token expires; the refresh token is good for weeks.
    server.call(
        "POST",
        "/__mock/clock",
        Some(json!({"advanceMs": 8 * 24 * 3600 * 1000u64})),
    );
    server.call(
        "POST",
        "/__mock/fail",
        Some(json!({"method": "POST", "path": "/api/auth/refresh", "status": 429})),
    );
    let _ = core.sync_heat();
    assert_eq!(
        ok(&core, "account.status", json!({}))["signedIn"],
        true,
        "one 429 from /api/auth/refresh signed the person out"
    );
}

// ---------------------------------------------------------------- export

/// Finding: index.html inlines the library as JSON in a <script> element
/// and escapes only "</". A title holding "<!--<script>" (a purchase, a
/// friend's .wwav's wmet) puts the HTML parser in the script's
/// double-escaped state, the element's own </script> no longer ends it,
/// and the whole page becomes one script element: nothing plays, offline
/// or not (checked in Chromium: the page shows no songs and its script
/// never runs).
#[test]
#[ignore = "review finding: a title with <!--<script> breaks the exported index.html"]
fn review_the_export_page_survives_any_title() {
    let setup = Setup::new();
    let core = setup.core();
    let id = import(&core, &[&corpus("original.wwav")])[0].clone();
    ok(
        &core,
        "library.rename",
        json!({"id": id, "title": "<!--<script>", "label": "rename"}),
    );
    let out = setup.dir.path().join("Export");
    ok(
        &core,
        "export.everything",
        json!({"to": out.display().to_string(), "zip": false}),
    );
    let page = std::fs::read_to_string(out.join("index.html")).unwrap();
    let data = page
        .split("<script type=\"application/json\" id=\"library\">")
        .nth(1)
        .unwrap();
    let data = &data[..data.find("</script>").unwrap()];
    assert!(
        !data.contains("<!--") && !data.to_lowercase().contains("<script"),
        "the inline JSON holds markup the HTML parser acts on: {data}"
    );
}

// ---------------------------------------------------------------- Heat sync

/// Finding: a background sync round writes other devices' changes as a
/// journal entry in Heat ("changes from your other devices"). That entry
/// is a new change in the room, so it throws away the person's redo, and
/// ⌘Z then reverts the other device's work instead of their own (2.7:
/// "⌘Z acts on the room you are in").
#[test]
#[ignore = "review finding: a background Heat sync clears the person's redo"]
fn review_a_sync_from_another_device_keeps_the_redo() {
    let server = MockServer::start();
    let (a, b) = (Setup::new(), Setup::new());
    let mac = sign_in(&a, &server);
    let air = sign_in(&b, &server);
    let put = |core: &Core, label: &str, id: &str, value: Value| {
        ok(
            core,
            "records.mutate",
            json!({"label": label, "room": "heat", "ops": [{"op": "put", "kind": "task", "id": id, "value": value}]}),
        )
    };
    put(&mac, "add task", "t1", json!({"title": "Essay"}));
    ok(
        &mac,
        "records.mutate",
        json!({"label": "mark done", "room": "heat", "ops": [{"op": "patch", "kind": "task", "id": "t1", "value": {"done": true}}]}),
    );
    ok(&mac, "history.undo", json!({"room": "heat"}));
    assert_eq!(
        ok(&mac, "history.get", json!({"room": "heat"}))["redo"],
        "Redo mark done"
    );
    // Meanwhile, the other Mac adds an unrelated task, and both sync.
    put(&air, "add task", "t9", json!({"title": "Reading"}));
    air.sync_heat().unwrap();
    mac.sync_heat().unwrap();
    let h = ok(&mac, "history.get", json!({"room": "heat"}));
    assert_eq!(
        h["redo"], "Redo mark done",
        "after a sync the Edit menu reads {h}"
    );
}

// ---------------------------------------------------------------- Claude

/// Finding: three of `assist.call`'s jobs have no consent switch at all
/// (`syllabus`, `review-note`, `release-plan`), so their first use sends
/// the person's text to mi-wwav.com without ever showing what will be
/// sent (2.11: "Each feature is off until its first use. The first use
/// shows exactly what will be sent").
#[test]
#[ignore = "review finding: syllabus, review-note and release-plan go out without consent"]
fn review_every_claude_job_asks_first() {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = sign_in(&setup, &server);
    let mut went_out = Vec::new();
    for (task, body) in [
        (
            "syllabus",
            json!({"text": "JPN 201. Quizzes 20%, exams 50%, homework 30%."}),
        ),
        ("review-note", json!({"facts": ["Classes: 6 tasks done"]})),
        (
            "release-plan",
            json!({"title": "World Ending", "date": "2026-11-01"}),
        ),
    ] {
        match core.invoke("assist.call", json!({"task": task, "body": body})) {
            Err(e) if e.code == "consent_needed" => {}
            other => went_out.push(format!("{task}: {other:?}")),
        }
    }
    assert!(
        went_out.is_empty(),
        "sent before the person turned the feature on: {went_out:#?}"
    );
}

/// Finding: only a change made with `room: "heat"` reaches Heat sync. A
/// Heat record written from another room (Space's "Plan in Heat" adds a
/// milestone from your sun, 2.6) never syncs, and an undo of it in that
/// room isn't seen either, so the other Mac never gets it.
#[test]
#[ignore = "review finding: Heat records changed from another room never sync"]
fn review_a_milestone_planned_from_space_syncs() {
    let server = MockServer::start();
    let (a, b) = (Setup::new(), Setup::new());
    let mac = sign_in(&a, &server);
    let air = sign_in(&b, &server);
    ok(
        &mac,
        "records.mutate",
        json!({"label": "plan in Heat", "room": "space", "ops": [{"op": "put", "kind": "milestone", "id": "m1", "value": {"title": "EP v1 mixed", "projectId": "p1"}}]}),
    );
    mac.sync_heat().unwrap();
    air.sync_heat().unwrap();
    assert!(
        air.invoke("records.get", json!({"kind": "milestone", "id": "m1"}))
            .is_ok(),
        "the milestone planned from Space never reached the other Mac"
    );
}
