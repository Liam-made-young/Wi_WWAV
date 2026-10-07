//! What each kind of record may hold, and the rules a write checks
//! (docs/HEAT.md, docs/SPEC.md 3.16). `heat.put` and `heat.patch` come
//! through here: unknown fields are refused, minutes are clamped, a seventh
//! habit is refused, and a record that belongs to another has to point at one
//! that is there.

use serde_json::{json, Map, Value};
use wi_heat::model::recurrence;

use crate::derive::World;
use crate::{kind, num, refused, Clock, Result};

pub(crate) struct Spec {
    pub name: &'static str,
    /// The field that is the record's key in the store.
    pub key: &'static str,
    pub fields: &'static [&'static str],
    /// Fields that may hold null. A null in any other field drops the field,
    /// which is how a patch clears an optional one.
    pub nullable: &'static [&'static str],
    /// Whether the kind has a Public switch (3.15).
    pub switch: bool,
}

const fn spec(
    name: &'static str,
    key: &'static str,
    fields: &'static [&'static str],
    nullable: &'static [&'static str],
    switch: bool,
) -> Spec {
    Spec {
        name,
        key,
        fields,
        nullable,
        switch,
    }
}

/// The kinds `heat.put`, `heat.patch` and `heat.delete` take. A `mailThread`
/// is Claude's alone, a `calendar` is added in Settings → Heat, and a
/// `profileShare` is made by Show: none is written here.
pub(crate) const SPECS: &[Spec] = &[
    spec(
        "space",
        "id",
        &[
            "id",
            "name",
            "hue",
            "groupKind",
            "groupLabel",
            "types",
            "typeDefs",
            "persona",
        ],
        &[],
        false,
    ),
    spec(
        "task",
        "id",
        &[
            "id",
            "spaceId",
            "title",
            "type",
            "typeBy",
            "courseId",
            "projectId",
            "milestoneId",
            "group",
            "parentTaskId",
            "due",
            "scheduledDate",
            "rrule",
            "difficulty",
            "difficultyBy",
            "estMin",
            "estBy",
            "estReason",
            "adjustMin",
            "notes",
            "link",
            "done",
            "doneAt",
            "source",
            "sourceId",
            "claudeReason",
            "tag",
            "public",
        ],
        &["due", "estMin", "doneAt"],
        true,
    ),
    spec(
        "taskOccurrence",
        "id",
        &["id", "taskId", "date", "doneAt"],
        &[],
        false,
    ),
    spec(
        "timeBlock",
        "id",
        &[
            "id", "taskId", "habitId", "date", "start", "minutes", "origin",
        ],
        &[],
        false,
    ),
    spec(
        "focusSession",
        "id",
        &[
            "id",
            "taskId",
            "habitId",
            "startedAt",
            "endedAt",
            "focusMin",
            "interruptions",
            "room",
            "view",
            "source",
            "public",
        ],
        &[],
        true,
    ),
    spec(
        "project",
        "id",
        &[
            "id",
            "spaceId",
            "title",
            "status",
            "targetDate",
            "types",
            "link",
            "source",
            "claudeReason",
            "public",
        ],
        &[],
        true,
    ),
    spec(
        "milestone",
        "id",
        &[
            "id",
            "spaceId",
            "projectId",
            "title",
            "date",
            "done",
            "order",
            "link",
            "source",
            "claudeReason",
            "public",
        ],
        &[],
        true,
    ),
    spec(
        "habit",
        "id",
        &["id", "title", "minutes", "log", "showCounter", "public"],
        &[],
        true,
    ),
    spec("term", "id", &["id", "name"], &[], false),
    spec(
        "course",
        "id",
        &[
            "id",
            "termId",
            "code",
            "name",
            "categories",
            "scale",
            "notes",
            "status",
            "types",
            "syllabusSource",
            "public",
        ],
        &[],
        true,
    ),
    spec(
        "grade",
        "id",
        &[
            "id",
            "courseId",
            "categoryId",
            "title",
            "score",
            "outOf",
            "dropped",
            "pending",
            "link",
            "postedAt",
            "source",
            "public",
        ],
        &["categoryId", "score"],
        true,
    ),
    spec(
        "capture",
        "id",
        &[
            "id",
            "text",
            "link",
            "triagedAt",
            "resultType",
            "resultId",
            "source",
            "claudeReason",
        ],
        &[],
        false,
    ),
    spec(
        "dailyNote",
        "date",
        &["date", "markdown", "public"],
        &[],
        true,
    ),
    spec(
        "note",
        "id",
        &[
            "id",
            "title",
            "markdown",
            "projectId",
            "link",
            "source",
            "claudeReason",
            "public",
        ],
        &[],
        true,
    ),
];

