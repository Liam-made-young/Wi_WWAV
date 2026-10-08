//! The tools the prompt box has for commitments, free time, notes and the
//! capture inbox (docs/NOTES.md, "The tools for ⌘K"). Each is a command of
//! the core, the one the views call, under the name and the arguments a
//! model is given.
//!
//! - A tool that **reads** runs at once and answers.
//! - A tool that **drafts** runs at once too, because all it makes is a
//!   draft: a schedule read and waiting, which the person previews and
//!   applies in Calendar. Nothing of theirs has changed.
//! - A tool that **changes** something is staged: [`stage`] checks it as
//!   far as it can without writing, and answers the core command, its
//!   arguments and one line for the preview. The prompt box runs the command
//!   when the person applies it, as one undo step with the rest.
//!
//! `heat.tools.list` and `heat.tools.call` are the same list and the same
//! calls as commands, so a host that isn't the prompt box can use them.

use serde_json::{json, Map, Value};
use wi_heat_store::{commit, notes};

use crate::args::Args;
use crate::heat_cmd;
use crate::{CoreError, Inner};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    Reads,
    Drafts,
    Changes,
}

impl Effect {
    fn as_str(self) -> &'static str {
        match self {
            Effect::Reads => "reads",
            Effect::Drafts => "drafts",
            Effect::Changes => "changes",
        }
    }
}

pub(crate) struct Tool {
    /// As the brief names it: `commitment.create`.
    pub name: &'static str,
    pub effect: Effect,
    /// What a panel shows while it runs.
    pub doing: &'static str,
    pub description: &'static str,
    pub schema: fn() -> Value,
}

impl Tool {
    /// The name as a model's tool list takes it: no dots.
    pub fn mcp_name(&self) -> String {
        self.name.replace('.', "_")
    }
}

/// One change waiting for the person's click.
#[derive(Clone, Debug)]
pub(crate) struct Staged {
    /// The core command that makes it.
    pub cmd: String,
    pub args: Value,
    /// What the preview says: "New class: JPN 101, MWF 10:00 AM to 10:50 AM."
    pub line: String,
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
}

fn time(what: &str) -> Value {
    json!({"type": "string", "description": format!("{what}, as a time of day: 10:00, 14:30 or 5pm.")})
}

fn day(what: &str) -> Value {
    json!({"type": "string", "description": format!("{what}, YYYY-MM-DD.")})
}

fn commitment_fields() -> Value {
    json!({
        "title": {"type": "string", "description": "What it is. A class is named by its course code."},
        "kind": {"type": "string", "enum": ["class", "work", "commute", "other"]},
        "days": {"type": "array", "items": {"type": "string", "enum": ["MO", "TU", "WE", "TH", "FR", "SA", "SU"]}, "description": "The weekdays it repeats on. Leave out for something on one day."},
        "date": day("The one day it happens on, when it doesn't repeat"),
        "start": time("When it starts"),
        "end": time("When it ends"),
        "from": day("The first day it can happen on"),
        "until": day("The last day. A class with none ends with the term"),
        "location": {"type": "string"},
        "buffer_before_min": {"type": "number", "description": "Minutes of travel before it. Nothing is planned in them."},
        "buffer_after_min": {"type": "number", "description": "Minutes of travel after it."},
        "flexible": {"type": "boolean", "description": "True if it can give way. Fixed unless said."},
        "course": {"type": "string", "description": "A course code, for a class. A class naming a course Learn doesn't hold makes one."},
        "space": {"type": "string", "description": "A space's name."}
    })
}

fn mode() -> Value {
    json!({"type": "string", "enum": ["schedule", "week", "breaks"], "description": "schedule: what repeats every week. week: one week's shifts, replacing that week only. breaks: an academic calendar's days off."})
}

