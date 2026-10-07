//! The `heat.*` commands (docs/HEAT.md): thin wrappers over `wi-heat-store`.
//! Each write is one store function in one journal entry; the core then
//! carries the entry's records to sync and tells the views, with the `heat`
//! event (and `records` and `history`, which the shell listens to).

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};
use wi_heat_store::snapshot::Window;
use wi_heat_store::{ops, snapshot, timer, Outcome};
use wi_store::{DocChange, Store};

use crate::args::Args;
use crate::{calendars, claude, heat, history, CoreError, Inner};

/// A refusal from the store is a sentence for the person.
pub(crate) fn core_error(e: wi_heat_store::Error) -> CoreError {
    match e {
        wi_heat_store::Error::Refused(why) => CoreError::new("refused", why),
        wi_heat_store::Error::Store(e) => e.into(),
    }
}

/// Tells the views and sync about records that changed: the `heat` event
/// with their kinds, `records` for the shell, and the Edit menus.
pub(crate) fn announce(i: &Inner, docs: &[DocChange], outside: &[&str]) -> Result<(), CoreError> {
    heat::wrote_entry(i, docs)?;
    let mut kinds: BTreeSet<String> = docs.iter().map(|d| d.kind.clone()).collect();
    kinds.extend(outside.iter().map(|k| k.to_string()));
    if kinds.is_empty() {
        return Ok(());
    }
    let kinds: Vec<String> = kinds.into_iter().collect();
    i.bus.emit("heat", json!({ "kinds": kinds }));
    history::records_changed(i, kinds);
    Ok(())
}

/// Runs one write: the store function under the library's lock, then the
/// announcement without it.
pub(crate) fn write(
    i: &Inner,
    f: impl FnOnce(&mut Store, &wi_heat_store::Clock) -> wi_heat_store::Result<Outcome>,
) -> Result<Value, CoreError> {
    let clock = i.clock();
    let (outcome, docs) = {
        let mut store = i.store();
        let outcome = f(&mut store, &clock).map_err(core_error)?;
        let mut docs = Vec::new();
        if let Some(txn) = &outcome.txn {
            // The watcher is to report the helper's entries, not this one.
            i.note_own(txn);
            docs = store.entry_docs(txn)?.map(|e| e.docs).unwrap_or_default();
        }
        (outcome, docs)
    };
    announce(i, &docs, &outcome.outside)?;
    Ok(outcome.value())
}

fn object<'a>(a: &'a Args, key: &str) -> Result<&'a Map<String, Value>, CoreError> {
    a.get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| CoreError::new("bad_args", format!("This command needs {key}, an object.")))
}

/// Opening Heat makes its first spaces; sync carries them.
pub(crate) fn open_heat(i: &Inner) -> Result<(), CoreError> {
    let made = {
        let mut store = i.store();
        ops::ensure_defaults(&mut store).map_err(core_error)?
    };
    if !made.is_empty() {
        heat::wrote_made(i, &made)?;
        i.bus.emit("heat", json!({"kinds": ["space"]}));
    }
    Ok(())
}

