//! Reading a table through a view: filters, a search, sorts, a grouping, a
//! summary per column, a pivot and a chart's numbers. The grid, the prompt
//! box's `database.query` tool and CSV export all read through here, so a
//! view means the same thing to each.
//!
//! A view's spec, as it is saved and as it is sent:
//!
//! ```jsonc
//! { "filters": [{"column": "due", "op": "lt", "value": "2026-10-12"}],
//!   "match": "all",                       // or "any"
//!   "search": "kanji",
//!   "sorts": [{"column": "due", "dir": "asc"}],
//!   "group": "courseId",
//!   "summary": {"estMin": "sum"},
//!   // what only the grid reads:
//!   "hidden": [], "order": [], "widths": {}, "frozen": 1,
//!   "pivot": {"rows": ["courseId", "type"], "values": [{"column": "estMin", "agg": "sum"}], "across": false},
//!   "charts": [{"id": "c1", "type": "bar", "x": "courseId", "y": ["estMin"], "agg": "sum"}] }
//! ```
//!
//! A column is named by its id or its name, in any case. Ops: `eq`, `ne`,
//! `contains`, `not_contains`, `starts`, `ends`, `gt`, `ge`, `lt`, `le`,
//! `blank`, `not_blank`, `is` (true or false), `in` (a list).

use std::cmp::Ordering;
use std::rc::Rc;

use serde_json::{json, Map, Value as Json};
use wi_formula::{compare, parse_date, parse_number, Value};

use super::catalog::ColType;
use super::sheet::{cell_json, cell_text, value_json, Cell, Sheets, Table};
use crate::CoreError;

fn bad(sentence: impl Into<String>) -> CoreError {
    CoreError::new("refused", sentence)
}

pub(crate) fn column_index(table: &Table, name: &str) -> Result<usize, CoreError> {
    table
        .col_index(name)
        .ok_or_else(|| bad(format!("{} has no column called '{name}'.", table.name)))
}

struct Filter {
    col: usize,
    op: String,
    value: Value,
    list: Vec<Value>,
}

/// A filter's value as the column's type reads it.
fn typed(v: &Json, ty: ColType) -> Value {
    match v {
        Json::Null => Value::Blank,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => n.as_f64().map_or(Value::Blank, Value::Number),
        Json::String(s) => {
            let t = s.trim();
            match ty {
                ColType::Number => {
                    parse_number(t).map_or(Value::Text(t.to_string()), Value::Number)
                }
                ColType::Date | ColType::DateTime => {
                    parse_date(t).map_or(Value::Text(t.to_string()), Value::Date)
                }
                ColType::Bool => match t.to_ascii_lowercase().as_str() {
                    "true" | "yes" | "done" | "checked" => Value::Bool(true),
                    "false" | "no" | "open" | "unchecked" => Value::Bool(false),
                    _ => Value::Text(t.to_string()),
                },
                // A formula's column holds whatever the formula gives: read
                // the value as the number or the date it looks like.
                ColType::Formula => parse_number(t)
                    .map(Value::Number)
                    .or_else(|| parse_date(t).map(Value::Date))
                    .unwrap_or_else(|| Value::Text(t.to_string())),
                _ => Value::Text(t.to_string()),
            }
        }
        other => Value::Text(other.to_string()),
    }
}