pub(crate) const TOOLS: &[Tool] = &[
    Tool {
        name: "commitment.create",
        effect: Effect::Changes,
        doing: "Adding a commitment",
        description: "Adds a fixed thing to the person's week that tasks are planned around: a class, a work shift, a commute. Give `days` for something weekly or `date` for one day.",
        schema: || object(commitment_fields(), &["title", "start", "end"]),
    },
    Tool {
        name: "commitment.update",
        effect: Effect::Changes,
        doing: "Changing a commitment",
        description: "Changes a commitment: its times, days, place, travel time or range. Only the fields in `set` change.",
        schema: || {
            object(
                json!({
                    "commitment": {"type": "string", "description": "Its id, or its title when only one has it."},
                    "set": object(commitment_fields(), &[])
                }),
                &["commitment", "set"],
            )
        },
    },
    Tool {
        name: "commitment.add_exception",
        effect: Effect::Changes,
        doing: "Changing one day",
        description: "Skips one day of a commitment, or moves it to another time or day. The day has to be one it meets on.",
        schema: || {
            object(
                json!({
                    "commitment": {"type": "string", "description": "Its id, or its title when only one has it."},
                    "date": day("The day it would have happened"),
                    "kind": {"type": "string", "enum": ["skip", "move"]},
                    "to_date": day("A move: the day it happens instead"),
                    "start": time("A move: when it starts instead"),
                    "end": time("A move: when it ends instead"),
                    "note": {"type": "string"}
                }),
                &["commitment", "date", "kind"],
            )
        },
    },
    Tool {
        name: "commitment.import_text",
        effect: Effect::Drafts,
        doing: "Reading a schedule",
        description: "Reads a pasted class or work schedule into a draft. Nothing is added: the person previews the draft in Calendar and applies it.",
        schema: || {
            object(
                json!({"text": {"type": "string"}, "mode": mode(), "week_of": day("For week: a day in the week the shifts are for")}),
                &["text"],
            )
        },
    },
    Tool {
        name: "commitment.import_image",
        effect: Effect::Drafts,
        doing: "Reading a schedule from a photo",
        description: "Reads a photo or screenshot of a schedule into a draft. Nothing is added: the person previews the draft in Calendar and applies it.",
        schema: || {
            object(
                json!({"path": {"type": "string", "description": "The image file's full path."}, "mode": mode(), "week_of": day("For week: a day in the week the shifts are for")}),
                &["path"],
            )
        },
    },
    Tool {
        name: "commitment.import_ics",
        effect: Effect::Drafts,
        doing: "Reading a calendar file",
        description: "Reads an .ics file or address into a draft the person previews and applies. With `subscribe` the address is kept and read again every hour, and what it holds is added at once.",
        schema: || {
            object(
                json!({
                    "path": {"type": "string", "description": "An .ics file's full path."},
                    "url": {"type": "string", "description": "A calendar's address (https or webcal)."},
                    "mode": mode(),
                    "subscribe": {"type": "boolean"},
                    "name": {"type": "string", "description": "What to call a subscribed calendar."}
                }),
                &[],
            )
        },
    },
    Tool {
        name: "planner.free_time",
        effect: Effect::Reads,
        doing: "Working out free time",
        description: "What a day has left once classes, shifts, their travel time, other calendars and sleep are taken out, beside what is planned, and what is fixed that day. Today when no day is given.",
        schema: || {
            object(
                json!({"date": day("One day"), "from": day("The first of a stretch of days"), "to": day("The last of the stretch")}),
                &[],
            )
        },
    },
    Tool {
        name: "note.create",
        effect: Effect::Changes,
        doing: "Writing a note",
        description: "Makes a markdown note. [[Title]] links another note, a course by its code, or a task by its title; #word is a tag; `- [ ]` is a checkbox.",
        schema: || {
            object(
                json!({
                    "title": {"type": "string"},
                    "markdown": {"type": "string"},
                    "course": {"type": "string", "description": "A course code to file it to."},
                    "space": {"type": "string", "description": "A space's name to file it to."}
                }),
                &["title"],
            )
        },
    },
    Tool {
        name: "note.search",
        effect: Effect::Reads,
        doing: "Searching notes",
        description: "Finds notes that hold every word of the query, in their title or text, including the text read from photographed pages. Answers each with the words around the match.",
        schema: || {
            object(
                json!({"query": {"type": "string"}, "limit": {"type": "number"}}),
                &["query"],
            )
        },
    },
    Tool {
        name: "note.link",
        effect: Effect::Changes,
        doing: "Linking a note",
        description: "Adds a link at the end of a note to another note, a course (by its code) or a task (by its title). A name nothing has becomes a new, empty note.",
        schema: || {
            object(
                json!({
                    "note": {"type": "string", "description": "The note's id or its title."},
                    "to": {"type": "string", "description": "What to link to, by name."}
                }),
                &["note", "to"],
            )
        },
    },
    Tool {
        name: "note.file",
        effect: Effect::Changes,
        doing: "Filing a note",
        description: "Moves a note to a course or a space, or to neither. A captured page waiting in the Notes inbox leaves it.",
        schema: || {
            object(
                json!({
                    "note": {"type": "string", "description": "The note's id or its title."},
                    "course": {"type": "string", "description": "A course code."},
                    "space": {"type": "string", "description": "A space's name."},
                    "none": {"type": "boolean", "description": "True to file it to no course and no space."}
                }),
                &["note"],
            )
        },
    },
    Tool {
        name: "capture.process",
        effect: Effect::Changes,
        doing: "Reading captured pages",
        description: "Reads photos and PDFs waiting in the capture inbox (or the one file at `path`) on this Mac, makes a note of each with the image on top and its text below, and files it to the class it was taken in.",
        schema: || {
            object(
                json!({"path": {"type": "string", "description": "One image or PDF, by its full path. Leave out for everything waiting."}}),
                &[],
            )
        },
    },
];

