//! Heat's private sync and Claude (docs/SPEC.md 2.8, 2.11, 3.12, 9.7), over
//! wi-heat's rules: wi-core does the HTTP and the storage.
//!
//! Every field of a record written in Heat goes into this library's
//! [`Replica`] with a stamp, and waits to go up. A round pushes what waits
//! to `/api/heat/changes` and pulls what changed since the cursor; the
//! replica keeps the newer stamp field by field, so a slow older write never
//! overwrites a newer one. Records another device changed are written into
//! the library as one journal entry ("changes from your other devices"), so
//! undo stays exact; an undo in Heat is a new local change and syncs like
//! one. Grades stay on this Mac (Open, 9.8, at its recommendation).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{json, Map, Value};
use wi_heat::assist::{self, Failure, Outcome, Step};
use wi_heat::sync::{Change, Key, Page, Replica, Saved};

use crate::args::Args;
use crate::bus::lock;
use crate::net::{encode, Fail};
use crate::{history, CoreError, Inner};

const REPLICA: &str = "heat.replica";
const ROUND: Duration = Duration::from_secs(30);
/// After a change, wait this long for more before pushing.
const SETTLE: Duration = Duration::from_secs(2);
const PULL_LIMIT: usize = 500;
pub const REMOTE_LABEL: &str = "changes from your other devices";

/// One sync round at a time; and the replica's load-change-save held only
/// briefly, never across a request, so an edit never waits on the network.
static ONE_ROUND: Mutex<()> = Mutex::new(());
static HELD: Mutex<()> = Mutex::new(());

/// Loads the replica, lets `f` change it, and saves it.
fn with_replica<T>(i: &Inner, f: impl FnOnce(&mut Replica) -> T) -> Result<T, CoreError> {
    let _held = lock(&HELD);
    let mut r = load(i)?;
    let out = f(&mut r);
    save(i, &r)?;
    Ok(out)
}

/// Kinds that never leave this Mac.
fn local_only(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("grade")
}

fn load(i: &Inner) -> Result<Replica, CoreError> {
    match i.kv.get(REPLICA)? {
        Some(v) => serde_json::from_value::<Saved>(v)
            .map(Replica::restore)
            .map_err(|e| CoreError::new("library", format!("Heat's sync state doesn't read: {e}"))),
        None => Ok(Replica::new(&wwav_ids::ulid())),
    }
}

fn save(i: &Inner, r: &Replica) -> Result<(), CoreError> {
    i.kv.set(REPLICA, &serde_json::to_value(r.save()).unwrap_or_default())
}

/// Local edits made in Heat, field by field, waiting to go up.
pub(crate) fn wrote(i: &Inner, changes: &[(&str, &str, &str, &Value)]) -> Result<(), CoreError> {
    let synced: Vec<_> = changes.iter().filter(|c| !local_only(c.0)).collect();
    if synced.is_empty() {
        return Ok(());
    }
    with_replica(i, |r| {
        for (kind, id, field, value) in synced {
            r.write(kind, id, field, (*value).clone());
        }
    })?;
    i.poke();
    Ok(())
}

/// After an undo or redo in Heat: every record that now differs from what
/// the replica holds is a new local change.
pub(crate) fn journal_moved(i: &Inner) -> Result<(), CoreError> {
    let _held = lock(&HELD);
    let mut r = load(i)?;
    let snapshot = r.snapshot();
    let kinds: BTreeSet<String> = snapshot.keys().map(|k| k.0.clone()).collect();
    let mut writes: Vec<(String, String, String, Value)> = Vec::new();
    for kind in &kinds {
        let docs = i.store().docs(kind)?;
        let mut present = BTreeSet::new();
        for d in docs {
            present.insert(d.key.clone());
            let gone = snapshot.get(&(kind.clone(), d.key.clone(), "deleted".into()))
                == Some(&json!(true));
            if gone {
                writes.push((kind.clone(), d.key.clone(), "deleted".into(), json!(false)));
            }
            for (field, value) in d.json.as_object().into_iter().flatten() {
                let k: Key = (kind.clone(), d.key.clone(), field.clone());
                if gone || snapshot.get(&k) != Some(value) {
                    writes.push((kind.clone(), d.key.clone(), field.clone(), value.clone()));
                }
            }
        }
        let ids: BTreeSet<&String> = snapshot
            .keys()
            .filter(|k| &k.0 == kind)
            .map(|k| &k.1)
            .collect();
        for id in ids.into_iter().filter(|id| !present.contains(*id)) {
            if snapshot.get(&(kind.clone(), id.clone(), "deleted".into())) != Some(&json!(true)) {
                writes.push((kind.clone(), id.clone(), "deleted".into(), json!(true)));
            }
        }
    }
    if !writes.is_empty() {
        for (kind, id, field, value) in writes {
            r.write(&kind, &id, &field, value);
        }
        save(i, &r)?;
        i.poke();
    }
    Ok(())
}