pub(crate) fn spec_of(kind: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.name == kind)
}

/// The kinds a person's put, patch or delete is refused for, with why.
pub(crate) fn restricted(k: &str) -> Option<&'static str> {
    match k {
        kind::MAIL => Some("Mail lists only what Claude recorded. Ask Claude to read it."),
        kind::CALENDAR => Some("Calendars are added and removed in Settings → Learn."),
        kind::SHARE => Some("Show and Hide put a line or a timeline on your public Learn view."),
        _ => None,
    }
}

/// The spec for a kind a write may reach, or why not.
pub(crate) fn writable(k: &str) -> Result<&'static Spec> {
    if let Some(why) = restricted(k) {
        return refused(why);
    }
    match spec_of(k) {
        Some(s) => Ok(s),
        None => refused(format!("Learn keeps no {k}.")),
    }
}

/// The kind in words, for the Edit menu: "dailyNote" reads "daily note".
pub(crate) fn words(k: &str) -> String {
    let mut out = String::new();
    for c in k.chars() {
        if c.is_ascii_uppercase() {
            out.push(' ');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// `YYYY-MM-DD` and a real day.
pub(crate) fn is_day(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
        && s.parse::<jiff::civil::Date>().is_ok()
}

fn text<'a>(m: &'a Map<String, Value>, k: &str) -> &'a str {
    m.get(k).and_then(Value::as_str).unwrap_or("").trim()
}

fn finite(m: &Map<String, Value>, k: &str) -> Option<f64> {
    m.get(k).and_then(Value::as_f64).filter(|n| n.is_finite())
}

fn one_of(m: &Map<String, Value>, k: &str, allowed: &[&str]) -> bool {
    m.get(k)
        .and_then(Value::as_str)
        .is_some_and(|v| allowed.contains(&v))
}

fn needs(m: &mut Map<String, Value>, k: &str, value: Value) {
    if m.get(k).map_or(true, Value::is_null) {
        m.insert(k.to_string(), value);
    }
}

/// A record ready to store, and whether a minutes value was clamped.
pub(crate) struct Finished {
    pub record: Map<String, Value>,
    pub clamped: bool,
}

/// Checks and completes a record. `existing` is what the store holds now
/// (None for a new record), `candidate` what the write would leave.
pub(crate) fn finish(
    sp: &Spec,
    existing: Option<&Value>,
    mut candidate: Map<String, Value>,
    world: &World,
    clock: &Clock,
    new_id: &mut dyn FnMut() -> String,
) -> Result<Finished> {
    let name = sp.name;
    for k in candidate.keys() {
        if !sp.fields.contains(&k.as_str()) {
            return refused(format!("A {} has no field called '{k}'.", words(name)));
        }
    }
    candidate.retain(|k, v| !v.is_null() || sp.nullable.contains(&k.as_str()));
    let was = |k: &str| existing.and_then(|e| e.get(k));
    // The Public switch is its own command (3.15): a write never sets it.
    if sp.switch {
        let before = was("public").and_then(Value::as_bool).unwrap_or(false);
        match candidate.get("public").and_then(Value::as_bool) {
            Some(true) if !before => return refused("The Public switch has its own command."),
            _ => {}
        }
        candidate.insert("public".into(), Value::Bool(before));
    }
    if sp.key == "id"
        && candidate
            .get("id")
            .and_then(Value::as_str)
            .map_or(true, str::is_empty)
    {
        candidate.insert("id".into(), Value::String(new_id()));
    }
    let mut clamped = false;
    match name {
        "space" => space(&mut candidate)?,
        "task" => clamped = task(&mut candidate, existing, world, clock)?,
        "taskOccurrence" => occurrence(&mut candidate, world, clock)?,
        "timeBlock" => block(&mut candidate, world)?,
        "focusSession" => session(&mut candidate, world, clock)?,
        "project" => project(&mut candidate, world)?,
        "milestone" => milestone(&mut candidate, existing, world)?,
        "habit" => habit(&mut candidate, existing, world)?,
        "term" => {
            if text(&candidate, "name").is_empty() {
                return refused("Give the term a name first.");
            }
        }
        "course" => {
            // A course a person makes is theirs; a stub is one a sync found.
            if existing.is_none() {
                needs(&mut candidate, "status", json!("confirmed"));
            }
            course(&mut candidate, world, new_id)?
        }
        "grade" => grade(&mut candidate, world)?,
        "capture" => {
            if text(&candidate, "text").is_empty() {
                return refused("Type something to capture.");
            }
        }
        "dailyNote" => {
            if !candidate
                .get("date")
                .and_then(Value::as_str)
                .is_some_and(is_day)
            {
                return refused("A daily note needs a day, as YYYY-MM-DD.");
            }
            needs(&mut candidate, "markdown", json!(""));
        }
        "note" => needs(&mut candidate, "markdown", json!("")),
        _ => {}
    }
    Ok(Finished {
        record: candidate,
        clamped,
    })
}

fn space(m: &mut Map<String, Value>) -> Result<()> {
    if text(m, "name").is_empty() {
        return refused("Give the space a name first.");
    }
    needs(m, "hue", json!(210));
    needs(m, "groupKind", json!("free"));
    if !one_of(m, "groupKind", &["course", "milestone", "free"]) {
        return refused("A space groups by course, milestone or free text.");
    }
    needs(m, "groupLabel", json!("Group"));
    needs(m, "types", json!(["Other"]));
    needs(m, "persona", json!(""));
    if !m
        .get("types")
        .and_then(Value::as_array)
        .is_some_and(|t| t.iter().all(Value::is_string))
    {
        return refused("A space's types are a list of words.");
    }
    type_defs(m, "typeDefs")
}

/// A home's or a space's task types, checked and tidied: each has a name;
/// its patterns are words; its minutes are 5 to 600 and its difficulty 1 to
/// 5, or left out for the level below to give.
fn type_defs(m: &mut Map<String, Value>, field: &str) -> Result<()> {
    let Some(list) = m.get_mut(field) else {
        return Ok(());
    };
    let Some(list) = list.as_array_mut() else {
        return refused("Task types are a list.");
    };
    for d in list.iter_mut() {
        let Some(d) = d.as_object_mut() else {
            return refused("Each task type has a name.");
        };
        let name = text(d, "name").to_string();
        if name.is_empty() {
            return refused("Give each task type a name.");
        }
        for k in d.keys() {
            if !["name", "patterns", "estMin", "difficulty", "category"].contains(&k.as_str()) {
                return refused(format!("A task type has no field called '{k}'."));
            }
        }
        d.insert("name".into(), json!(name));
        let patterns: Vec<Value> = match d.get("patterns") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(a)) if a.iter().all(Value::is_string) => a
                .iter()
                .filter_map(Value::as_str)
                .map(|p| p.trim().to_lowercase())
                .filter(|p| !p.is_empty())
                .map(Value::String)
                .collect(),
            _ => return refused("A task type's patterns are a list of words."),
        };
        d.insert("patterns".into(), Value::Array(patterns));
        match d.get("estMin").cloned().unwrap_or(Value::Null) {
            Value::Null => d.insert("estMin".into(), Value::Null),
            v => match v.as_f64().filter(|n| n.is_finite()) {
                Some(n) => d.insert(
                    "estMin".into(),
                    num(n.round().clamp(crate::mcp::MINUTES.0, crate::mcp::MINUTES.1)),
                ),
                None => return refused("A task type's minutes are a number."),
            },
        };
        match d.get("difficulty").cloned().unwrap_or(Value::Null) {
            Value::Null => d.insert("difficulty".into(), Value::Null),
            v => match v.as_f64().filter(|n| n.is_finite()) {
                Some(n) => d.insert("difficulty".into(), num(n.round().clamp(1.0, 5.0))),
                None => return refused("A task type's difficulty is 1 to 5."),
            },
        };
        match d.get("category").cloned() {
            Some(Value::String(c)) if !c.trim().is_empty() => {
                d.insert("category".into(), json!(c.trim()));
            }
            Some(Value::String(_)) | Some(Value::Null) => {
                d.remove("category");
            }
            None => {}
            Some(_) => return refused("A task type's category is a category's name."),
        }
    }
    Ok(())
}

