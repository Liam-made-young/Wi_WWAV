//! The tools Claude has in the prompt box (docs/ASK.md). Each is a command
//! of the core, the one the views call, so anything a person can click
//! Claude can call, by the same rules.
//!
//! - A tool that **reads** runs at once and answers.
//! - A tool that **changes** something only stages the change: it is checked
//!   as far as it can be without being made (the row is there, the column
//!   can be typed in, the value reads), described in a line for the
//!   preview, and kept on the run. Nothing is written until the person
//!   applies the run (`ask.apply`), and then all of it is one undo step.
//! - A tool whose effect **leaves this Mac** (making a record public) is
//!   staged apart, and each one waits for its own click, every time.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use serde_json::{json, Map, Value};

use crate::args::Args;
use crate::bus::lock;
use crate::{ask_index, db, heat_cmd, wiki, CoreError, Inner};

/// One change waiting for the person's click.
#[derive(Clone, Debug)]
pub(crate) struct Staged {
    /// The core command that makes it.
    pub cmd: String,
    pub args: Value,
    /// What the preview says: "Kanji quiz: Due 2026-10-09 23:59 → 2026-10-08 23:59".
    pub line: String,
    /// How the summary counts it: ("task", "tasks", "moved"), once per row.
    pub counts: Vec<(String, String, String)>,
    /// The stand-in ids (`new:1`) of the rows this makes, in order.
    pub makes: Vec<String>,
}

/// One question to Claude, while it runs and until its changes are applied
/// or let go.
pub(crate) struct Run {
    pub id: String,
    /// What Claude's calls must carry to be this run's.
    pub token: String,
    pub staged: Mutex<Vec<Staged>>,
    pub outward: Mutex<Vec<Staged>>,
    /// What Claude asked the window to show: an article, a row, a tab.
    pub opens: Mutex<Vec<Value>>,
    /// Articles Claude read, for the links under a general-knowledge answer.
    pub wiki: Mutex<Vec<String>>,
    /// Every tool called, in order.
    pub calls: Mutex<Vec<String>>,
    pub cancelled: AtomicBool,
    next_new: AtomicUsize,
}

impl Run {
    pub fn new(id: String, token: String) -> Run {
        Run {
            id,
            token,
            staged: Mutex::default(),
            outward: Mutex::default(),
            opens: Mutex::default(),
            wiki: Mutex::default(),
            calls: Mutex::default(),
            cancelled: AtomicBool::new(false),
            next_new: AtomicUsize::new(1),
        }
    }

    fn new_id(&self) -> String {
        format!("new:{}", self.next_new.fetch_add(1, Ordering::SeqCst))
    }

    fn stage(&self, s: Staged) {
        lock(&self.staged).push(s);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effect {
    Reads,
    Changes,
    LeavesThisMac,
}

pub(crate) struct Tool {
    pub name: &'static str,
    pub effect: Effect,
    /// What the panel shows while it runs.
    pub doing: &'static str,
    pub description: &'static str,
    pub schema: fn() -> Value,
}

const OUTWARD_NOTE: &str = "Not done. This leaves this Mac, so the person is asked about it on its own and decides. Tell them it is waiting for their answer.";

const STAGED_NOTE: &str = "Staged, not done. Nothing has changed yet: the person sees a preview and applies it with one click.";

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
}

fn filters_schema() -> Value {
    json!({
        "type": "array",
        "description": "Rows must pass every filter (or any, with match: \"any\").",
        "items": {
            "type": "object",
            "properties": {
                "column": {"type": "string", "description": "A column's name, as the table lists it."},
                "op": {"type": "string", "enum": ["eq", "ne", "contains", "not_contains", "starts", "ends", "gt", "ge", "lt", "le", "blank", "not_blank", "in"]},
                "value": {"description": "What to compare with. A date is YYYY-MM-DD (it matches any time that day); true or false for a checkbox; a list for \"in\"."}
            },
            "required": ["column", "op"],
            "additionalProperties": false
        }
    })
}

fn sorts_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": {"column": {"type": "string"}, "dir": {"type": "string", "enum": ["asc", "desc"]}},
            "required": ["column"],
            "additionalProperties": false
        }
    })
}

fn values_schema(what: &str) -> Value {
    json!({
        "type": "object",
        "description": format!("{what} Keys are column names as the table lists them. A date is YYYY-MM-DD, or YYYY-MM-DD HH:MM for a time. A link to another table takes that row's name (a course's code, a task's title) or its id. An empty string clears a cell."),
        "additionalProperties": true
    })
}

