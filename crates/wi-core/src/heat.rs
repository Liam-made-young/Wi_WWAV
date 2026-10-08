//! Heat's private sync (docs/SPEC.md 2.8, 8.7), over wi-heat's rules: wi-core
//! does the HTTP and the storage. The app calls no model: Claude reaches Heat
//! through the MCP server (2.11), so there is no `assist.call` here.
//!
//! Every field of a record written in any view goes into this library's
//! [`Replica`] with a stamp, and waits to go up (a milestone planned from
//! Space is Heat's as much as one planned in Heat). A round pushes what
//! waits to `/api/heat/changes` and pulls what changed since the cursor; the
//! replica keeps the newer stamp field by field, so a slow older write never
//! overwrites a newer one. Records another device changed are written into
//! the library as one journal entry ("changes from your other devices"), in
//! a journal room no view's ⌘Z acts on, so undo stays exact and a sync never
//! takes the person's redo; an undo in any view is a new local change and
//! syncs like one. Grades and courses stay on this Mac (8.7): one goes up
//! only while its Public switch is on, as a copy of the fields 3.15 lists for
//! it, and its copy is deleted when the switch goes off. A private course
//! goes up as nothing but its code and name, and only while one of its grades
//! is public, because that grade names it. A calendar record, what a
//! calendar's feed held, the timer and the settings never go up.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{json, Map, Value};
use wi_heat::sync::{Change, Key, Page, Replica, Saved};
use wi_store::DocChange;

use crate::bus::lock;
use crate::net::Fail;
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

/// Kinds that never go up: what a calendar's feed held and the calendar
/// records themselves (their addresses are in the Keychain, and another Mac
/// has no use for this one's), what isn't a record, the text of your
/// mail, which stays on this Mac (3.10), and a syllabus waiting to be accepted.
fn local_only(kind: &str) -> bool {
    [
        "calendar",
        "calendarEvent",
        "heatState",
        "heatSetting",
        "mailText",
        "mailState",
        "mailAction",
        "syllabusDraft",
    ]
    .iter()
    // The Database tab's own tables, columns and views stay on this Mac.
    .chain(crate::db::KINDS.iter())
    .any(|k| kind.eq_ignore_ascii_case(k))
}

fn is_grade(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("grade")
}

fn is_course(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("course")
}

/// Kinds that stay on this Mac unless their Public switch is on (8.7).
fn held_back(kind: &str) -> bool {
    is_grade(kind) || is_course(kind)
}

/// What a grade's public copy holds (3.15): the course it names, the item, the
/// score and what it was out of, and the switch itself.
const GRADE_COPY: [&str; 5] = ["courseId", "title", "score", "outOf", "public"];

/// What a course shows, with the switch (3.15). A private course whose grade
/// is public goes up as the first two alone.
const COURSE_COPY: [&str; 3] = ["code", "name", "public"];

/// A field written only if it isn't what the replica holds already, so a
/// change noticed twice is stamped once.
fn write_if_changed(r: &mut Replica, kind: &str, id: &str, field: &str, value: &Value) {
    if r.value(kind, id, field) != Some(value) {
        r.write(kind, id, field, value.clone());
    }
}

/// A grade, as the server should hold it: a copy in the clear while its
/// switch is on, and no copy otherwise. `now` is the grade as it stands here.
fn sync_grade(r: &mut Replica, id: &str, now: Option<&Value>) -> bool {
    match now.filter(|g| g.get("public") == Some(&json!(true))) {
        Some(grade) => {
            let mut wrote = false;
            for field in GRADE_COPY {
                let value = grade.get(field).unwrap_or(&Value::Null);
                if r.value("grade", id, field) != Some(value) {
                    r.write("grade", id, field, value.clone());
                    wrote = true;
                }
            }
            if r.value("grade", id, "deleted") == Some(&json!(true)) {
                r.write("grade", id, "deleted", json!(false));
                wrote = true;
            }
            wrote
        }
        // Switched back, or gone: the copy is deleted, once.
        None if r.value("grade", id, "public") == Some(&json!(true)) => {
            r.write("grade", id, "public", json!(false));
            r.write("grade", id, "deleted", json!(true));
            true
        }
        None => false,
    }
}