fn task(
    m: &mut Map<String, Value>,
    existing: Option<&Value>,
    world: &World,
    _clock: &Clock,
) -> Result<bool> {
    let title = text(m, "title").to_string();
    if title.is_empty() {
        return refused("Give the task a name first.");
    }
    m.insert("title".into(), Value::String(title));
    let Some(space) = m
        .get("spaceId")
        .and_then(Value::as_str)
        .and_then(|id| world.spaces.iter().find(|s| s.id == id))
    else {
        return refused("Pick a space for the task.");
    };
    let _ = space;
    // A type or a difficulty that comes with the write is the writer's own
    // choice; one that is left out is the type's to give (homes.rs).
    let was = |k: &str| existing.and_then(|e| e.get(k)).cloned();
    let moved = |m: &Map<String, Value>, k: &str| existing.is_some() && m.get(k).cloned() != was(k);
    let chosen = |m: &Map<String, Value>, k: &str, by: &str| match existing {
        None => !m.contains_key(by),
        Some(_) => moved(m, k) && !moved(m, by),
    };
    match m.get("type").and_then(Value::as_str).map(str::trim) {
        Some(t) if !t.is_empty() => {
            if chosen(m, "type", "typeBy") {
                m.insert("typeBy".into(), json!("you"));
            }
        }
        _ => {
            m.remove("type");
        }
    }
    match finite(m, "difficulty") {
        Some(d) => {
            m.insert("difficulty".into(), num(d.round().clamp(1.0, 5.0)));
            if chosen(m, "difficulty", "difficultyBy") {
                m.insert("difficultyBy".into(), json!("you"));
            }
        }
        None => {
            m.remove("difficulty");
        }
    }
    let mut clamped = false;
    if let Some(minutes) = m.get("estMin").and_then(Value::as_f64) {
        let set = minutes.clamp(crate::mcp::MINUTES.0, crate::mcp::MINUTES.1);
        clamped = set != minutes;
        m.insert("estMin".into(), num(set));
    } else {
        m.insert("estMin".into(), Value::Null);
    }
    // Whoever types the minutes made the estimate, and Claude's reason went with
    // Claude's number.
    let before = existing
        .and_then(|e| e.get("estMin"))
        .cloned()
        .unwrap_or(Value::Null);
    if m.get("estMin") != Some(&before) {
        if m.get("estMin").is_some_and(|v| !v.is_null()) {
            m.insert("estBy".into(), json!("you"));
        } else {
            m.insert("estBy".into(), json!("default"));
        }
        m.remove("estReason");
    }
    needs(m, "adjustMin", json!(0));
    needs(m, "notes", json!(""));
    needs(m, "done", json!(false));
    if !m.contains_key("doneAt") {
        m.insert("doneAt".into(), Value::Null);
    }
    if !m.contains_key("due") {
        m.insert("due".into(), Value::Null);
    }
    needs(m, "source", json!("you"));
    if !one_of(
        m,
        "source",
        &["you", "calendar", "ical", "mail", "capture", "claude"],
    ) {
        return refused("A task comes from you, a calendar, mail, a capture or Claude.");
    }
    if m.get("estBy").is_some() && !one_of(m, "estBy", &["you", "claude", "type", "default"]) {
        return refused("An estimate is yours, Claude's, its type's or the default.");
    }
    if m.get("difficultyBy").is_some()
        && !one_of(m, "difficultyBy", &["you", "claude", "type", "default"])
    {
        return refused("A difficulty is yours, Claude's, its type's or the default.");
    }
    if m.get("typeBy").is_some() && !one_of(m, "typeBy", &["you", "claude", "rule"]) {
        return refused("A type is yours, Claude's or picked by the title.");
    }
    if let Some(day) = m.get("scheduledDate").and_then(Value::as_str) {
        if !is_day(day) {
            return refused("A scheduled day is written YYYY-MM-DD.");
        }
    }
    if let Some(rule) = m.get("rrule").and_then(Value::as_str) {
        if rule.is_empty() {
            m.remove("rrule");
        } else if recurrence::parse_rule(rule).is_none() {
            return refused("Learn can't read that repeat rule.");
        }
    }
    for (field, kind_of) in [
        ("courseId", "course"),
        ("projectId", "project"),
        ("milestoneId", "milestone"),
    ] {
        if let Some(id) = m.get(field).and_then(Value::as_str) {
            let found = match kind_of {
                "course" => world.courses.iter().any(|c| c.id == id),
                "project" => world.projects.iter().any(|p| p.id == id),
                _ => world.milestones.iter().any(|x| x.id == id),
            };
            if !found {
                return refused(format!("That {kind_of} isn't in Learn any more."));
            }
        }
    }
    if let (Some(parent), Some(me)) = (
        m.get("parentTaskId").and_then(Value::as_str),
        m.get("id").and_then(Value::as_str),
    ) {
        // Walk up the parents: a task is never its own ancestor.
        let mut at = Some(parent.to_string());
        let mut steps = 0;
        while let Some(id) = at {
            if id == me {
                return refused("A task can't be its own parent.");
            }
            steps += 1;
            if steps > 64 {
                break;
            }
            at = world
                .tasks
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.parent_task_id.clone());
        }
        if !world.tasks.iter().any(|t| t.id == parent) {
            return refused("That parent task isn't in Learn any more.");
        }
    }
    // A new task is matched to a home and a type and filled from it. One that
    // changes its type, its home or its space follows the new one in what
    // nobody set by hand; so does one whose estimate was cleared, and one
    // retitled while its title still picks its type.
    let cleared = moved(m, "estMin") && m.get("estMin").map_or(true, Value::is_null);
    let follows = existing.is_none()
        || ["type", "typeBy", "courseId", "projectId", "spaceId"]
            .iter()
            .any(|k| moved(m, k))
        || cleared
        || (moved(m, "title") && text(m, "typeBy") == "rule");
    if follows {
        crate::homes::inherit(m, &crate::homes::Ctx::load(world), existing.is_none(), false);
    }
    if !m.contains_key("type") {
        m.insert("type".into(), json!(wi_heat::model::types::OTHER));
    }
    if !m.contains_key("difficulty") {
        m.insert("difficulty".into(), json!(3));
    }
    Ok(clamped)
}

