//! The Focus layout in the core (docs/FOCUS.md): `snapshot.focus`, the
//! `entropy` event, and the commands that dismiss an interrupt, raise one,
//! and put the fix off for the day. The rules are focus.rs's; this file only
//! keeps what they need between snapshots and tells the views.
//!
//! What is kept is in `core_kv` (kv.rs), so none of it is journaled and ⌘Z
//! never touches it: `focus.memory` (the queue of events that happened, what
//! was dismissed and when, the day the fix was put off) and `focus.entropy`
//! (the entropy last told to the views).

use std::sync::Mutex;

use serde_json::{json, Value};
use wi_store::DocChange;

use crate::args::Args;
use crate::bus::lock;
use crate::focus::{self, Action, Config, Do, Event, Memory, Priority};
use crate::heat_cmd::wrote_outside;
use crate::{CoreError, Inner};

const MEMORY: &str = "focus.memory";
const ENTROPY: &str = "focus.entropy";
/// The queue holds this many events, the newest.
const QUEUE_MAX: usize = 50;
/// A dismissal is forgotten after this long.
const DISMISSED_KEEP_MS: f64 = 30.0 * 24.0 * 3_600_000.0;

/// One change to the memory at a time: the watcher queues while a command dismisses.
static CHANGING: Mutex<()> = Mutex::new(());

fn memory(i: &Inner) -> Result<Memory, CoreError> {
    Ok(i.kv
        .get(MEMORY)?
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

fn change(i: &Inner, f: impl FnOnce(&mut Memory)) -> Result<(), CoreError> {
    let _one = lock(&CHANGING);
    let mut mem = memory(i)?;
    f(&mut mem);
    if mem.queue.len() > QUEUE_MAX {
        let over = mem.queue.len() - QUEUE_MAX;
        mem.queue.drain(..over);
    }
    let stored = serde_json::to_value(&mem)
        .map_err(|e| CoreError::new("library", format!("Focus couldn't be saved: {e}")))?;
    i.kv.set(MEMORY, &stored)
}

/// Sets `snapshot.focus`, and tells the views when the entropy is no longer
/// the one they were last told: the `entropy` event, `{score, level, parts}`.
pub(crate) fn for_snapshot(i: &Inner, snap: &mut Value) -> Result<(), CoreError> {
    let state = focus::focus_state(snap, &memory(i)?, &Config::default());
    let entropy = state["entropy"].clone();
    snap["focus"] = state;
    if i.kv.get(ENTROPY)?.as_ref() != Some(&entropy) {
        i.kv.set(ENTROPY, &entropy)?;
        i.bus.emit("entropy", entropy);
    }
    Ok(())
}

/// Records another process wrote (Claude through the MCP helper, as the mail
/// job does), queued as events: a due date moved, or mail made a task. The
/// person's own edits never come here (watch.rs).
pub(crate) fn noticed(i: &Inner, docs: &[DocChange]) {
    let events = focus::events_from_changes(docs, i.clock().now_ms);
    if events.is_empty() {
        return;
    }
    let _ = change(i, |mem| {
        for e in events {
            if !mem.queue.contains(&e) {
                mem.queue.push(e);
            }
        }
    });
}

fn is_custom(e: &Event, with_id: &str) -> bool {
    matches!(e, Event::Custom { id, .. } if id == with_id)
}

/// The `focus` object of a snapshot for the day asked for.
fn state(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let mut snap = crate::heat_cmd::invoke(i, "heat.snapshot", a)?;
    Ok(snap["focus"].take())
}

pub(crate) fn invoke(i: &Inner, cmd: &str, a: &Args) -> Result<Value, CoreError> {
    match cmd {
        // `heat.focus.state {date?}`: what `snapshot.focus` holds.
        "heat.focus.state" => state(i, a),
        // `heat.entropy {date?}`: `{score, level, parts}`.
        "heat.entropy" => {
            let mut focus = state(i, a)?;
            Ok(focus["entropy"].take())
        }
        // `heat.interrupt.dismiss {id}`: the line goes, and doesn't come back.
        "heat.interrupt.dismiss" => {
            let id = a.str("id")?;
            let now = i.clock().now_ms;
            change(i, |mem| {
                mem.dismissed.insert(id.to_string(), now);
                mem.dismissed.retain(|_, at| *at >= now - DISMISSED_KEEP_MS);
                mem.queue.retain(|e| !is_custom(e, id));
            })?;
            wrote_outside(i, &["focus"]);
            Ok(json!({}))
        }
        // `heat.interrupt.raise {id, source, line, action?, changesNext, priority?}`:
        // a view's own event, queued for `should_interrupt` like any other.
        "heat.interrupt.raise" => {
            let (id, source, line) = (a.str("id")?, a.str("source")?, a.str("line")?);
            if id.trim().is_empty() || line.trim().is_empty() {
                return Err(CoreError::new(
                    "bad_args",
                    "heat.interrupt.raise needs an id and a line, neither empty.",
                ));
            }
            let changes_next = a.opt_bool("changesNext")?.ok_or_else(|| {
                CoreError::new(
                    "bad_args",
                    "heat.interrupt.raise needs changesNext, true or false.",
                )
            })?;
            let action: Option<Action> = match a.get("action") {
                None => None,
                Some(v) => Some(serde_json::from_value(v.clone()).map_err(|_| {
                    CoreError::new(
                        "bad_args",
                        "heat.interrupt.raise needs action, an object with a label and what it does: current, task, view, break, plan or command.",
                    )
                })?),
            };
            // An action may run a command, and only one of Learn's own.
            if let Some(act) = action.as_ref().filter(|x| x.act == Do::Command) {
                if !act.cmd.as_deref().is_some_and(|c| c.starts_with("heat.")) {
                    return Err(CoreError::new(
                        "bad_args",
                        "A command action needs cmd, one of Learn's own commands (heat.…).",
                    ));
                }
            }
            let priority: Priority = match a.get("priority") {
                None => Priority::Normal,
                Some(v) => serde_json::from_value(v.clone()).map_err(|_| {
                    CoreError::new(
                        "bad_args",
                        "heat.interrupt.raise needs priority, one of low, normal or high.",
                    )
                })?,
            };
            let event = Event::Custom {
                id: id.to_string(),
                source: source.to_string(),
                line: line.to_string(),
                action,
                changes_next,
                priority,
                at: i.clock().now_ms,
            };
            change(i, |mem| {
                mem.queue.retain(|e| !is_custom(e, id));
                mem.queue.push(event);
            })?;
            wrote_outside(i, &["focus"]);
            Ok(json!({"queued": true}))
        }
        // `heat.focus.snooze`: the fix isn't offered again today.
        "heat.focus.snooze" => {
            let today = i.clock().today();
            change(i, |mem| mem.fix_snoozed = Some(today))?;
            wrote_outside(i, &["focus"]);
            Ok(json!({}))
        }
        _ => Err(CoreError::new(
            "unknown_command",
            format!("There is no command called '{cmd}'."),
        )),
    }
}