/// A course, as the server should hold it: a copy with its switch while it is
/// public; its code and name alone while a public grade names it; else nothing.
/// `now` is the course as it stands here, `named` whether a public grade does.
fn sync_course(r: &mut Replica, id: &str, now: Option<&Value>, named: bool) -> bool {
    let public = now.is_some_and(|c| c.get("public") == Some(&json!(true)));
    match now.filter(|_| public || named) {
        Some(course) => {
            let mut wrote = false;
            for field in COURSE_COPY {
                let value = match field {
                    "public" => json!(public),
                    _ => course.get(field).cloned().unwrap_or(Value::Null),
                };
                // The switch is sent only once it has been on.
                if field == "public" && !public && r.value("course", id, "public").is_none() {
                    continue;
                }
                if r.value("course", id, field) != Some(&value) {
                    r.write("course", id, field, value);
                    wrote = true;
                }
            }
            if r.value("course", id, "deleted") == Some(&json!(true)) {
                r.write("course", id, "deleted", json!(false));
                wrote = true;
            }
            wrote
        }
        // Private and named by nothing, or gone: what went up is deleted, once.
        None if r.value("course", id, "code").is_some()
            && r.value("course", id, "deleted") != Some(&json!(true)) =>
        {
            r.write("course", id, "public", json!(false));
            r.write("course", id, "deleted", json!(true));
            true
        }
        None => false,
    }
}

/// What the held-back kinds stand at in the library now: the grades named,
/// every course, and the courses a public grade names.
struct Held {
    grades: BTreeMap<String, Option<Value>>,
    courses: BTreeMap<String, Value>,
    named: BTreeSet<String>,
}

fn held_now(i: &Inner, grade_ids: &BTreeSet<String>) -> Result<Held, CoreError> {
    let store = i.store();
    let mut grades = BTreeMap::new();
    for id in grade_ids {
        grades.insert(id.clone(), store.doc("grade", id)?.map(|d| d.json));
    }
    let courses = store
        .docs("course")?
        .into_iter()
        .map(|d| (d.key, d.json))
        .collect();
    let named = store
        .docs("grade")?
        .into_iter()
        .filter(|d| d.json.get("public") == Some(&json!(true)))
        .filter_map(|d| {
            d.json
                .get("courseId")
                .and_then(Value::as_str)
                .map(String::from)
        })
        .collect();
    Ok(Held {
        grades,
        courses,
        named,
    })
}

/// Brings the replica's grades and courses to what `held` says, for the
/// grades it names and for every course there is or was. True if anything moved.
fn sync_held(r: &mut Replica, held: &Held) -> bool {
    let mut moved = false;
    for (id, grade) in &held.grades {
        moved |= sync_grade(r, id, grade.as_ref());
    }
    let mut courses: BTreeSet<String> = held.courses.keys().cloned().collect();
    courses.extend(
        r.snapshot()
            .keys()
            .filter(|k| is_course(&k.0))
            .map(|k| k.1.clone()),
    );
    for id in courses {
        moved |= sync_course(r, &id, held.courses.get(&id), held.named.contains(&id));
    }
    moved
}

fn load(i: &Inner) -> Result<Replica, CoreError> {
    match i.kv.get(REPLICA)? {
        Some(v) => serde_json::from_value::<Saved>(v)
            .map(Replica::restore)
            .map_err(|e| {
                CoreError::new("library", format!("Learn's sync state doesn't read: {e}"))
            }),
        None => Ok(Replica::new(&wwav_ids::ulid())),
    }
}

fn save(i: &Inner, r: &Replica) -> Result<(), CoreError> {
    i.kv.set(REPLICA, &serde_json::to_value(r.save()).unwrap_or_default())
}

/// Local edits made in Heat, field by field, waiting to go up.
pub(crate) fn wrote(i: &Inner, changes: &[(&str, &str, &str, &Value)]) -> Result<(), CoreError> {
    let grades: BTreeSet<String> = changes
        .iter()
        .filter(|c| is_grade(c.0))
        .map(|c| c.1.to_string())
        .collect();
    let held = changes.iter().any(|c| held_back(c.0));
    let synced: Vec<_> = changes
        .iter()
        .filter(|c| !held_back(c.0) && !local_only(c.0))
        .collect();
    if synced.is_empty() && !held {
        return Ok(());
    }
    let now = if held {
        Some(held_now(i, &grades)?)
    } else {
        None
    };
    with_replica(i, |r| {
        for (kind, id, field, value) in synced {
            r.write(kind, id, field, (*value).clone());
        }
        if let Some(now) = &now {
            sync_held(r, now);
        }
    })?;
    i.poke();
    Ok(())
}