pub(crate) const TOOLS: &[Tool] = &[
    Tool {
        name: "search",
        effect: Effect::Reads,
        doing: "Searching Learn",
        description: "Find records anywhere in Learn by words they hold: tasks, courses, notes, habits, mail notes, grades, projects, and rows of the person's own tables. Answers {kind, id, title, hint, table}. Use this when you don't know which table something is in.",
        schema: || object(json!({
            "query": {"type": "string", "description": "Words the record holds. Every word must be there; each matches as a prefix."},
            "limit": {"type": "integer", "minimum": 1, "maximum": 50}
        }), &["query"]),
    },
    Tool {
        name: "list_rows",
        effect: Effect::Reads,
        doing: "Reading a table",
        description: "Read rows of one table, filtered and sorted. This is how to read tasks, courses, grades, habits, focus sessions, mail notes, notes, projects, milestones and the person's own tables. Answers {total, rows: [{id, <column name>: value}]} with empty cells left out. Ask only for the columns you need.",
        schema: || object(json!({
            "table": {"type": "string", "description": "The table's name, such as Tasks."},
            "filters": filters_schema(),
            "match": {"type": "string", "enum": ["all", "any"]},
            "search": {"type": "string", "description": "Words any cell of the row holds."},
            "sorts": sorts_schema(),
            "columns": {"type": "array", "items": {"type": "string"}, "description": "The columns to answer with. Left out: all of them."},
            "view": {"type": "string", "description": "A saved view's name, instead of filters and sorts."},
            "limit": {"type": "integer", "minimum": 1, "maximum": 200, "description": "Default 50."}
        }), &["table"]),
    },
    Tool {
        name: "get_row",
        effect: Effect::Reads,
        doing: "Reading a record",
        description: "Read one record whole: every column of one row, a note's full text, and for a mail note the saved text of its messages.",
        schema: || object(json!({
            "table": {"type": "string"},
            "id": {"type": "string"}
        }), &["table", "id"]),
    },
    Tool {
        name: "today",
        effect: Effect::Reads,
        doing: "Reading the day",
        description: "A day as Today shows it: the blocks planned, calendar events, what is due, the hottest tasks not yet planned, habits, the inbox count and the focus timer.",
        schema: || object(json!({
            "date": {"type": "string", "description": "YYYY-MM-DD. Left out: today."}
        }), &[]),
    },
    Tool {
        name: "summarize",
        effect: Effect::Reads,
        doing: "Totalling a table",
        description: "Group a table's rows by one or two columns and total others: minutes per course, tasks per type, average score per category. Answers each group with its count and totals.",
        schema: || object(json!({
            "table": {"type": "string"},
            "group_by": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 2},
            "values": {"type": "array", "items": {
                "type": "object",
                "properties": {
                    "column": {"type": "string"},
                    "total": {"type": "string", "enum": ["sum", "average", "count", "min", "max", "median", "unique", "filled", "checked"]}
                },
                "required": ["column", "total"],
                "additionalProperties": false
            }},
            "filters": filters_schema()
        }), &["table", "group_by"]),
    },
    Tool {
        name: "grade_needed",
        effect: Effect::Reads,
        doing: "Working out a grade",
        description: "What it would take to finish a course with a letter grade: the percentage needed on what is left.",
        schema: || object(json!({
            "course": {"type": "string", "description": "The course's code or id."},
            "letter": {"type": "string", "description": "Such as B or A-."}
        }), &["course", "letter"]),
    },
    Tool {
        name: "wiki_search",
        effect: Effect::Reads,
        doing: "Searching Wikipedia",
        description: "Search Wikipedia for articles. Answers titles with a snippet of each. Use it for general-knowledge questions, to find the article to link.",
        schema: || object(json!({
            "query": {"type": "string"}
        }), &["query"]),
    },
    Tool {
        name: "wiki_get_article",
        effect: Effect::Reads,
        doing: "Reading Wikipedia",
        description: "Read a Wikipedia article as plain text, whole or one section. Use it when the person is reading an article in the Wiki tab and asks about it, or when a search snippet isn't enough.",
        schema: || object(json!({
            "title": {"type": "string"},
            "section": {"type": "string", "description": "A section's title. An empty string is the lead, before the first heading. Left out: the whole article, cut to a length."}
        }), &["title"]),
    },
    Tool {
        name: "open",
        effect: Effect::Reads,
        doing: "Opening",
        description: "Show something in the window when the person asks to see it: a Wikipedia article in the Wiki tab, a row, a table, or a tab of Learn. Changes no data.",
        schema: || object(json!({
            "what": {"type": "string", "enum": ["wiki", "row", "table", "tab"]},
            "title": {"type": "string", "description": "For wiki: the article's title."},
            "table": {"type": "string", "description": "For row and table."},
            "id": {"type": "string", "description": "For row."},
            "tab": {"type": "string", "enum": ["today", "tasks", "calendar", "grades", "habits", "mail", "database", "wiki"]}
        }), &["what"]),
    },
    Tool {
        name: "update_rows",
        effect: Effect::Changes,
        doing: "Staging changes",
        description: "Change cells of rows that exist: move a task's due date, rename it, set its estimate or course, enter a grade's score, edit a note, check a task done. One call takes many rows. Staged: the person previews and applies.",
        schema: || object(json!({
            "table": {"type": "string"},
            "changes": {"type": "array", "minItems": 1, "maxItems": 500, "items": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "The row's id, from list_rows or search."},
                    "set": values_schema("The cells to change and their new values.")
                },
                "required": ["id", "set"],
                "additionalProperties": false
            }}
        }), &["table", "changes"]),
    },
    Tool {
        name: "create_rows",
        effect: Effect::Changes,
        doing: "Staging new rows",
        description: "Add rows: new tasks, notes, projects, milestones, habits, grades, courses, or rows of the person's own tables. Flashcards and study notes are rows of Notes (Title, and Markdown for the text). Answers a stand-in id for each row (new:1) that later calls in this conversation can use. Staged: the person previews and applies.",
        schema: || object(json!({
            "table": {"type": "string"},
            "rows": {"type": "array", "minItems": 1, "maxItems": 500, "items": values_schema("The new row's cells.")}
        }), &["table", "rows"]),
    },
    Tool {
        name: "delete_rows",
        effect: Effect::Changes,
        doing: "Staging a delete",
        description: "Delete rows. Use only when the person asks to delete or remove. Staged: the person previews and applies, and ⌘Z brings them back.",
        schema: || object(json!({
            "table": {"type": "string"},
            "ids": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 500}
        }), &["table", "ids"]),
    },
    Tool {
        name: "complete_tasks",
        effect: Effect::Changes,
        doing: "Staging tasks done",
        description: "Mark tasks done, or not done again. Staged: the person previews and applies.",
        schema: || object(json!({
            "ids": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 200},
            "done": {"type": "boolean", "description": "Default true."}
        }), &["ids"]),
    },
    Tool {
        name: "schedule_block",
        effect: Effect::Changes,
        doing: "Staging a block",
        description: "Put a task on the day's time column: a block of time on a date. Starts snap to 15 minutes, between 7 AM and midnight. Staged: the person previews and applies.",
        schema: || object(json!({
            "task": {"type": "string", "description": "The task's id."},
            "date": {"type": "string", "description": "YYYY-MM-DD"},
            "start": {"type": "string", "description": "HH:MM, 24-hour, local time."},
            "minutes": {"type": "integer", "minimum": 15, "maximum": 600}
        }), &["task", "date", "start", "minutes"]),
    },
    Tool {
        name: "plan_day",
        effect: Effect::Changes,
        doing: "Planning the day",
        description: "Run Learn's own Plan my day for a date: it drafts blocks for the hottest tasks in the free time left, by its written rule. Answers the drafts with each one's reason. The drafts show on Today at once as drafts; accepting them is staged for the person to apply.",
        schema: || object(json!({
            "date": {"type": "string", "description": "YYYY-MM-DD. Left out: today."}
        }), &[]),
    },
    Tool {
        name: "tick_habit",
        effect: Effect::Changes,
        doing: "Staging a habit",
        description: "Tick a habit done for a day, or untick it. Staged: the person previews and applies.",
        schema: || object(json!({
            "habit": {"type": "string", "description": "The habit's id."},
            "date": {"type": "string", "description": "YYYY-MM-DD. Left out: today."},
            "done": {"type": "boolean", "description": "Default true."}
        }), &["habit"]),
    },
    Tool {
        name: "capture",
        effect: Effect::Changes,
        doing: "Staging a capture",
        description: "Add a line to the inbox for the person to triage later. Staged: the person previews and applies.",
        schema: || object(json!({
            "text": {"type": "string"}
        }), &["text"]),
    },
    Tool {
        name: "add_formula_column",
        effect: Effect::Changes,
        doing: "Staging a formula",
        description: "Add a computed column to a table in the Database tab, from a spreadsheet formula you write. [Name] is this row's value in a column; Table[Name] is a whole column of any table, for SUM, COUNTIF, SUMIFS and LOOKUP. Dates subtract to days; TODAY() and STARTOFWEEK(TODAY()) are dates. Example, on Courses: ROUND(SUMIFS(Focus sessions[Minutes], Focus sessions[Course], [Code], Focus sessions[Date], \">=\" & STARTOFWEEK(TODAY())) / 60, 2). The formula is checked now; a mistake comes back as an error to fix. Staged: the person previews and applies.",
        schema: || object(json!({
            "table": {"type": "string"},
            "name": {"type": "string", "description": "The new column's name."},
            "formula": {"type": "string"}
        }), &["table", "name", "formula"]),
    },
    Tool {
        name: "save_view",
        effect: Effect::Changes,
        doing: "Staging a view",
        description: "Save a named view of a table in the Database tab: filters, sorts, a grouping, and optionally a pivot or a chart. Staged: the person previews and applies.",
        schema: || object(json!({
            "table": {"type": "string"},
            "name": {"type": "string"},
            "filters": filters_schema(),
            "match": {"type": "string", "enum": ["all", "any"]},
            "sorts": sorts_schema(),
            "group": {"type": "string", "description": "A column to group the rows by."},
            "hidden": {"type": "array", "items": {"type": "string"}, "description": "Columns to hide."},
            "pivot": {"type": "object", "properties": {
                "rows": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 2},
                "values": {"type": "array", "items": {"type": "object", "properties": {
                    "column": {"type": "string"},
                    "agg": {"type": "string", "enum": ["sum", "average", "count", "min", "max", "median", "unique"]}
                }, "required": ["column", "agg"], "additionalProperties": false}}
            }, "required": ["rows"], "additionalProperties": false},
            "chart": {"type": "object", "properties": {
                "type": {"type": "string", "enum": ["bar", "line", "scatter"]},
                "x": {"type": "string"},
                "y": {"type": "array", "items": {"type": "string"}},
                "agg": {"type": "string", "enum": ["sum", "average", "count", "min", "max"], "description": "Total y per x. Leave out for a point per row."}
            }, "required": ["type", "x", "y"], "additionalProperties": false}
        }), &["table", "name"]),
    },
    Tool {
        name: "create_table",
        effect: Effect::Changes,
        doing: "Staging a table",
        description: "Make a new table of the person's own in the Database tab, with its columns and optionally its first rows. Staged: the person previews and applies.",
        schema: || object(json!({
            "name": {"type": "string"},
            "columns": {"type": "array", "minItems": 1, "maxItems": 40, "items": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "type": {"type": "string", "enum": ["text", "number", "checkbox", "date", "formula"]},
                    "formula": {"type": "string"}
                },
                "required": ["name", "type"],
                "additionalProperties": false
            }},
            "rows": {"type": "array", "maxItems": 500, "items": values_schema("One row's cells.")}
        }), &["name", "columns"]),
    },
    Tool {
        name: "mail_send",
        effect: Effect::LeavesThisMac,
        doing: "Asking to send mail",
        description: "Write a new mail for the person to send from Learn's Mail. It leaves this Mac, so it is never part of the one-click apply: the person reads the whole mail and is asked about it on its own, every time. Write the mail exactly as it should be sent, in their voice, with no placeholders.",
        schema: || object(json!({
            "to": {"type": "string", "description": "The address, or addresses separated by commas."},
            "cc": {"type": "string"},
            "subject": {"type": "string"},
            "body": {"type": "string", "description": "Plain text."},
            "account": {"type": "string", "description": "Which of their addresses it goes from. Left out: their first."}
        }), &["to", "subject", "body"]),
    },
    Tool {
        name: "mail_reply",
        effect: Effect::LeavesThisMac,
        doing: "Asking to send a reply",
        description: "Write a reply on a mail thread that is in Learn's Mail. It leaves this Mac, so the person reads the whole reply and is asked about it on its own, every time. Read the thread first (get_row on Mail) so the reply answers what was said.",
        schema: || object(json!({
            "thread": {"type": "string", "description": "The Mail row's id."},
            "body": {"type": "string", "description": "Plain text."}
        }), &["thread", "body"]),
    },
    Tool {
        name: "mail_file",
        effect: Effect::LeavesThisMac,
        doing: "Asking to change a thread",
        description: "Archive a mail thread, bring it back to the inbox, or mark it read or unread. This changes their mailbox, which is not on this Mac, so the person is asked about each one, every time.",
        schema: || object(json!({
            "thread": {"type": "string", "description": "The Mail row's id."},
            "action": {"type": "string", "enum": ["archive", "unarchive", "mark_read", "mark_unread"]}
        }), &["thread", "action"]),
    },
    Tool {
        name: "make_public",
        effect: Effect::LeavesThisMac,
        doing: "Asking to make something public",
        description: "Switch a record onto, or off, the person's public Learn view, where anyone who opens their sun can see it. This leaves this Mac, so it is never part of the one-click apply: the person is asked about each one, every time. Use only when they ask for it in so many words.",
        schema: || object(json!({
            "table": {"type": "string"},
            "id": {"type": "string"},
            "public": {"type": "boolean"}
        }), &["table", "id", "public"]),
    },
];