impl Filter {
    fn matches(&self, cell: &Cell) -> bool {
        let v = &cell.v;
        let text = || cell_text(v).to_lowercase();
        let want = || cell_text(&self.value).to_lowercase();
        // A relation matches by what it shows or by the id it points at.
        let same = |other: &Value| {
            compare(v, other) == Ordering::Equal
                || cell
                    .link
                    .as_deref()
                    .is_some_and(|id| matches!(other, Value::Text(t) if t == id))
        };
        // A day matches every time on it: "due = 2026-10-09" finds 11:59 PM.
        let day_only = |x: &Value| match x {
            Value::Date(d) => Value::Date(d.floor()),
            other => other.clone(),
        };
        let whole_day = matches!(&self.value, Value::Date(d) if d.fract() == 0.0);
        let (lhs, rhs) = if whole_day {
            (day_only(v), self.value.clone())
        } else {
            (v.clone(), self.value.clone())
        };
        match self.op.as_str() {
            "eq" | "=" | "is" if whole_day => compare(&lhs, &rhs) == Ordering::Equal,
            "eq" | "=" | "is" => same(&self.value),
            "ne" | "<>" | "!=" | "is_not" if whole_day => compare(&lhs, &rhs) != Ordering::Equal,
            "ne" | "<>" | "!=" | "is_not" => !same(&self.value),
            "contains" => text().contains(&want()),
            "not_contains" => !text().contains(&want()),
            "starts" => text().starts_with(&want()),
            "ends" => text().ends_with(&want()),
            "blank" => v.is_blank(),
            "not_blank" => !v.is_blank(),
            "in" => self.list.iter().any(same),
            "gt" | ">" | "ge" | ">=" | "lt" | "<" | "le" | "<=" => {
                // An empty cell is neither before nor after anything.
                if v.is_blank() || v.is_error() {
                    return false;
                }
                let o = compare(&lhs, &rhs);
                match self.op.as_str() {
                    "gt" | ">" => o == Ordering::Greater,
                    "ge" | ">=" => o != Ordering::Less,
                    "lt" | "<" => o == Ordering::Less,
                    _ => o != Ordering::Greater,
                }
            }
            _ => true,
        }
    }
}

const OPS: &[&str] = &[
    "eq",
    "=",
    "is",
    "ne",
    "<>",
    "!=",
    "is_not",
    "contains",
    "not_contains",
    "starts",
    "ends",
    "blank",
    "not_blank",
    "in",
    "gt",
    ">",
    "ge",
    ">=",
    "lt",
    "<",
    "le",
    "<=",
];

/// A view's spec, read against a table.
pub(crate) struct Spec {
    filters: Vec<Filter>,
    any: bool,
    search: String,
    sorts: Vec<(usize, bool)>,
    pub group: Option<usize>,
    /// Only these rows, when the tab asks about a selection.
    only: Option<Vec<String>>,
}

impl Spec {
    pub fn read(table: &Table, spec: &Json) -> Result<Spec, CoreError> {
        let mut filters = Vec::new();
        for f in spec["filters"].as_array().into_iter().flatten() {
            let name = f["column"]
                .as_str()
                .ok_or_else(|| bad("A filter names the column it reads."))?;
            let col = column_index(table, name)?;
            let op = f["op"].as_str().unwrap_or("eq").to_string();
            if !OPS.contains(&op.as_str()) {
                return Err(bad(format!(
                    "A filter can't '{op}'. It can: eq, ne, contains, not_contains, starts, ends, gt, ge, lt, le, blank, not_blank, in."
                )));
            }
            let ty = table.columns[col].ty;
            filters.push(Filter {
                col,
                op,
                value: typed(&f["value"], ty),
                list: f["value"]
                    .as_array()
                    .map(|l| l.iter().map(|v| typed(v, ty)).collect())
                    .unwrap_or_default(),
            });
        }
        let mut sorts = Vec::new();
        for s in spec["sorts"].as_array().into_iter().flatten() {
            let name = s["column"]
                .as_str()
                .ok_or_else(|| bad("A sort names the column it reads."))?;
            sorts.push((
                column_index(table, name)?,
                s["dir"].as_str() == Some("desc"),
            ));
        }
        let group = match spec["group"].as_str().filter(|g| !g.is_empty()) {
            Some(name) => Some(column_index(table, name)?),
            None => None,
        };
        Ok(Spec {
            filters,
            any: spec["match"].as_str() == Some("any"),
            search: spec["search"].as_str().unwrap_or("").trim().to_lowercase(),
            sorts,
            group,
            only: spec["rows"].as_array().map(|ids| {
                ids.iter()
                    .filter_map(|i| i.as_str().map(String::from))
                    .collect()
            }),
        })
    }

