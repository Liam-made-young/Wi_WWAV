//! Mail in the core (docs/SPEC.md 3.10): the commands Mail's tab calls, and
//! the worker that keeps the mailbox and Learn in step.
//!
//! The app holds no password and no token for any mailbox. What moves mail
//! is the person's own Claude Code, run from here (claude_cli.rs) with their
//! Gmail connector, in two jobs that never share a run:
//!
//! - **Read.** It may search and read mail and record it in Learn. It has
//!   no tool that sends, labels or deletes, so whatever a message says to
//!   it, the most it can do is record something wrong, which is undoable.
//! - **Send.** It may send, reply and move threads between labels, and read
//!   and finish the outbox. It has no tool that reads mail, so nothing but
//!   what the person wrote in Learn can reach it.
//!
//! Sending and replying start a run at once. Marks and archives wait for
//! the worker's next minute, so a burst of them is one run. Mail is read
//! when asked (Sync), and on its own every half hour while the app is open
//! and the person hasn't turned that off.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{json, Value};
use wi_heat_store::{all, kind, mail, snapshot};

use crate::args::Args;
use crate::bus::lock;
use crate::claude_cli::{self, Ask};
use crate::heat_cmd::{core_error, open_heat, wrote_outside};
use crate::{CoreError, Inner};

/// The person's Gmail connector in Claude Code, as its tools are named.
const GMAIL: &str = "mcp__claude_ai_Gmail__";
/// Wi_WWAV's own MCP server, as Settings → Claude's lines name it.
const LEARN: &str = "mcp__wi-wwav__";

/// What the read job may call: read mail, and record it. Nothing that sends.
const READ_TOOLS: [(&str, &str); 10] = [
    (GMAIL, "search_threads"),
    (GMAIL, "get_thread"),
    (LEARN, "list_mail_accounts"),
    (LEARN, "list_mail"),
    (LEARN, "list_tasks"),
    (LEARN, "get_grades"),
    (LEARN, "add_task"),
    (LEARN, "add_pending_grade"),
    (LEARN, "record_mail_thread"),
    (LEARN, "save_mail_text"),
];

/// What the send job may call: the outbox, and Gmail's writes. Nothing that reads mail.
const SEND_TOOLS: [(&str, &str); 6] = [
    (LEARN, "list_mail_outbox"),
    (LEARN, "finish_mail_action"),
    (GMAIL, "send_message"),
    (GMAIL, "reply"),
    (GMAIL, "label_thread"),
    (GMAIL, "unlabel_thread"),
];

/// The smallest model does both jobs, and they run often.
const MODEL: &str = "haiku";
const READ_EVERY_MS: f64 = 30.0 * 60_000.0;
/// The first read goes back a week; the ones after, two days.
const FIRST_DAYS: i64 = 7;
const LATER_DAYS: i64 = 2;

/// What the worker has been asked for since it last looked, and what it is
/// doing now.
#[derive(Default)]
pub(crate) struct Mailbox {
    send: AtomicBool,
    read: AtomicBool,
    /// "reading" or "sending" while a run is on, for the status line.
    doing: Mutex<Option<&'static str>>,
}

fn names(tools: &[(&str, &str)]) -> Vec<String> {
    tools
        .iter()
        .map(|(server, tool)| format!("{server}{tool}"))
        .collect()
}

fn run(
    i: &Inner,
    what: &'static str,
    prompt: &str,
    tools: &[(&str, &str)],
    minutes: u64,
) -> Result<String, CoreError> {
    *lock(&i.mail.doing) = Some(what);
    wrote_outside(i, &["heatSetting"]);
    let tools = names(tools);
    let tools: Vec<&str> = tools.iter().map(String::as_str).collect();
    let answer = claude_cli::run(
        i,
        &Ask {
            prompt,
            allowed_tools: &tools,
            json_schema: None,
            model: Some(MODEL),
            timeout: Duration::from_secs(minutes * 60),
        },
    );
    *lock(&i.mail.doing) = None;
    answer
}