/// What a tool from another part of Learn asks the box to do: a command of
/// the core with its arguments, and the line the preview shows for it.
pub(crate) struct Staging {
    pub cmd: String,
    pub args: Value,
    /// "New commitment: JPN 101, Mon Wed Fri 10:00". Not read for a tool that only reads.
    pub line: String,
    /// How the summary counts it: ("commitment", "commitments", "added").
    /// None counts it as a change.
    pub counts: Option<(String, String, String)>,
}

/// Tools another part of Learn brings to the box. Its `tools` say what each
/// is and whether it reads, changes or leaves this Mac; its `stage` turns a
/// call into the core command that does it. The box does the rest by the
/// tool's effect, exactly as for its own: a read is run at once and
/// answered, a change is kept for the preview and made by `ask.apply`, and
/// what leaves this Mac waits for its own yes.
pub(crate) struct Extra {
    pub tools: &'static [Tool],
    pub stage: fn(&Inner, &str, &Value) -> Result<Staging, String>,
}

/// Mount a list here, one line each:
/// `&Extra { tools: learn_tools::TOOLS, stage: learn_tools::stage }`.
/// A name here must not be one of [`TOOLS`]'s, and can't hold a dot.
pub(crate) const EXTRA: &[&Extra] = &[&Extra {
    tools: crate::learn_tools::FOR_ASK,
    stage: crate::learn_tools::stage_for_ask,
}];

/// Every tool the box offers: its own, then what was mounted.
pub(crate) fn all() -> impl Iterator<Item = &'static Tool> {
    TOOLS
        .iter()
        .chain(EXTRA.iter().flat_map(|e| e.tools.iter()))
}

pub(crate) fn tool(name: &str) -> Option<&'static Tool> {
    all().find(|t| t.name == name)
}

