//! The prompt box's instant results (docs/ASK.md): what shows as you type,
//! before Claude is asked anything. It reads the library's own full-text
//! index (`wi-store`'s `docs_fts`), which every Learn record and every row of
//! a person's tables is in, so a search is one indexed lookup and needs no
//! network and no Claude.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::db;
use crate::heat_cmd::core_error;
use crate::{CoreError, Inner};

/// The kinds a search can open, best first when two match equally, with the
/// word shown beside a result and the field that names it.
const KINDS: &[(&str, &str, &str)] = &[
    ("task", "Task", "title"),
    ("course", "Course", "code"),
    ("note", "Note", "title"),
    ("dailyNote", "Daily note", "date"),
    ("habit", "Habit", "title"),
    ("mailThread", "Mail", "subject"),
    ("grade", "Grade", "title"),
    ("project", "Project", "title"),
    ("milestone", "Milestone", "title"),
    ("capture", "Inbox", "text"),
    ("space", "Space", "name"),
    (db::kind::ROW, "Row", ""),
    (db::kind::TABLE, "Table", "name"),
    (db::kind::VIEW, "View", "name"),
];

fn first_line(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .chars()
        .take(120)
        .collect()
}

/// `ask.search {q, limit?, kinds?}`: records whose text holds every word
/// typed, as `{kind, id, title, hint, table}`. `table` is the Database
/// table the record is a row of.
pub(crate) fn search(
    i: &Inner,
    q: &str,
    limit: usize,
    kinds: &[String],
) -> Result<Value, CoreError> {
    let q = q.trim();
    if q.is_empty() {
        return Ok(json!({"results": []}));
    }
    let limit = limit.clamp(1, 100);
    let clock = i.clock();
    let store = i.store();
    // More than asked for: some kinds are not shown, and the best are picked after.
    let docs = store.search_docs(q, limit * 4 + 40)?;
    let all = |k: &str| wi_heat_store::all(&store, k).map_err(core_error);
    let names = |k: &str, field: &str| -> Result<HashMap<String, String>, CoreError> {
        Ok(all(k)?
            .iter()
            .filter_map(|r| {
                Some((
                    r["id"].as_str()?.to_string(),
                    r[field].as_str()?.to_string(),
                ))
            })
            .collect())
    };
    let courses = names("course", "code")?;
    let tables = names(db::kind::TABLE, "name")?;
    let first_columns: HashMap<String, String> = {
        // A person's row is called by its first column's cell.
        let mut cols = all(db::kind::COLUMN)?;
        cols.sort_by(|a, b| {
            a["order"]
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&b["order"].as_f64().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut map = HashMap::new();
        for c in cols {
            if let (Some(t), Some(id)) = (c["table"].as_str(), c["id"].as_str()) {
                map.entry(t.to_string()).or_insert_with(|| id.to_string());
            }
        }
        map
    };
    let lower = q.to_lowercase();
    let mut found: Vec<(usize, usize, Value)> = Vec::new();
    for (n, d) in docs.iter().enumerate() {
        let Some(rank) = KINDS.iter().position(|(k, _, _)| *k == d.kind) else {
            continue;
        };
        if !kinds.is_empty() && !kinds.iter().any(|k| k.eq_ignore_ascii_case(&d.kind)) {
            continue;
        }
        let (kind, word, field) = KINDS[rank];
        let r = &d.json;
        let text = |k: &str| r[k].as_str().unwrap_or("");
        let (title, hint, table) = match kind {
            "task" => {
                let mut hint = vec![];
                if let Some(code) = r["courseId"].as_str().and_then(|c| courses.get(c)) {
                    hint.push(code.clone());
                }
                if r["done"] == true {
                    hint.push("Done".to_string());
                } else if let Some(due) = r["due"].as_f64() {
                    hint.push(format!("Due {}", clock.date_of(due)));
                }
                (
                    first_line(text("title")),
                    hint.join(" · "),
                    "task".to_string(),
                )
            }
            "course" => (
                text("code").to_string(),
                text("name").to_string(),
                "course".to_string(),
            ),
            "note" => {
                let title = first_line(text("title"));
                let body = first_line(text("markdown"));
                if title.is_empty() {
                    (body, word.to_string(), "note".to_string())
                } else {
                    (title, body, "note".to_string())
                }
            }
            "dailyNote" => (
                format!("Daily note, {}", text("date")),
                first_line(text("markdown")),
                "dailyNote".to_string(),
            ),
            "mailThread" => (
                first_line(text("subject")),
                text("from").to_string(),
                "mailThread".to_string(),
            ),
            "grade" => {
                let course = r["courseId"]
                    .as_str()
                    .and_then(|c| courses.get(c))
                    .cloned()
                    .unwrap_or_default();
                let score = match (r["score"].as_f64(), r["outOf"].as_f64()) {
                    (Some(s), Some(o)) => format!("{s} of {o}"),
                    _ => "Pending".to_string(),
                };
                (
                    first_line(text("title")),
                    [course, score]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · "),
                    "grade".to_string(),
                )
            }
            k if k == db::kind::ROW => {
                let table = text("tableId").to_string();
                let title = first_columns
                    .get(&table)
                    .and_then(|c| r["cells"].get(c))
                    .map(|v| match v {
                        Value::String(s) => first_line(s),
                        other => other.to_string(),
                    })
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| first_line(&d.text));
                (
                    title,
                    tables
                        .get(&table)
                        .cloned()
                        .unwrap_or_else(|| "Table".to_string()),
                    table,
                )
            }
            k if k == db::kind::TABLE => {
                (text("name").to_string(), "Table".to_string(), d.key.clone())
            }
            k if k == db::kind::VIEW => {
                let table = text("table").to_string();
                (text("name").to_string(), "Saved view".to_string(), table)
            }
            _ => (first_line(text(field)), word.to_string(), kind.to_string()),
        };
        if title.is_empty() {
            continue;
        }
        // A title that starts with what was typed comes first, then one that holds it.
        let t = title.to_lowercase();
        let closeness = if t.starts_with(&lower) {
            0
        } else if t.contains(&lower) {
            1
        } else {
            2
        };
        found.push((
            closeness * 100 + rank,
            n,
            json!({"kind": kind, "id": d.key, "title": title, "hint": hint, "word": word, "table": table}),
        ));
    }
    found.sort_by_key(|(score, n, _)| (*score, *n));
    let results: Vec<Value> = found.into_iter().take(limit).map(|(_, _, v)| v).collect();
    Ok(json!({"results": results}))
}