/// The send job: everything waiting in the outbox, done in Gmail.
fn send(i: &Inner) {
    if mail::outbox(&i.store()).map_or(true, |o| o.is_empty()) {
        return;
    }
    let before: Vec<Value> = mail::outbox(&i.store()).unwrap_or_default();
    let ran = run(i, "sending", &wi_mcp::prompts::send_mail(), &SEND_TOOLS, 5);
    // What the run left waiting didn't go. A run that never started (Claude
    // Code missing, signed out) fails each one with why, so the person sees
    // it in the outbox rather than waiting on nothing.
    if let Err(e) = ran {
        let clock = i.clock();
        let mut store = i.store();
        let still: Vec<Value> = mail::outbox(&store).unwrap_or_default();
        for a in before
            .iter()
            .filter(|a| still.iter().any(|s| s["id"] == a["id"]))
        {
            if let Some(id) = a["id"].as_str() {
                let _ = mail::finish(&mut store, &clock, id, false, &e.message);
            }
        }
    }
    wrote_outside(i, &["mailThread", "heatSetting"]);
}

/// The read job: new mail recorded, and the line that says how it went.
fn read(i: &Inner) {
    let count = |i: &Inner| all(&i.store(), kind::MAIL).map_or(0, |t| t.len());
    let before = count(i);
    let first = mail::sync_status(&i.store())
        .ok()
        .map_or(true, |s| s.is_null());
    let days = if first { FIRST_DAYS } else { LATER_DAYS };
    let ran = run(
        i,
        "reading",
        &wi_mcp::prompts::read_mail(days, None),
        &READ_TOOLS,
        15,
    );
    let clock = i.clock();
    let now = jiff::Timestamp::now().as_millisecond() as f64;
    let at = snapshot::clock_text(&clock, now);
    let line = match ran {
        Ok(_) => match count(i).saturating_sub(before) {
            0 => format!("Mail read {at}: nothing new"),
            1 => format!("Mail read {at}: 1 new thread"),
            n => format!("Mail read {at}: {n} new threads"),
        },
        Err(e) => format!("Couldn't read mail at {at}. {}", e.message),
    };
    let _ = mail::set_sync_status(&mut i.store(), now, &line);
    wrote_outside(i, &["mailThread", "heatSetting"]);
}

/// The worker: one run at a time, sending before reading.
pub(crate) fn start(inner: &Arc<Inner>, background: bool) -> JoinHandle<()> {
    let i = inner.clone();
    std::thread::Builder::new()
        .name("wi-mail".into())
        .spawn(move || {
            while !i.closing() {
                let asked_send = i.mail.send.swap(false, Ordering::Relaxed);
                let asked_read = i.mail.read.swap(false, Ordering::Relaxed);
                // Marks and archives were not sent for: they go on the minute, together.
                let waiting = background && mail::outbox(&i.store()).is_ok_and(|o| !o.is_empty());
                if asked_send || waiting {
                    send(&i);
                }
                let due = background
                    && mail::background(&i.store()).unwrap_or(true)
                    && mail::sync_status(&i.store()).ok().is_some_and(|s| {
                        let now = jiff::Timestamp::now().as_millisecond() as f64;
                        s["at"]
                            .as_f64()
                            .map_or(true, |at| now - at >= READ_EVERY_MS)
                    });
                if (asked_read || due) && !i.closing() {
                    read(&i);
                }
                // Asked again while a run was on: go round at once.
                if i.mail.send.load(Ordering::Relaxed) || i.mail.read.load(Ordering::Relaxed) {
                    continue;
                }
                i.nap(Duration::from_secs(60));
            }
        })
        .expect("a thread for Learn's mail")
}

/// What the views draw of the mailbox, for the snapshot: each thread's
/// place, the outbox's counts, and the line about the last read.
pub(crate) fn for_snapshot(i: &Inner, snap: &mut Value) -> Result<(), CoreError> {
    let store = i.store();
    snap["mailState"] = mail::states(&store).map_err(core_error)?;
    let actions = mail::actions(&store).map_err(core_error)?;
    let count = |status: &str| actions.iter().filter(|a| a["status"] == status).count();
    let status = mail::sync_status(&store).map_err(core_error)?;
    snap["mailSync"] = json!({
        "line": status["line"],
        "at": status["at"],
        "doing": *lock(&i.mail.doing),
        "background": mail::background(&store).map_err(core_error)?,
        "queued": count("queued"),
        "failed": count("failed"),
    });
    Ok(())
}

