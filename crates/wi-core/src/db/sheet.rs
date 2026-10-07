//! Tables as the Database tab reads them: Learn's records and a person's own
//! rows turned into typed cells, with formula columns worked out on demand.
//!
//! A [`Sheets`] is one reading of the library. It loads a table the first
//! time it is asked for (the one on screen, then any other a formula names),
//! and works a formula column out once for all its rows. A formula that
//! reads another formula column waits for that one; two that read each other
//! are `#CYCLE!` rather than a hang.

use std::cell::{OnceCell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use jiff::tz::TimeZone;
use jiff::Timestamp;
use serde_json::{json, Value as Json};
use wi_formula::{
    civil_from_days, days_from_civil, format_date, format_number, parse_date, time_of, Context,
    Error as FError, Formula, Value,
};
use wi_heat_store::snapshot::{self, Window};
use wi_heat_store::tables::KindFields;
use wi_heat_store::Clock;
use wi_store::Store;

use super::catalog::{
    self, columns_of, derived_table_columns, kind, label_field, learn_tables, table_name, ColType,
    Column, Source,
};
use crate::heat_cmd::core_error;
use crate::CoreError;

/// One cell: its value, and for a relation the row it points at.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cell {
    pub v: Value,
    pub link: Option<String>,
}

impl Cell {
    pub fn of(v: Value) -> Cell {
        Cell { v, link: None }
    }
}

pub(crate) struct Row {
    pub id: String,
    /// One per column. A formula column's cell is blank here; its values
    /// come from [`Sheets::column`].
    pub cells: Vec<Cell>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    /// One of Learn's kinds of record.
    Learn,
    /// Made from part of a record: a habit's days, a course's categories.
    Derived,
    /// A table the person made.
    User,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::Learn => "learn",
            Origin::Derived => "derived",
            Origin::User => "user",
        }
    }
}

pub(crate) struct Table {
    /// The record kind, or a `dbTable`'s id.
    pub id: String,
    pub name: String,
    pub origin: Origin,
    /// Why nothing in it can be typed over, when that is so.
    pub locked: Option<String>,
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    pub kind: Option<KindFields>,
}

impl Table {
    /// A column by its id, else by its name in any case.
    pub fn col_index(&self, name: &str) -> Option<usize> {
        let name = name.trim();
        self.columns
            .iter()
            .position(|c| c.id == name)
            .or_else(|| {
                self.columns
                    .iter()
                    .position(|c| c.name.eq_ignore_ascii_case(name))
            })
            .or_else(|| {
                self.columns
                    .iter()
                    .position(|c| c.id.eq_ignore_ascii_case(name))
            })
    }

    pub fn row_index(&self, id: &str) -> Option<usize> {
        self.rows.iter().position(|r| r.id == id)
    }
}

/// An instant as a day count in the person's zone, the time as the fraction.
pub(crate) fn local_days(ms: f64, tz: &TimeZone) -> Option<f64> {
    if !ms.is_finite() {
        return None;
    }
    let z = Timestamp::from_millisecond(ms as i64)
        .ok()?
        .to_zoned(tz.clone());
    let day = days_from_civil(z.year() as i64, z.month() as i64, z.day() as i64);
    let secs = z.hour() as f64 * 3600.0 + z.minute() as f64 * 60.0 + z.second() as f64;
    Some(day + secs / 86_400.0)
}

/// A local day count as an instant, in epoch milliseconds.
pub(crate) fn instant_ms(days: f64, tz: &TimeZone) -> Option<f64> {
    let (y, m, d) = civil_from_days(days);
    let (h, min, s) = time_of(days);
    let dt = jiff::civil::DateTime::new(
        i16::try_from(y).ok()?,
        m as i8,
        d as i8,
        h as i8,
        min as i8,
        s as i8,
        0,
    )
    .ok()?;
    let z = dt.to_zoned(tz.clone()).ok()?;
    Some(z.timestamp().as_millisecond() as f64)
}

/// `YYYY-MM-DD` for a whole day count.
pub(crate) fn day_key(days: f64) -> String {
    format_date(days.floor())
}

