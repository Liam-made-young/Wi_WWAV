//! What the Database tab writes. An edit to one of Learn's records goes
//! through the same `heat.*` command a view would use (`heat.patch`,
//! `heat.done`, `heat.public.set`), so every rule in `schema.rs` holds here
//! too and the change is the real record's. A person's own tables, the
//! columns they add and their saved views are records of this module's
//! kinds, written straight to the journal. Either way a paste or a fill is
//! one ⌘Z (`batch.rs`).

use std::collections::BTreeMap;

use serde_json::{json, Map, Value as Json};
use wi_formula::{parse_date, parse_number, Formula, Value};
use wi_store::{Actor, Room, Txn};

use super::catalog::{kind, ColType, Column, Source};
use super::csv;
use super::query::{read, Spec};
use super::sheet::{cell_text, day_key, instant_ms, Origin, Sheets, Table};
use crate::args::Args;
use crate::heat_cmd::{self, announce};
use crate::{batch, CoreError, Inner};

fn refused(sentence: impl Into<String>) -> CoreError {
    CoreError::new("refused", sentence)
}

/// One change to this module's own records, journaled in Learn.
fn journal(
    i: &Inner,
    label: &str,
    f: impl FnOnce(&mut Txn<'_>) -> Result<(), CoreError>,
) -> Result<Option<String>, CoreError> {
    let (id, docs) = {
        let mut store = i.store();
        let mut txn = store.begin_by(Room::Heat, label, Actor::You)?;
        f(&mut txn)?;
        let id = txn.commit()?;
        let docs = match &id {
            Some(id) => store.entry_docs(id)?.map(|e| e.docs).unwrap_or_default(),
            None => Vec::new(),
        };
        (id, docs)
    };
    if let Some(id) = &id {
        i.note_own(id);
    }
    announce(i, &docs, &[])?;
    Ok(id.map(|_| format!("Undo {label}")))
}

fn new_id() -> String {
    wwav_ids::ulid()
}

/// A name a formula can write: something, not too long, with no brackets.
fn clean_name(name: &str, what: &str) -> Result<String, CoreError> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(refused(format!("Give the {what} a name first.")));
    }
    if name.chars().count() > 60 {
        return Err(refused(format!(
            "A {what}'s name is 60 characters at most."
        )));
    }
    if name.contains(['[', ']']) {
        return Err(refused(format!(
            "A {what}'s name can't hold [ or ]: formulas use them to name columns."
        )));
    }
    Ok(name)
}

/// What a person typed or pasted into a cell, as the value its column keeps.
/// `Ok(Json::Null)` clears the cell.
pub(crate) fn parse_input(
    sheets: &Sheets,
    table: &Table,
    col: &Column,
    value: &Json,
) -> Result<Json, String> {
    let text = match value {
        Json::Null => return Ok(Json::Null),
        Json::String(s) if s.trim().is_empty() => return Ok(Json::Null),
        Json::String(s) => Some(s.trim().to_string()),
        _ => None,
    };
    match col.ty {
        ColType::Number => match (value, &text) {
            (Json::Number(_), _) => Ok(value.clone()),
            (Json::Bool(b), _) => Ok(json!(if *b { 1 } else { 0 })),
            (_, Some(s)) => parse_number(s)
                .map(super::sheet::value_json_number)
                .ok_or_else(|| format!("'{s}' isn't a number.")),
            _ => Err("A number goes here.".to_string()),
        },
        ColType::Bool => match (value, &text) {
            (Json::Bool(_), _) => Ok(value.clone()),
            (Json::Number(n), _) => Ok(json!(n.as_f64().unwrap_or(0.0) != 0.0)),
            (_, Some(s)) => match s.to_ascii_lowercase().as_str() {
                "true" | "yes" | "y" | "x" | "1" | "done" | "checked" | "✓" => Ok(json!(true)),
                "false" | "no" | "n" | "0" | "open" | "unchecked" => Ok(json!(false)),
                _ => Err(format!("'{s}' isn't yes or no.")),
            },
            _ => Err("Yes or no goes here.".to_string()),
        },
        ColType::Date => match &text {
            Some(s) => parse_date(s).map(|d| json!(day_key(d))).ok_or_else(|| {
                format!("'{s}' isn't a date. Write it like 2026-10-09 or Oct 9, 2026.")
            }),
            None => Err("A date goes here.".to_string()),
        },
        ColType::DateTime => match (value, &text) {
            // An instant sent as one stays one.
            (Json::Number(_), _) => Ok(value.clone()),
            (_, Some(s)) => {
                let days = parse_date(s).ok_or_else(|| {
                    format!("'{s}' isn't a date. Write it like 2026-10-09 or 2026-10-09 23:59.")
                })?;
                // A due date with no time is due at the end of that day, as
                // Learn's own "New task" makes it.
                let days = if days.fract() == 0.0 && col.id == "due" {
                    days + (23.0 * 60.0 + 59.0) / 1440.0
                } else {
                    days
                };
                instant_ms(days, &sheets.clock.zone)
                    .map(|ms| json!(ms as i64))
                    .ok_or_else(|| format!("'{s}' isn't a time Learn can keep."))
            }
            _ => Err("A date goes here.".to_string()),
        },
        ColType::Relation => {
            let target = col.relation.as_deref().unwrap_or("");
            let labels = sheets.labels(target);
            let given = value
                .get("id")
                .and_then(Json::as_str)
                .map(String::from)
                .or(text.clone())
                .unwrap_or_default();
            if labels.contains_key(&given) {
                return Ok(json!(given));
            }
            let named: Vec<&String> = labels
                .iter()
                .filter(|(_, label)| label.eq_ignore_ascii_case(&given))
                .map(|(id, _)| id)
                .collect();
            match named.as_slice() {
                [one] => Ok(json!(one)),
                [] => Err(format!(
                    "No {} is called '{given}'.",
                    super::catalog::words(target).to_lowercase()
                )),
                _ => Err(format!(
                    "More than one {} is called '{given}'. Pick it from the list.",
                    super::catalog::words(target).to_lowercase()
                )),
            }
        }
        ColType::Text => Ok(match value {
            Json::String(s) => json!(s),
            Json::Number(n) => json!(n.to_string()),
            Json::Bool(b) => json!(if *b { "true" } else { "false" }),
            other => json!(other.to_string()),
        }),
        ColType::Json | ColType::Formula => Err(col
            .locked
            .clone()
            .unwrap_or_else(|| format!("{} can't be typed over.", table.name))),
    }
}

