//! The tools of docs/SPEC.md 3.13: names, what Claude is told about each,
//! and the JSON Schema its arguments are checked against. The first eight
//! are the mail and planning tools; the ten after them read the rest of the
//! view and draft into it, under the same rule; the last two read the mail
//! accounts and what is recorded from each. The schemas
//! are the door (8.8): every one closes `additionalProperties`, so no call
//! can carry `done`, a score or `public`.

use serde_json::{json, Value};

/// Every write tool's description opens with these two lines (3.13).
pub const WRITE_PREAMBLE: &str = "Estimates and drafts. Never decides: the person accepts, edits or undoes every change.\nNever invent metrics: only restate numbers this app returned.";

/// Tool names, in the order 3.13's table gives them.
pub const NAMES: [&str; 21] = [
    "list_tasks",
    "add_task",
    "update_task",
    "plan_day",
    "get_grades",
    "add_pending_grade",
    "log_focus",
    "record_mail_thread",
    "get_schedule",
    "draft_block",
    "list_habits",
    "list_projects",
    "add_project",
    "add_milestone",
    "get_notes",
    "add_note",
    "list_inbox",
    "add_capture",
    "list_mail_accounts",
    "list_mail",
    "save_mail_text",
];

/// What changing a tool leaves in the journal: `Undo ` + this.
pub fn undo_label(tool: &str) -> Option<&'static str> {
    match tool {
        "add_task" => Some("Claude's task"),
        "update_task" => Some("Claude's estimate"),
        "add_pending_grade" => Some("Claude's pending grade"),
        "log_focus" => Some("Claude's focus log"),
        "record_mail_thread" => Some("Claude's mail note"),
        "add_project" => Some("Claude's project"),
        "add_milestone" => Some("Claude's milestone"),
        "add_note" => Some("Claude's note"),
        "add_capture" => Some("Claude's capture"),
        _ => None,
    }
}

/// The pure reads: they write nothing at all.
pub fn is_pure_read(tool: &str) -> bool {
    matches!(tool, "list_tasks" | "get_grades" | "get_schedule" | "list_habits" | "list_projects" | "get_notes" | "list_inbox" | "list_mail_accounts" | "list_mail")
}

/// Changes nothing the journal keeps. `plan_day` and `draft_block` write only
/// drafts, which aren't a change until the person accepts them (3.13).
pub fn is_read_only(tool: &str) -> bool {
    is_pure_read(tool) || matches!(tool, "plan_day" | "draft_block")
}

/// Gets the two-line preamble: everything but the pure reads.
fn opens_with_preamble(tool: &str) -> bool {
    !is_pure_read(tool)
}

fn day(what: &str) -> Value {
    json!({"type": "string", "pattern": "^\\d{4}-\\d{2}-\\d{2}$", "description": format!("{what}, YYYY-MM-DD.")})
}