fn occurrence(m: &mut Map<String, Value>, world: &World, clock: &Clock) -> Result<()> {
    if !m
        .get("taskId")
        .and_then(Value::as_str)
        .is_some_and(|id| world.task_index(id).is_some())
    {
        return refused("No task has that id.");
    }
    if !m.get("date").and_then(Value::as_str).is_some_and(is_day) {
        return refused("An occurrence needs a day, as YYYY-MM-DD.");
    }
    needs(m, "doneAt", num(clock.now_ms));
    Ok(())
}

fn block(m: &mut Map<String, Value>, world: &World) -> Result<()> {
    if !m.get("date").and_then(Value::as_str).is_some_and(is_day) {
        return refused("A block needs a day, as YYYY-MM-DD.");
    }
    let (task, habit) = (
        text(m, "taskId").to_string(),
        text(m, "habitId").to_string(),
    );
    match (task.is_empty(), habit.is_empty()) {
        (false, true) if world.task_index(&task).is_none() => {
            return refused("No task has that id.")
        }
        (true, false) if !world.habits.iter().any(|h| h.id == habit) => {
            return refused("No habit has that id.")
        }
        (false, false) | (true, true) => return refused("A block is for one task or one habit."),
        _ => {}
    }
    if finite(m, "start").is_none() || finite(m, "minutes").map_or(true, |n| n <= 0.0) {
        return refused("A block needs a start and a length in minutes.");
    }
    needs(m, "origin", json!("you"));
    if !one_of(m, "origin", &["you", "plan"]) {
        return refused("A block is yours or the plan's.");
    }
    Ok(())
}

