//! The eight tools of docs/SPEC.md 3.13: names, what Claude is told about
//! each, and the JSON Schema its arguments are checked against. The schemas
//! are the door (8.8): every one closes `additionalProperties`, so no call
//! can carry `done`, a score or `public`.

use serde_json::{json, Value};

/// Every write tool's description opens with these two lines (3.13).
pub const WRITE_PREAMBLE: &str = "Estimates and drafts. Never decides: the person accepts, edits or undoes every change.\nNever invent metrics: only restate numbers this app returned.";

/// Tool names, in the order 3.13's table gives them.
pub const NAMES: [&str; 8] = [
    "list_tasks",
    "add_task",
    "update_task",
    "plan_day",
    "get_grades",
    "add_pending_grade",
    "log_focus",
    "record_mail_thread",
];

/// What changing a tool leaves in the journal: `Undo ` + this.
pub fn undo_label(tool: &str) -> Option<&'static str> {
    match tool {
        "add_task" => Some("Claude's task"),
        "update_task" => Some("Claude's estimate"),
        "add_pending_grade" => Some("Claude's pending grade"),
        "log_focus" => Some("Claude's focus log"),
        "record_mail_thread" => Some("Claude's mail note"),
        _ => None,
    }
}

/// Changes nothing the journal keeps. `plan_day` writes only drafts, which
/// aren't a change until the person accepts them (3.13).
pub fn is_read_only(tool: &str) -> bool {
    matches!(tool, "list_tasks" | "get_grades" | "plan_day")
}

/// Gets the two-line preamble: everything but the two pure reads.
fn opens_with_preamble(tool: &str) -> bool {
    !matches!(tool, "list_tasks" | "get_grades")
}

fn reason() -> Value {
    json!({"type": "string", "minLength": 1, "maxLength": 200,
           "description": "One sentence saying why. Heat shows it beside the change."})
}

fn instant(what: &str) -> Value {
    json!({"type": "string", "format": "date-time", "maxLength": 40,
           "description": format!("{what}, ISO 8601 with the local offset, such as 2026-10-07T23:59:00-04:00.")})
}

fn text(max: u64, description: &str) -> Value {
    json!({"type": "string", "minLength": 1, "maxLength": max, "description": description})
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
}

/// The schema for one tool's arguments.
pub fn input_schema(tool: &str) -> Value {
    match tool {
        "list_tasks" => object(
            json!({
                "status": {"type": "string", "enum": ["open", "done", "all"], "description": "Which tasks. Default open."},
                "space": text(100, "A space's name, such as Classes, or its id."),
                "due_before": instant("Only tasks due before this"),
                "limit": {"type": "integer", "minimum": 1, "maximum": 200, "description": "At most this many. Default 50."}
            }),
            &[],
        ),
        "add_task" => object(
            json!({
                "title": text(200, "The task, as the person would write it."),
                "space": text(100, "A space's name or id. Default: the person's first space."),
                "type": text(60, "The task's type, such as Homework or Reading; one of the space's types."),
                "course": text(40, "A course code, such as JPN 201."),
                "due": instant("When it's due"),
                "notes": {"type": "string", "maxLength": 4000, "description": "Plain text. Notes from mail start \"From mail:\"."},
                "source_id": text(200, "Where it came from, such as a Gmail message id. The same source_id never makes a second task."),
                "mail_thread_id": text(200, "The Gmail thread it came from, if any."),
                "reason": reason()
            }),
            &["title", "reason"],
        ),
        "update_task" => object(
            json!({
                "id": text(100, "The task's id, from list_tasks."),
                "difficulty": {"type": "integer", "minimum": 1, "maximum": 5, "description": "1 (easy) to 5 (hard)."},
                "estimate_min": {"type": "integer", "description": "Minutes it will take. Clamped to 5–600."},
                "reason": reason()
            }),
            &["id", "reason"],
        ),
        "plan_day" => object(
            json!({
                "date": {"type": "string", "pattern": "^\\d{4}-\\d{2}-\\d{2}$", "description": "YYYY-MM-DD. Default today."},
                "day_ends": {"type": "string", "pattern": "^([01]\\d|2[0-3]):[0-5]\\d$", "description": "HH:MM, 24-hour. Default 23:00."}
            }),
            &[],
        ),
        "get_grades" => object(
            json!({
                "course": text(40, "A course code, such as JPN 201. Default: every course this term.")
            }),
            &[],
        ),
        "add_pending_grade" => object(
            json!({
                "course": text(40, "The course code, such as JPN 201."),
                "item": text(200, "What was graded, as the notice names it."),
                "posted_at": instant("When the grade was posted"),
                "link": {"type": "string", "maxLength": 2000, "description": "The Brightspace address for the grade."},
                "mail_thread_id": text(200, "The Gmail thread of the notice, if any."),
                "reason": reason()
            }),
            &["course", "item", "reason"],
        ),
        "log_focus" => object(
            json!({
                "task_id": text(100, "The task's id, from list_tasks."),
                "minutes": {"type": "integer", "minimum": 1, "maximum": 600, "description": "Whole minutes spent."},
                "started_at": instant("When the time started. Default: now minus the minutes"),
                "reason": reason()
            }),
            &["task_id", "minutes", "reason"],
        ),
        "record_mail_thread" => object(
            json!({
                "thread_id": text(200, "Gmail's thread id."),
                "subject": text(300, "The thread's subject."),
                "from": text(300, "The sender."),
                "received_at": instant("When it arrived"),
                "course": text(40, "The course code it is about, if any."),
                "state": {"type": "string", "enum": ["grade", "task", "nothing"],
                          "description": "grade: a grade was posted. task: it asks for something, and a task was made. nothing: nothing to do."},
                "task_id": text(100, "The task made from it, if any."),
                "reason": reason()
            }),
            &["thread_id", "subject", "from", "received_at", "state", "reason"],
        ),
        _ => Value::Null,
    }
}