/// Runs a mounted tool by its effect.
fn mounted(i: &Inner, run: &Run, extra: &Extra, t: &Tool, args: &Value) -> Result<Value, String> {
    let staging = (extra.stage)(i, t.name, args)?;
    match t.effect {
        Effect::Reads => invoke(i, &staging.cmd, &staging.args).map_err(said),
        Effect::Changes => {
            let line = staging.line.clone();
            run.stage(Staged {
                cmd: staging.cmd,
                args: staging.args,
                line: staging.line,
                counts: vec![staging
                    .counts
                    .unwrap_or_else(|| ("change".into(), "changes".into(), "made".into()))],
                makes: Vec::new(),
            });
            Ok(json!({"staged": 1, "preview": [line], "note": STAGED_NOTE}))
        }
        Effect::LeavesThisMac => {
            let line = staging.line.clone();
            lock(&run.outward).push(Staged {
                cmd: staging.cmd,
                args: staging.args,
                line: staging.line,
                counts: Vec::new(),
                makes: Vec::new(),
            });
            Ok(json!({"asked": true, "preview": [line], "note": OUTWARD_NOTE}))
        }
    }
}

/// The tools as MCP lists them.
pub(crate) fn list() -> Value {
    Value::Array(
        all()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": (t.schema)(),
                    "annotations": {"readOnlyHint": t.effect == Effect::Reads},
                })
            })
            .collect(),
    )
}

/// Runs a core command by its name: the ones the tools use.
pub(crate) fn invoke(i: &Inner, cmd: &str, args: &Value) -> Result<Value, CoreError> {
    let a = Args::new(cmd, args);
    if cmd.starts_with("heat.") {
        heat_cmd::invoke(i, cmd, &a)
    } else if cmd.starts_with("db.") {
        db::invoke(i, cmd, &a)
    } else if cmd.starts_with("wiki.") {
        wiki::invoke(i, cmd, &a)
    } else if cmd == "ask.habit.tick" {
        tick_habit(i, args)
    } else {
        Err(CoreError::new(
            "unknown_command",
            format!("There is no command called '{cmd}'."),
        ))
    }
}

/// A habit's day ticked or unticked, as Habits does it: the habit's log
/// with that day set, written through `heat.patch`.
fn tick_habit(i: &Inner, args: &Value) -> Result<Value, CoreError> {
    let (id, date, done) = (
        args["habit"].as_str().unwrap_or_default(),
        args["date"].as_str().unwrap_or_default(),
        args["done"].as_bool().unwrap_or(true),
    );
    let habit = wi_heat_store::one(&i.store(), "habit", id)
        .map_err(heat_cmd::core_error)?
        .ok_or_else(|| CoreError::new("refused", "No habit has that id."))?;
    let mut log = habit["log"].as_object().cloned().unwrap_or_default();
    if done {
        log.insert(date.to_string(), json!(true));
    } else {
        log.remove(date);
    }
    invoke(
        i,
        "heat.patch",
        &json!({"kind": "habit", "id": id, "set": {"log": log}}),
    )
}

fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args[key]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("This tool needs {key}."))
}

fn said(e: CoreError) -> String {
    e.message
}

/// "Tasks" as one of them: "task". A person's own table has no word of its
/// own, so a row of it is "row in Reading list".
fn nouns(table_id: &str, table_name: &str) -> (String, String) {
    let known = [
        ("task", "task", "tasks"),
        ("course", "course", "courses"),
        ("grade", "grade", "grades"),
        ("habit", "habit", "habits"),
        ("note", "note", "notes"),
        ("dailyNote", "daily note", "daily notes"),
        ("project", "project", "projects"),
        ("milestone", "milestone", "milestones"),
        ("space", "space", "spaces"),
        ("term", "term", "terms"),
        ("capture", "capture", "captures"),
        ("timeBlock", "block", "blocks"),
    ];
    match known.iter().find(|(id, _, _)| *id == table_id) {
        Some((_, one, many)) => (one.to_string(), many.to_string()),
        None => (
            format!("row in {table_name}"),
            format!("rows in {table_name}"),
        ),
    }
}

fn is_ymd(s: &str) -> bool {
    s.len() == 10 && wi_formula::parse_date(s).is_some()
}

/// The spec of a view from a tool's arguments.
fn spec_from(args: &Value) -> Value {
    let mut spec = Map::new();
    for key in ["filters", "match", "search", "sorts", "group", "hidden"] {
        if !args[key].is_null() {
            spec.insert(key.to_string(), args[key].clone());
        }
    }
    Value::Object(spec)
}