fn session(m: &mut Map<String, Value>, world: &World, _clock: &Clock) -> Result<()> {
    let (task, habit) = (
        text(m, "taskId").to_string(),
        text(m, "habitId").to_string(),
    );
    match (task.is_empty(), habit.is_empty()) {
        (false, true) if world.task_index(&task).is_none() => {
            return refused("No task has that id.")
        }
        (true, false) if !world.habits.iter().any(|h| h.id == habit) => {
            return refused("No habit has that id.")
        }
        (false, false) | (true, true) => {
            return refused("A focus record is for one task or one habit.")
        }
        _ => {}
    }
    let (Some(started), Some(minutes)) = (finite(m, "startedAt"), finite(m, "focusMin")) else {
        return refused("A focus record needs a start and its minutes.");
    };
    if minutes < 0.0 {
        return refused("Minutes can't be less than 0.");
    }
    needs(m, "endedAt", num(started + minutes * 60_000.0));
    needs(m, "interruptions", json!(0));
    // The model writes `room`; 3.16 calls it `view`. Either is read, `room` is kept.
    if let Some(view) = m.remove("view") {
        m.entry("room").or_insert(view);
    }
    needs(m, "room", json!("heat"));
    if !one_of(m, "room", &["heat", "space", "console"]) {
        return refused("A focus record was made in Learn, Space or the Console.");
    }
    if m.get("source").is_some() && !one_of(m, "source", &["timer", "claude"]) {
        return refused("A focus record comes from the timer or Claude.");
    }
    Ok(())
}

