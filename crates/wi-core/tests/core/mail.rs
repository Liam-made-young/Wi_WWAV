//! Mail as a client (docs/SPEC.md 3.10; docs/PLAN.md S2.18), with a script
//! standing in for the Claude Code command line. What a fail looks like:
//! - the send job is given a tool that reads mail, or the read job a tool
//!   that sends, labels or deletes: the two must never share a run;
//! - a run has the shell, files or the web, or runs on anything but the
//!   small model;
//! - a mail the person sent isn't in the outbox word for word, or a run
//!   that couldn't start leaves it waiting with no reason;
//! - archiving or marking a thread waits on a run to show in Learn;
//! - a core that wasn't told to reads mail on its own.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

const NOW: &str = "2026-10-07 09:00";

/// A stand-in for `claude`: appends each run's arguments and prompt to a
/// log, and answers as a run that went well.
fn fake_claude(dir: &Path) -> (PathBuf, PathBuf) {
    let (bin, log) = (dir.join("claude"), dir.join("runs.log"));
    let script = format!(
        "#!/bin/sh\n{{ echo '== run'; printf '%s\\n' \"$@\"; echo '-- prompt'; cat; echo; }} >> {log}\necho '{{\"is_error\":false,\"result\":\"ok\"}}'\n",
        log = log.display()
    );
    std::fs::write(&bin, script).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    (bin, log)
}

fn core_with(setup: &Setup, claude: PathBuf) -> Core {
    let mut config = setup.config(NO_SERVER, no_browser());
    config.now = Some(ny(NOW));
    config.claude = Some(claude);
    let core = Core::open(&setup.library(), config).unwrap();
    ok(
        &core,
        "app.settings.set",
        json!({"patch": {"heat": {"timeZone": "America/New_York"}}}),
    );
    snap(&core, "2026-10-07");
    core
}

/// A thread as Claude records it, with its text saved.
fn record(setup: &Setup, id: &str, subject: &str, text: &str) {
    let mut store = wi_store::Store::open(&setup.library()).unwrap();
    let clock = wi_heat_store::Clock::system();
    let thread = json!({"thread_id": id, "subject": subject, "from": "Prof <p@uri.edu>", "received_at": "2026-10-07T08:00:00-04:00", "state": "nothing", "unread": true, "reason": "Nothing to do."});
    wi_heat_store::mcp::call(
        &mut store,
        &clock,
        "record_mail_thread",
        thread.as_object().unwrap(),
    )
    .unwrap();
    let body = json!({"thread_id": id, "messages": [{"from": "Prof <p@uri.edu>", "sent_at": "2026-10-07T08:00:00-04:00", "text": text}]});
    wi_heat_store::mcp::call(
        &mut store,
        &clock,
        "save_mail_text",
        body.as_object().unwrap(),
    )
    .unwrap();
}

/// The runs logged so far: each one's arguments and its prompt.
fn runs(log: &Path) -> Vec<(Vec<String>, String)> {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    text.split("== run\n")
        .skip(1)
        .map(|run| {
            let (args, prompt) = run.split_once("-- prompt\n").unwrap_or((run, ""));
            (args.lines().map(String::from).collect(), prompt.to_string())
        })
        .collect()
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "waited 10 s for {what}");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn tools_of(args: &[String]) -> Vec<String> {
    let at = args
        .iter()
        .position(|a| a == "--allowedTools")
        .expect("a run names its tools");
    args[at + 1].split(',').map(String::from).collect()
}

