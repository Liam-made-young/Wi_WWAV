//! Labelled undo per view (docs/SPEC.md 2.7, 9.6), and the views' journaled
//! records (docs/COMMANDS.md history.*, records.*). There is one journal in
//! wi-store; ⌘Z acts on the view it is pressed in (a `room` in the commands:
//! heat, space, console, or library for the drawer), and every change says
//! what it was: "Undo move clip". After any change the core sends `history`
//! with each view's labels, so the Edit menu never goes stale.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};
use wi_store::{History, Menu, Room};

use crate::args::Args;
use crate::{heat, CoreError, Inner};

/// The rooms ⌘Z is pressed in. Room::Sync, where other devices' changes are
/// kept, has no menu.
const ROOMS: [Room; 4] = [Room::Heat, Room::Space, Room::Console, Room::Library];

fn menu_json(room: Room, h: &History) -> Value {
    let ready = |m: &Menu, verb: &str| match m {
        Menu::Ready(label) => json!(format!("{verb} {label}")),
        _ => Value::Null,
    };
    let held = |m: &Menu| match m {
        Menu::Held(why) => json!(why),
        _ => Value::Null,
    };
    json!({
        "room": room.as_str(),
        "undo": ready(&h.undo, "Undo"),
        "redo": ready(&h.redo, "Redo"),
        "cant": held(&h.undo),
        "cantRedo": held(&h.redo),
    })
}

/// Sends every view's labels (an undo in one can hold or free another's), and the save state: every committed change is saved (9.6).
fn send_history(i: &Inner) {
    i.bus.status("save", "Saved on this Mac");
    for room in ROOMS {
        if let Ok(h) = i.store().history(room) {
            i.bus.emit("history", menu_json(room, &h));
        }
    }
}

/// After a change to the library: views refetch, menus relabel.
pub(crate) fn changed(i: &Inner, ids: &[String]) {
    i.bus.emit("library", json!({"ids": ids}));
    send_history(i);
}

pub(crate) fn records_changed(i: &Inner, kinds: Vec<String>) {
    i.bus.emit("records", json!({"kinds": kinds}));
    send_history(i);
}

fn all_kinds(i: &Inner) -> Vec<String> {
    i.kv.query_strings("SELECT DISTINCT kind FROM docs ORDER BY kind")
        .unwrap_or_default()
}

pub(crate) fn get(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let room = a.room()?;
    let h = i.store().history(room)?;
    Ok(menu_json(room, &h))
}

fn step(i: &Inner, a: &Args, back: bool) -> Result<Value, CoreError> {
    let room = a.room()?;
    let done = {
        let mut store = i.store();
        if back {
            store.undo(room)
        } else {
            store.redo(room)
        }
    };
    let label = match done {
        Ok(Some(label)) => label,
        Ok(None) if back => return Err(CoreError::new("nothing_to_undo", "Nothing to undo.")),
        Ok(None) => return Err(CoreError::new("nothing_to_redo", "Nothing to redo.")),
        Err(wi_store::Error::Refused(why)) => {
            let code = if back { "cant_undo" } else { "cant_redo" };
            return Err(CoreError::new(code, why));
        }
        Err(e) => return Err(e.into()),
    };
    // Any row may have changed: every view refetches. A record is Heat's
    // whichever view changed it (Space plans a milestone in Heat from your
    // sun), so an undo or redo anywhere may have changed what syncs.
    i.bus.emit("library", json!({"ids": []}));
    records_changed(i, all_kinds(i));
    heat::journal_moved(i)?;
    i.poke();
    Ok(json!({"label": label}))
}

pub(crate) fn undo(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    step(i, a, true)
}

/// `history.undoEntry {txnId}`: one entry undone out of order, for Settings →
/// Claude's list. Refused, with a sentence, when a later change touched the
/// same records ("This changed again since. Undo the later change first.").
pub(crate) fn undo_entry(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let id = a.str("txnId")?;
    let done = i.store().undo_entry(id);
    let label = match done {
        Ok(label) => label,
        Err(wi_store::Error::Refused(why)) => return Err(CoreError::new("cant_undo", why)),
        Err(e) => return Err(e.into()),
    };
    // What the entry changed is back as it was: every view refetches, and
    // sync takes the records as new local changes.
    let kinds = all_kinds(i);
    i.bus.emit("library", json!({"ids": []}));
    i.bus.emit("heat", json!({"kinds": kinds}));
    records_changed(i, kinds);
    heat::journal_moved(i)?;
    i.poke();
    Ok(json!({"label": label}))
}

pub(crate) fn redo(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    step(i, a, false)
}

// ----- records -----

/// A record as the rooms see it: its JSON, with its id.
fn record_json(key: &str, json: &Value) -> Value {
    let mut v = json.clone();
    if let Some(m) = v.as_object_mut() {
        m.entry("id").or_insert_with(|| json!(key));
    }
    v
}