    fn keeps(&self, table: &Table, row: usize, cells: &[Cell]) -> bool {
        if let Some(only) = &self.only {
            if !only.contains(&table.rows[row].id) {
                return false;
            }
        }
        if !self.filters.is_empty() {
            let mut hits = self.filters.iter().map(|f| f.matches(&cells[f.col]));
            let ok = if self.any {
                hits.any(|h| h)
            } else {
                hits.all(|h| h)
            };
            if !ok {
                return false;
            }
        }
        if !self.search.is_empty()
            && !cells
                .iter()
                .any(|c| cell_text(&c.v).to_lowercase().contains(&self.search))
        {
            return false;
        }
        true
    }
}

/// Blanks last whichever way a sort runs, as a spreadsheet leaves them.
fn sort_order(a: &Value, b: &Value, desc: bool) -> Ordering {
    match (a.is_blank(), b.is_blank()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        _ => {
            let o = compare(a, b);
            if desc {
                o.reverse()
            } else {
                o
            }
        }
    }
}

/// A table read through a view: the rows that pass, in order.
pub(crate) struct Read {
    pub grid: Vec<Vec<Cell>>,
    /// Indexes into `table.rows` and `grid`, in the view's order.
    pub order: Vec<usize>,
    /// Where each group starts in `order`, when the view groups.
    pub groups: Vec<(Value, usize, usize)>,
}

pub(crate) fn read(sheets: &Sheets, table: &Rc<Table>, spec: &Spec) -> Read {
    let grid = sheets.grid(table);
    let mut order: Vec<usize> = (0..table.rows.len())
        .filter(|&r| spec.keeps(table, r, &grid[r]))
        .collect();
    // A stable sort by each key, the last first, so the first key decides.
    for (col, desc) in spec.sorts.iter().rev() {
        order.sort_by(|a, b| sort_order(&grid[*a][*col].v, &grid[*b][*col].v, *desc));
    }
    let mut groups = Vec::new();
    if let Some(g) = spec.group {
        order.sort_by(|a, b| sort_order(&grid[*a][g].v, &grid[*b][g].v, false));
        let mut start = 0;
        while start < order.len() {
            let key = grid[order[start]][g].v.clone();
            let mut end = start + 1;
            while end < order.len() && compare(&grid[order[end]][g].v, &key) == Ordering::Equal {
                end += 1;
            }
            groups.push((key, start, end - start));
            start = end;
        }
    }
    Read {
        grid,
        order,
        groups,
    }
}

pub(crate) const AGGS: &[&str] = &[
    "sum",
    "average",
    "count",
    "filled",
    "empty",
    "unique",
    "min",
    "max",
    "median",
    "checked",
    "unchecked",
];