#[test]
fn sending_and_reading_are_two_runs_that_share_no_tool() {
    let setup = Setup::new();
    let (bin, log) = fake_claude(setup.dir.path());
    let core = core_with(&setup, bin);
    record(&setup, "th-1", "Quiz 4", "The quiz moves to Thursday.");
    // A core that wasn't told to reads nothing on its own.
    std::thread::sleep(Duration::from_millis(300));
    assert!(runs(&log).is_empty(), "a run started by itself");

    let sent = ok(
        &core,
        "heat.mail.reply",
        json!({"threadId": "th-1", "body": "Thank you, せんせい.\nLiam"}),
    );
    assert_eq!(sent["action"]["status"], "queued");
    assert_eq!(sent["action"]["body"], "Thank you, せんせい.\nLiam");
    wait_for("the send run", || runs(&log).len() == 1);
    let (args, prompt) = runs(&log).remove(0);
    let send_tools = tools_of(&args);
    assert_eq!(
        send_tools,
        [
            "mcp__wi-wwav__list_mail_outbox",
            "mcp__wi-wwav__finish_mail_action",
            "mcp__claude_ai_Gmail__send_message",
            "mcp__claude_ai_Gmail__reply",
            "mcp__claude_ai_Gmail__label_thread",
            "mcp__claude_ai_Gmail__unlabel_thread"
        ]
    );
    // No shell, files or web; no skills; the small model.
    let flag = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .map(|at| args[at + 1].clone())
    };
    assert_eq!(flag("--tools").as_deref(), Some(""));
    assert_eq!(flag("--model").as_deref(), Some("haiku"));
    assert!(args.contains(&"--disable-slash-commands".to_string()));
    assert!(
        prompt.contains("list_mail_outbox") && prompt.contains("character for character"),
        "{prompt}"
    );
    // The mail itself is in the outbox, not in the prompt: the run reads it through its tool.
    assert!(!prompt.contains("せんせい"));

    ok(&core, "heat.mail.sync", json!({}));
    wait_for("the read run", || {
        runs(&log)
            .iter()
            .any(|(a, _)| tools_of(a).iter().any(|t| t.ends_with("get_thread")))
    });
    let all = runs(&log);
    let (args, prompt) = all
        .iter()
        .find(|(a, _)| tools_of(a).iter().any(|t| t.ends_with("get_thread")))
        .unwrap();
    let read_tools = tools_of(args);
    assert!(
        prompt.contains("list_mail_accounts") && prompt.contains("newer_than:7d"),
        "the first read goes back a week: {prompt}"
    );
    for t in &read_tools {
        assert!(!send_tools.contains(t), "{t} is in both runs");
        for writes in [
            "send", "reply", "label", "trash", "draft", "forward", "delete", "spam", "outbox",
            "finish",
        ] {
            assert!(
                !t.rsplit("__").next().unwrap().contains(writes),
                "the read run may {t}"
            );
        }
    }
    for t in &send_tools {
        for reads in [
            "search",
            "get_thread",
            "get_message",
            "list_mail_accounts",
            "record",
            "save_mail_text",
        ] {
            assert!(!t.contains(reads), "the send run may {t}");
        }
    }
    wait_for("the read's line", || {
        snap(&core, "2026-10-07")["mailSync"]["line"].is_string()
    });
    let sync = snap(&core, "2026-10-07")["mailSync"].clone();
    assert!(
        sync["line"].as_str().unwrap().starts_with("Mail read ")
            && sync["line"].as_str().unwrap().ends_with(": nothing new"),
        "{sync}"
    );
    assert_eq!(
        (
            sync["background"].clone(),
            sync["queued"].clone(),
            sync["failed"].clone()
        ),
        (json!(true), json!(1), json!(0))
    );
    // The next read goes back two days.
    ok(&core, "heat.mail.sync", json!({}));
    wait_for("a second read", || {
        runs(&log)
            .iter()
            .filter(|(_, p)| p.contains("newer_than:2d"))
            .count()
            == 1
    });
}