pub(crate) fn tool(name: &str) -> Option<&'static Tool> {
    TOOLS
        .iter()
        .find(|t| t.name == name || t.mcp_name() == name)
}

/// The tools as a model's tool list takes them.
pub(crate) fn list() -> Value {
    Value::Array(
        TOOLS
            .iter()
            .map(|t| {
                json!({
                    "name": t.name, "mcpName": t.mcp_name(), "effect": t.effect.as_str(),
                    "doing": t.doing, "description": t.description, "inputSchema": (t.schema)(),
                })
            })
            .collect(),
    )
}

fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("This needs {key}."))
}

/// A model's commitment fields as the command's.
fn commitment_args(from: &Value) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    for (k, v) in from.as_object().cloned().unwrap_or_default() {
        if v.is_null() {
            continue;
        }
        let key = match k.as_str() {
            "buffer_before_min" => "bufferBefore",
            "buffer_after_min" => "bufferAfter",
            "flexible" => {
                out.insert(
                    "hardness".into(),
                    json!(if v == json!(true) {
                        "flexible"
                    } else {
                        "fixed"
                    }),
                );
                continue;
            }
            "title" | "kind" | "days" | "date" | "start" | "end" | "from" | "until"
            | "location" | "course" | "space" => k.as_str(),
            other => return Err(format!("A commitment has no field called '{other}'.")),
        };
        out.insert(key.to_string(), v);
    }
    Ok(out)
}