fn reason() -> Value {
    json!({"type": "string", "minLength": 1, "maxLength": 200,
           "description": "One sentence saying why. Learn shows it beside the change."})
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
                "account": text(254, "The mail account it belongs to, as list_mail_accounts gives its address. Default: the only account, if there is one."),
                "priority": {"type": "string", "enum": ["urgent", "high", "normal", "low"],
                             "description": "urgent: the person must act today or tomorrow. high: they must act this week, or a person is waiting on them. normal: worth knowing, nothing to do. low: bulk mail, promotions, automatic notices. Default normal."},
                "category": {"type": "string", "enum": ["school", "work", "money", "people", "updates", "promotions", "other"],
                             "description": "What it is about. school: courses, grades, the university. work: jobs, internships, clients. money: bills, aid, payments. people: a person writing to them. updates: accounts, bookings, receipts. promotions: marketing."},
                "reason": reason()
            }),
            &["thread_id", "subject", "from", "received_at", "state", "reason"],
        ),
        "list_mail_accounts" => object(json!({}), &[]),
        "save_mail_text" => object(
            json!({
                "thread_id": text(200, "Gmail's thread id, as recorded with record_mail_thread."),
                "messages": {
                    "type": "array", "minItems": 1, "maxItems": 50,
                    "description": "Every message of the thread, oldest first.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "message_id": {"type": "string", "maxLength": 200, "description": "Gmail's message id."},
                            "from": {"type": "string", "minLength": 1, "maxLength": 300, "description": "The sender, as the mail names them."},
                            "to": {"type": "string", "maxLength": 1000, "description": "Who it was sent to."},
                            "sent_at": instant("When it was sent"),
                            "text": {"type": "string", "minLength": 1, "maxLength": 100000, "description": "The message's own words as plain text, with its paragraphs and line breaks. No HTML. Leave out the quoted earlier messages, tracking pixels' alt text and unsubscribe footers. Keep links as plain addresses. Never summarise, shorten or reword."}
                        },
                        "required": ["from", "sent_at", "text"],
                        "additionalProperties": false
                    }
                }
            }),
            &["thread_id", "messages"],
        ),
        "list_mail" => object(
            json!({
                "account": text(254, "Only this account's threads: its address."),
                "state": {"type": "string", "enum": ["grade", "task", "nothing"], "description": "Only threads in this state."},
                "priority": {"type": "string", "enum": ["urgent", "high", "normal", "low"], "description": "Only threads of this priority."},
                "category": {"type": "string", "enum": ["school", "work", "money", "people", "updates", "promotions", "other"], "description": "Only threads in this category."},
                "since": instant("Only threads received at or after this"),
                "limit": {"type": "integer", "minimum": 1, "maximum": 200, "description": "At most this many, newest first. Default 50."}
            }),
            &[],
        ),
        "get_schedule" => object(
            json!({
                "from": day("The first day. Default today"),
                "to": day("The last day, at most 31 days on. Default the first day")
            }),
            &[],
        ),
        "draft_block" => object(
            json!({
                "task_id": text(100, "The task's id, from list_tasks."),
                "date": day("The day. Default today"),
                "start": {"type": "string", "pattern": "^([01]\\d|2[0-3]):[0-5]\\d$", "description": "HH:MM, 24-hour, in the person's time zone. Rounded to 15 minutes; between 07:00 and midnight."},
                "minutes": {"type": "integer", "minimum": 15, "maximum": 240, "description": "How long. Rounded to 15 minutes."},
                "reason": reason()
            }),
            &["task_id", "start", "minutes", "reason"],
        ),
        "list_habits" => object(json!({}), &[]),
        "list_projects" => object(
            json!({
                "space": text(100, "A space's name or id. Default: every space."),
                "status": {"type": "string", "enum": ["active", "on_hold", "someday", "archived", "all"], "description": "Which projects. Default active."}
            }),
            &[],
        ),
        "add_project" => object(
            json!({
                "title": text(200, "The project's name, as the person would write it."),
                "space": text(100, "A space's name or id. Default: the person's first space."),
                "target_date": day("When the person means to finish, if they said"),
                "reason": reason()
            }),
            &["title", "reason"],
        ),
        "add_milestone" => object(
            json!({
                "title": text(200, "The milestone, as the person would write it."),
                "date": day("The day it falls on"),
                "project_id": text(100, "The project's id, from list_projects. Without it the milestone stands alone in its space."),
                "space": text(100, "A space's name or id. Default: the project's space, else the person's first space."),
                "reason": reason()
            }),
            &["title", "date", "reason"],
        ),
        "get_notes" => object(
            json!({
                "date": day("The day whose daily note to read. Default today"),
                "limit": {"type": "integer", "minimum": 1, "maximum": 100, "description": "At most this many notes, newest first. Default 20."}
            }),
            &[],
        ),
        "add_note" => object(
            json!({
                "title": text(200, "The note's title."),
                "text": {"type": "string", "minLength": 1, "maxLength": 20000, "description": "The note, in Markdown."},
                "project_id": text(100, "A project's id, from list_projects, if the note belongs to one."),
                "reason": reason()
            }),
            &["title", "text", "reason"],
        ),
        "list_inbox" => object(json!({}), &[]),
        "add_capture" => object(
            json!({
                "text": text(2000, "The line to keep, as the person would jot it. The first line becomes a title if they make it a task."),
                "reason": reason()
            }),
            &["text", "reason"],
        ),
        _ => Value::Null,
    }
}