pub(crate) fn invoke(i: &Inner, cmd: &str, a: &Args) -> Result<Value, CoreError> {
    match cmd {
        // ----- reading -----
        "heat.snapshot" => {
            open_heat(i)?;
            let window = Window {
                date: a.opt_str("date").map(String::from),
                from: a.opt_str("from").map(String::from),
                to: a.opt_str("to").map(String::from),
                synced_at: i.kv.get("heat.syncedAt")?.and_then(|v| v.as_f64()),
            };
            snapshot::snapshot(&i.store(), &i.clock(), &window).map_err(core_error)
        }
        "heat.whatItWouldTake" => {
            snapshot::what_it_would_take(&i.store(), a.str("courseId")?, a.str("letter")?)
                .map_err(core_error)
        }
        "heat.review.week" => {
            snapshot::review_week(&i.store(), &i.clock(), a.str("weekStart")?).map_err(core_error)
        }
        "heat.publicView" => snapshot::public_view(&i.store(), &i.clock()).map_err(core_error),

        // ----- records -----
        "heat.put" => {
            let (kind, record) = (
                a.str("kind")?,
                a.get("record").cloned().unwrap_or(Value::Null),
            );
            write(i, |s, c| ops::put_record(s, c, kind, &record))
        }
        "heat.patch" => {
            let (kind, id, set) = (a.str("kind")?, a.str("id")?, object(a, "set")?);
            write(i, |s, c| ops::patch_record(s, c, kind, id, set))
        }
        "heat.delete" => {
            let (kind, id) = (a.str("kind")?, a.str("id")?);
            write(i, |s, c| ops::delete_record(s, c, kind, id))
        }
        "heat.done" => {
            let (id, done) = (
                a.str("taskId")?,
                a.opt_bool("done")?.ok_or_else(|| {
                    CoreError::new("bad_args", "heat.done needs done, true or false.")
                })?,
            );
            let date = a.opt_str("date");
            write(i, |s, c| ops::done(s, c, id, done, date))
        }
        "heat.estimate" => {
            let id = a.str("taskId")?;
            let difficulty = a.opt_f64("difficulty")?;
            // `estMin: null` clears the estimate; leaving it out leaves it be.
            let est = match a.raw("estMin") {
                None => None,
                Some(Value::Null) => Some(None),
                Some(_) => Some(Some(a.f64("estMin")?)),
            };
            write(i, |s, c| ops::estimate(s, c, id, difficulty, est))
        }
        "heat.tookTime" => {
            let (id, minutes) = (a.str("taskId")?, a.f64("minutes")?);
            write(i, |s, c| ops::took_time(s, c, id, minutes))
        }

        // ----- Today -----
        "heat.plan.make" => {
            let (date, ends) = (a.opt_str("date"), a.opt_f64("dayEnds")?);
            write(i, |s, c| timer::plan_make(s, c, date, ends))
        }
        "heat.plan.accept" => {
            let ids = if a.get("taskIds").is_some() {
                Some(a.strings("taskIds")?)
            } else {
                None
            };
            let date = match a.opt_str("date") {
                Some(d) => d.to_string(),
                None => i.clock().today(),
            };
            write(i, |s, c| timer::plan_accept(s, c, &date, ids.as_deref()))
        }
        "heat.plan.clear" => write(i, |s, _| timer::plan_clear(s)),
        "heat.block.put" => {
            let args = a.object().clone();
            write(i, |s, c| ops::put_block(s, c, &args))
        }
        "heat.current.set" => {
            let id = a.opt_str("taskId");
            write(i, |s, c| timer::set_current(s, c, id))
        }
        "heat.focus.start"
        | "heat.focus.pause"
        | "heat.focus.resume"
        | "heat.focus.interrupt"
        | "heat.focus.stop"
        | "heat.focus.finish" => {
            let step = cmd.trim_start_matches("heat.focus.");
            let (task, length) = (a.opt_str("taskId"), a.opt_f64("length")?);
            write(i, |s, c| timer::focus(s, c, step, task, length))
        }

        // ----- capture, notes, the weekly review -----
        "heat.capture.add" => {
            let (text, link) = (a.str("text")?, a.get("link").cloned());
            write(i, |s, c| ops::capture_add(s, c, text, link.as_ref()))
        }
        "heat.capture.triage" => {
            let (id, to) = (a.str("id")?, a.str("to")?);
            let record = a.get("record").and_then(Value::as_object);
            write(i, |s, c| ops::capture_triage(s, c, id, to, record))
        }
        "heat.review.complete" => {
            let (week, note) = (a.str("weekStart")?, a.str("note")?);
            write(i, |s, c| ops::review_complete(s, c, week, note))
        }

        // ----- grades, privacy and what is shown -----
        "heat.score" => {
            let (id, score, out_of) = (a.str("gradeId")?, a.f64("score")?, a.opt_f64("outOf")?);
            write(i, |s, c| ops::score(s, c, id, score, out_of))
        }
        "heat.public.set" => {
            let (kind, id) = (a.str("kind")?, a.str("id")?);
            let public = a.opt_bool("public")?.ok_or_else(|| {
                CoreError::new("bad_args", "heat.public.set needs public, true or false.")
            })?;
            write(i, |s, c| ops::set_public(s, c, kind, id, public))
        }
        "heat.share.now" => {
            let (id, text) = (a.str("taskId")?, a.str("text")?);
            write(i, |s, c| ops::share_now(s, c, id, text))
        }
        "heat.share.timeline" => {
            let (project, target) = (a.str("projectId")?, a.str("targetId")?);
            write(i, |s, c| ops::share_timeline(s, c, project, target))
        }
        "heat.share.hide" => {
            let id = a.str("id")?;
            write(i, |s, c| ops::share_hide(s, c, id))
        }

        // ----- moving in -----
        "heat.import" => {
            let json = a.get("json").cloned().unwrap_or(Value::Null);
            write(i, |s, c| ops::import(s, c, &json))
        }

        // ----- calendars and the school -----
        "heat.calendars.add" => calendars::add(i, a),
        "heat.calendars.remove" => calendars::remove(i, a),
        "heat.calendars.sync" => calendars::sync(i, a),
        "heat.school.set" => calendars::set_school(i, a),

        // ----- Settings → Claude -----
        "heat.claude.get" => claude::get(i),
        "heat.claude.setTool" => claude::set_tool(i, a),

        _ => Err(CoreError::new(
            "unknown_command",
            format!("There is no command called '{cmd}'."),
        )),
    }
}

/// Settings written outside the journal, announced to the views.
pub(crate) fn wrote_outside(i: &Inner, kinds: &[&str]) {
    let kinds: Vec<String> = kinds.iter().map(|k| k.to_string()).collect();
    i.bus.emit("heat", json!({ "kinds": kinds }));
}

/// A calendar's records changed outside the journal, announced.
pub(crate) fn calendars_changed(i: &Inner) {
    wrote_outside(i, &["calendar", "calendarEvent"]);
}