/// The command a tool call comes to, and the line that describes it. For a
/// tool that changes something, nothing is written: the checks run, and a
/// refusal comes back as its sentence.
pub(crate) fn stage(i: &Inner, name: &str, args: &Value) -> Result<Staged, String> {
    let Some(t) = tool(name) else {
        return Err(format!("There is no tool called '{name}'."));
    };
    let staged = |cmd: &str, args: Value, line: String| Staged {
        cmd: cmd.to_string(),
        args,
        line,
    };
    let clock = i.clock();
    Ok(match t.name {
        "commitment.create" => {
            let fields = commitment_args(args)?;
            let line =
                commit::describe(&i.store(), &clock, None, &fields).map_err(|e| e.to_string())?;
            staged(
                "heat.commitment.create",
                Value::Object(fields),
                format!("New commitment: {line}"),
            )
        }
        "commitment.update" => {
            let id = text(args, "commitment")?;
            let set = commitment_args(&args["set"])?;
            let found = commit::find(&i.store(), id).map_err(|e| e.to_string())?;
            let line =
                commit::describe(&i.store(), &clock, Some(id), &set).map_err(|e| e.to_string())?;
            staged(
                "heat.commitment.update",
                json!({"id": found["id"], "set": set}),
                format!("Change to: {line}"),
            )
        }
        "commitment.add_exception" => {
            let id = text(args, "commitment")?;
            let found = commit::find(&i.store(), id).map_err(|e| e.to_string())?;
            let date = text(args, "date")?;
            let moves = text(args, "kind")? == "move";
            let mut out = json!({"id": found["id"], "date": date, "kind": if moves { "move" } else { "skip" }, "source": "claude"});
            for (from, to) in [
                ("to_date", "toDate"),
                ("start", "start"),
                ("end", "end"),
                ("note", "note"),
            ] {
                if let Some(v) = args.get(from).filter(|v| !v.is_null()) {
                    out[to] = v.clone();
                }
            }
            let title = found["title"].as_str().unwrap_or("It");
            let day = wi_heat::commitments::day_words(date);
            staged(
                "heat.commitment.addException",
                out,
                if moves {
                    format!("{title} on {day} moves.")
                } else {
                    format!("{title} on {day} is skipped.")
                },
            )
        }
        "commitment.import_text" => staged(
            "heat.commitment.importText",
            json!({"text": text(args, "text")?, "mode": args.get("mode"), "weekOf": args.get("week_of")}),
            "Read the schedule into a draft to preview.".to_string(),
        ),
        "commitment.import_image" => staged(
            "heat.commitment.importImage",
            json!({"path": text(args, "path")?, "mode": args.get("mode"), "weekOf": args.get("week_of")}),
            "Read the photo into a draft to preview.".to_string(),
        ),
        "commitment.import_ics" => {
            let subscribe = args["subscribe"] == json!(true);
            let mut out =
                json!({"mode": args.get("mode"), "subscribe": subscribe, "name": args.get("name")});
            match (args["path"].as_str(), args["url"].as_str()) {
                (Some(p), _) => out["path"] = json!(p),
                (None, Some(u)) => out["url"] = json!(u),
                (None, None) => return Err("This needs path or url.".to_string()),
            }
            staged(
                "heat.commitment.importIcs",
                out,
                if subscribe {
                    "Subscribe to the calendar and add what it holds.".to_string()
                } else {
                    "Read the calendar into a draft to preview.".to_string()
                },
            )
        }
        "planner.free_time" => staged(
            "heat.planner.freeTime",
            json!({"date": args.get("date"), "from": args.get("from"), "to": args.get("to")}),
            "Free time".to_string(),
        ),
        "note.create" => {
            let title = text(args, "title")?;
            if notes::find(&i.store(), title).is_ok() {
                return Err(format!("A note is already called {title}."));
            }
            let mut out =
                json!({"title": title, "markdown": args["markdown"].as_str().unwrap_or("")});
            for k in ["course", "space"] {
                if let Some(v) = args[k].as_str() {
                    out[k] = json!(v);
                }
            }
            staged("heat.note.create", out, format!("New note: {title}"))
        }
        "note.search" => staged(
            "heat.note.search",
            json!({"q": text(args, "query")?, "limit": args.get("limit")}),
            "Search notes".to_string(),
        ),
        "note.link" => {
            let note = notes::find(&i.store(), text(args, "note")?).map_err(|e| e.to_string())?;
            let to = text(args, "to")?;
            staged(
                "heat.note.link",
                json!({"id": note["id"], "to": to}),
                format!("{}: link to {to}", note["title"].as_str().unwrap_or("Note")),
            )
        }
        "note.file" => {
            let note = notes::find(&i.store(), text(args, "note")?).map_err(|e| e.to_string())?;
            let title = note["title"].as_str().unwrap_or("Note");
            let mut out = json!({"id": note["id"]});
            let line = match (args["course"].as_str(), args["space"].as_str()) {
                (Some(c), _) => {
                    out["course"] = json!(c);
                    format!("{title}: file to {c}")
                }
                (None, Some(s)) => {
                    out["space"] = json!(s);
                    format!("{title}: file to {s}")
                }
                (None, None) if args["none"] == json!(true) => {
                    out["none"] = json!(true);
                    format!("{title}: file to no course or space")
                }
                (None, None) => {
                    return Err("Say where the note goes: a course, a space, or none.".to_string())
                }
            };
            staged("heat.note.file", out, line)
        }
        "capture.process" => match args["path"].as_str() {
            Some(path) => staged(
                "heat.capture.process",
                json!({ "path": path }),
                "Read the file into a note and file it.".to_string(),
            ),
            None => staged(
                "heat.capture.process",
                json!({"wait": true}),
                "Read what waits in the capture inbox.".to_string(),
            ),
        },
        other => return Err(format!("There is no tool called '{other}'.")),
    })
}