pub(crate) fn start(inner: &Arc<Inner>) -> JoinHandle<()> {
    let i = inner.clone();
    std::thread::Builder::new()
        .name("heat sync".into())
        .spawn(move || {
            while !i.closing() {
                if i.nap(ROUND) {
                    // Poked by a change: let a burst of edits settle first.
                    i.nap(SETTLE);
                }
                if i.closing() {
                    break;
                }
                if i.net.signed_in() {
                    let _ = sync_now(&i);
                }
            }
        })
        .expect("a thread for Heat sync")
}

fn change_json(c: &Change) -> Value {
    json!({"kind": c.table, "id": c.id, "field": c.field, "value": c.value, "seq": c.seq})
}

/// The mock's and the server patch's shape (`kind`) as wi-heat's (`table`).
fn change_from(v: &Value) -> Option<Change> {
    Some(Change {
        table: v.get("kind")?.as_str()?.to_string(),
        id: v.get("id")?.as_str()?.to_string(),
        field: v.get("field")?.as_str()?.to_string(),
        value: v.get("value").cloned().unwrap_or(Value::Null),
        seq: v.get("seq")?.as_u64()?,
        device: v.get("device")?.as_str()?.to_string(),
    })
}

/// "3:41 PM", by this machine's clock and zone.
fn clock_time() -> String {
    let now = jiff::Zoned::now();
    now.strftime("%-I:%M %p").to_string()
}

/// One round: push everything waiting, then pull everything new.
pub(crate) fn sync_now(i: &Inner) -> Result<Value, CoreError> {
    let _one = lock(&ONE_ROUND);
    let mut touched: BTreeSet<(String, String)> = BTreeSet::new();
    let result = round(i, &mut touched);
    apply(i, &touched)?;
    match result {
        Ok(()) => {
            let sentence = format!("Synced {}", clock_time());
            i.bus.status("sync", &sentence);
            Ok(json!({"sentence": sentence, "changed": touched.len()}))
        }
        Err(Fail::Offline) => {
            i.bus
                .status("sync", "Offline. Heat syncs when you're back.");
            Err(Fail::Offline.into())
        }
        Err(f) => Err(f.into()),
    }
}