/// A journal entry's records, noticed: each field that changed goes up, a
/// deleted record as `deleted`, and a grade by its own rule. Used for the
/// core's own Heat writes and for the MCP helper's, which only the app syncs
/// (docs/SPEC.md 8.7).
pub(crate) fn wrote_entry(i: &Inner, docs: &[DocChange]) -> Result<(), CoreError> {
    let grades: BTreeSet<String> = docs
        .iter()
        .filter(|d| is_grade(&d.kind))
        .map(|d| d.key.clone())
        .collect();
    let held = docs.iter().any(|d| held_back(&d.kind));
    let others: Vec<&DocChange> = docs
        .iter()
        .filter(|d| !held_back(&d.kind) && !local_only(&d.kind))
        .collect();
    if others.is_empty() && !held {
        return Ok(());
    }
    let now = if held {
        Some(held_now(i, &grades)?)
    } else {
        None
    };
    with_replica(i, |r| {
        for d in &others {
            let (kind, id) = (d.kind.as_str(), d.key.as_str());
            match &d.after {
                None => write_if_changed(r, kind, id, "deleted", &json!(true)),
                Some(after) => {
                    if r.value(kind, id, "deleted") == Some(&json!(true)) {
                        r.write(kind, id, "deleted", json!(false));
                    }
                    let before = d.before.as_ref().and_then(Value::as_object);
                    for (field, value) in after.as_object().into_iter().flatten() {
                        if before.map_or(true, |b| b.get(field) != Some(value)) {
                            write_if_changed(r, kind, id, field, value);
                        }
                    }
                    // A field the change took away goes up as null.
                    for field in before.into_iter().flat_map(|b| b.keys()) {
                        if after.get(field).is_none() {
                            write_if_changed(r, kind, id, field, &Value::Null);
                        }
                    }
                }
            }
        }
        if let Some(now) = &now {
            sync_held(r, now);
        }
    })?;
    i.poke();
    Ok(())
}

/// Records made outside a journal entry (the spaces Heat makes the first time
/// it opens), carried the same way.
pub(crate) fn wrote_made(i: &Inner, made: &[(&'static str, Value)]) -> Result<(), CoreError> {
    let docs: Vec<DocChange> = made
        .iter()
        .map(|(k, v)| DocChange {
            kind: (*k).to_string(),
            key: v
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            before: None,
            after: Some(v.clone()),
        })
        .collect();
    wrote_entry(i, &docs)
}

/// After an undo or redo in Heat, or when the library may have changed
/// behind the core's back (the MCP helper wrote while the app was closed):
/// every record that now differs from what the replica holds is a new local
/// change.
pub(crate) fn journal_moved(i: &Inner) -> Result<(), CoreError> {
    let grade_ids: BTreeSet<String> = {
        let store = i.store();
        store.docs("grade")?.into_iter().map(|d| d.key).collect()
    };
    let mut now = held_now(i, &grade_ids)?;
    let _held = lock(&HELD);
    let mut r = load(i)?;
    let snapshot = r.snapshot();
    // A grade that went up and is gone now still has its copy to delete.
    for k in snapshot.keys().filter(|k| is_grade(&k.0)) {
        now.grades.entry(k.1.clone()).or_insert(None);
    }
    let mut kinds: BTreeSet<String> = snapshot.keys().map(|k| k.0.clone()).collect();
    kinds.extend(
        i.kv.query_strings("SELECT DISTINCT kind FROM docs ORDER BY kind")
            .unwrap_or_default(),
    );
    let mut writes: Vec<(String, String, String, Value)> = Vec::new();
    for kind in kinds.iter().filter(|k| !held_back(k) && !local_only(k)) {
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
    let mut moved = !writes.is_empty();
    moved |= sync_held(&mut r, &now);
    for (kind, id, field, value) in writes {
        r.write(&kind, &id, &field, value);
    }
    if moved {
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
        .expect("a thread for Learn sync")
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
            let _ = i.kv.set(
                "heat.syncedAt",
                &json!(jiff::Timestamp::now().as_millisecond()),
            );
            i.bus.status("sync", &sentence);
            Ok(json!({"sentence": sentence, "changed": touched.len()}))
        }
        Err(Fail::Offline) => {
            i.bus
                .status("sync", "Offline. Learn syncs when you're back.");
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
    for (kind, id) in touched
        .iter()
        .filter(|(kind, _)| !held_back(kind) && !local_only(kind))
    {
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