/// What search reads for one of a person's rows: every cell's text.
fn row_text(cells: &Map<String, Json>) -> String {
    cells
        .values()
        .filter_map(|v| match v {
            Json::String(s) if !s.is_empty() => Some(s.clone()),
            Json::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

struct Edit {
    row: String,
    col: usize,
    value: Json,
}

/// `db.cells.set {table, edits: [{row, column, value}], label?}`: one edit, a
/// paste or a fill, as one undo step. An edit that can't be made (a locked
/// column, text where a number goes, a rule of Learn's) is left out and said
/// in `failed`; the others are made.
pub(crate) fn cells_set(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let table_name = a.str("table")?;
    let edits_in = a
        .get("edits")
        .and_then(Json::as_array)
        .ok_or_else(|| CoreError::new("bad_args", "db.cells.set needs edits, a list."))?;
    if edits_in.len() > 20_000 {
        return Err(refused(
            "That is more than 20,000 cells at once. Paste it in parts.",
        ));
    }
    let clock = i.clock();
    let mut failed: Vec<Json> = Vec::new();
    // Read what the edits mean while the library is held, then let go of it.
    let (table_id, origin, kind_key, plan, user_rows) = {
        let store = i.store();
        let sheets = Sheets::new(&store, &clock);
        let id = sheets
            .resolve(table_name)
            .ok_or_else(|| refused(format!("There is no table called '{table_name}'.")))?;
        let table = sheets.table(&id)?;
        let mut plan: Vec<Edit> = Vec::new();
        for e in edits_in {
            let (row, column) = (
                e["row"].as_str().unwrap_or_default().to_string(),
                e["column"].as_str().unwrap_or_default(),
            );
            let mut fail = |message: String| {
                failed.push(json!({"row": row, "column": column, "message": message}));
            };
            let Some(col) = table.col_index(column) else {
                fail(format!("{} has no column called '{column}'.", table.name));
                continue;
            };
            let c = &table.columns[col];
            if let Some(why) = table.locked.as_ref().or(c.locked.as_ref()) {
                fail(why.clone());
                continue;
            }
            if table.row_index(&row).is_none() {
                fail("That row isn't there any more.".to_string());
                continue;
            }
            match parse_input(&sheets, &table, c, &e["value"]) {
                Ok(value) => plan.push(Edit {
                    row: row.clone(),
                    col,
                    value,
                }),
                Err(message) => fail(message),
            }
        }
        let user_rows: BTreeMap<String, Json> = if table.origin == Origin::User {
            wi_heat_store::all(&store, kind::ROW)
                .map_err(heat_cmd::core_error)?
                .into_iter()
                .filter(|r| r["tableId"].as_str() == Some(id.as_str()))
                .filter_map(|r| Some((r["id"].as_str()?.to_string(), r)))
                .collect()
        } else {
            BTreeMap::new()
        };
        let columns: Vec<(String, ColType, Source)> = table
            .columns
            .iter()
            .map(|c| (c.id.clone(), c.ty, c.source.clone()))
            .collect();
        (
            id,
            table.origin,
            table.kind.as_ref().map(|k| k.kind),
            plan.into_iter()
                .map(|e| (e.row, columns[e.col].0.clone(), e.value))
                .collect::<Vec<_>>(),
            user_rows,
        )
    };
    let label = a.opt_str("label").map(String::from).unwrap_or_else(|| {
        if plan.len() == 1 {
            "edit cell".to_string()
        } else {
            format!("edit {} cells", plan.len())
        }
    });
    let forced = a.opt_str("label").is_some();
    let mut changed = 0usize;

    if origin == Origin::User {
        // A person's own rows: all the edits in one entry.
        let mut by_row: BTreeMap<String, Vec<(String, Json)>> = BTreeMap::new();
        for (row, col, value) in plan {
            by_row.entry(row).or_default().push((col, value));
        }
        let count: usize = by_row.values().map(Vec::len).sum();
        let undo = journal(i, &label, |txn| {
            for (row, sets) in &by_row {
                let Some(mut doc) = user_rows.get(row).cloned() else {
                    continue;
                };
                let mut cells = doc["cells"].as_object().cloned().unwrap_or_default();
                for (col, value) in sets {
                    if value.is_null() {
                        cells.remove(col);
                    } else {
                        cells.insert(col.clone(), value.clone());
                    }
                }
                let text = row_text(&cells);
                doc["cells"] = Json::Object(cells);
                txn.put_doc(kind::ROW, row, &doc, &text)?;
            }
            Ok(())
        })?;
        if undo.is_some() {
            changed = count;
        }
        return Ok(json!({"changed": changed, "failed": failed, "undo": undo}));
    }

    let Some(k) = kind_key else {
        return Err(refused(
            "That table is made from other records, so it changes where they do.",
        ));
    };
    // One of Learn's kinds: each row's edits are one command of its own.
    let mut by_row: Vec<(String, Vec<(String, Json)>)> = Vec::new();
    for (row, col, value) in plan {
        match by_row.iter_mut().find(|(r, _)| *r == row) {
            Some((_, sets)) => sets.push((col, value)),
            None => by_row.push((row, vec![(col, value)])),
        }
    }
    let today = clock.today();
    let (_, step) = batch::one_step(i, &label, !forced, || {
        for (row, sets) in &by_row {
            let mut patch = Map::new();
            for (col, value) in sets {
                let outcome = match (k, col.as_str()) {
                    // A task is checked off by its own command, which logs the day.
                    ("task", "done") => heat_cmd::invoke(
                        i,
                        "heat.done",
                        &Args::new(
                            "heat.done",
                            &json!({"taskId": row, "done": value.as_bool().unwrap_or(false), "date": today}),
                        ),
                    )
                    .map(|_| ()),
                    (_, "public") => heat_cmd::invoke(
                        i,
                        "heat.public.set",
                        &Args::new(
                            "heat.public.set",
                            &json!({"kind": k, "id": row, "public": value.as_bool().unwrap_or(false)}),
                        ),
                    )
                    .map(|_| ()),
                    _ => {
                        patch.insert(col.clone(), value.clone());
                        continue;
                    }
                };
                match outcome {
                    Ok(()) => changed += 1,
                    Err(e) => failed.push(json!({"row": row, "column": col, "message": e.message})),
                }
            }
            if patch.is_empty() {
                continue;
            }
            let n = patch.len();
            let args = json!({"kind": k, "id": row, "set": patch});
            match heat_cmd::invoke(i, "heat.patch", &Args::new("heat.patch", &args)) {
                Ok(_) => changed += n,
                Err(e) => {
                    for col in args["set"].as_object().into_iter().flat_map(|m| m.keys()) {
                        failed.push(json!({"row": row, "column": col, "message": e.message}));
                    }
                }
            }
        }
    })?;
    let _ = table_id;
    Ok(json!({"changed": changed, "failed": failed, "undo": step.undo}))
}

/// `db.rows.add {table, rows: [{<column>: value}], label?}`: new rows in a
/// person's table, or new records of one of Learn's kinds.
pub(crate) fn rows_add(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let table_name = a.str("table")?;
    let rows_in: Vec<Json> = a
        .get("rows")
        .and_then(Json::as_array)
        .cloned()
        .unwrap_or_else(|| vec![json!({})]);
    if rows_in.len() > 50_000 {
        return Err(refused("That is more than 50,000 rows at once."));
    }
    let clock = i.clock();
    let mut failed: Vec<Json> = Vec::new();
    let (table_id, origin, kind_key, parsed, next_order, first_space) = {
        let store = i.store();
        let sheets = Sheets::new(&store, &clock);
        let id = sheets
            .resolve(table_name)
            .ok_or_else(|| refused(format!("There is no table called '{table_name}'.")))?;
        let table = sheets.table(&id)?;
        if let Some(why) = &table.locked {
            return Err(refused(why.clone()));
        }
        let mut parsed: Vec<Map<String, Json>> = Vec::new();
        for (n, row) in rows_in.iter().enumerate() {
            let mut cells = Map::new();
            for (name, value) in row.as_object().into_iter().flatten() {
                let Some(col) = table.col_index(name) else {
                    failed.push(json!({"row": n, "column": name, "message": format!("{} has no column called '{name}'.", table.name)}));
                    continue;
                };
                let c = &table.columns[col];
                if c.locked.is_some() {
                    continue;
                }
                match parse_input(&sheets, &table, c, value) {
                    Ok(Json::Null) => {}
                    Ok(v) => {
                        cells.insert(c.id.clone(), v);
                    }
                    Err(message) => {
                        failed.push(json!({"row": n, "column": name, "message": message}))
                    }
                }
            }
            parsed.push(cells);
        }
        let next_order = wi_heat_store::all(&store, kind::ROW)
            .map_err(heat_cmd::core_error)?
            .iter()
            .filter(|r| r["tableId"].as_str() == Some(id.as_str()))
            .filter_map(|r| r["order"].as_f64())
            .fold(0.0, f64::max)
            + 1.0;
        let first_space = wi_heat_store::all(&store, "space")
            .map_err(heat_cmd::core_error)?
            .first()
            .and_then(|s| s["id"].as_str().map(String::from));
        (
            id,
            table.origin,
            table.kind.as_ref().map(|k| k.kind),
            parsed,
            next_order,
            first_space,
        )
    };
    let n = parsed.len();
    let label = a.opt_str("label").map(String::from).unwrap_or_else(|| {
        if n == 1 {
            "add row".to_string()
        } else {
            format!("add {n} rows")
        }
    });
    let mut ids: Vec<String> = Vec::new();
    if origin == Origin::User {
        let made: Vec<(String, Json, String)> = parsed
            .into_iter()
            .enumerate()
            .map(|(k, cells)| {
                let id = new_id();
                let text = row_text(&cells);
                (
                    id.clone(),
                    json!({"id": id, "tableId": table_id, "order": next_order + k as f64, "cells": cells}),
                    text,
                )
            })
            .collect();
        let undo = journal(i, &label, |txn| {
            for (id, doc, text) in &made {
                txn.put_doc(kind::ROW, id, doc, text)?;
            }
            Ok(())
        })?;
        ids = made.into_iter().map(|(id, _, _)| id).collect();
        return Ok(json!({"ids": ids, "failed": failed, "undo": undo}));
    }
    let Some(k) = kind_key else {
        return Err(refused(
            "That table is made from other records, so rows are added where they are.",
        ));
    };
    let forced = a.opt_str("label").is_some();
    let (_, step) = batch::one_step(i, &label, !forced, || {
        for (n, mut record) in parsed.into_iter().enumerate() {
            // A task and a project live in a space; a row added here goes in the first.
            if matches!(k, "task" | "project" | "milestone") && !record.contains_key("spaceId") {
                if let Some(space) = &first_space {
                    record.insert("spaceId".into(), json!(space));
                }
            }
            let args = json!({"kind": k, "record": record});
            match heat_cmd::invoke(i, "heat.put", &Args::new("heat.put", &args)) {
                Ok(v) => {
                    let key = if k == "dailyNote" { "date" } else { "id" };
                    if let Some(id) = v["record"][key].as_str() {
                        ids.push(id.to_string());
                    }
                }
                Err(e) => {
                    failed.push(json!({"row": n, "column": Json::Null, "message": e.message}))
                }
            }
        }
    })?;
    Ok(json!({"ids": ids, "failed": failed, "undo": step.undo}))
}

/// `db.rows.delete {table, rows: [id]}`
pub(crate) fn rows_delete(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let table_name = a.str("table")?;
    let rows = a.strings("rows")?;
    let clock = i.clock();
    let (origin, kind_key, locked) = {
        let store = i.store();
        let sheets = Sheets::new(&store, &clock);
        let id = sheets
            .resolve(table_name)
            .ok_or_else(|| refused(format!("There is no table called '{table_name}'.")))?;
        let table = sheets.table(&id)?;
        (
            table.origin,
            table.kind.as_ref().map(|k| k.kind),
            table.locked.clone(),
        )
    };
    if let Some(why) = locked {
        return Err(refused(why));
    }
    let label = if rows.len() == 1 {
        "delete row".to_string()
    } else {
        format!("delete {} rows", rows.len())
    };
    if origin == Origin::User {
        let undo = journal(i, &label, |txn| {
            for id in &rows {
                if txn.doc(kind::ROW, id)?.is_some() {
                    txn.delete_doc(kind::ROW, id)?;
                }
            }
            Ok(())
        })?;
        return Ok(json!({"deleted": rows.len(), "failed": [], "undo": undo}));
    }
    let Some(k) = kind_key else {
        return Err(refused(
            "That table is made from other records, so rows are removed where they are.",
        ));
    };
    let mut failed: Vec<Json> = Vec::new();
    let mut deleted = 0;
    let (_, step) = batch::one_step(i, &label, true, || {
        for id in &rows {
            let args = json!({"kind": k, "id": id});
            match heat_cmd::invoke(i, "heat.delete", &Args::new("heat.delete", &args)) {
                Ok(_) => deleted += 1,
                Err(e) => failed.push(json!({"row": id, "message": e.message})),
            }
        }
    })?;
    Ok(json!({"deleted": deleted, "failed": failed, "undo": step.undo}))
}

/// Every table's name, for refusing a second one of the same name.
fn taken_table_names(i: &Inner) -> Vec<(String, String)> {
    let clock = i.clock();
    let store = i.store();
    let sheets = Sheets::new(&store, &clock);
    sheets
        .list()
        .into_iter()
        .map(|(id, name, _)| (id, name))
        .collect()
}

fn column_doc(table: &str, name: &str, ty: ColType, order: f64, formula: Option<&str>) -> Json {
    let mut doc = json!({
        "id": new_id(), "table": table, "name": name, "type": ty.as_str(), "order": order,
    });
    if let Some(f) = formula {
        doc["formula"] = json!(f);
    }
    doc
}

/// `db.table.create {name, columns?: [{name, type}]}`: a blank table of the
/// person's own, with one text column called Name unless columns are given.
pub(crate) fn table_create(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let name = clean_name(a.str("name")?, "table")?;
    if taken_table_names(i)
        .iter()
        .any(|(id, n)| n.eq_ignore_ascii_case(&name) || id.eq_ignore_ascii_case(&name))
    {
        return Err(refused(format!(
            "There is already a table called '{name}'."
        )));
    }
    let id = new_id();
    let mut columns: Vec<Json> = Vec::new();
    for (n, c) in a
        .get("columns")
        .and_then(Json::as_array)
        .cloned()
        .unwrap_or_else(|| vec![json!({"name": "Name", "type": "text"})])
        .iter()
        .enumerate()
    {
        let cname = clean_name(c["name"].as_str().unwrap_or(""), "column")?;
        let ty = ColType::parse_user(c["type"].as_str().unwrap_or("text"))
            .ok_or_else(|| refused("A column is text, number, checkbox, date or formula."))?;
        let formula = c["formula"].as_str();
        if ty == ColType::Formula {
            check_formula(formula.unwrap_or(""))?;
        }
        if columns.iter().any(|have| {
            have["name"]
                .as_str()
                .is_some_and(|h| h.eq_ignore_ascii_case(&cname))
        }) {
            return Err(refused(format!(
                "Two columns can't both be called '{cname}'."
            )));
        }
        columns.push(column_doc(&id, &cname, ty, n as f64 + 1.0, formula));
    }
    let doc = json!({"id": id, "name": name});
    let undo = journal(i, "add table", |txn| {
        txn.put_doc(kind::TABLE, &id, &doc, &name)?;
        for c in &columns {
            txn.put_doc(kind::COLUMN, c["id"].as_str().unwrap_or_default(), c, "")?;
        }
        Ok(())
    })?;
    Ok(json!({"table": {"id": id, "name": name}, "undo": undo}))
}

fn user_table(i: &Inner, id_or_name: &str) -> Result<(String, String), CoreError> {
    let clock = i.clock();
    let store = i.store();
    let sheets = Sheets::new(&store, &clock);
    let id = sheets
        .resolve(id_or_name)
        .ok_or_else(|| refused(format!("There is no table called '{id_or_name}'.")))?;
    match sheets.list().into_iter().find(|(t, _, _)| *t == id) {
        Some((id, name, Origin::User)) => Ok((id, name)),
        Some((_, name, _)) => Err(refused(format!(
            "{name} is one of Learn's own tables, so it can't be renamed or deleted."
        ))),
        None => Err(refused("That table isn't there any more.")),
    }
}

/// `db.table.rename {table, name}`
pub(crate) fn table_rename(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let (id, _) = user_table(i, a.str("table")?)?;
    let name = clean_name(a.str("name")?, "table")?;
    if taken_table_names(i)
        .iter()
        .any(|(other, n)| *other != id && n.eq_ignore_ascii_case(&name))
    {
        return Err(refused(format!(
            "There is already a table called '{name}'."
        )));
    }
    let doc = json!({"id": id, "name": name});
    let undo = journal(i, "rename table", |txn| {
        txn.put_doc(kind::TABLE, &id, &doc, &name)?;
        Ok(())
    })?;
    Ok(json!({"table": doc, "undo": undo}))
}

/// `db.table.delete {table}`: the table, its rows, its columns and its views,
/// as one entry, so ⌘Z brings all of it back.
pub(crate) fn table_delete(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let (id, _) = user_table(i, a.str("table")?)?;
    let (rows, columns, views) = {
        let store = i.store();
        let of = |k: &str, field: &str| -> Result<Vec<String>, CoreError> {
            Ok(wi_heat_store::all(&store, k)
                .map_err(heat_cmd::core_error)?
                .iter()
                .filter(|r| r[field].as_str() == Some(id.as_str()))
                .filter_map(|r| r["id"].as_str().map(String::from))
                .collect())
        };
        (
            of(kind::ROW, "tableId")?,
            of(kind::COLUMN, "table")?,
            of(kind::VIEW, "table")?,
        )
    };
    let undo = journal(i, "delete table", |txn| {
        for r in &rows {
            txn.delete_doc(kind::ROW, r)?;
        }
        for c in &columns {
            txn.delete_doc(kind::COLUMN, c)?;
        }
        for v in &views {
            txn.delete_doc(kind::VIEW, v)?;
        }
        txn.delete_doc(kind::TABLE, &id)?;
        Ok(())
    })?;
    Ok(json!({"undo": undo}))
}

/// A formula that can be read, or the sentence for why it can't.
fn check_formula(src: &str) -> Result<(), CoreError> {
    Formula::parse(src)
        .map(|_| ())
        .map_err(|e| refused(format!("That formula can't be read. {}", e.message)))
}

/// `db.column.add {table, name, type, formula?}`: a column in a person's
/// table, or a formula column beside one of Learn's.
pub(crate) fn column_add(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let name = clean_name(a.str("name")?, "column")?;
    let ty = ColType::parse_user(a.opt_str("type").unwrap_or("text"))
        .ok_or_else(|| refused("A column is text, number, checkbox, date or formula."))?;
    let formula = a.opt_str("formula");
    if ty == ColType::Formula {
        check_formula(formula.unwrap_or(""))?;
    }
    let clock = i.clock();
    let (table_id, order) = {
        let store = i.store();
        let sheets = Sheets::new(&store, &clock);
        let id = sheets
            .resolve(a.str("table")?)
            .ok_or_else(|| refused("There is no table of that name."))?;
        let table = sheets.table(&id)?;
        if table.origin != Origin::User && ty != ColType::Formula {
            return Err(refused(format!(
                "{} is one of Learn's own tables. A formula column can be added to it; other columns can't.",
                table.name
            )));
        }
        if table
            .columns
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(&name) || c.id.eq_ignore_ascii_case(&name))
        {
            return Err(refused(format!(
                "{} already has a column called '{name}'.",
                table.name
            )));
        }
        let order = sheets
            .added_columns()
            .iter()
            .filter(|c| c["table"].as_str() == Some(id.as_str()))
            .filter_map(|c| c["order"].as_f64())
            .fold(0.0, f64::max)
            + 1.0;
        (id, order)
    };
    let doc = column_doc(
        &table_id,
        &name,
        ty,
        order,
        formula.filter(|_| ty == ColType::Formula),
    );
    let id = doc["id"].as_str().unwrap_or_default().to_string();
    let undo = journal(i, "add column", |txn| {
        txn.put_doc(kind::COLUMN, &id, &doc, "")?;
        Ok(())
    })?;
    Ok(json!({"column": doc, "undo": undo}))
}

fn added_column(i: &Inner, table: &str, column: &str) -> Result<(String, Json), CoreError> {
    let clock = i.clock();
    let store = i.store();
    let sheets = Sheets::new(&store, &clock);
    let id = sheets
        .resolve(table)
        .ok_or_else(|| refused(format!("There is no table called '{table}'.")))?;
    let t = sheets.table(&id)?;
    let idx = t
        .col_index(column)
        .ok_or_else(|| refused(format!("{} has no column called '{column}'.", t.name)))?;
    let c = &t.columns[idx];
    if c.source != Source::Added {
        return Err(refused(format!(
            "{} is one of {}'s own columns, so it stays as it is. It can be hidden.",
            c.name, t.name
        )));
    }
    let doc = sheets
        .added_columns()
        .iter()
        .find(|d| d["id"].as_str() == Some(c.id.as_str()))
        .cloned()
        .ok_or_else(|| refused("That column isn't there any more."))?;
    Ok((id, doc))
}

/// `db.column.update {table, column, name?, formula?, type?}`
pub(crate) fn column_update(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let (table_id, mut doc) = added_column(i, a.str("table")?, a.str("column")?)?;
    let id = doc["id"].as_str().unwrap_or_default().to_string();
    let mut label = "edit column";
    if let Some(name) = a.opt_str("name") {
        let name = clean_name(name, "column")?;
        let clock = i.clock();
        let store = i.store();
        let sheets = Sheets::new(&store, &clock);
        let t = sheets.table(&table_id)?;
        if t.columns
            .iter()
            .any(|c| c.id != id && c.name.eq_ignore_ascii_case(&name))
        {
            return Err(refused(format!(
                "{} already has a column called '{name}'.",
                t.name
            )));
        }
        if doc["name"].as_str() != Some(name.as_str()) {
            label = "rename column";
        }
        doc["name"] = json!(name);
    }
    if let Some(ty) = a.opt_str("type") {
        let ty = ColType::parse_user(ty)
            .ok_or_else(|| refused("A column is text, number, checkbox, date or formula."))?;
        doc["type"] = json!(ty.as_str());
    }
    if let Some(formula) = a.opt_str("formula") {
        check_formula(formula)?;
        doc["formula"] = json!(formula);
        if label == "edit column" {
            label = "edit formula";
        }
    }
    if doc["type"] == "formula" {
        check_formula(doc["formula"].as_str().unwrap_or(""))?;
    }
    let undo = journal(i, label, |txn| {
        txn.put_doc(kind::COLUMN, &id, &doc, "")?;
        Ok(())
    })?;
    Ok(json!({"column": doc, "undo": undo}))
}

/// `db.column.delete {table, column}`: an added column, with what its cells
/// held, as one entry.
pub(crate) fn column_delete(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let (table_id, doc) = added_column(i, a.str("table")?, a.str("column")?)?;
    let id = doc["id"].as_str().unwrap_or_default().to_string();
    let rows: Vec<Json> = wi_heat_store::all(&i.store(), kind::ROW)
        .map_err(heat_cmd::core_error)?
        .into_iter()
        .filter(|r| {
            r["tableId"].as_str() == Some(table_id.as_str()) && r["cells"].get(&id).is_some()
        })
        .collect();
    let undo = journal(i, "delete column", |txn| {
        for mut r in rows {
            let mut cells = r["cells"].as_object().cloned().unwrap_or_default();
            cells.remove(&id);
            let text = row_text(&cells);
            r["cells"] = Json::Object(cells);
            txn.put_doc(kind::ROW, r["id"].as_str().unwrap_or_default(), &r, &text)?;
        }
        txn.delete_doc(kind::COLUMN, &id)?;
        Ok(())
    })?;
    Ok(json!({"undo": undo}))
}

/// `db.view.save {id?, table, name, spec}`: a named view of a table: its
/// filters, sorts, columns, pivot and charts.
pub(crate) fn view_save(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let name = clean_name(a.str("name")?, "view")?;
    let spec = a.get("spec").cloned().unwrap_or_else(|| json!({}));
    if !spec.is_object() {
        return Err(CoreError::new(
            "bad_args",
            "db.view.save needs spec, an object.",
        ));
    }
    let clock = i.clock();
    let (table_id, existing) = {
        let store = i.store();
        let sheets = Sheets::new(&store, &clock);
        let table_id = sheets
            .resolve(a.str("table")?)
            .ok_or_else(|| refused("There is no table of that name."))?;
        // A view that can't be read back isn't saved.
        let table = sheets.table(&table_id)?;
        Spec::read(&table, &spec)?;
        let views = wi_heat_store::all(&store, kind::VIEW).map_err(heat_cmd::core_error)?;
        let existing = a
            .opt_str("id")
            .and_then(|id| views.iter().find(|v| v["id"].as_str() == Some(id)).cloned());
        let clash = views.iter().any(|v| {
            v["table"].as_str() == Some(table_id.as_str())
                && v["name"]
                    .as_str()
                    .is_some_and(|n| n.eq_ignore_ascii_case(&name))
                && v["id"] != existing.as_ref().map_or(Json::Null, |e| e["id"].clone())
        });
        if clash {
            return Err(refused(format!(
                "This table already has a view called '{name}'."
            )));
        }
        (table_id, existing)
    };
    let id = existing
        .as_ref()
        .and_then(|e| e["id"].as_str().map(String::from))
        .unwrap_or_else(new_id);
    let doc = json!({"id": id, "table": table_id, "name": name, "spec": spec});
    let label = if existing.is_some() {
        "save view"
    } else {
        "add view"
    };
    let undo = journal(i, label, |txn| {
        txn.put_doc(kind::VIEW, &id, &doc, &name)?;
        Ok(())
    })?;
    Ok(json!({"view": doc, "undo": undo}))
}

/// `db.view.delete {id}`
pub(crate) fn view_delete(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let id = a.str("id")?;
    let undo = journal(i, "delete view", |txn| {
        if txn.doc(kind::VIEW, id)?.is_none() {
            return Err(refused("That view isn't there any more."));
        }
        txn.delete_doc(kind::VIEW, id)?;
        Ok(())
    })?;
    Ok(json!({"undo": undo}))
}

/// `db.layout.set {table, spec}`: how a table was last left (widths, order,
/// what is hidden, its filters and sorts). Not a journal entry.
pub(crate) fn layout_set(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let table = a.str("table")?;
    let spec = a.get("spec").cloned().unwrap_or_else(|| json!({}));
    i.store().set_doc(
        kind::LAYOUT,
        table,
        &json!({"table": table, "spec": spec}),
        "",
    )?;
    Ok(json!({}))
}

/// What a column of imported text holds, from every value in it.
fn infer(values: &[&str]) -> ColType {
    let filled: Vec<&str> = values
        .iter()
        .copied()
        .filter(|v| !v.trim().is_empty())
        .collect();
    if filled.is_empty() {
        return ColType::Text;
    }
    if filled.iter().all(|v| parse_number(v).is_some()) {
        return ColType::Number;
    }
    if filled.iter().all(|v| parse_date(v).is_some()) {
        return ColType::Date;
    }
    if filled.iter().all(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "false" | "yes" | "no"
        )
    }) {
        return ColType::Bool;
    }
    ColType::Text
}