fn round(i: &Inner, touched: &mut BTreeSet<(String, String)>) -> Result<(), Fail> {
    let held = |f: &mut dyn FnMut(&mut Replica)| {
        with_replica(i, |r| f(r)).map_err(|e| Fail::Status {
            status: 500,
            body: json!({"error": e.message}),
        })
    };
    let mut device = String::new();
    loop {
        let (mut batch, mut cursor) = (Vec::new(), 0);
        held(&mut |r| {
            device = r.save().device;
            batch = r.push_batch();
            cursor = r.cursor();
        })?;
        if batch.is_empty() {
            break;
        }
        let body = json!({
            "device": device,
            "cursor": cursor,
            "changes": batch.iter().map(change_json).collect::<Vec<_>>(),
        });
        let answer = i.net.api("POST", "/api/heat/changes", Some(&body))?;
        // Fields where the server already held something newer, and the
        // cursor moved past this device's own changes.
        let kept: Vec<Change> = answer["kept"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(change_from)
            .collect();
        touched.extend(kept.iter().map(|c| (c.table.clone(), c.id.clone())));
        let moved = answer["cursor"].as_u64().unwrap_or(cursor);
        let mut kept = Some(kept);
        held(&mut |r| {
            r.acked(&batch);
            r.pulled(Page {
                changes: kept.take().unwrap_or_default(),
                cursor: moved,
                more: false,
            });
        })?;
    }
    loop {
        let mut cursor = 0;
        held(&mut |r| cursor = r.cursor())?;
        let path = format!("/api/heat/changes?cursor={cursor}&limit={PULL_LIMIT}");
        let page = match i.net.api("GET", &path, None) {
            Ok(p) => p,
            // A cursor from before a reset or a restore: pull again from 0.
            Err(f) if f.code() == Some("cursor_ahead") => {
                held(&mut |r| {
                    let mut saved = r.save();
                    saved.cursor = 0;
                    *r = Replica::restore(saved);
                })?;
                continue;
            }
            Err(f) => return Err(f),
        };
        let changes: Vec<Change> = page["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(change_from)
            .collect();
        touched.extend(
            changes
                .iter()
                .filter(|c| c.device != device)
                .map(|c| (c.table.clone(), c.id.clone())),
        );
        let more = page["more"].as_bool().unwrap_or(false);
        let next = page["cursor"].as_u64().unwrap_or(cursor);
        let mut changes = Some(changes);
        held(&mut |r| {
            r.pulled(Page {
                changes: changes.take().unwrap_or_default(),
                cursor: next,
                more,
            })
        })?;
        if !more {
            return Ok(());
        }
    }
}

/// Writes each touched record as the replica now has it into the library.
fn apply(i: &Inner, touched: &BTreeSet<(String, String)>) -> Result<(), CoreError> {
    if touched.is_empty() {
        return Ok(());
    }
    let snapshot = with_replica(i, |r| r.snapshot())?;
    let mut records = Vec::new();
    for (kind, id) in touched {
        let fields: BTreeMap<&str, &Value> = snapshot
            .range((kind.clone(), id.clone(), String::new())..)
            .take_while(|(k, _)| &k.0 == kind && &k.1 == id)
            .map(|(k, v)| (k.2.as_str(), v))
            .collect();
        if fields.get("deleted") == Some(&&json!(true)) {
            records.push((kind.clone(), id.clone(), None));
            continue;
        }
        // Keep the record's own key order; the replica's values win.
        let mut whole: Map<String, Value> = i
            .store()
            .doc(kind, id)?
            .and_then(|d| d.json.as_object().cloned())
            .unwrap_or_default();
        for (f, v) in fields.into_iter().filter(|(f, _)| *f != "deleted") {
            whole.insert(f.to_string(), v.clone());
        }
        records.push((kind.clone(), id.clone(), Some(Value::Object(whole))));
    }
    history::apply_remote(i, REMOTE_LABEL, records)
}

// ----- Claude -----

/// The jobs `/api/assist/:task` answers, and the switch each one needs.
const TASKS: [(&str, Option<&str>); 8] = [
    ("score", Some("scoring")),
    ("score-batch", Some("scoring")),
    ("read-mail", Some("mail")),
    ("syllabus", None),
    ("review-note", None),
    ("release-plan", None),
    ("feedback", Some("feedback")),
    ("clerk", Some("clerk")),
];

fn failure_code(f: &Failure) -> &'static str {
    match f {
        Failure::NotGranted => "not_granted",
        Failure::RateLimited => "rate_limited",
        Failure::Offline => "offline",
        Failure::DailyLimit { .. } => "daily_limit",
        Failure::Unavailable => "unavailable",
        Failure::Refused => "refused",
    }
}

fn failure(f: Failure) -> CoreError {
    CoreError::new(failure_code(&f), f.sentence())
}

/// wi-heat's rules on what an answer may say, where the answer's shape is
/// the one wi-heat reads: an estimate is clamped and needs its reason, and a
/// review draft holding a number the facts don't is dropped, leaving the
/// facts alone ("never invent metrics").
fn held_to_the_rules(task: &str, body: &Value, result: Value) -> Result<Value, CoreError> {
    match task {
        "score" => assist::parse_score(&result.to_string())
            .map(|s| json!({"difficulty": s.difficulty, "minutes": s.minutes, "reason": s.reason}))
            .ok_or_else(|| failure(Failure::Unavailable)),
        "review-note" => {
            let facts: Vec<String> = body["facts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|f| f.as_str().map(String::from))
                .collect();
            Ok(json!({"draft": assist::check_review(&result.to_string(), &facts)}))
        }
        _ => Ok(result),
    }
}

/// `assist.call {task, body}`: one Claude job through mi-wwav.com, with
/// 2.11's consent, failures and one retry. The answer is a draft for the
/// person; nothing here applies it.
pub(crate) fn assist_call(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let task = a.str("task")?;
    let body = a.get("body").cloned().unwrap_or_else(|| json!({}));
    let Some((_, switch)) = TASKS.iter().find(|(t, _)| *t == task) else {
        return Err(CoreError::new(
            "bad_args",
            format!("Claude has no job called '{task}'."),
        ));
    };
    if let Some(switch) = switch {
        let settings = crate::settings::read(i)?;
        match settings["claude"][switch].as_str() {
            Some("on") => {}
            Some("off") => return Err(failure(Failure::NotGranted)),
            _ => {
                let what = match *switch {
                    "scoring" => assist::SCORING_CONSENT,
                    "mail" => assist::MAIL_CONSENT,
                    _ => "Turn this on in Settings → Claude first.",
                };
                return Err(CoreError::new("consent_needed", what));
            }
        }
    }
    if !i.net.signed_in() {
        return Err(CoreError::new(
            "signed_out",
            "Sign in to mi-wwav.com to ask Claude.",
        ));
    }
    let mut rng = rand::thread_rng();
    let mut attempt = 1;
    loop {
        let outcome = match i.net.api(
            "POST",
            &format!("/api/assist/{}", encode(task)),
            Some(&body),
        ) {
            Ok(v) => Outcome::Answered {
                status: 200,
                body: v.to_string(),
            },
            Err(Fail::Status { status, body }) => Outcome::Answered {
                status,
                body: body.to_string(),
            },
            Err(Fail::Offline) => Outcome::Offline,
            Err(Fail::SignedOut) => {
                return Err(CoreError::new(
                    "signed_out",
                    "Sign in to mi-wwav.com to ask Claude.",
                ))
            }
        };
        match assist::step(attempt, outcome, &mut rng) {
            Step::Done(Ok(text)) => {
                let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                let result = v.get("result").cloned().unwrap_or(v);
                return held_to_the_rules(task, &body, result).map(|r| json!({"result": r}));
            }
            Step::Done(Err(f)) => return Err(failure(f)),
            Step::RetryAfter(d) => {
                std::thread::sleep(d);
                attempt += 1;
            }
        }
    }
}