fn queue(i: &Inner, action: Value, now: bool) -> Result<Value, CoreError> {
    open_heat(i)?;
    let clock = i.clock();
    let doc = mail::queue(
        &mut i.store(),
        &clock,
        action.as_object().expect("an object"),
    )
    .map_err(core_error)?;
    wrote_outside(i, &["mailThread", "heatSetting"]);
    if now {
        i.mail.send.store(true, Ordering::Relaxed);
        i.poke();
    }
    Ok(json!({ "action": doc }))
}

/// The `heat.mail.*` commands this file answers; None for any other.
pub(crate) fn call(i: &Inner, cmd: &str, a: &Args) -> Option<Result<Value, CoreError>> {
    Some(match cmd {
        // `heat.mail.send {to, cc?, subject, body, account?}`: a new mail, into the outbox and out.
        "heat.mail.send" => {
            let mut action = Value::Object(a.object());
            action["kind"] = json!("send");
            queue(i, action, true)
        }
        // `heat.mail.reply {threadId, body, to?, cc?}`
        "heat.mail.reply" => {
            let mut action = Value::Object(a.object());
            action["kind"] = json!("reply");
            queue(i, action, true)
        }
        // `heat.mail.archive {threadId, archived}`: out of the inbox here at once, in Gmail within the minute.
        "heat.mail.archive" => (|| {
            let archived = a.opt_bool("archived")?.unwrap_or(true);
            let what = if archived { "archive" } else { "unarchive" };
            queue(
                i,
                json!({"kind": what, "threadId": a.str("threadId")?}),
                false,
            )
        })(),
        // `heat.mail.mark {threadId, unread}`
        "heat.mail.mark" => (|| {
            let unread = a.opt_bool("unread")?.unwrap_or(false);
            let what = if unread { "markUnread" } else { "markRead" };
            queue(
                i,
                json!({"kind": what, "threadId": a.str("threadId")?}),
                false,
            )
        })(),
        // `heat.mail.search {q}` → `{threadIds}`: every word, in the subject, the sender or the text.
        "heat.mail.search" => (|| {
            open_heat(i)?;
            let found =
                mail::search(&i.store(), a.opt_str("q").unwrap_or("")).map_err(core_error)?;
            Ok(json!({ "threadIds": found }))
        })(),
        // `heat.mail.outbox {}` → `{actions}`: waiting, failed, and the last twenty done.
        "heat.mail.outbox" => (|| {
            open_heat(i)?;
            Ok(json!({ "actions": mail::actions(&i.store()).map_err(core_error)? }))
        })(),
        // `heat.mail.outbox.retry {id}` and `.discard {id}`
        "heat.mail.outbox.retry" | "heat.mail.outbox.discard" => (|| {
            let retry = cmd.ends_with("retry");
            mail::retry_or_discard(&mut i.store(), a.str("id")?, retry).map_err(core_error)?;
            wrote_outside(i, &["heatSetting"]);
            if retry {
                i.mail.send.store(true, Ordering::Relaxed);
                i.poke();
            }
            Ok(json!({}))
        })(),
        // `heat.mail.sync {}`: send what waits, then read what is new. Answers at once; the
        // snapshot's `mailSync` says how it is going.
        "heat.mail.sync" => {
            i.mail.send.store(true, Ordering::Relaxed);
            i.mail.read.store(true, Ordering::Relaxed);
            i.poke();
            Ok(json!({ "started": true }))
        }
        // `heat.mail.background.set {on}`: whether mail is read on its own.
        "heat.mail.background.set" => (|| {
            let on = a.opt_bool("on")?.unwrap_or(true);
            mail::set_background(&mut i.store(), on).map_err(core_error)?;
            wrote_outside(i, &["heatSetting"]);
            Ok(json!({}))
        })(),
        _ => return None,
    })
}