fn project(m: &mut Map<String, Value>, world: &World) -> Result<()> {
    if text(m, "title").is_empty() {
        return refused("Give the project a name first.");
    }
    if !m
        .get("spaceId")
        .and_then(Value::as_str)
        .is_some_and(|id| world.spaces.iter().any(|s| s.id == id))
    {
        return refused("Pick a space for the project.");
    }
    needs(m, "status", json!("active"));
    if !one_of(m, "status", &["active", "on_hold", "someday", "archived"]) {
        return refused("A project is active, on hold, someday or archived.");
    }
    if let Some(day) = m.get("targetDate").and_then(Value::as_str) {
        if !is_day(day) {
            return refused("A target date is written YYYY-MM-DD.");
        }
    }
    type_defs(m, "types")
}

fn milestone(m: &mut Map<String, Value>, existing: Option<&Value>, world: &World) -> Result<()> {
    if text(m, "title").is_empty() {
        return refused("Give the milestone a name first.");
    }
    let Some(space) = m.get("spaceId").and_then(Value::as_str).map(str::to_string) else {
        return refused("Pick a space for the milestone.");
    };
    if !world.spaces.iter().any(|s| s.id == space) {
        return refused("Pick a space for the milestone.");
    }
    if !m.get("date").and_then(Value::as_str).is_some_and(is_day) {
        return refused("A milestone needs a day, as YYYY-MM-DD.");
    }
    if let Some(project) = m.get("projectId").and_then(Value::as_str) {
        if !world.projects.iter().any(|p| p.id == project) {
            return refused("That project isn't in Learn any more.");
        }
    }
    needs(m, "done", json!(false));
    if finite(m, "order").is_none() {
        let order = match existing.and_then(|e| e.get("order")) {
            Some(v) => v.clone(),
            None => num(world
                .milestones
                .iter()
                .filter(|x| x.space_id == space)
                .count() as f64),
        };
        m.insert("order".into(), order);
    }
    Ok(())
}

fn habit(m: &mut Map<String, Value>, existing: Option<&Value>, world: &World) -> Result<()> {
    if text(m, "title").is_empty() {
        return refused("Give the habit a name first.");
    }
    if existing.is_none() && world.habits.len() >= wi_heat::model::habits::HABIT_LIMIT {
        return refused(wi_heat::model::copy::habits::LIMIT);
    }
    if let Some(minutes) = m.get("minutes") {
        if !minutes.as_f64().is_some_and(|n| n.is_finite() && n >= 1.0) {
            return refused("A habit's length is a whole number of minutes.");
        }
    }
    needs(m, "log", json!({}));
    if !m.get("log").is_some_and(Value::is_object) {
        return refused("A habit's log is a set of days.");
    }
    needs(m, "showCounter", json!(false));
    Ok(())
}