#[test]
fn a_threads_place_changes_at_once_and_search_reads_the_text() {
    let setup = Setup::new();
    let (bin, log) = fake_claude(setup.dir.path());
    let core = core_with(&setup, bin);
    record(
        &setup,
        "th-1",
        "Quiz 4",
        "The quiz moves to Thursday in Swan Hall.",
    );
    record(&setup, "th-2", "Lab 5", "Bring your breadboard.");
    let place = |core: &Core, id: &str| snap(core, "2026-10-07")["mailState"][id].clone();
    assert_eq!(
        place(&core, "th-1"),
        json!({"unread": true, "archived": false})
    );

    ok(
        &core,
        "heat.mail.mark",
        json!({"threadId": "th-1", "unread": false}),
    );
    ok(
        &core,
        "heat.mail.archive",
        json!({"threadId": "th-1", "archived": true}),
    );
    assert_eq!(
        place(&core, "th-1"),
        json!({"unread": false, "archived": true})
    );
    assert_eq!(
        place(&core, "th-2"),
        json!({"unread": true, "archived": false})
    );
    ok(
        &core,
        "heat.mail.archive",
        json!({"threadId": "th-1", "archived": false}),
    );
    assert_eq!(place(&core, "th-1")["archived"], false);
    // Marks and archives wait for the worker's minute: no run is started for them.
    std::thread::sleep(Duration::from_millis(300));
    assert!(runs(&log).is_empty());
    let waiting: Vec<Value> = ok(&core, "heat.mail.outbox", json!({}))["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["kind"].clone())
        .collect();
    assert_eq!(
        waiting,
        [json!("markRead"), json!("unarchive")],
        "a change of mind replaces the one waiting"
    );

    assert_eq!(
        ok(&core, "heat.mail.search", json!({"q": "swan thursday"}))["threadIds"],
        json!(["th-1"])
    );
    assert_eq!(
        ok(&core, "heat.mail.search", json!({"q": "breadboard"}))["threadIds"],
        json!(["th-2"])
    );
    assert_eq!(
        ok(&core, "heat.mail.search", json!({"q": "prof"}))["threadIds"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        ok(&core, "heat.mail.search", json!({"q": "nothing like it"}))["threadIds"],
        json!([])
    );

    assert_eq!(
        refused(&core, "heat.mail.archive", json!({"threadId": "nope"})).1,
        "That thread isn't in Mail."
    );
    assert_eq!(
        refused(
            &core,
            "heat.mail.send",
            json!({"to": "p@uri.edu", "subject": "Hi", "body": " "})
        )
        .1,
        "Write the mail first."
    );
    assert_eq!(
        refused(
            &core,
            "heat.mail.send",
            json!({"to": "someone", "subject": "Hi", "body": "x"})
        )
        .1,
        "\"someone\" isn't a mail address (To)."
    );
    assert_eq!(
        refused(&core, "heat.mail.delete", json!({"threadId": "th-1"})).1,
        "Mail has no command called heat.mail.delete."
    );
    ok(&core, "heat.mail.background.set", json!({"on": false}));
    assert_eq!(snap(&core, "2026-10-07")["mailSync"]["background"], false);
}

#[test]
fn a_mail_that_cant_go_says_why_and_can_be_tried_again_or_thrown_away() {
    let setup = Setup::new();
    let core = core_with(&setup, setup.dir.path().join("no-claude-here"));
    let sent = ok(
        &core,
        "heat.mail.send",
        json!({"to": "Prof <p@uri.edu>", "subject": "Lab 6", "body": "When is it due?"}),
    );
    let id = sent["action"]["id"].as_str().unwrap().to_string();
    let action = |core: &Core| {
        ok(core, "heat.mail.outbox", json!({}))["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"] == id.as_str())
            .cloned()
    };
    wait_for("the run to fail", || {
        action(&core).is_some_and(|a| a["status"] == "failed")
    });
    let failed = action(&core).unwrap();
    assert!(
        failed["error"]
            .as_str()
            .unwrap()
            .ends_with("which isn't there."),
        "{failed}"
    );
    assert_eq!(failed["body"], "When is it due?", "the mail is kept");
    assert_eq!(snap(&core, "2026-10-07")["mailSync"]["failed"], 1);

    ok(&core, "heat.mail.outbox.retry", json!({"id": id}));
    wait_for("it to fail again", || {
        action(&core).is_some_and(|a| a["status"] == "failed")
    });
    ok(&core, "heat.mail.outbox.discard", json!({"id": id}));
    assert!(action(&core).is_none());
    assert_eq!(
        refused(&core, "heat.mail.outbox.discard", json!({"id": id})).1,
        "No action in the outbox has that id."
    );
}