/// Runs one tool for a run. The error is a sentence for Claude to act on.
pub(crate) fn call(i: &Inner, run: &Run, name: &str, args: &Value) -> Result<Value, String> {
    lock(&run.calls).push(name.to_string());
    match name {
        // ----- reading -----
        "search" => {
            let limit = args["limit"].as_u64().unwrap_or(12) as usize;
            ask_index::search(i, text(args, "query")?, limit, &[]).map_err(said)
        }
        "list_rows" => {
            let table = text(args, "table")?;
            let limit = (args["limit"].as_u64().unwrap_or(50) as usize).clamp(1, 200);
            let mut q = json!({"table": table, "limit": limit});
            match args["view"].as_str() {
                Some(view) => q["view"] = json!(view),
                None => q["spec"] = spec_from(args),
            }
            let result = invoke(i, "db.query", &q).map_err(said)?;
            let columns: Option<Vec<String>> = args["columns"].as_array().map(|c| {
                c.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect()
            });
            let rows = db::rows_as_objects(&result, columns.as_deref());
            Ok(json!({
                "table": result["table"]["name"],
                "total": result["total"],
                "shown": rows.len(),
                "rows": rows,
            }))
        }
        "get_row" => {
            let (table, id) = (text(args, "table")?, text(args, "id")?);
            let q = json!({"table": table, "spec": {"rows": [id]}});
            let result = invoke(i, "db.query", &q).map_err(said)?;
            let all: Vec<String> = result["columns"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["name"].as_str().map(String::from))
                .collect();
            let mut row = db::rows_as_objects(&result, Some(&all))
                .into_iter()
                .next()
                .ok_or_else(|| format!("{table} has no row with the id '{id}'."))?;
            // A mail note's own words, when they were saved on this Mac.
            if result["table"]["id"] == "mailThread" {
                let record = invoke(
                    i,
                    "db.query",
                    &json!({"table": "mailThread", "spec": {"rows": [id]}, "limit": 1}),
                )
                .ok()
                .and_then(|r| {
                    db::rows_as_objects(&r, Some(&["Gmail thread".to_string()]))
                        .into_iter()
                        .next()
                });
                if let Some(thread) =
                    record.and_then(|r| r["Gmail thread"].as_str().map(String::from))
                {
                    if let Ok(saved) = invoke(i, "heat.mail.text", &json!({"threadId": thread})) {
                        if saved["messages"].is_array() {
                            row["messages"] = saved["messages"].clone();
                        }
                    }
                }
            }
            Ok(row)
        }
        "today" => today(i, args["date"].as_str()),
        "summarize" => {
            let table = text(args, "table")?;
            let values: Vec<Value> = args["values"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|v| json!({"column": v["column"], "agg": v["total"]}))
                .collect();
            let q = json!({
                "table": table, "rows": args["group_by"], "values": values, "spec": spec_from(args),
            });
            invoke(i, "db.pivot", &q).map_err(said)
        }
        "grade_needed" => {
            let (course, letter) = (text(args, "course")?, text(args, "letter")?);
            let found = invoke(i, "db.query", &json!({"table": "course", "spec": {"match": "any", "filters": [
                {"column": "id", "op": "eq", "value": course}, {"column": "code", "op": "eq", "value": course},
            ]}}))
            .map_err(said)?;
            let id = found["rows"][0]["id"]
                .as_str()
                .ok_or_else(|| format!("No course is called '{course}'."))?;
            invoke(
                i,
                "heat.whatItWouldTake",
                &json!({"courseId": id, "letter": letter}),
            )
            .map_err(said)
        }
        "wiki_search" => invoke(
            i,
            "wiki.search",
            &json!({"q": text(args, "query")?, "limit": 6}),
        )
        .map_err(said),
        "wiki_get_article" => {
            let title = text(args, "title")?;
            let mut q = json!({"title": title, "maxChars": 14_000});
            if let Some(section) = args["section"].as_str() {
                q["section"] = json!(section);
                q["maxChars"] = json!(20_000);
            }
            let got = invoke(i, "wiki.text", &q).map_err(said)?;
            if let Some(t) = got["title"].as_str() {
                let mut read = lock(&run.wiki);
                if !read.iter().any(|x| x == t) {
                    read.push(t.to_string());
                }
            }
            Ok(got)
        }
        "open" => {
            let what = text(args, "what")?;
            let open = match what {
                "wiki" => json!({"what": "wiki", "title": text(args, "title")?}),
                "row" => {
                    json!({"what": "row", "table": text(args, "table")?, "id": text(args, "id")?})
                }
                "table" => json!({"what": "table", "table": text(args, "table")?}),
                "tab" => json!({"what": "tab", "tab": text(args, "tab")?}),
                other => return Err(format!("There is nothing called '{other}' to open.")),
            };
            lock(&run.opens).push(open);
            Ok(json!({"ok": true, "note": "It opens when your answer is shown."}))
        }

        // ----- changing: staged -----
        "update_rows" => {
            let table = text(args, "table")?;
            let changes = args["changes"]
                .as_array()
                .ok_or("This tool needs changes, a list.")?;
            let mut edits: Vec<(String, String, Value)> = Vec::new();
            for c in changes {
                let id = c["id"].as_str().ok_or("Each change needs the row's id.")?;
                let set = c["set"]
                    .as_object()
                    .filter(|s| !s.is_empty())
                    .ok_or("Each change needs set, the cells to change.")?;
                for (column, value) in set {
                    edits.push((id.to_string(), column.clone(), value.clone()));
                }
            }
            let (table_id, table_name, lines) =
                db::preview_edits(i, table, &edits).map_err(said)?;
            let (one, many) = nouns(&table_id, &table_name);
            // One staged change per row, so the preview has a line for each.
            let mut previews = Vec::new();
            let mut at = 0;
            for c in changes {
                let id = c["id"].as_str().unwrap_or_default();
                let n = c["set"].as_object().map_or(0, Map::len);
                let mine: &[db::EditLine] = &lines[at..at + n];
                at += n;
                let label = mine.first().map(|l| l.label.clone()).unwrap_or_default();
                let parts: Vec<String> = mine
                    .iter()
                    .map(|l| {
                        if l.before.is_empty() {
                            format!(
                                "{} set to {}",
                                l.column,
                                if l.after.is_empty() {
                                    "empty"
                                } else {
                                    &l.after
                                }
                            )
                        } else if l.after.is_empty() {
                            format!("{} cleared (was {})", l.column, l.before)
                        } else {
                            format!("{} {} → {}", l.column, l.before, l.after)
                        }
                    })
                    .collect();
                let moved = table_id == "task"
                    && mine
                        .iter()
                        .any(|l| l.column_id == "due" || l.column_id == "scheduledDate");
                let only_done = mine.len() == 1 && mine[0].column_id == "done";
                let verb = if moved {
                    "moved"
                } else if only_done && mine[0].after.eq_ignore_ascii_case("true") {
                    "completed"
                } else if only_done {
                    "reopened"
                } else {
                    "updated"
                };
                let line = format!("{label}: {}", parts.join("; "));
                previews.push(line.clone());
                let edits: Vec<Value> = c["set"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(column, value)| json!({"row": id, "column": column, "value": value}))
                    .collect();
                run.stage(Staged {
                    cmd: "db.cells.set".into(),
                    args: json!({"table": table_id, "edits": edits}),
                    line,
                    counts: vec![(one.clone(), many.clone(), verb.to_string())],
                    makes: Vec::new(),
                });
            }
            Ok(json!({"staged": previews.len(), "preview": previews, "note": STAGED_NOTE}))
        }
        "create_rows" => {
            let table = text(args, "table")?;
            let rows = args["rows"]
                .as_array()
                .filter(|r| !r.is_empty())
                .ok_or("This tool needs rows, a list.")?;
            let mut ids = Vec::new();
            let mut previews = Vec::new();
            let mut staged = Vec::new();
            for row in rows {
                let cells = row
                    .as_object()
                    .filter(|c| !c.is_empty())
                    .ok_or("Each row needs at least one cell.")?;
                let new_id = run.new_id();
                let edits: Vec<(String, String, Value)> = cells
                    .iter()
                    .map(|(column, value)| (new_id.clone(), column.clone(), value.clone()))
                    .collect();
                let (table_id, table_name, lines) =
                    db::preview_edits(i, table, &edits).map_err(said)?;
                let (one, many) = nouns(&table_id, &table_name);
                let shown: Vec<String> = lines
                    .iter()
                    .filter(|l| !l.after.is_empty())
                    .map(|l| {
                        let value: String = l
                            .after
                            .lines()
                            .next()
                            .unwrap_or("")
                            .chars()
                            .take(70)
                            .collect();
                        format!("{} {value}", l.column)
                    })
                    .collect();
                let line = format!("New {one}: {}", shown.join(", "));
                previews.push(line.clone());
                staged.push(Staged {
                    cmd: "db.rows.add".into(),
                    args: json!({"table": table_id, "rows": [row]}),
                    line,
                    counts: vec![(one, many, "created".into())],
                    makes: vec![new_id.clone()],
                });
                ids.push(new_id);
            }
            staged.into_iter().for_each(|s| run.stage(s));
            Ok(json!({"staged": ids.len(), "ids": ids, "preview": previews, "note": STAGED_NOTE}))
        }
        "delete_rows" => {
            let table = text(args, "table")?;
            let ids: Vec<String> = args["ids"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if ids.is_empty() {
                return Err("This tool needs ids, a list.".into());
            }
            let (table_id, table_name, labels) = db::row_labels(i, table, &ids).map_err(said)?;
            let (one, many) = nouns(&table_id, &table_name);
            let previews: Vec<String> = labels
                .iter()
                .map(|l| format!("Delete {one}: {l}"))
                .collect();
            for (id, line) in ids.iter().zip(&previews) {
                run.stage(Staged {
                    cmd: "db.rows.delete".into(),
                    args: json!({"table": table_id, "rows": [id]}),
                    line: line.clone(),
                    counts: vec![(one.clone(), many.clone(), "deleted".into())],
                    makes: Vec::new(),
                });
            }
            Ok(json!({"staged": ids.len(), "preview": previews, "note": STAGED_NOTE}))
        }
        "complete_tasks" => {
            let done = args["done"].as_bool().unwrap_or(true);
            let changes: Vec<Value> = args["ids"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|id| json!({"id": id, "set": {"done": done}}))
                .collect();
            call(
                i,
                run,
                "update_rows",
                &json!({"table": "task", "changes": changes}),
            )
        }
        "schedule_block" => {
            let (task, date, start) = (
                text(args, "task")?,
                text(args, "date")?,
                text(args, "start")?,
            );
            if !is_ymd(date) {
                return Err(format!("'{date}' isn't a date. Write it YYYY-MM-DD."));
            }
            let (h, m) = start
                .split_once(':')
                .and_then(|(h, m)| {
                    Some((h.trim().parse::<u32>().ok()?, m.trim().parse::<u32>().ok()?))
                })
                .filter(|(h, m)| *h < 24 && *m < 60)
                .ok_or_else(|| format!("'{start}' isn't a time. Write it HH:MM, such as 14:30."))?;
            let minutes = args["minutes"].as_u64().unwrap_or(30);
            let label = if task.starts_with("new:") {
                task.to_string()
            } else {
                db::row_labels(i, "task", &[task.to_string()])
                    .map_err(said)?
                    .2
                    .remove(0)
            };
            let line = format!("Block for {label}: {date} at {h:02}:{m:02}, {minutes} min");
            run.stage(Staged {
                cmd: "heat.block.put".into(),
                args: json!({"taskId": task, "date": date, "start": h * 60 + m, "minutes": minutes}),
                line: line.clone(),
                counts: vec![("block".into(), "blocks".into(), "scheduled".into())],
                makes: Vec::new(),
            });
            Ok(json!({"staged": 1, "preview": [line], "note": STAGED_NOTE}))
        }
        "plan_day" => {
            let date = match args["date"].as_str() {
                Some(d) if is_ymd(d) => d.to_string(),
                Some(d) => return Err(format!("'{d}' isn't a date. Write it YYYY-MM-DD.")),
                None => i.clock().today(),
            };
            let made = invoke(i, "heat.plan.make", &json!({"date": date})).map_err(said)?;
            let drafts = made["drafts"].as_array().cloned().unwrap_or_default();
            if drafts.is_empty() {
                return Ok(
                    json!({"drafts": [], "note": "Plan my day found nothing to draft: no open task fits the free time left."}),
                );
            }
            let ids: Vec<String> = drafts
                .iter()
                .filter_map(|d| d["taskId"].as_str().map(String::from))
                .collect();
            let labels = db::row_labels(i, "task", &ids)
                .map(|l| l.2)
                .unwrap_or_default();
            let shown: Vec<Value> = drafts
                .iter()
                .zip(labels.iter().chain(std::iter::repeat(&String::new())))
                .map(|(d, label)| {
                    let start = d["start"].as_f64().unwrap_or(0.0) as u32;
                    json!({
                        "task": label, "taskId": d["taskId"],
                        "start": format!("{:02}:{:02}", start / 60, start % 60),
                        "minutes": d["minutes"], "reason": d["reason"],
                    })
                })
                .collect();
            let line = format!(
                "Plan for {date}: {} blocks accepted from Plan my day",
                drafts.len()
            );
            run.stage(Staged {
                cmd: "heat.plan.accept".into(),
                args: json!({"date": date}),
                line: line.clone(),
                counts: vec![("day".into(), "days".into(), "planned".into())],
                makes: Vec::new(),
            });
            Ok(
                json!({"drafts": shown, "unplanned": made["unplanned"].as_array().map_or(0, Vec::len), "preview": [line],
                      "note": "The drafts show on Today now, as drafts. Accepting them is staged: the person applies it with one click."}),
            )
        }
        "tick_habit" => {
            let habit = text(args, "habit")?;
            let date = match args["date"].as_str() {
                Some(d) if is_ymd(d) => d.to_string(),
                Some(d) => return Err(format!("'{d}' isn't a date. Write it YYYY-MM-DD.")),
                None => i.clock().today(),
            };
            let done = args["done"].as_bool().unwrap_or(true);
            let label = db::row_labels(i, "habit", &[habit.to_string()])
                .map_err(said)?
                .2
                .remove(0);
            let line = format!(
                "{label}: {} for {date}",
                if done { "ticked" } else { "unticked" }
            );
            run.stage(Staged {
                cmd: "ask.habit.tick".into(),
                args: json!({"habit": habit, "date": date, "done": done}),
                line: line.clone(),
                counts: vec![(
                    "habit".into(),
                    "habits".into(),
                    if done { "ticked" } else { "unticked" }.into(),
                )],
                makes: Vec::new(),
            });
            Ok(json!({"staged": 1, "preview": [line], "note": STAGED_NOTE}))
        }
        "capture" => {
            let words = text(args, "text")?;
            let line = format!("Capture: {}", words.lines().next().unwrap_or(""));
            run.stage(Staged {
                cmd: "heat.capture.add".into(),
                args: json!({"text": words}),
                line: line.clone(),
                counts: vec![("capture".into(), "captures".into(), "added".into())],
                makes: Vec::new(),
            });
            Ok(json!({"staged": 1, "preview": [line], "note": STAGED_NOTE}))
        }
        "add_formula_column" => {
            let (table, name, formula) = (
                text(args, "table")?,
                text(args, "name")?,
                text(args, "formula")?,
            );
            // Checked against the table now, so a mistake can be put right before the person sees it.
            let check = invoke(
                i,
                "db.formula.check",
                &json!({"table": table, "formula": formula}),
            )
            .map_err(said)?;
            if check["ok"] != true {
                return Err(format!(
                    "That formula can't be used: {} Fix it and call this again.",
                    check["message"].as_str().unwrap_or("it doesn't read.")
                ));
            }
            let sample = check["sample"].clone();
            if let Some(err) = sample
                .as_array()
                .and_then(|s| s.iter().find(|v| v["error"].is_string()))
            {
                return Err(format!(
                    "That formula reads, but gives {} on the table's first rows: {} Fix it and call this again.",
                    err["error"].as_str().unwrap_or("an error"),
                    err["message"].as_str().unwrap_or("")
                ));
            }
            let line = format!("New column in {table}: {name} = {formula}");
            run.stage(Staged {
                cmd: "db.column.add".into(),
                args: json!({"table": table, "name": name, "type": "formula", "formula": formula}),
                line: line.clone(),
                counts: vec![("column".into(), "columns".into(), "added".into())],
                makes: Vec::new(),
            });
            Ok(json!({"staged": 1, "preview": [line], "first_values": sample, "note": STAGED_NOTE}))
        }
        "save_view" => {
            let (table, name) = (text(args, "table")?, text(args, "name")?);
            let mut spec = spec_from(args);
            if args["pivot"].is_object() {
                spec["pivot"] = args["pivot"].clone();
            }
            if args["chart"].is_object() {
                let mut chart = args["chart"].clone();
                chart["id"] = json!("c1");
                spec["charts"] = json!([chart]);
            }
            // Read through the view now: a column that isn't there is said at once.
            let seen = invoke(
                i,
                "db.query",
                &json!({"table": table, "spec": spec, "limit": 1}),
            )
            .map_err(said)?;
            if let Some(p) = spec.get("pivot") {
                invoke(
                    i,
                    "db.pivot",
                    &json!({"table": table, "rows": p["rows"], "values": p["values"]}),
                )
                .map_err(said)?;
            }
            if let Some(c) = spec["charts"].get(0) {
                invoke(
                    i,
                    "db.chart",
                    &json!({"table": table, "x": c["x"], "y": c["y"], "agg": c["agg"]}),
                )
                .map_err(said)?;
            }
            let line = format!(
                "New view of {}: {name} ({} rows)",
                seen["table"]["name"].as_str().unwrap_or(table),
                seen["total"]
            );
            run.stage(Staged {
                cmd: "db.view.save".into(),
                args: json!({"table": table, "name": name, "spec": spec}),
                line: line.clone(),
                counts: vec![("view".into(), "views".into(), "saved".into())],
                makes: Vec::new(),
            });
            Ok(json!({"staged": 1, "preview": [line], "note": STAGED_NOTE}))
        }
        "create_table" => {
            let name = text(args, "name")?;
            let columns = args["columns"]
                .as_array()
                .filter(|c| !c.is_empty())
                .ok_or("This tool needs columns, a list.")?;
            let rows = args["rows"].as_array().cloned().unwrap_or_default();
            let cols: Vec<&str> = columns.iter().filter_map(|c| c["name"].as_str()).collect();
            let line = format!(
                "New table: {name} ({}){}",
                cols.join(", "),
                if rows.is_empty() {
                    String::new()
                } else {
                    format!(", {} rows", rows.len())
                }
            );
            run.stage(Staged {
                cmd: "db.table.create".into(),
                args: json!({"name": name, "columns": columns}),
                line: line.clone(),
                counts: vec![("table".into(), "tables".into(), "created".into())],
                makes: Vec::new(),
            });
            if !rows.is_empty() {
                run.stage(Staged {
                    cmd: "db.rows.add".into(),
                    args: json!({"table": name, "rows": rows}),
                    line: format!("{} rows in {name}", rows.len()),
                    counts: Vec::new(),
                    makes: Vec::new(),
                });
            }
            Ok(json!({"staged": 1, "preview": [line], "note": STAGED_NOTE}))
        }

        // ----- leaving this Mac: asked about one by one -----
        "mail_send" => {
            let (to, subject, body) = (
                text(args, "to")?,
                text(args, "subject")?,
                text(args, "body")?,
            );
            let mut mail = json!({"to": to, "subject": subject, "body": body});
            let mut line = format!("Send this mail?\nTo: {to}\n");
            if let Some(cc) = args["cc"].as_str().filter(|c| !c.trim().is_empty()) {
                mail["cc"] = json!(cc);
                line.push_str(&format!("Cc: {cc}\n"));
            }
            if let Some(account) = args["account"].as_str().filter(|c| !c.trim().is_empty()) {
                mail["account"] = json!(account);
                line.push_str(&format!("From: {account}\n"));
            }
            line.push_str(&format!("Subject: {subject}\n\n{body}"));
            lock(&run.outward).push(Staged {
                cmd: "heat.mail.send".into(),
                args: mail,
                line: line.clone(),
                counts: Vec::new(),
                makes: Vec::new(),
            });
            Ok(json!({"asked": true, "preview": [line], "note": OUTWARD_NOTE}))
        }
        "mail_reply" | "mail_file" => {
            let row = text(args, "thread")?;
            // The Mail row names the thread as Gmail does: that is what Mail's commands take.
            let found = invoke(
                i,
                "db.query",
                &json!({"table": "mailThread", "spec": {"rows": [row]}}),
            )
            .map_err(said)?;
            let thread = db::rows_as_objects(
                &found,
                Some(&[
                    "Gmail thread".to_string(),
                    "Subject".to_string(),
                    "From".to_string(),
                ]),
            )
            .into_iter()
            .next()
            .ok_or_else(|| {
                format!(
                    "Mail has no row with the id '{row}'. Find the thread with list_rows on Mail."
                )
            })?;
            let gmail = thread["Gmail thread"]
                .as_str()
                .ok_or("That row has no thread to act on.")?;
            let subject = thread["Subject"].as_str().unwrap_or("(no subject)");
            let (cmd, send, line) = if name == "mail_reply" {
                let body = text(args, "body")?;
                let from = thread["From"].as_str().unwrap_or("");
                (
                    "heat.mail.reply",
                    json!({"threadId": gmail, "body": body}),
                    format!("Send this reply?\nTo: {from}\nSubject: Re: {subject}\n\n{body}"),
                )
            } else {
                match text(args, "action")? {
                    "archive" => (
                        "heat.mail.archive",
                        json!({"threadId": gmail, "archived": true}),
                        format!("Archive '{subject}' in your mailbox?"),
                    ),
                    "unarchive" => (
                        "heat.mail.archive",
                        json!({"threadId": gmail, "archived": false}),
                        format!("Move '{subject}' back to the inbox in your mailbox?"),
                    ),
                    "mark_read" => (
                        "heat.mail.mark",
                        json!({"threadId": gmail, "unread": false}),
                        format!("Mark '{subject}' read in your mailbox?"),
                    ),
                    "mark_unread" => (
                        "heat.mail.mark",
                        json!({"threadId": gmail, "unread": true}),
                        format!("Mark '{subject}' unread in your mailbox?"),
                    ),
                    other => {
                        return Err(format!(
                        "Mail can archive, unarchive, mark_read and mark_unread, not '{other}'."
                    ))
                    }
                }
            };
            lock(&run.outward).push(Staged {
                cmd: cmd.into(),
                args: send,
                line: line.clone(),
                counts: Vec::new(),
                makes: Vec::new(),
            });
            Ok(json!({"asked": true, "preview": [line], "note": OUTWARD_NOTE}))
        }
        "make_public" => {
            let (table, id) = (text(args, "table")?, text(args, "id")?);
            let public = args["public"]
                .as_bool()
                .ok_or("This tool needs public, true or false.")?;
            let (table_id, table_name, labels) =
                db::row_labels(i, table, &[id.to_string()]).map_err(said)?;
            let (one, _) = nouns(&table_id, &table_name);
            let line = if public {
                format!(
                    "Make the {one} '{}' public: anyone who opens your sun can see it.",
                    labels[0]
                )
            } else {
                format!("Make the {one} '{}' private again.", labels[0])
            };
            lock(&run.outward).push(Staged {
                cmd: "heat.public.set".into(),
                args: json!({"kind": table_id, "id": id, "public": public}),
                line: line.clone(),
                counts: Vec::new(),
                makes: Vec::new(),
            });
            Ok(json!({"asked": true, "preview": [line], "note": OUTWARD_NOTE}))
        }
        other => {
            // A tool another part of Learn mounted.
            for extra in EXTRA {
                if let Some(t) = extra.tools.iter().find(|t| t.name == other) {
                    return mounted(i, run, extra, t, args);
                }
            }
            Err(format!("There is no tool called '{other}'."))
        }
    }
}

/// A day as Today shows it, for Claude to read.
fn today(i: &Inner, date: Option<&str>) -> Result<Value, String> {
    let date = match date {
        Some(d) if is_ymd(d) => d.to_string(),
        Some(d) => return Err(format!("'{d}' isn't a date. Write it YYYY-MM-DD.")),
        None => i.clock().today(),
    };
    let snap = invoke(
        i,
        "heat.snapshot",
        &json!({"date": date, "from": date, "to": date}),
    )
    .map_err(said)?;
    let clock = i.clock();
    let find = |kind: &str, id: &str| -> Value {
        snap["records"][kind]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["id"].as_str() == Some(id))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let hhmm = |minutes: f64| format!("{:02}:{:02}", minutes as u32 / 60, minutes as u32 % 60);
    let task_line = |id: &Value| -> Value {
        let t = find("task", id.as_str().unwrap_or(""));
        let d = &snap["derived"]["tasks"][id.as_str().unwrap_or("")];
        json!({
            "id": id, "title": t["title"], "heat": d["heat"]["level"],
            "due": t["due"].as_f64().map(|ms| clock.iso(ms)),
            "minutes": d["estimate"]["min"],
        })
    };
    let mut blocks: Vec<Value> = snap["records"]["timeBlock"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["date"].as_str() == Some(date.as_str()))
        .map(|b| {
            let title = match (b["taskId"].as_str(), b["habitId"].as_str()) {
                (Some(t), _) => find("task", t)["title"].clone(),
                (_, Some(h)) => find("habit", h)["title"].clone(),
                _ => Value::Null,
            };
            json!({"start": hhmm(b["start"].as_f64().unwrap_or(0.0)), "minutes": b["minutes"], "title": title, "taskId": b["taskId"], "sort": b["start"]})
        })
        .collect();
    blocks.sort_by(|a, b| {
        a["sort"]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&b["sort"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for b in &mut blocks {
        if let Some(m) = b.as_object_mut() {
            m.remove("sort");
        }
    }
    let events: Vec<Value> = snap["events"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| {
            json!({
                "title": e["title"], "allDay": e["allDay"],
                "start": e["start"].as_f64().map(|ms| clock.iso(ms)),
                "end": e["end"].as_f64().map(|ms| clock.iso(ms)),
            })
        })
        .collect();
    let list = |v: &Value, max: usize| -> Vec<Value> {
        v.as_array()
            .into_iter()
            .flatten()
            .take(max)
            .map(task_line)
            .collect()
    };
    let habits: Vec<Value> = snap["records"]["habit"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|h| {
            json!({
                "id": h["id"], "title": h["title"],
                "done": h["log"][&date].as_bool().unwrap_or(false),
            })
        })
        .collect();
    Ok(json!({
        "date": date,
        "summary": snap["derived"]["today"]["header"],
        "blocks": blocks,
        "events": events,
        "dueToday": list(&snap["derived"]["today"]["dueToday"], 30),
        "hotNotPlanned": list(&snap["derived"]["today"]["hotUnplanned"], 10),
        "habits": habits,
        "inbox": snap["derived"]["lists"]["inbox"].as_array().map_or(0, Vec::len),
        "timer": snap["derived"]["timer"]["line"],
    }))
}

/// "4 tasks moved, 1 task created, 2 grades updated", from what is staged.
pub(crate) fn summary(staged: &[Staged]) -> String {
    let mut buckets: Vec<((String, String, String), usize)> = Vec::new();
    for s in staged {
        for c in &s.counts {
            match buckets.iter_mut().find(|(k, _)| k == c) {
                Some((_, n)) => *n += 1,
                None => buckets.push((c.clone(), 1)),
            }
        }
    }
    buckets
        .iter()
        .map(|((one, many, verb), n)| format!("{n} {} {verb}", if *n == 1 { one } else { many }))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staged(one: &str, many: &str, verb: &str) -> Staged {
        Staged {
            cmd: String::new(),
            args: Value::Null,
            line: String::new(),
            counts: vec![(one.into(), many.into(), verb.into())],
            makes: Vec::new(),
        }
    }

    #[test]
    fn the_summary_counts_each_kind_of_change() {
        let all = vec![
            staged("task", "tasks", "moved"),
            staged("task", "tasks", "moved"),
            staged("task", "tasks", "created"),
            staged("task", "tasks", "moved"),
            staged("grade", "grades", "updated"),
            staged("task", "tasks", "moved"),
        ];
        assert_eq!(
            summary(&all),
            "4 tasks moved, 1 task created, 1 grade updated"
        );
        assert_eq!(summary(&[]), "");
    }

    #[test]
    fn every_tool_has_a_schema_that_takes_nothing_extra() {
        let mut names = Vec::new();
        for t in all() {
            let schema = (t.schema)();
            assert_eq!(schema["type"], "object", "{}", t.name);
            assert_eq!(schema["additionalProperties"], false, "{}", t.name);
            for r in schema["required"].as_array().unwrap() {
                assert!(
                    schema["properties"][r.as_str().unwrap()].is_object(),
                    "{}.{r}",
                    t.name
                );
            }
            assert!(!names.contains(&t.name), "two tools called {}", t.name);
            names.push(t.name);
            // A name goes into `mcp__learn__<name>`, which can't hold a dot.
            assert!(
                t.name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{}",
                t.name
            );
            // A tool that changes anything says it is staged; one that leaves the Mac says it asks.
            match t.effect {
                Effect::Changes => assert!(
                    t.description.contains("Staged") || t.description.contains("staged"),
                    "{}",
                    t.name
                ),
                Effect::LeavesThisMac => {
                    assert!(t.description.contains("every time"), "{}", t.name)
                }
                Effect::Reads => {}
            }
        }
        assert_eq!(list().as_array().unwrap().len(), all().count());
    }
}