fn course(
    m: &mut Map<String, Value>,
    world: &World,
    new_id: &mut dyn FnMut() -> String,
) -> Result<()> {
    if text(m, "code").is_empty() {
        return refused("Give the course a code first.");
    }
    let code = wi_heat::homes::kept_code(
        &crate::homes::school_of(world).course_pattern,
        text(m, "code"),
    );
    m.insert("code".into(), json!(code));
    if m.get("status").is_some() && !one_of(m, "status", &["stub", "confirmed"]) {
        return refused("A course is a stub or confirmed.");
    }
    if m.get("syllabusSource").is_some_and(|s| !s.is_object()) {
        return refused("A course's syllabus source is its file's name and pages.");
    }
    type_defs(m, "types")?;
    if !m
        .get("termId")
        .and_then(Value::as_str)
        .is_some_and(|id| world.terms.iter().any(|t| t.id == id))
    {
        return refused("Add a term first, then add the course to it.");
    }
    if text(m, "name").is_empty() {
        let code = text(m, "code").to_string();
        m.insert("name".into(), Value::String(code));
    }
    needs(m, "notes", json!(""));
    needs(m, "categories", json!([]));
    let Some(cats) = m.get_mut("categories").and_then(Value::as_array_mut) else {
        return refused("A course's categories are a list.");
    };
    for c in cats.iter_mut() {
        let Some(c) = c.as_object_mut() else {
            return refused("Each category has a name and a weight.");
        };
        if c.get("id")
            .and_then(Value::as_str)
            .map_or(true, str::is_empty)
        {
            c.insert("id".into(), Value::String(new_id()));
        }
        if c.get("name")
            .and_then(Value::as_str)
            .map_or(true, |n| n.trim().is_empty())
        {
            return refused("Give each category a name.");
        }
        if !c
            .get("weight")
            .and_then(Value::as_f64)
            .is_some_and(|w| w.is_finite() && w >= 0.0)
        {
            return refused("A category's weight is a number, 0 or more.");
        }
        c.entry("keywords").or_insert_with(|| json!([]));
        match c.get("dropLowest").cloned() {
            None => {}
            Some(Value::Null) => {
                c.remove("dropLowest");
            }
            Some(v) => match v.as_f64().filter(|n| n.is_finite() && *n >= 0.0) {
                Some(n) if n.floor() == 0.0 => {
                    c.remove("dropLowest");
                }
                Some(n) => {
                    c.insert("dropLowest".into(), num(n.floor()));
                }
                None => return refused("A category drops a whole number of its lowest scores."),
            },
        }
    }
    Ok(())
}

fn grade(m: &mut Map<String, Value>, world: &World) -> Result<()> {
    if text(m, "title").is_empty() {
        return refused("Give the grade a name first.");
    }
    if !m
        .get("courseId")
        .and_then(Value::as_str)
        .is_some_and(|id| world.courses.iter().any(|c| c.id == id))
    {
        return refused("Pick a course for the grade.");
    }
    needs(m, "outOf", json!(100));
    if !finite(m, "outOf").is_some_and(|n| n > 0.0) {
        return refused("A grade is out of more than 0.");
    }
    if let Some(score) = m.get("score") {
        if !score.is_null() && !score.as_f64().is_some_and(|n| n.is_finite() && n >= 0.0) {
            return refused("A score is a number, 0 or more.");
        }
    }
    if !m.contains_key("score") {
        m.insert("score".into(), Value::Null);
    }
    if !m.contains_key("categoryId") {
        m.insert("categoryId".into(), Value::Null);
    }
    needs(m, "dropped", json!(false));
    needs(m, "pending", json!(false));
    needs(m, "source", json!("you"));
    if !one_of(m, "source", &["you", "mail", "valence", "claude"]) {
        return refused("A grade comes from you, mail, Valence or Claude.");
    }
    if let Some(cat) = m.get("categoryId").and_then(Value::as_str) {
        let course = world
            .courses
            .iter()
            .find(|c| Some(c.id.as_str()) == m.get("courseId").and_then(Value::as_str));
        if !course.is_some_and(|c| c.categories.iter().any(|k| k.id == cat)) {
            return refused("That category isn't in the course.");
        }
    }
    Ok(())
}

/// The Edit menu's words for a write to a record that exists already.
pub(crate) fn edit_label(k: &str, old: &Value, new: &Value) -> String {
    let changed = |f: &str| old.get(f) != new.get(f);
    match k {
        "task" if changed("title") => "rename task".into(),
        "task" if changed("difficulty") || changed("estMin") => "estimate".into(),
        "habit" => {
            let days = |v: &Value| {
                v.get("log")
                    .and_then(Value::as_object)
                    .map_or(0, |l| l.values().filter(|d| **d == json!(true)).count())
            };
            let only_log = new
                .as_object()
                .is_some_and(|n| n.iter().all(|(f, v)| f == "log" || old.get(f) == Some(v)));
            if changed("log") && only_log {
                if days(new) >= days(old) {
                    "tick habit".into()
                } else {
                    "untick habit".into()
                }
            } else {
                "edit habit".into()
            }
        }
        "timeBlock" if changed("start") || changed("date") => "move block".into(),
        "timeBlock" if changed("minutes") => "resize block".into(),
        "timeBlock" => "edit block".into(),
        "grade" if changed("score") && !changed("title") && !changed("courseId") => {
            "enter score".into()
        }
        _ => format!("edit {}", words(k)),
    }
}