/// `db.csv.import {name, csv}` or `{name, path}`: a CSV file as a new table
/// of the person's own. The first line names the columns; each column's
/// type is read from what it holds. One entry: ⌘Z takes the table away.
pub(crate) fn csv_import(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let text = match (a.opt_str("csv"), a.opt_str("path")) {
        (Some(text), _) => text.to_string(),
        (None, Some(path)) => {
            let bytes = std::fs::read(path)
                .map_err(|e| refused(format!("That file couldn't be read: {e}")))?;
            if bytes.len() > 64 * 1024 * 1024 {
                return Err(refused(
                    "That file is over 64 MB, which is more than a table here holds.",
                ));
            }
            String::from_utf8_lossy(&bytes).into_owned()
        }
        _ => {
            return Err(CoreError::new(
                "bad_args",
                "db.csv.import needs csv, the text, or path.",
            ))
        }
    };
    let mut rows = csv::parse(&text);
    if rows.is_empty() {
        return Err(refused("That file has no rows."));
    }
    if rows.len() > 50_001 {
        return Err(refused(
            "That file has more than 50,000 rows, which is more than a table here holds.",
        ));
    }
    let header = rows.remove(0);
    let width = rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(header.len());
    // Name the table after the file, and make the name its own.
    let base = clean_name(
        a.opt_str("name")
            .map(|n| n.trim_end_matches(".csv").trim_end_matches(".CSV"))
            .filter(|n| !n.trim().is_empty())
            .unwrap_or("Imported"),
        "table",
    )?;
    let taken = taken_table_names(i);
    let mut name = base.clone();
    let mut n = 2;
    while taken
        .iter()
        .any(|(id, t)| t.eq_ignore_ascii_case(&name) || id.eq_ignore_ascii_case(&name))
    {
        name = format!("{base} {n}");
        n += 1;
    }
    let table_id = new_id();
    let mut columns: Vec<Json> = Vec::new();
    let mut types: Vec<ColType> = Vec::new();
    for c in 0..width {
        let raw = header
            .get(c)
            .map(String::as_str)
            .unwrap_or("")
            .replace(['[', ']'], "");
        let mut cname = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if cname.is_empty() {
            cname = format!("Column {}", c + 1);
        }
        let cname: String = cname.chars().take(60).collect();
        let mut unique = cname.clone();
        let mut k = 2;
        while columns.iter().any(|d| {
            d["name"]
                .as_str()
                .is_some_and(|h| h.eq_ignore_ascii_case(&unique))
        }) {
            unique = format!("{cname} {k}");
            k += 1;
        }
        let values: Vec<&str> = rows
            .iter()
            .map(|r| r.get(c).map_or("", String::as_str))
            .collect();
        let ty = infer(&values);
        types.push(ty);
        columns.push(column_doc(&table_id, &unique, ty, c as f64 + 1.0, None));
    }
    let docs: Vec<(String, Json, String)> = rows
        .iter()
        .enumerate()
        .map(|(r, row)| {
            let mut cells = Map::new();
            for (c, ty) in types.iter().enumerate() {
                let raw = row.get(c).map_or("", String::as_str);
                if raw.trim().is_empty() {
                    continue;
                }
                let v = match ty {
                    ColType::Number => parse_number(raw).map(super::sheet::value_json_number),
                    ColType::Date => parse_date(raw).map(|d| json!(day_key(d))),
                    ColType::Bool => Some(json!(matches!(
                        raw.trim().to_ascii_lowercase().as_str(),
                        "true" | "yes"
                    ))),
                    _ => Some(json!(raw)),
                };
                if let Some(v) = v {
                    cells.insert(columns[c]["id"].as_str().unwrap_or_default().to_string(), v);
                }
            }
            let id = new_id();
            let text = row_text(&cells);
            (
                id.clone(),
                json!({"id": id, "tableId": table_id, "order": r as f64 + 1.0, "cells": cells}),
                text,
            )
        })
        .collect();
    let table_doc = json!({"id": table_id, "name": name});
    let undo = journal(i, "import CSV", |txn| {
        txn.put_doc(kind::TABLE, &table_id, &table_doc, &name)?;
        for c in &columns {
            txn.put_doc(kind::COLUMN, c["id"].as_str().unwrap_or_default(), c, "")?;
        }
        for (id, doc, text) in &docs {
            txn.put_doc(kind::ROW, id, doc, text)?;
        }
        Ok(())
    })?;
    Ok(json!({"table": table_doc, "rows": docs.len(), "columns": columns.len(), "undo": undo}))
}