/// One total over some values: what a summary row, a pivot and a chart share.
pub(crate) fn aggregate(values: &[&Value], agg: &str) -> Value {
    let numbers: Vec<f64> = values
        .iter()
        .filter_map(|v| match v {
            Value::Number(n) | Value::Date(n) => Some(*n),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        })
        .collect();
    let dates = !values.is_empty()
        && values
            .iter()
            .filter(|v| !v.is_blank())
            .all(|v| matches!(v, Value::Date(_)));
    let num_or_date = |x: f64| {
        if dates {
            Value::Date(x)
        } else {
            Value::Number(x)
        }
    };
    match agg {
        "count" => Value::Number(values.len() as f64),
        "filled" => Value::Number(values.iter().filter(|v| !v.is_blank()).count() as f64),
        "empty" => Value::Number(values.iter().filter(|v| v.is_blank()).count() as f64),
        "checked" => Value::Number(
            values
                .iter()
                .filter(|v| matches!(v, Value::Bool(true)))
                .count() as f64,
        ),
        "unchecked" => Value::Number(
            values
                .iter()
                .filter(|v| matches!(v, Value::Bool(false)))
                .count() as f64,
        ),
        "unique" => {
            let mut seen: Vec<&Value> = Vec::new();
            for v in values.iter().filter(|v| !v.is_blank()) {
                if !seen.iter().any(|s| compare(s, v) == Ordering::Equal) {
                    seen.push(v);
                }
            }
            Value::Number(seen.len() as f64)
        }
        _ if numbers.is_empty() => Value::Blank,
        "sum" => Value::Number(numbers.iter().sum()),
        "average" | "avg" => Value::Number(numbers.iter().sum::<f64>() / numbers.len() as f64),
        "min" => num_or_date(numbers.iter().cloned().fold(f64::INFINITY, f64::min)),
        "max" => num_or_date(numbers.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
        "median" => {
            let mut xs = numbers;
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
            let mid = xs.len() / 2;
            num_or_date(if xs.len() % 2 == 1 {
                xs[mid]
            } else {
                (xs[mid - 1] + xs[mid]) / 2.0
            })
        }
        _ => Value::Blank,
    }
}

fn check_agg(agg: &str) -> Result<(), CoreError> {
    if AGGS.contains(&agg) || agg == "avg" {
        Ok(())
    } else {
        Err(bad(format!(
            "There is no total called '{agg}'. There are: {}.",
            AGGS.join(", ")
        )))
    }
}

/// Rounds a total so 0.1 + 0.2 reads 0.3.
fn tidy(v: Value) -> Value {
    match v {
        Value::Number(n) => Value::Number((n * 1e9).round() / 1e9),
        other => other,
    }
}

pub(crate) fn column_json(c: &super::catalog::Column) -> Json {
    let mut m = Map::new();
    m.insert("id".into(), json!(c.id));
    m.insert("name".into(), json!(c.name));
    m.insert("type".into(), json!(c.ty.as_str()));
    m.insert("locked".into(), json!(c.locked.is_some()));
    if let Some(why) = &c.locked {
        m.insert("why".into(), json!(why));
    }
    if let Some(t) = &c.relation {
        m.insert("relation".into(), json!(t));
    }
    if !c.options.is_empty() {
        m.insert("options".into(), json!(c.options));
    }
    if let Some(f) = &c.formula {
        m.insert("formula".into(), json!(f));
    }
    m.insert(
        "added".into(),
        json!(c.source == super::catalog::Source::Added),
    );
    Json::Object(m)
}

/// `db.query`: the rows of a view, with every column's summary.
pub(crate) fn query(
    sheets: &Sheets,
    table: &Rc<Table>,
    spec_json: &Json,
    limit: Option<usize>,
    offset: usize,
) -> Result<Json, CoreError> {
    let spec = Spec::read(table, spec_json)?;
    let r = read(sheets, table, &spec);
    let total = r.order.len();
    let rows: Vec<Json> = r
        .order
        .iter()
        .skip(offset)
        .take(limit.unwrap_or(usize::MAX))
        .map(|&i| {
            json!({
                "id": table.rows[i].id,
                "cells": r.grid[i].iter().map(cell_json).collect::<Vec<_>>(),
            })
        })
        .collect();
    // Each column's totals over the rows that passed, for the summary row.
    let mut summary = Map::new();
    for (c, col) in table.columns.iter().enumerate() {
        let values: Vec<&Value> = r.order.iter().map(|&i| &r.grid[i][c].v).collect();
        let numeric = values
            .iter()
            .any(|v| matches!(v, Value::Number(_) | Value::Date(_)));
        let mut one = Map::new();
        one.insert("count".into(), json!(values.len()));
        one.insert("filled".into(), value_json(&aggregate(&values, "filled")));
        one.insert("unique".into(), value_json(&aggregate(&values, "unique")));
        if numeric {
            for agg in ["sum", "average", "min", "max", "median"] {
                // Adding dates up means nothing; their first and last do.
                if matches!(col.ty, ColType::Date | ColType::DateTime)
                    && matches!(agg, "sum" | "average")
                {
                    continue;
                }
                one.insert(agg.into(), value_json(&tidy(aggregate(&values, agg))));
            }
        }
        if values.iter().any(|v| matches!(v, Value::Bool(_))) {
            one.insert("checked".into(), value_json(&aggregate(&values, "checked")));
        }
        summary.insert(col.id.clone(), Json::Object(one));
    }
    let groups: Vec<Json> = r
        .groups
        .iter()
        .map(|(key, start, count)| json!({"key": value_json(key), "label": cell_text(key), "start": start, "count": count}))
        .collect();
    Ok(json!({
        "table": {
            "id": table.id, "name": table.name, "origin": table.origin.as_str(),
            "locked": table.locked.is_some(), "why": table.locked,
        },
        "columns": table.columns.iter().map(column_json).collect::<Vec<_>>(),
        "rows": rows,
        "total": total,
        "all": table.rows.len(),
        "summary": summary,
        "groups": groups,
    }))
}

/// `db.pivot`: the rows of a view grouped by one or two columns, each group
/// with its count and the totals asked for.
pub(crate) fn pivot(
    sheets: &Sheets,
    table: &Rc<Table>,
    spec_json: &Json,
    args: &Json,
) -> Result<Json, CoreError> {
    let spec = Spec::read(table, spec_json)?;
    let r = read(sheets, table, &spec);
    let by: Vec<usize> = args["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Json::as_str)
        .map(|name| column_index(table, name))
        .collect::<Result<_, _>>()?;
    if by.is_empty() || by.len() > 2 {
        return Err(bad("A pivot groups by one or two columns."));
    }
    let mut values: Vec<(usize, String)> = Vec::new();
    for v in args["values"].as_array().into_iter().flatten() {
        let name = v["column"]
            .as_str()
            .ok_or_else(|| bad("A pivot's value names its column."))?;
        let agg = v["agg"].as_str().unwrap_or("sum").to_string();
        check_agg(&agg)?;
        values.push((column_index(table, name)?, agg));
    }
    let key_of =
        |row: usize| -> Vec<Value> { by.iter().map(|c| r.grid[row][*c].v.clone()).collect() };
    let same = |a: &[Value], b: &[Value]| {
        a.iter()
            .zip(b)
            .all(|(x, y)| compare(x, y) == Ordering::Equal)
    };
    let mut order = r.order.clone();
    order.sort_by(|a, b| {
        let (ka, kb) = (key_of(*a), key_of(*b));
        ka.iter()
            .zip(&kb)
            .map(|(x, y)| sort_order(x, y, false))
            .find(|o| *o != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    });
    let total_of = |rows: &[usize]| -> Vec<Json> {
        values
            .iter()
            .map(|(c, agg)| {
                let vs: Vec<&Value> = rows.iter().map(|&i| &r.grid[i][*c].v).collect();
                value_json(&tidy(aggregate(&vs, agg)))
            })
            .collect()
    };
    let mut groups = Vec::new();
    let mut start = 0;
    while start < order.len() {
        let key = key_of(order[start]);
        let mut end = start + 1;
        while end < order.len() && same(&key_of(order[end]), &key) {
            end += 1;
        }
        let rows = &order[start..end];
        groups.push(json!({
            "keys": key.iter().map(cell_text).collect::<Vec<_>>(),
            "count": rows.len(),
            "values": total_of(rows),
        }));
        start = end;
    }
    Ok(json!({
        "by": by.iter().map(|c| json!({"id": table.columns[*c].id, "name": table.columns[*c].name})).collect::<Vec<_>>(),
        "values": values.iter().map(|(c, agg)| json!({
            "id": table.columns[*c].id, "name": table.columns[*c].name, "agg": agg,
        })).collect::<Vec<_>>(),
        "groups": groups,
        "total": {"count": order.len(), "values": total_of(&order)},
    }))
}

/// `db.chart`: a chart's numbers. With `agg`, the rows are grouped by `x`
/// and each `y` is totalled per group (a bar or a line per category).
/// Without it every row is a point (a scatter, or a line over time).
pub(crate) fn chart(
    sheets: &Sheets,
    table: &Rc<Table>,
    spec_json: &Json,
    args: &Json,
) -> Result<Json, CoreError> {
    let spec = Spec::read(table, spec_json)?;
    let r = read(sheets, table, &spec);
    let x = column_index(
        table,
        args["x"]
            .as_str()
            .ok_or_else(|| bad("A chart names the column along the bottom."))?,
    )?;
    let ys: Vec<usize> = args["y"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Json::as_str)
        .map(|name| column_index(table, name))
        .collect::<Result<_, _>>()?;
    let agg = args["agg"].as_str().filter(|a| !a.is_empty());
    if let Some(a) = agg {
        check_agg(a)?;
    }
    if ys.is_empty() && agg != Some("count") {
        return Err(bad("A chart needs at least one column of numbers to draw."));
    }
    let number = |v: &Value| match v {
        Value::Number(n) | Value::Date(n) => super::sheet::value_json_number(*n),
        Value::Bool(b) => json!(if *b { 1 } else { 0 }),
        _ => Json::Null,
    };
    let x_is_number = r
        .order
        .iter()
        .any(|&i| matches!(r.grid[i][x].v, Value::Number(_)))
        && !r
            .order
            .iter()
            .any(|&i| matches!(r.grid[i][x].v, Value::Text(_)));
    let x_is_date = matches!(table.columns[x].ty, ColType::Date | ColType::DateTime)
        || (r
            .order
            .iter()
            .any(|&i| matches!(r.grid[i][x].v, Value::Date(_)))
            && !r
                .order
                .iter()
                .any(|&i| matches!(r.grid[i][x].v, Value::Text(_) | Value::Number(_))));
    let x_kind = if x_is_date {
        "date"
    } else if x_is_number {
        "number"
    } else {
        "category"
    };
    let mut order = r.order.clone();
    order.sort_by(|a, b| sort_order(&r.grid[*a][x].v, &r.grid[*b][x].v, false));
    let mut labels = Vec::new();
    let mut xs = Vec::new();
    let mut series: Vec<Vec<Json>> = vec![Vec::new(); ys.len().max(1)];
    match agg {
        Some(agg) => {
            let mut start = 0;
            while start < order.len() {
                let key = &r.grid[order[start]][x].v;
                // Dates group by the day, so a column of times charts per day.
                let bucket = |v: &Value| match v {
                    Value::Date(d) => Value::Date(d.floor()),
                    other => other.clone(),
                };
                let key = bucket(key);
                let mut end = start + 1;
                while end < order.len()
                    && compare(&bucket(&r.grid[order[end]][x].v), &key) == Ordering::Equal
                {
                    end += 1;
                }
                let rows = &order[start..end];
                labels.push(if key.is_blank() {
                    "(empty)".to_string()
                } else {
                    cell_text(&key)
                });
                xs.push(number(&key));
                if ys.is_empty() {
                    series[0].push(json!(rows.len()));
                }
                for (s, c) in ys.iter().enumerate() {
                    let vs: Vec<&Value> = rows.iter().map(|&i| &r.grid[i][*c].v).collect();
                    series[s].push(number(&tidy(aggregate(&vs, agg))));
                }
                start = end;
            }
        }
        None => {
            for &i in &order {
                let key = &r.grid[i][x].v;
                labels.push(cell_text(key));
                xs.push(number(key));
                for (s, c) in ys.iter().enumerate() {
                    series[s].push(number(&r.grid[i][*c].v));
                }
            }
        }
    }
    let names: Vec<String> = if ys.is_empty() {
        vec!["Count".to_string()]
    } else {
        ys.iter()
            .map(|c| match agg {
                Some(a) if a != "sum" => format!("{} ({a})", table.columns[*c].name),
                _ => table.columns[*c].name.clone(),
            })
            .collect()
    };
    Ok(json!({
        "x": {"id": table.columns[x].id, "name": table.columns[x].name, "kind": x_kind},
        "labels": labels,
        "xs": xs,
        "series": names.iter().zip(series).map(|(name, values)| json!({"name": name, "values": values})).collect::<Vec<_>>(),
    }))
}