fn summary(tool: &str) -> &'static str {
    match tool {
        "list_tasks" => "Lists the person's tasks with Learn's numbers: due date, difficulty, estimate and who made it, and heat. Also the person's average minutes and count by task type, and each space's persona, so estimates fit the kind of work.",
        "add_task" => "Adds a task, labelled as Claude's. If source_id matches a task already made, nothing is added and created is false. It never marks anything done and never changes a due date.",
        "update_task" => "Sets a task's difficulty, its estimate in minutes, or both, with a reason. Returns the task with its new heat, and clamped: true if the minutes were moved into 5–600.",
        "plan_day" => "Runs Learn's Plan my day rule: open tasks in heat order, each a block the length of its estimate rounded up to 15 minutes and capped at 90, in the first gap that fits before the day ends. The blocks are drafts the person accepts or clears; this changes nothing.",
        "get_grades" => "Reads the term's courses, their categories and weights, every graded item, and Learn's own percentages and letter. Read only.",
        "add_pending_grade" => "Adds a pending grade from a grade notice: no score, which only the person types. A grade for the same course and item is not made twice.",
        "log_focus" => "Logs minutes the person spent on a task, as a focus record. The minutes add to the task's time taken.",
        "record_mail_thread" => "Records a mail thread for Learn's Mail tab: which account it is from, subject, sender, time, how pressing it is, what it is about, and what was done with it. Never the message body. The same thread_id again updates the row and leaves one.",
        "list_mail_accounts" => "Reads the mail accounts the person set up, with the Gmail search that finds each account's mail and no other's, how many threads are recorded from each, and the priorities and categories to sort into. Read only. Call it before reading mail.",
        "list_mail" => "Reads the threads already recorded, newest first, with their account, priority, category and state, and whether each has its text saved. Read only. Use it to see what has been through and to answer what needs the person's attention.",
        "save_mail_text" => "Saves a thread's messages as plain text so the person can read them in Learn's Mail, which shows them in a reader view. The thread must be recorded first. The text is the mail's own words, never a summary. It stays on this Mac: it is not synced or exported. Saving a thread again replaces what was saved. Not an undo step.",
        "get_schedule" => "Reads what the time column and Calendar show between two days: the blocks already planned, calendar events, open tasks due, and drafts waiting to be accepted. Read only. Read it before draft_block, to find a free time.",
        "draft_block" => "Drafts one block for a task at a time you name, for when the person asks for a specific time rather than Learn's own rule. The block is a draft the person accepts or clears; this changes nothing. A time already held by a block, a draft or a calendar event is refused. A second draft for the same task and day replaces the first.",
        "list_habits" => "Reads the person's habits: whether each is ticked today, and its record line as Learn words it. Read only. Only the person ticks a habit; there is no tool that does.",
        "list_projects" => "Reads projects with their milestones in order and how many open tasks each holds, and the milestones that stand alone. Read only.",
        "add_project" => "Adds a project, marked as Claude's, active and empty. A space never gets two projects with the same name. It never archives, renames or deletes a project.",
        "add_milestone" => "Adds a milestone on a day, marked as Claude's, to a project or alone in a space. It is never made done and never moved: only the person does that.",
        "get_notes" => "Reads the person's notes, newest first, and one day's daily note. Long notes are cut at 4,000 characters and say so. Read only.",
        "add_note" => "Adds a note, marked as Claude's, in Markdown. It never edits a note the person wrote and never writes the daily note.",
        "list_inbox" => "Reads the captures waiting in the Inbox. Read only: only the person triages them.",
        "add_capture" => "Drops one line into the Inbox, marked as Claude's, for the person to make into a task, a note or a project, or to throw away. Use it when you aren't sure something is a task: the person decides what it becomes.",
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
        "get_schedule" => "Read the schedule",
        "draft_block" => "Draft a block",
        "list_habits" => "List habits",
        "list_projects" => "List projects",
        "add_project" => "Add a project",
        "add_milestone" => "Add a milestone",
        "get_notes" => "Read notes",
        "add_note" => "Add a note",
        "list_inbox" => "Read the inbox",
        "add_capture" => "Add to the inbox",
        "list_mail_accounts" => "List mail accounts",
        "list_mail" => "List recorded mail",
        "save_mail_text" => "Save a thread's text",
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
            "idempotentHint": matches!(tool, "add_pending_grade" | "record_mail_thread") || is_pure_read(tool) || tool == "plan_day",
            "openWorldHint": false
        }
    })
}