/// Stages a call and, unless it is only to be staged, runs it. A tool that
/// reads or drafts is always run.
pub(crate) fn call(
    i: &Inner,
    name: &str,
    args: &Value,
    only_stage: bool,
) -> Result<Value, CoreError> {
    let t = tool(name).ok_or_else(|| {
        CoreError::new("unknown_tool", format!("There is no tool called '{name}'."))
    })?;
    let s = stage(i, name, args).map_err(|sentence| CoreError::new("refused", sentence))?;
    if only_stage && t.effect == Effect::Changes {
        return Ok(json!({"staged": {"cmd": s.cmd, "args": s.args, "line": s.line}}));
    }
    let args = strip_nulls(s.args);
    let result = heat_cmd::invoke(i, &s.cmd, &Args::new(&s.cmd, &args))?;
    Ok(json!({"cmd": s.cmd, "line": s.line, "result": result}))
}

fn strip_nulls(v: Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.into_iter().filter(|(_, v)| !v.is_null()).collect()),
        other => other,
    }
}

pub(crate) fn invoke(i: &Inner, cmd: &str, a: &Args) -> Result<Value, CoreError> {
    match cmd {
        "heat.tools.list" => Ok(json!({ "tools": list() })),
        // `heat.tools.call {name, args, stage?}`
        "heat.tools.call" => {
            heat_cmd::open_heat(i)?;
            let args = a.get("args").cloned().unwrap_or_else(|| json!({}));
            call(
                i,
                a.str("name")?,
                &args,
                a.opt_bool("stage")?.unwrap_or(false),
            )
        }
        _ => Err(CoreError::new(
            "unknown_command",
            format!("There is no command called '{cmd}'."),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_twelve_tools_are_named_as_the_brief_names_them_and_take_nothing_extra() {
        let names: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            [
                "commitment.create",
                "commitment.update",
                "commitment.add_exception",
                "commitment.import_text",
                "commitment.import_image",
                "commitment.import_ics",
                "planner.free_time",
                "note.create",
                "note.search",
                "note.link",
                "note.file",
                "capture.process",
            ]
        );
        for t in TOOLS {
            let schema = (t.schema)();
            assert_eq!(schema["type"], "object", "{}", t.name);
            assert_eq!(schema["additionalProperties"], false, "{}", t.name);
            assert!(!t.mcp_name().contains('.'), "{}", t.name);
            assert!(t.description.len() > 40, "{} says too little", t.name);
        }
    }
}
