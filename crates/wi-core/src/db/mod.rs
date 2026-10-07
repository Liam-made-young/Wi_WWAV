//! The Database tab's commands, `db.*` (docs/ASK.md): Learn's own records
//! and a person's tables as a spreadsheet.
//!
//! - `catalog`: which tables there are and what each column is.
//! - `sheet`: a table's cells, with formula columns worked out.
//! - `query`: a table read through a view; pivots and charts.
//! - `write`: edits, rows, columns, tables, views, CSV.
//!
//! Reads hold the library for as long as they take and write nothing.
//! Writes to Learn's records are the `heat.*` commands the views use; the
//! rest are journal entries of this module's own kinds, kept on this Mac.

mod catalog;
mod csv;
mod query;
mod sheet;
mod write;

use serde_json::{json, Map, Value as Json};
use wi_formula::Formula;

pub(crate) use catalog::kind;
use sheet::{value_json, Origin, Sheets};

use crate::args::Args;
use crate::heat_cmd::{core_error, open_heat};
use crate::{CoreError, Inner};

/// The kinds this module keeps, none of which leaves this Mac.
pub(crate) const KINDS: [&str; 5] = [
    kind::TABLE,
    kind::ROW,
    kind::COLUMN,
    kind::VIEW,
    kind::LAYOUT,
];

fn refused(sentence: impl Into<String>) -> CoreError {
    CoreError::new("refused", sentence)
}

/// `db.tables {}`: every table for the sidebar, with how many rows each
/// holds, the saved views, each table's last layout, and the functions a
/// formula may call.
fn tables(i: &Inner) -> Result<Json, CoreError> {
    open_heat(i)?;
    let clock = i.clock();
    let store = i.store();
    let sheets = Sheets::new(&store, &clock);
    let user_rows = wi_heat_store::all(&store, kind::ROW).map_err(core_error)?;
    let mut out = Vec::new();
    for (id, name, origin) in sheets.list() {
        let count = match origin {
            Origin::User => user_rows
                .iter()
                .filter(|r| r["tableId"].as_str() == Some(id.as_str()))
                .count(),
            Origin::Learn => store.docs(&id)?.len(),
            Origin::Derived => sheets.table(&id)?.rows.len(),
        };
        out.push(json!({"id": id, "name": name, "origin": origin.as_str(), "count": count}));
    }
    let views = wi_heat_store::all(&store, kind::VIEW).map_err(core_error)?;
    let mut layouts = Map::new();
    for l in wi_heat_store::all(&store, kind::LAYOUT).map_err(core_error)? {
        if let Some(t) = l["table"].as_str() {
            layouts.insert(t.to_string(), l["spec"].clone());
        }
    }
    Ok(json!({
        "tables": out,
        "views": views,
        "layouts": layouts,
        "functions": wi_formula::function_names(),
    }))
}

/// The table a command names, by its id or its name.
fn with_table<T>(
    i: &Inner,
    a: &Args,
    f: impl FnOnce(&Sheets, &std::rc::Rc<sheet::Table>) -> Result<T, CoreError>,
) -> Result<T, CoreError> {
    open_heat(i)?;
    let name = a.str("table")?;
    let clock = i.clock();
    let store = i.store();
    let sheets = Sheets::new(&store, &clock);
    let id = sheets
        .resolve(name)
        .ok_or_else(|| refused(format!("There is no table called '{name}'.")))?;
    let table = sheets.table(&id)?;
    f(&sheets, &table)
}

/// A view's spec: the one sent, or the saved view named by `view`.
fn spec_of(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    if let Some(spec) = a.get("spec") {
        return Ok(spec.clone());
    }
    let Some(view) = a.opt_str("view") else {
        return Ok(json!({}));
    };
    let views = wi_heat_store::all(&i.store(), kind::VIEW).map_err(core_error)?;
    views
        .iter()
        .find(|v| {
            v["id"].as_str() == Some(view)
                || v["name"]
                    .as_str()
                    .is_some_and(|n| n.eq_ignore_ascii_case(view))
        })
        .map(|v| v["spec"].clone())
        .ok_or_else(|| refused(format!("There is no saved view called '{view}'.")))
}

/// `db.formula.check {table, formula}`: whether a formula reads, and what it
/// gives for the first rows, for the editor to show as it is typed.
fn formula_check(i: &Inner, a: &Args) -> Result<Json, CoreError> {
    let src = a.str("formula")?;
    let formula = match Formula::parse(src) {
        Ok(f) => f,
        Err(e) => return Ok(json!({"ok": false, "message": e.message, "at": e.at})),
    };
    with_table(i, a, |sheets, table| {
        let sample: Vec<Json> = sheets
            .sample(table, &formula, 5)
            .iter()
            .map(value_json)
            .collect();
        // A column the formula names that isn't there is said before it is saved.
        let missing = formula.refs().into_iter().find_map(|r| match &r.table {
            None => table
                .col_index(&r.column)
                .is_none()
                .then(|| format!("{} has no column called '{}'.", table.name, r.column)),
            Some(t) => match sheets.resolve(t) {
                None => Some(format!("There is no table called '{t}'.")),
                Some(id) => match sheets.table(&id) {
                    Ok(other) => other
                        .col_index(&r.column)
                        .is_none()
                        .then(|| format!("{} has no column called '{}'.", other.name, r.column)),
                    Err(e) => Some(e.message),
                },
            },
        });
        Ok(match missing {
            Some(message) => json!({"ok": false, "message": message, "at": 0, "sample": sample}),
            None => json!({"ok": true, "sample": sample}),
        })
    })
}

pub(crate) fn invoke(i: &Inner, cmd: &str, a: &Args) -> Result<Json, CoreError> {
    match cmd {
        "db.tables" => tables(i),
        "db.query" => {
            let spec = spec_of(i, a)?;
            let (limit, offset) = (a.opt_usize("limit")?, a.opt_usize("offset")?.unwrap_or(0));
            with_table(i, a, |sheets, table| {
                query::query(sheets, table, &spec, limit, offset)
            })
        }
        "db.pivot" => {
            let spec = spec_of(i, a)?;
            let args = Json::Object(a.object());
            with_table(i, a, |sheets, table| {
                query::pivot(sheets, table, &spec, &args)
            })
        }
        "db.chart" => {
            let spec = spec_of(i, a)?;
            let args = Json::Object(a.object());
            with_table(i, a, |sheets, table| {
                query::chart(sheets, table, &spec, &args)
            })
        }
        "db.formula.check" => formula_check(i, a),

        "db.cells.set" => write::cells_set(i, a),
        "db.rows.add" => write::rows_add(i, a),
        "db.rows.delete" => write::rows_delete(i, a),
        "db.table.create" => write::table_create(i, a),
        "db.table.rename" => write::table_rename(i, a),
        "db.table.delete" => write::table_delete(i, a),
        "db.column.add" => write::column_add(i, a),
        "db.column.update" => write::column_update(i, a),
        "db.column.delete" => write::column_delete(i, a),
        "db.view.save" => write::view_save(i, a),
        "db.view.delete" => write::view_delete(i, a),
        "db.layout.set" => write::layout_set(i, a),
        "db.csv.import" => write::csv_import(i, a),
        "db.csv.export" => write::csv_export(i, a),

        _ => Err(CoreError::new(
            "unknown_command",
            format!("There is no command called '{cmd}'."),
        )),
    }
}