fn summary(tool: &str) -> &'static str {
    match tool {
        "list_tasks" => "Lists the person's tasks with Heat's numbers: due date, difficulty, estimate and who made it, and heat. Also the person's average minutes and count by task type, and each space's persona, so estimates fit the kind of work.",
        "add_task" => "Adds a task, labelled as Claude's. If source_id matches a task already made, nothing is added and created is false. It never marks anything done and never changes a due date.",
        "update_task" => "Sets a task's difficulty, its estimate in minutes, or both, with a reason. Returns the task with its new heat, and clamped: true if the minutes were moved into 5–600.",
        "plan_day" => "Runs Heat's Plan my day rule: open tasks in heat order, each a block the length of its estimate rounded up to 15 minutes and capped at 90, in the first gap that fits before the day ends. The blocks are drafts the person accepts or clears; this changes nothing.",
        "get_grades" => "Reads the term's courses, their categories and weights, every graded item, and Heat's own percentages and letter. Read only.",
        "add_pending_grade" => "Adds a pending grade from a grade notice: no score, which only the person types. A grade for the same course and item is not made twice.",
        "log_focus" => "Logs minutes the person spent on a task, as a focus record. The minutes add to the task's time taken.",
        "record_mail_thread" => "Records a school mail thread for Heat's Mail tab: subject, sender, time, course, and what was done with it. Never the message body. The same thread_id again updates its state and reason and leaves one row.",
        _ => "",
    }
}

/// The description Claude reads for a tool: the preamble first on writes.
pub fn description(tool: &str) -> String {
    if opens_with_preamble(tool) {
        format!("{WRITE_PREAMBLE}\n{}", summary(tool))
    } else {
        summary(tool).to_string()
    }
}

fn title(tool: &str) -> &'static str {
    match tool {
        "list_tasks" => "List tasks",
        "add_task" => "Add a task",
        "update_task" => "Estimate a task",
        "plan_day" => "Draft a day's plan",
        "get_grades" => "Read grades",
        "add_pending_grade" => "Add a pending grade",
        "log_focus" => "Log focus time",
        "record_mail_thread" => "Record a mail thread",
        _ => "",
    }
}

/// One entry of `tools/list`.
pub fn definition(tool: &str) -> Value {
    let read_only = is_read_only(tool);
    json!({
        "name": tool,
        "title": title(tool),
        "description": description(tool),
        "inputSchema": input_schema(tool),
        "annotations": {
            "title": title(tool),
            "readOnlyHint": read_only,
            "destructiveHint": false,
            "idempotentHint": matches!(tool, "add_pending_grade" | "record_mail_thread") || read_only,
            "openWorldHint": false
        }
    })
}