/// A list or an object as a cell shows it.
fn json_text(v: &Json) -> String {
    match v {
        Json::Null => String::new(),
        Json::String(s) => s.clone(),
        Json::Array(items) if items.iter().all(|i| i.is_string() || i.is_number()) => items
            .iter()
            .map(|i| match i {
                Json::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(", "),
        Json::Array(items)
            if items
                .iter()
                .all(|i| i.get("name").and_then(Json::as_str).is_some()) =>
        {
            items
                .iter()
                .map(|i| i["name"].as_str().unwrap_or("").to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
        Json::Object(m) if m.values().all(Json::is_boolean) => {
            format!(
                "{} days",
                m.values().filter(|b| **b == Json::Bool(true)).count()
            )
        }
        other => other.to_string(),
    }
}

/// A stored value as a cell of a column's type.
pub(crate) fn cell_of(v: Option<&Json>, ty: ColType, tz: &TimeZone) -> Value {
    let Some(v) = v.filter(|v| !v.is_null()) else {
        return if ty == ColType::Bool {
            Value::Bool(false)
        } else {
            Value::Blank
        };
    };
    match ty {
        ColType::Number => match v {
            Json::Number(n) => n.as_f64().map_or(Value::Blank, Value::Number),
            Json::String(s) => wi_formula::parse_number(s).map_or_else(
                || {
                    if s.is_empty() {
                        Value::Blank
                    } else {
                        Value::Text(s.clone())
                    }
                },
                Value::Number,
            ),
            Json::Bool(b) => Value::Number(if *b { 1.0 } else { 0.0 }),
            other => Value::Text(json_text(other)),
        },
        ColType::Bool => match v {
            Json::Bool(b) => Value::Bool(*b),
            Json::Number(n) => Value::Bool(n.as_f64().unwrap_or(0.0) != 0.0),
            Json::String(s) => Value::Bool(matches!(
                s.trim().to_ascii_lowercase().as_str(),
                "true" | "yes" | "y" | "x" | "1" | "done"
            )),
            _ => Value::Bool(false),
        },
        ColType::Date => match v {
            Json::String(s) => parse_date(s).map_or_else(|| Value::Text(s.clone()), Value::Date),
            Json::Number(n) => n
                .as_f64()
                .and_then(|ms| local_days(ms, tz))
                .map_or(Value::Blank, |d| Value::Date(d.floor())),
            other => Value::Text(json_text(other)),
        },
        ColType::DateTime => match v {
            Json::Number(n) => n
                .as_f64()
                .and_then(|ms| local_days(ms, tz))
                .map_or(Value::Blank, Value::Date),
            Json::String(s) => parse_date(s).map_or_else(|| Value::Text(s.clone()), Value::Date),
            other => Value::Text(json_text(other)),
        },
        ColType::Json => Value::Text(json_text(v)),
        ColType::Text | ColType::Relation | ColType::Formula => match v {
            Json::String(s) => Value::Text(s.clone()),
            Json::Number(n) => n.as_f64().map_or(Value::Blank, Value::Number),
            Json::Bool(b) => Value::Bool(*b),
            other => Value::Text(json_text(other)),
        },
    }
}

/// A cell as the tab receives it: a relation as `{id, label}`, a failed
/// formula as `{error, message}`, a date as text, the rest as themselves.
pub(crate) fn cell_json(cell: &Cell) -> Json {
    if let Some(id) = &cell.link {
        return json!({"id": id, "label": cell.v.to_text()});
    }
    value_json(&cell.v)
}

pub(crate) fn value_json(v: &Value) -> Json {
    match v {
        Value::Blank => Json::Null,
        Value::Number(n) => {
            if n.is_finite() && n.fract() == 0.0 && n.abs() < 9e15 {
                json!(*n as i64)
            } else if n.is_finite() {
                json!(n)
            } else {
                Json::Null
            }
        }
        Value::Text(s) => json!(s),
        Value::Bool(b) => json!(b),
        Value::Date(d) => json!(format_date(*d)),
        Value::List(_) => json!(v.to_text()),
        Value::Error(e) => json!({"error": e.code, "message": e.message}),
    }
}

/// What a cell reads as in a search, a CSV file and a group's heading.
pub(crate) fn cell_text(v: &Value) -> String {
    match v {
        Value::Number(n) => format_number(*n),
        other => other.to_text(),
    }
}

type Key = (String, usize);

/// One reading of the library, table by table as they are asked for.
pub(crate) struct Sheets<'a> {
    pub store: &'a Store,
    pub clock: &'a Clock,
    tables: RefCell<HashMap<String, Rc<Table>>>,
    columns: RefCell<HashMap<Key, Rc<Vec<Value>>>>,
    computing: RefCell<HashSet<Key>>,
    derived: OnceCell<Json>,
    user_tables: OnceCell<Vec<Json>>,
    user_rows: OnceCell<Vec<Json>>,
    added: OnceCell<Vec<Json>>,
    labels: RefCell<HashMap<String, Rc<HashMap<String, String>>>>,
    records: RefCell<HashMap<String, Rc<Vec<Json>>>>,
}

fn err(e: wi_heat_store::Error) -> CoreError {
    core_error(e)
}

impl<'a> Sheets<'a> {
    pub fn new(store: &'a Store, clock: &'a Clock) -> Sheets<'a> {
        Sheets {
            store,
            clock,
            tables: RefCell::default(),
            columns: RefCell::default(),
            computing: RefCell::default(),
            derived: OnceCell::new(),
            user_tables: OnceCell::new(),
            user_rows: OnceCell::new(),
            added: OnceCell::new(),
            labels: RefCell::default(),
            records: RefCell::default(),
        }
    }

    pub fn today(&self) -> f64 {
        local_days(self.clock.now_ms, &self.clock.zone).map_or(0.0, f64::floor)
    }

    fn records(&self, k: &str) -> Result<Rc<Vec<Json>>, CoreError> {
        if let Some(have) = self.records.borrow().get(k) {
            return Ok(have.clone());
        }
        let list = Rc::new(wi_heat_store::all(self.store, k).map_err(err)?);
        self.records
            .borrow_mut()
            .insert(k.to_string(), list.clone());
        Ok(list)
    }

    fn user_tables(&self) -> &Vec<Json> {
        self.user_tables
            .get_or_init(|| wi_heat_store::all(self.store, kind::TABLE).unwrap_or_default())
    }

    fn user_rows(&self) -> &Vec<Json> {
        self.user_rows
            .get_or_init(|| wi_heat_store::all(self.store, kind::ROW).unwrap_or_default())
    }

    /// The columns people added, to their own tables and to Learn's.
    pub fn added_columns(&self) -> &Vec<Json> {
        self.added
            .get_or_init(|| wi_heat_store::all(self.store, kind::COLUMN).unwrap_or_default())
    }

    /// What Learn works out, as the snapshot gives it.
    fn derived(&self) -> &Json {
        self.derived.get_or_init(|| {
            snapshot::snapshot(self.store, self.clock, &Window::default())
                .map(|mut s| s["derived"].take())
                .unwrap_or(Json::Null)
        })
    }

    /// Every table there is: id, name, origin, in the sidebar's order.
    pub fn list(&self) -> Vec<(String, String, Origin)> {
        let mut out: Vec<(String, String, Origin)> = learn_tables()
            .into_iter()
            .map(|(id, k)| {
                let origin = if k.is_some() {
                    Origin::Learn
                } else {
                    Origin::Derived
                };
                (id.clone(), table_name(&id), origin)
            })
            .collect();
        for t in self.user_tables() {
            if let (Some(id), Some(name)) = (t["id"].as_str(), t["name"].as_str()) {
                out.push((id.to_string(), name.to_string(), Origin::User));
            }
        }
        out
    }

    /// A table's id from its id or its name, in any case.
    pub fn resolve(&self, name: &str) -> Option<String> {
        let name = name.trim();
        let all = self.list();
        all.iter()
            .find(|(id, _, _)| id == name)
            .or_else(|| all.iter().find(|(_, n, _)| n.eq_ignore_ascii_case(name)))
            .or_else(|| all.iter().find(|(id, _, _)| id.eq_ignore_ascii_case(name)))
            // "tasks" for "task": the plural of a kind.
            .or_else(|| {
                all.iter().find(|(id, _, o)| {
                    *o != Origin::User && format!("{id}s").eq_ignore_ascii_case(name)
                })
            })
            .map(|(id, _, _)| id.clone())
    }

    /// What a row of `table` is called, by its id.
    pub fn labels(&self, table: &str) -> Rc<HashMap<String, String>> {
        if let Some(have) = self.labels.borrow().get(table) {
            return have.clone();
        }
        let mut map = HashMap::new();
        if table == "gradeCategory" {
            for course in self.records("course").unwrap_or_default().iter() {
                for cat in course["categories"].as_array().into_iter().flatten() {
                    if let (Some(id), Some(name)) = (cat["id"].as_str(), cat["name"].as_str()) {
                        map.insert(id.to_string(), name.to_string());
                    }
                }
            }
        } else {
            let field = label_field(table);
            for r in self.records(table).unwrap_or_default().iter() {
                let Some(id) = r["id"].as_str().or_else(|| r["date"].as_str()) else {
                    continue;
                };
                let label = r[field]
                    .as_str()
                    .or_else(|| r["title"].as_str())
                    .or_else(|| r["name"].as_str())
                    .unwrap_or("");
                let label = label.lines().next().unwrap_or("").trim();
                map.insert(id.to_string(), label.to_string());
            }
        }
        let map = Rc::new(map);
        self.labels
            .borrow_mut()
            .insert(table.to_string(), map.clone());
        map
    }

    fn relation_cell(&self, target: &str, id: Option<&str>) -> Cell {
        match id.filter(|s| !s.is_empty()) {
            None => Cell::of(Value::Blank),
            Some(id) => {
                let label = self
                    .labels(target)
                    .get(id)
                    .cloned()
                    .filter(|l| !l.is_empty())
                    .unwrap_or_else(|| id.to_string());
                Cell {
                    v: Value::Text(label),
                    link: Some(id.to_string()),
                }
            }
        }
    }

    /// The columns a person added to `table`, in their order.
    fn added_to(&self, table: &str) -> Vec<Column> {
        let mut docs: Vec<&Json> = self
            .added_columns()
            .iter()
            .filter(|c| c["table"].as_str() == Some(table))
            .collect();
        docs.sort_by(|a, b| {
            let (x, y) = (
                a["order"].as_f64().unwrap_or(0.0),
                b["order"].as_f64().unwrap_or(0.0),
            );
            x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
        });
        docs.into_iter()
            .filter_map(|c| {
                let ty = ColType::parse_user(c["type"].as_str().unwrap_or("text"))?;
                Some(Column {
                    id: c["id"].as_str()?.to_string(),
                    name: c["name"].as_str().unwrap_or("Column").to_string(),
                    ty,
                    locked: (ty == ColType::Formula).then(|| {
                        "A formula works this out. Edit the formula to change it.".to_string()
                    }),
                    relation: None,
                    options: c["options"]
                        .as_array()
                        .map(|o| {
                            o.iter()
                                .filter_map(|s| s.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    formula: c["formula"].as_str().map(String::from),
                    source: Source::Added,
                })
            })
            .collect()
    }

    /// A table with its rows, or the sentence for why there is none.
    pub fn table(&self, id: &str) -> Result<Rc<Table>, CoreError> {
        if let Some(have) = self.tables.borrow().get(id) {
            return Ok(have.clone());
        }
        let table = Rc::new(self.load(id)?);
        self.tables
            .borrow_mut()
            .insert(id.to_string(), table.clone());
        Ok(table)
    }

    fn load(&self, id: &str) -> Result<Table, CoreError> {
        if let Some(t) = self
            .user_tables()
            .iter()
            .find(|t| t["id"].as_str() == Some(id))
        {
            return Ok(self.load_user(id, t["name"].as_str().unwrap_or("Table")));
        }
        if catalog::DERIVED_TABLES.contains(&id) {
            return self.load_derived(id);
        }
        let Some((_, Some(k))) = learn_tables().into_iter().find(|(k, _)| k == id) else {
            return Err(CoreError::new(
                "refused",
                format!("There is no table called '{id}'."),
            ));
        };
        self.load_kind(k)
    }

    fn load_user(&self, id: &str, name: &str) -> Table {
        let columns = self.added_to(id);
        let tz = &self.clock.zone;
        let mut docs: Vec<&Json> = self
            .user_rows()
            .iter()
            .filter(|r| r["tableId"].as_str() == Some(id))
            .collect();
        docs.sort_by(|a, b| {
            let (x, y) = (
                a["order"].as_f64().unwrap_or(0.0),
                b["order"].as_f64().unwrap_or(0.0),
            );
            x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
        });
        let rows = docs
            .into_iter()
            .filter_map(|r| {
                Some(Row {
                    id: r["id"].as_str()?.to_string(),
                    cells: columns
                        .iter()
                        .map(|c| {
                            if c.ty == ColType::Formula {
                                Cell::of(Value::Blank)
                            } else {
                                Cell::of(cell_of(r["cells"].get(&c.id), c.ty, tz))
                            }
                        })
                        .collect(),
                })
            })
            .collect();
        Table {
            id: id.to_string(),
            name: name.to_string(),
            origin: Origin::User,
            locked: None,
            columns,
            rows,
            kind: None,
        }
    }

    fn load_derived(&self, id: &str) -> Result<Table, CoreError> {
        let columns = derived_table_columns(id);
        let mut rows = Vec::new();
        if id == "habitLog" {
            for habit in self.records("habit")?.iter() {
                let Some(hid) = habit["id"].as_str() else {
                    continue;
                };
                let log: BTreeMap<String, bool> = habit["log"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_bool().unwrap_or(false)))
                            .collect()
                    })
                    .unwrap_or_default();
                for (date, done) in log {
                    rows.push(Row {
                        id: format!("{hid}/{date}"),
                        cells: vec![
                            Cell::of(Value::Text(format!("{hid}/{date}"))),
                            self.relation_cell("habit", Some(hid)),
                            Cell::of(parse_date(&date).map_or(Value::Text(date), Value::Date)),
                            Cell::of(Value::Bool(done)),
                        ],
                    });
                }
            }
        } else {
            for course in self.records("course")?.iter() {
                let Some(cid) = course["id"].as_str() else {
                    continue;
                };
                for cat in course["categories"].as_array().into_iter().flatten() {
                    let Some(id) = cat["id"].as_str() else {
                        continue;
                    };
                    rows.push(Row {
                        id: id.to_string(),
                        cells: vec![
                            Cell::of(Value::Text(id.to_string())),
                            self.relation_cell("course", Some(cid)),
                            Cell::of(Value::Text(cat["name"].as_str().unwrap_or("").to_string())),
                            Cell::of(cat["weight"].as_f64().map_or(Value::Blank, Value::Number)),
                        ],
                    });
                }
            }
        }
        let mut columns = columns;
        columns.extend(self.added_to(id));
        for r in &mut rows {
            r.cells.resize(columns.len(), Cell::of(Value::Blank));
        }
        Ok(Table {
            id: id.to_string(),
            name: table_name(id),
            origin: Origin::Derived,
            locked: columns.first().and_then(|c| c.locked.clone()),
            columns,
            rows,
            kind: None,
        })
    }

    fn load_kind(&self, k: KindFields) -> Result<Table, CoreError> {
        let records = self.records(k.kind)?;
        let tz = &self.clock.zone;
        // A kind the schema doesn't spell out: its columns are the fields its records hold.
        let mut seen: Vec<String> = Vec::new();
        if k.fields.is_empty() {
            for r in records.iter() {
                for field in r.as_object().into_iter().flat_map(|m| m.keys()) {
                    if !seen.contains(field) {
                        seen.push(field.clone());
                    }
                }
            }
        }
        let mut columns = columns_of(&k, &seen);
        // A field this build has no type for is typed from what it holds.
        for c in columns.iter_mut().filter(|c| c.source == Source::Field) {
            if catalog::type_is_known(k.kind, &c.id) {
                continue;
            }
            let values: Vec<&Json> = records
                .iter()
                .filter_map(|r| r.get(&c.id))
                .filter(|v| !v.is_null())
                .collect();
            if values.is_empty() {
                continue;
            }
            if values.iter().all(|v| v.is_boolean()) {
                c.ty = ColType::Bool;
            } else if values.iter().all(|v| v.is_number()) {
                let instants = c.id.ends_with("At")
                    && values.iter().all(|v| v.as_f64().is_some_and(|n| n > 1e11));
                c.ty = if instants {
                    ColType::DateTime
                } else {
                    ColType::Number
                };
            } else if values.iter().any(|v| v.is_object() || v.is_array()) {
                c.ty = ColType::Json;
                c.locked.get_or_insert_with(|| {
                    "This holds a list. Change it where Learn shows it.".to_string()
                });
            } else if values.iter().all(|v| {
                v.as_str()
                    .is_some_and(|s| s.len() == 10 && parse_date(s).is_some())
            }) {
                c.ty = ColType::Date;
            }
        }
        columns.extend(self.added_to(k.kind));

        let derived = if columns
            .iter()
            .any(|c| matches!(c.source, Source::Derived(_)))
        {
            self.derived().clone()
        } else {
            Json::Null
        };
        let task_course: HashMap<String, String> = if k.kind == "focusSession" {
            self.records("task")?
                .iter()
                .filter_map(|t| {
                    Some((
                        t["id"].as_str()?.to_string(),
                        t["courseId"].as_str()?.to_string(),
                    ))
                })
                .collect()
        } else {
            HashMap::new()
        };
        let today_key = self.clock.today();
        let rows = records
            .iter()
            .filter_map(|r| {
                let id = r[k.key].as_str()?.to_string();
                let cells = columns
                    .iter()
                    .map(|c| match &c.source {
                        Source::Added => Cell::of(Value::Blank),
                        Source::Field => match (&c.relation, c.ty) {
                            (Some(target), ColType::Relation) => {
                                self.relation_cell(target, r[&c.id].as_str())
                            }
                            _ => Cell::of(cell_of(r.get(&c.id), c.ty, tz)),
                        },
                        Source::Derived(what) => {
                            let num = |v: &Json| v.as_f64().map_or(Value::Blank, Value::Number);
                            match (k.kind, *what) {
                                ("task", "heat") => {
                                    Cell::of(num(&derived["tasks"][&id]["heat"]["v"]))
                                }
                                ("task", "level") => Cell::of(cell_of(
                                    derived["tasks"][&id]["heat"].get("level"),
                                    ColType::Text,
                                    tz,
                                )),
                                ("task", "planned") => {
                                    Cell::of(num(&derived["tasks"][&id]["estimate"]["min"]))
                                }
                                ("task", "logged") => {
                                    Cell::of(num(&derived["tasks"][&id]["actualMin"]))
                                }
                                ("task", "next") => Cell::of(cell_of(
                                    derived["tasks"][&id].get("next"),
                                    ColType::Date,
                                    tz,
                                )),
                                ("course", "pct") => Cell::of(
                                    derived["courses"][&id]["currentPct"]
                                        .as_f64()
                                        .map_or(Value::Blank, |p| {
                                            Value::Number((p * 100.0).round() / 100.0)
                                        }),
                                ),
                                ("course", "letter") => Cell::of(cell_of(
                                    derived["courses"][&id].get("letter"),
                                    ColType::Text,
                                    tz,
                                )),
                                ("grade", "pct") => {
                                    match (r["score"].as_f64(), r["outOf"].as_f64()) {
                                        (Some(s), Some(o)) if o != 0.0 => Cell::of(Value::Number(
                                            (s / o * 10_000.0).round() / 100.0,
                                        )),
                                        _ => Cell::of(Value::Blank),
                                    }
                                }
                                ("habit", "today") => Cell::of(Value::Bool(
                                    r["log"][&today_key].as_bool().unwrap_or(false),
                                )),
                                ("habit", "days") => {
                                    Cell::of(Value::Number(r["log"].as_object().map_or(0, |m| {
                                        m.values().filter(|v| **v == Json::Bool(true)).count()
                                    })
                                        as f64))
                                }
                                ("focusSession", "date") => Cell::of(
                                    r["startedAt"]
                                        .as_f64()
                                        .and_then(|ms| local_days(ms, tz))
                                        .map_or(Value::Blank, |d| Value::Date(d.floor())),
                                ),
                                ("focusSession", "course") => self.relation_cell(
                                    "course",
                                    r["taskId"]
                                        .as_str()
                                        .and_then(|t| task_course.get(t))
                                        .map(String::as_str),
                                ),
                                _ => Cell::of(Value::Blank),
                            }
                        }
                    })
                    .collect();
                Some(Row { id, cells })
            })
            .collect();
        Ok(Table {
            id: k.kind.to_string(),
            name: table_name(k.kind),
            origin: Origin::Learn,
            locked: k.locked.map(String::from),
            columns,
            rows,
            kind: Some(k),
        })
    }

    /// One column's values for every row, a formula column worked out.
    pub fn column(&self, table: &Rc<Table>, idx: usize) -> Rc<Vec<Value>> {
        let key = (table.id.clone(), idx);
        if let Some(have) = self.columns.borrow().get(&key) {
            return have.clone();
        }
        let col = &table.columns[idx];
        let values: Vec<Value> = match (&col.formula, col.ty) {
            (Some(src), ColType::Formula) => {
                if !self.computing.borrow_mut().insert(key.clone()) {
                    // Asked for while it is being worked out: it reads itself.
                    let e = FError::new(
                        "#CYCLE!",
                        format!(
                            "'{}' reads itself, directly or through another formula.",
                            col.name
                        ),
                    );
                    return Rc::new(vec![Value::Error(e); table.rows.len()]);
                }
                let out = match Formula::parse(src) {
                    Err(e) => vec![Value::Error(FError::name(e.message)); table.rows.len()],
                    Ok(f) => (0..table.rows.len())
                        .map(|row| {
                            f.eval(&RowCtx {
                                sheets: self,
                                table: table.clone(),
                                row,
                            })
                        })
                        .collect(),
                };
                self.computing.borrow_mut().remove(&key);
                out
            }
            _ => table.rows.iter().map(|r| r.cells[idx].v.clone()).collect(),
        };
        let values = Rc::new(values);
        self.columns.borrow_mut().insert(key, values.clone());
        values
    }

    /// Every cell of a table, formulas worked out: `grid[row][column]`.
    pub fn grid(&self, table: &Rc<Table>) -> Vec<Vec<Cell>> {
        let formulas: Vec<Option<Rc<Vec<Value>>>> = table
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| (c.ty == ColType::Formula).then(|| self.column(table, i)))
            .collect();
        table
            .rows
            .iter()
            .enumerate()
            .map(|(r, row)| {
                row.cells
                    .iter()
                    .enumerate()
                    .map(|(c, cell)| match &formulas[c] {
                        Some(values) => Cell::of(values[r].clone()),
                        None => cell.clone(),
                    })
                    .collect()
            })
            .collect()
    }

    /// Works a formula out for the first rows of a table, for the editor to
    /// show as it is typed.
    pub fn sample(&self, table: &Rc<Table>, formula: &Formula, rows: usize) -> Vec<Value> {
        (0..table.rows.len().min(rows))
            .map(|row| {
                formula.eval(&RowCtx {
                    sheets: self,
                    table: table.clone(),
                    row,
                })
            })
            .collect()
    }
}

/// What a formula sees from one row.
struct RowCtx<'s, 'a> {
    sheets: &'s Sheets<'a>,
    table: Rc<Table>,
    row: usize,
}

impl Context for RowCtx<'_, '_> {
    fn field(&self, name: &str) -> Option<Value> {
        let idx = self.table.col_index(name)?;
        self.sheets.column(&self.table, idx).get(self.row).cloned()
    }

    fn column(&self, table: &str, name: &str) -> Result<Rc<Vec<Value>>, FError> {
        let id = self
            .sheets
            .resolve(table)
            .ok_or_else(|| FError::reference(format!("There is no table called '{table}'.")))?;
        let t = if id == self.table.id {
            self.table.clone()
        } else {
            self.sheets
                .table(&id)
                .map_err(|e| FError::reference(e.message))?
        };
        let idx = t.col_index(name).ok_or_else(|| {
            FError::reference(format!("{} has no column called '{name}'.", t.name))
        })?;
        Ok(self.sheets.column(&t, idx))
    }

    fn today(&self) -> f64 {
        self.sheets.today()
    }

    fn now(&self) -> f64 {
        local_days(self.sheets.clock.now_ms, &self.sheets.clock.zone).unwrap_or(0.0)
    }

    fn row_number(&self) -> usize {
        self.row + 1
    }
}

/// A number as JSON the way the records keep it: whole numbers with no
/// fraction.
pub(crate) fn value_json_number(n: f64) -> Json {
    value_json(&Value::Number(n))
}