/// What search reads: every string the record holds at its top level.
fn search_text(v: &Value) -> String {
    v.as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| k.as_str() != "id")
                .filter_map(|(_, v)| v.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

pub(crate) fn records_list(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let kind = a.str("kind")?;
    let filter: Map<String, Value> = a
        .get("where")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let records: Vec<Value> = i
        .store()
        .docs(kind)?
        .into_iter()
        .map(|d| record_json(&d.key, &d.json))
        .filter(|r| filter.iter().all(|(k, v)| r.get(k) == Some(v)))
        .collect();
    Ok(json!({"records": records}))
}

pub(crate) fn records_get(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let (kind, id) = (a.str("kind")?, a.str("id")?);
    match i.store().doc(kind, id)? {
        Some(d) => Ok(json!({"record": record_json(&d.key, &d.json)})),
        None => Err(CoreError::new(
            "not_found",
            "That record isn't here any more.",
        )),
    }
}

/// One op of `records.mutate`, read and checked before anything is written.
struct Op {
    kind: String,
    id: String,
    /// The record after the op; None when deleted.
    after: Option<Value>,
    /// The fields the op set, for sync; `deleted` for a delete.
    fields: Vec<(String, Value)>,
}

fn read_ops(i: &Inner, a: &Args) -> Result<Vec<Op>, CoreError> {
    let ops = a
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| CoreError::new("bad_args", "records.mutate needs ops, a list."))?;
    let store = i.store();
    let mut out: Vec<Op> = Vec::new();
    for op in ops {
        let field = |k: &str| op.get(k).and_then(Value::as_str);
        let (Some(verb), Some(kind), Some(id)) = (field("op"), field("kind"), field("id")) else {
            return Err(CoreError::new("bad_args", "Each op has op, kind and id."));
        };
        // An earlier op in the same change may already have touched it.
        let before = match out.iter().rev().find(|o| o.kind == kind && o.id == id) {
            Some(o) => o.after.clone(),
            None => store.doc(kind, id)?.map(|d| d.json),
        };
        let value = op.get("value").and_then(Value::as_object);
        let (after, fields) = match (verb, value) {
            ("put", Some(v)) => {
                let mut v = v.clone();
                v.insert("id".into(), json!(id));
                let fields = v.iter().map(|(k, x)| (k.clone(), x.clone())).collect();
                (Some(Value::Object(v)), fields)
            }
            ("patch", Some(v)) => {
                let Some(Value::Object(mut whole)) = before else {
                    return Err(CoreError::new(
                        "not_found",
                        "That record isn't here any more.",
                    ));
                };
                for (k, x) in v {
                    whole.insert(k.clone(), x.clone());
                }
                let fields = v.iter().map(|(k, x)| (k.clone(), x.clone())).collect();
                (Some(Value::Object(whole)), fields)
            }
            ("delete", _) => (None, vec![("deleted".to_string(), json!(true))]),
            _ => {
                return Err(CoreError::new(
                    "bad_args",
                    "An op is put or patch (with a value object) or delete.",
                ))
            }
        };
        out.push(Op {
            kind: kind.to_string(),
            id: id.to_string(),
            after,
            fields,
        });
    }
    Ok(out)
}

/// Writes `ops` as one journal entry under `label` in `room`.
fn write_records(i: &Inner, room: Room, label: &str, ops: &[Op]) -> Result<(), CoreError> {
    let mut store = i.store();
    let mut tx = store.begin(room, label)?;
    for op in ops {
        match &op.after {
            Some(v) => tx.put_doc(&op.kind, &op.id, v, &search_text(v))?,
            None => tx.delete_doc(&op.kind, &op.id)?,
        }
    }
    // The watcher is to tell the views about other processes' entries, not this one.
    if let Some(id) = tx.commit()? {
        i.note_own(&id);
    }
    Ok(())
}

pub(crate) fn records_mutate(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let label = a.label()?;
    let room = a.room()?;
    let ops = read_ops(i, a)?;
    write_records(i, room, label, &ops)?;
    // Records sync whichever view wrote them: a milestone planned from Space
    // is as much Heat's as one planned in Heat (3.15, 8.7). Grades stay on
    // this Mac wherever they were written (heat::local_only).
    let changes: Vec<(&str, &str, &str, &Value)> = ops
        .iter()
        .flat_map(|o| {
            o.fields
                .iter()
                .map(move |(f, v)| (o.kind.as_str(), o.id.as_str(), f.as_str(), v))
        })
        .collect();
    heat::wrote(i, &changes)?;
    let kinds: BTreeSet<String> = ops.iter().map(|o| o.kind.clone()).collect();
    records_changed(i, kinds.into_iter().collect());
    let store = i.store();
    let mut records = Vec::new();
    for o in &ops {
        if let Some(d) = store.doc(&o.kind, &o.id)? {
            records.push(record_json(&d.key, &d.json));
        }
    }
    Ok(json!({"records": records}))
}

/// Records changed elsewhere (another device), each as it now stands (None:
/// deleted), applied as one journal entry so undo stays exact. The entry is
/// kept in [`Room::Sync`], which no view's ⌘Z reaches: in Heat's own journal
/// it would be a new change, throwing away the redo the person has waiting
/// and making ⌘Z undo the other device's work instead of theirs. The journal's
/// rules still see it: it takes the redo of any change that touched the same
/// records, and holds an undo that would put such a record back.
pub(crate) fn apply_remote(
    i: &Inner,
    label: &str,
    records: Vec<(String, String, Option<Value>)>,
) -> Result<(), CoreError> {
    let ops: Vec<Op> = records
        .into_iter()
        .map(|(kind, id, after)| Op {
            kind,
            id,
            after,
            fields: Vec::new(),
        })
        .collect();
    if ops.is_empty() {
        return Ok(());
    }
    write_records(i, Room::Sync, label, &ops)?;
    let kinds: BTreeSet<String> = ops.iter().map(|o| o.kind.clone()).collect();
    let kinds: Vec<String> = kinds.into_iter().collect();
    i.bus.emit("heat", json!({"kinds": kinds}));
    records_changed(i, kinds);
    Ok(())
}