/// `db.csv.export {table, spec?, to?}`: a view as CSV: the rows that pass
/// its filters, in its order, with its shown columns in its column order.
/// With `to`, a file is written there; without, the text comes back.
pub(crate) fn csv_export(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let clock = i.clock();
    let spec_json = a.get("spec").cloned().unwrap_or_else(|| json!({}));
    let (name, text, count) = {
        let store = i.store();
        let sheets = Sheets::new(&store, &clock);
        let id = sheets
            .resolve(a.str("table")?)
            .ok_or_else(|| refused("There is no table of that name."))?;
        let table = sheets.table(&id)?;
        let spec = Spec::read(&table, &spec_json)?;
        let r = read(&sheets, &table, &spec);
        let hidden: Vec<&str> = spec_json["hidden"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
            .collect();
        // The view's column order first, then any column it doesn't place.
        let mut cols: Vec<usize> = spec_json["order"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
            .filter_map(|c| table.col_index(c))
            .collect();
        for c in 0..table.columns.len() {
            if !cols.contains(&c) {
                cols.push(c);
            }
        }
        cols.retain(|c| !hidden.contains(&table.columns[*c].id.as_str()));
        let mut out: Vec<Vec<String>> = vec![cols
            .iter()
            .map(|c| table.columns[*c].name.clone())
            .collect()];
        for &row in &r.order {
            out.push(
                cols.iter()
                    .map(|c| match &r.grid[row][*c].v {
                        Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
                        other => cell_text(other),
                    })
                    .collect(),
            );
        }
        (table.name.clone(), csv::write(&out), r.order.len())
    };
    match a.opt_str("to") {
        // The Downloads folder, under a name that takes nothing's place.
        Some("downloads") => {
            let folder = std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|h| h.join("Downloads"))
                .filter(|d| d.is_dir())
                .ok_or_else(|| refused("There is no Downloads folder to save to."))?;
            let safe: String = name
                .chars()
                .map(|c| {
                    if c == '/' || c == ':' || c == '\\' {
                        '-'
                    } else {
                        c
                    }
                })
                .collect();
            let mut path = folder.join(format!("{safe}.csv"));
            let mut n = 2;
            while path.exists() {
                path = folder.join(format!("{safe} {n}.csv"));
                n += 1;
            }
            std::fs::write(&path, text.as_bytes())
                .map_err(|e| refused(format!("That file couldn't be written: {e}")))?;
            let file = path.file_name().map(|f| f.to_string_lossy().into_owned());
            Ok(json!({"path": path, "file": file, "rows": count}))
        }
        Some(path) => {
            std::fs::write(path, text.as_bytes())
                .map_err(|e| refused(format!("That file couldn't be written: {e}")))?;
            Ok(json!({"path": path, "rows": count}))
        }
        None => Ok(json!({"csv": text, "name": format!("{name}.csv"), "rows": count})),
    }
}

/// One edit as a preview shows it: which row, which column, from what to what.
pub(crate) struct EditLine {
    pub label: String,
    pub column_id: String,
    pub column: String,
    pub before: String,
    pub after: String,
}

/// What a row is called: its title, its code, or its first cell.
fn row_label(table: &Table, grid_row: &[super::sheet::Cell]) -> String {
    let field = match table.origin {
        Origin::User => table
            .columns
            .first()
            .map(|c| c.id.clone())
            .unwrap_or_default(),
        _ => super::catalog::label_field(&table.id).to_string(),
    };
    let idx = table
        .col_index(&field)
        .or_else(|| table.col_index("title"))
        .or_else(|| table.col_index("name"))
        .unwrap_or(0);
    let text = grid_row
        .get(idx)
        .map(|c| cell_text(&c.v))
        .unwrap_or_default();
    let text = text.lines().next().unwrap_or("").trim().to_string();
    if text.is_empty() {
        "(untitled)".to_string()
    } else {
        text.chars().take(80).collect()
    }
}

/// Checks edits without making them, and says what each would do. The first
/// one that can't be made is the error, in a sentence that names the row and
/// the column, so whoever asked can put it right. A row whose id starts
/// `new:` is one a staged change will make: only its column is checked.
pub(crate) fn preview_edits(
    i: &Inner,
    table_name: &str,
    edits: &[(String, String, Json)],
) -> Result<(String, String, Vec<EditLine>), CoreError> {
    let clock = i.clock();
    let store = i.store();
    let sheets = Sheets::new(&store, &clock);
    let id = sheets
        .resolve(table_name)
        .ok_or_else(|| refused(format!("There is no table called '{table_name}'.")))?;
    let table = sheets.table(&id)?;
    if let Some(why) = &table.locked {
        return Err(refused(format!(
            "{} can't be changed here. {why}",
            table.name
        )));
    }
    let grid = sheets.grid(&table);
    let mut out = Vec::new();
    for (row, column, value) in edits {
        let col = table.col_index(column).ok_or_else(|| {
            let names: Vec<&str> = table
                .columns
                .iter()
                .filter(|c| c.locked.is_none())
                .map(|c| c.name.as_str())
                .collect();
            refused(format!(
                "{} has no column called '{column}'. The ones that can be changed are: {}.",
                table.name,
                names.join(", ")
            ))
        })?;
        let c = &table.columns[col];
        if let Some(why) = &c.locked {
            return Err(refused(format!("'{}' can't be changed. {why}", c.name)));
        }
        let (label, before) = if row.starts_with("new:") {
            (row.clone(), String::new())
        } else {
            let r = table.row_index(row).ok_or_else(|| {
                refused(format!("{} has no row with the id '{row}'.", table.name))
            })?;
            (row_label(&table, &grid[r]), cell_text(&grid[r][col].v))
        };
        let stored = parse_input(&sheets, &table, c, value)
            .map_err(|why| refused(format!("{label}, {}: {why}", c.name)))?;
        // What the cell will read once the value is kept.
        let after = match (&c.relation, &stored) {
            (Some(target), Json::String(target_id)) => sheets
                .labels(target)
                .get(target_id)
                .cloned()
                .unwrap_or_else(|| target_id.clone()),
            _ => cell_text(&super::sheet::cell_of(Some(&stored), c.ty, &clock.zone)),
        };
        out.push(EditLine {
            label,
            column_id: c.id.clone(),
            column: c.name.clone(),
            before,
            after,
        });
    }
    Ok((table.id.clone(), table.name.clone(), out))
}

/// What rows are called, by their ids: for a preview of rows to be deleted.
pub(crate) fn row_labels(
    i: &Inner,
    table_name: &str,
    rows: &[String],
) -> Result<(String, String, Vec<String>), CoreError> {
    let clock = i.clock();
    let store = i.store();
    let sheets = Sheets::new(&store, &clock);
    let id = sheets
        .resolve(table_name)
        .ok_or_else(|| refused(format!("There is no table called '{table_name}'.")))?;
    let table = sheets.table(&id)?;
    if let Some(why) = &table.locked {
        return Err(refused(format!(
            "{} can't be changed here. {why}",
            table.name
        )));
    }
    let grid = sheets.grid(&table);
    let mut out = Vec::new();
    for row in rows {
        let r = table
            .row_index(row)
            .ok_or_else(|| refused(format!("{} has no row with the id '{row}'.", table.name)))?;
        out.push(row_label(&table, &grid[r]));
    }
    Ok((table.id.clone(), table.name.clone(), out))
}
