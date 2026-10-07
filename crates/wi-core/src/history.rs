//! Labelled undo per room (docs/SPEC.md 2.7, 9.6), and the rooms' journaled
//! records (docs/COMMANDS.md history.*, records.*). There is one journal in
//! wi-store; ⌘Z acts on the room it is pressed in, and every change says
//! what it was: "Undo move clip". After any change the core sends `history`
//! with each room's labels, so the Edit menu never goes stale.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};
use wi_store::{History, Menu, Room};

use crate::args::Args;
use crate::{heat, CoreError, Inner};

const ROOMS: [Room; 5] = [
    Room::Heat,
    Room::Space,
    Room::Console,
    Room::Unquantized,
    Room::Library,
];

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

/// Sends every room's labels: an undo in one room can hold or free another's.
fn send_history(i: &Inner) {
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
    i.kv
        .query_strings("SELECT DISTINCT kind FROM docs ORDER BY kind")
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
    // Any row may have changed: every view refetches.
    i.bus.emit("library", json!({"ids": []}));
    records_changed(i, all_kinds(i));
    if room == Room::Heat {
        heat::journal_moved(i)?;
    }
    i.poke();
    Ok(json!({"label": label}))
}

pub(crate) fn undo(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    step(i, a, true)
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
        None => Err(CoreError::new("not_found", "That record isn't here any more.")),
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
                    return Err(CoreError::new("not_found", "That record isn't here any more."));
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
    tx.commit()?;
    Ok(())
}

pub(crate) fn records_mutate(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let label = a.label()?;
    let room = a.room()?;
    let ops = read_ops(i, a)?;
    write_records(i, room, label, &ops)?;
    if room == Room::Heat {
        let changes: Vec<(&str, &str, &str, &Value)> = ops
            .iter()
            .flat_map(|o| {
                o.fields
                    .iter()
                    .map(move |(f, v)| (o.kind.as_str(), o.id.as_str(), f.as_str(), v))
            })
            .collect();
        heat::wrote(i, &changes)?;
    }
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
/// deleted), applied as one journal entry so undo stays exact.
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
    write_records(i, Room::Heat, label, &ops)?;
    let kinds: BTreeSet<String> = ops.iter().map(|o| o.kind.clone()).collect();
    records_changed(i, kinds.into_iter().collect());
    Ok(())
}
