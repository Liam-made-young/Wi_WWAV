//! Notes in the library (docs/NOTES.md). A note is a `note` record, as it
//! always was, and a markdown file in the notes folder; the core keeps the
//! two the same (`notes_cmd.rs`), and this module is the record's side:
//! making and renaming notes, what a note links to and what links to it,
//! turning a checkbox into a task, filing a captured page, and the quiet
//! notices that say what Learn just did.
//!
//! - A title is a note's name everywhere: its file's name, and what
//!   `[[Title]]` finds. No two notes share one.
//! - A link finds a note by its title, else a course by its code, else a
//!   task by its title.
//! - A captured page is two entries: `capture` makes the note in the Notes
//!   inbox, and `file note` puts it with its class. Undo takes back the
//!   filing first, and the page stays.
//! - What Claude suggests for a page (tasks, key terms) is kept beside the
//!   note as a `noteSuggestion`, outside the journal. It is never applied.
//! - A `notice` is one line Learn shows quietly: "Filed to JPN 101 · Oct 7".

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};
use wi_heat::homes::{course_label, squash};
use wi_heat::model::format::{self as fmt, short_month_day};
use wi_heat::model::zone;
use wi_heat::notes::{self as rules, SuggestedTask};
use wi_store::{Actor, Store};

use crate::derive::World;
use crate::schema::{self, is_day, spec_of};
use crate::{all, commit, kind, num, one, put, refused, ulid, Clock, Outcome, Result};

/// A notice goes away by itself after a day.
const NOTICE_LASTS_MS: f64 = 86_400_000.0;
const NOTICE_LIMIT: usize = 12;

fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn title_of(note: &Value) -> String {
    text(note, "title").trim().to_string()
}

/// Checks and completes a note (`schema::finish` comes through here).
pub(crate) fn check(
    m: &mut Map<String, Value>,
    existing: Option<&Value>,
    world: &World,
    clock: &Clock,
) -> Result<()> {
    if m.get("markdown").map_or(true, |v| !v.is_string()) {
        m.insert("markdown".into(), json!(""));
    }
    match m.get("title").and_then(Value::as_str).map(str::trim) {
        Some("") | None => {
            m.remove("title");
        }
        Some(t) => {
            let t = t.to_string();
            m.insert("title".into(), json!(t));
        }
    }
    if let Some(id) = m.get("courseId").and_then(Value::as_str) {
        if !world.courses.iter().any(|c| c.id == id) {
            return refused("That course isn't in Learn any more.");
        }
    }
    if let Some(id) = m.get("spaceId").and_then(Value::as_str) {
        if !world.spaces.iter().any(|s| s.id == id) {
            return refused("That space isn't in Learn any more.");
        }
    }
    if m.get("inbox") == Some(&json!(false)) {
        m.remove("inbox");
    }
    if let Some(file) = m.get("file").and_then(Value::as_str) {
        if file.split(['/', '\\']).any(|part| part == "..") || file.starts_with('/') {
            return refused("A note's file is inside the notes folder.");
        }
    }
    // When it was made, and when its words last changed.
    let was = |k: &str| existing.and_then(|e| e.get(k));
    if existing.is_none() {
        m.entry("createdAt").or_insert_with(|| num(clock.now_ms));
        m.entry("updatedAt").or_insert_with(|| num(clock.now_ms));
    } else if m.get("markdown") != was("markdown") || m.get("title") != was("title") {
        m.insert("updatedAt".into(), num(clock.now_ms));
    }
    Ok(())
}

/// Every note, as stored.
pub fn notes(store: &Store) -> Result<Vec<Value>> {
    all(store, kind::NOTE)
}

/// A note by its id, or by its title in any case.
pub fn find(store: &Store, q: &str) -> Result<Value> {
    if let Some(found) = one(store, kind::NOTE, q)? {
        return Ok(found);
    }
    let want = q.trim().to_lowercase();
    match notes(store)?
        .into_iter()
        .find(|n| title_of(n).to_lowercase() == want)
    {
        Some(n) => Ok(n),
        None => refused(format!("No note is called {q}.")),
    }
}

fn titles(all_notes: &[Value], but: Option<&str>) -> BTreeSet<String> {
    all_notes
        .iter()
        .filter(|n| but.map_or(true, |id| n["id"] != id))
        .map(|n| title_of(n).to_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Where a note is filed, from args that name a course by `courseId` or
/// `course` (its code) and a space by `spaceId` or `space` (its name).
fn place(world: &World, args: &Map<String, Value>) -> Result<(Option<String>, Option<String>)> {
    let s = |k: &str| {
        args.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    let course = match (s("courseId"), s("course")) {
        (Some(id), _) => Some(world.course(id)?.id.clone()),
        (None, Some(code)) => Some(world.course(code)?.id.clone()),
        _ => None,
    };
    let space = match (s("spaceId"), s("space")) {
        (Some(id), _) => Some(world.space(id)?.id.clone()),
        (None, Some(name)) => Some(world.space(name)?.id.clone()),
        _ => None,
    };
    Ok((course, space))
}

fn finished(
    world: &World,
    clock: &Clock,
    existing: Option<&Value>,
    record: Value,
) -> Result<Value> {
    let sp = spec_of(kind::NOTE).expect("note");
    let done = schema::finish(
        sp,
        existing,
        record.as_object().cloned().unwrap_or_default(),
        world,
        clock,
        &mut ulid,
    )?;
    Ok(Value::Object(done.record))
}

/// `heat.note.create {title?, markdown?, course?, space?, ifMissing?}`. A
/// title another note has is refused, unless `ifMissing` asks for the note
/// that is there.
pub fn create(
    store: &mut Store,
    clock: &Clock,
    args: &Map<String, Value>,
    actor: Actor,
) -> Result<Outcome> {
    let world = World::load(store)?;
    let all_notes = notes(store)?;
    let taken = titles(&all_notes, None);
    let asked = args
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let title = if asked.is_empty() {
        rules::free_name("Untitled", &taken)
    } else if taken.contains(&asked.to_lowercase()) {
        if args.get("ifMissing") == Some(&json!(true)) {
            let note = find(store, asked)?;
            return Ok(Outcome::unchanged(json!({"note": note, "existed": true})));
        }
        return refused(format!("A note is already called {asked}."));
    } else {
        asked.to_string()
    };
    let (course, space) = place(&world, args)?;
    let mut record = json!({"title": title, "markdown": args.get("markdown").and_then(Value::as_str).unwrap_or("")});
    if let Some(c) = course {
        record["courseId"] = json!(c);
    }
    if let Some(s) = space {
        record["spaceId"] = json!(s);
    }
    let by_claude = matches!(actor, Actor::Claude { .. });
    if by_claude {
        record["source"] = json!("claude");
    }
    let note = finished(&world, clock, None, record)?;
    let label = if by_claude {
        "Claude's note"
    } else {
        "add note"
    };
    let c = commit(store, label, actor, |txn| put(txn, kind::NOTE, &note))?;
    Ok(Outcome::new(json!({"note": note}), c))
}

/// `heat.note.save {id, markdown?, title?}`. A new title is the note's new
/// name everywhere: every link to the old one is rewritten in the same entry.
pub fn save(
    store: &mut Store,
    clock: &Clock,
    id: &str,
    title: Option<&str>,
    markdown: Option<&str>,
) -> Result<Outcome> {
    let old = find(store, id)?;
    let world = World::load(store)?;
    let all_notes = notes(store)?;
    let mut record = old.clone();
    let mut renamed: Vec<Value> = Vec::new();
    if let Some(md) = markdown {
        record["markdown"] = json!(md);
    }
    let mut label = "edit note";
    if let Some(new) = title.map(str::trim) {
        let was = title_of(&old);
        if new.is_empty() {
            return refused("Give the note a name first.");
        }
        if new != was {
            if titles(&all_notes, old["id"].as_str()).contains(&new.to_lowercase()) {
                return refused(format!("A note is already called {new}."));
            }
            record["title"] = json!(new);
            label = "rename note";
            if !was.is_empty() {
                for other in all_notes.iter().filter(|n| n["id"] != old["id"]) {
                    let md = text(other, "markdown");
                    let moved = rules::rename_links(md, &was, new);
                    if moved != md {
                        let mut o = other.clone();
                        o["markdown"] = json!(moved);
                        renamed.push(finished(&world, clock, Some(other), o)?);
                    }
                }
                // A note that links to itself follows too.
                let own = text(&record, "markdown").to_string();
                record["markdown"] = json!(rules::rename_links(&own, &was, new));
            }
        }
    }
    let note = finished(&world, clock, Some(&old), record)?;
    let c = commit(store, label, Actor::You, |txn| {
        put(txn, kind::NOTE, &note)?;
        for r in &renamed {
            put(txn, kind::NOTE, r)?;
        }
        Ok(())
    })?;
    Ok(Outcome::new(
        json!({"note": note, "renamed": renamed.len()}),
        c,
    ))
}

/// "JPN 101", "School", or nothing: where a note is filed, in words.
fn place_words(world: &World, note: &Value) -> Option<String> {
    if let Some(c) = note["courseId"]
        .as_str()
        .and_then(|id| world.courses.iter().find(|c| c.id == id))
    {
        return Some(c.code.clone());
    }
    note["spaceId"]
        .as_str()
        .and_then(|id| world.spaces.iter().find(|s| s.id == id))
        .map(|s| s.name.clone())
}

/// `heat.note.file {id, course? | space? | none?}`: where a note belongs.
/// Filing one that waits in the Notes inbox takes it out of there.
pub fn file(
    store: &mut Store,
    clock: &Clock,
    id: &str,
    args: &Map<String, Value>,
    actor: Actor,
) -> Result<Outcome> {
    let old = find(store, id)?;
    let world = World::load(store)?;
    let (course, space) = place(&world, args)?;
    let nowhere = args.get("none") == Some(&json!(true));
    if course.is_none() && space.is_none() && !nowhere {
        return refused("Say where the note goes: a course, a space, or none.");
    }
    let mut record = old.clone();
    if let Some(m) = record.as_object_mut() {
        m.remove("courseId");
        m.remove("spaceId");
        m.remove("inbox");
        if let Some(c) = &course {
            m.insert("courseId".into(), json!(c));
        }
        if let Some(s) = &space {
            m.insert("spaceId".into(), json!(s));
        }
    }
    // A captured page takes its class's name when it is filed to it.
    if let Some(title) = args
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let taken = titles(&notes(store)?, old["id"].as_str());
        record["title"] = json!(rules::free_name(title, &taken));
    }
    let note = finished(&world, clock, Some(&old), record)?;
    let c = commit(store, "file note", actor, |txn| put(txn, kind::NOTE, &note))?;
    let line = match place_words(&world, &note) {
        Some(_) if args.contains_key("title") => format!("Filed to {}", title_of(&note)),
        Some(p) => format!("Filed to {p}"),
        None => "Filed with no course or space".to_string(),
    };
    Ok(Outcome::new(json!({"note": note, "line": line}), c))
}

/// What a link's words point at.
enum Target {
    Note(String),
    Course(String),
    Task(String),
    Missing,
}

struct Names<'a> {
    notes: BTreeMap<String, &'a Value>,
    world: &'a World,
}

impl<'a> Names<'a> {
    fn new(all_notes: &'a [Value], world: &'a World) -> Names<'a> {
        let mut notes = BTreeMap::new();
        for n in all_notes {
            let t = title_of(n).to_lowercase();
            if !t.is_empty() {
                notes.entry(t).or_insert(n);
            }
        }
        Names { notes, world }
    }

    /// A note by its title, else a course by its code, else a task by its
    /// title (an open one before a done one).
    fn resolve(&self, target: &str) -> Target {
        let want = target.trim().to_lowercase();
        if let Some(n) = self.notes.get(&want) {
            return Target::Note(text(n, "id").to_string());
        }
        let code = squash(target);
        if let Some(c) = self.world.courses.iter().find(|c| squash(&c.code) == code) {
            return Target::Course(c.id.clone());
        }
        let named = |done: bool| {
            self.world
                .tasks
                .iter()
                .find(|t| t.done == done && t.title.trim().to_lowercase() == want)
        };
        match named(false).or_else(|| named(true)) {
            Some(t) => Target::Task(t.id.clone()),
            None => Target::Missing,
        }
    }
}

/// `heat.note.link {id, to}`: a link to a note, a course or a task, added
/// at the end of the note. A name nothing has becomes a new, empty note.
pub fn link(store: &mut Store, clock: &Clock, id: &str, to: &str, actor: Actor) -> Result<Outcome> {
    let old = find(store, id)?;
    let to = to.trim();
    if to.is_empty() {
        return refused("Say what to link to.");
    }
    let world = World::load(store)?;
    let all_notes = notes(store)?;
    let names = Names::new(&all_notes, &world);
    let (target, made) = match names.resolve(to) {
        Target::Note(nid) => (
            all_notes
                .iter()
                .find(|n| n["id"] == nid.as_str())
                .map(title_of)
                .unwrap_or_else(|| to.to_string()),
            None,
        ),
        Target::Course(cid) => (
            world
                .courses
                .iter()
                .find(|c| c.id == cid)
                .map_or_else(|| to.to_string(), |c| c.code.clone()),
            None,
        ),
        Target::Task(tid) => (
            world
                .tasks
                .iter()
                .find(|t| t.id == tid)
                .map_or_else(|| to.to_string(), |t| t.title.clone()),
            None,
        ),
        Target::Missing => (
            to.to_string(),
            Some(finished(
                &world,
                clock,
                None,
                json!({"title": to, "markdown": ""}),
            )?),
        ),
    };
    let mut record = old.clone();
    record["markdown"] = json!(rules::add_link(text(&old, "markdown"), &target));
    let note = finished(&world, clock, Some(&old), record)?;
    let c = commit(store, "link note", actor, |txn| {
        if let Some(m) = &made {
            put(txn, kind::NOTE, m)?;
        }
        put(txn, kind::NOTE, &note)
    })?;
    Ok(Outcome::new(
        json!({"note": note, "created": made, "line": format!("Linked to {target}")}),
        c,
    ))
}

/// The space a note's task goes in: the note's own, else the one grouped
/// by course when the note is a course's, else the first.
fn task_space(world: &World, note: &Value) -> Option<String> {
    if let Some(s) = note["spaceId"].as_str() {
        return Some(s.to_string());
    }
    let by_course = world
        .raw_spaces
        .iter()
        .find(|s| s["groupKind"] == "course")
        .and_then(|s| s["id"].as_str());
    match (note["courseId"].as_str(), by_course) {
        (Some(_), Some(s)) => Some(s.to_string()),
        _ => world.spaces.first().map(|s| s.id.clone()),
    }
}

fn new_task(
    world: &World,
    clock: &Clock,
    note: &Value,
    title: &str,
    due: Option<f64>,
) -> Result<Value> {
    let Some(space) = task_space(world, note) else {
        return refused("Add a space first, then make the task.");
    };
    let mut task = json!({
        "spaceId": space, "title": title.replace("[[", "").replace("]]", ""),
        "notes": format!("From the note [[{}]].", title_of(note)), "noteId": note["id"], "source": "you",
    });
    if let Some(c) = note["courseId"].as_str() {
        task["courseId"] = json!(c);
    }
    if let Some(d) = due {
        task["due"] = num(d);
    }
    let sp = spec_of(kind::TASK).expect("task");
    let done = schema::finish(
        sp,
        None,
        task.as_object().cloned().unwrap_or_default(),
        world,
        clock,
        &mut ulid,
    )?;
    Ok(Value::Object(done.record))
}

/// `heat.note.taskFromLine {id, line}`: a checkbox line becomes a task, and
/// the line keeps the task's id so the two stay linked. One entry.
pub fn task_from_line(store: &mut Store, clock: &Clock, id: &str, line: usize) -> Result<Outcome> {
    let old = find(store, id)?;
    let md = text(&old, "markdown");
    let Some(b) = rules::checkboxes(md).into_iter().find(|b| b.line == line) else {
        return refused("That line isn't a checkbox.");
    };
    if b.task_id.is_some() {
        return refused("That line is a task already.");
    }
    if b.text.is_empty() {
        return refused("Write what the task is first.");
    }
    let world = World::load(store)?;
    let task = new_task(&world, clock, &old, &b.text, None)?;
    let Some(linked) = rules::link_task(md, line, text(&task, "id")) else {
        return refused("That line isn't a checkbox.");
    };
    let mut record = old.clone();
    record["markdown"] = json!(linked);
    let note = finished(&world, clock, Some(&old), record)?;
    let c = commit(store, "task from note", Actor::You, |txn| {
        put(txn, kind::TASK, &task)?;
        put(txn, kind::NOTE, &note)
    })?;
    Ok(Outcome::new(json!({"task": task, "note": note}), c))
}

/// `heat.note.toggleBox {id, line, done}`: a checkbox ticked or cleared. A
/// line that is a task marks the task too, in the same entry.
pub fn toggle_box(
    store: &mut Store,
    clock: &Clock,
    id: &str,
    line: usize,
    done: bool,
) -> Result<Outcome> {
    let old = find(store, id)?;
    let md = text(&old, "markdown");
    let Some(b) = rules::checkboxes(md).into_iter().find(|b| b.line == line) else {
        return refused("That line isn't a checkbox.");
    };
    let world = World::load(store)?;
    let mut record = old.clone();
    if let Some(changed) = rules::set_checkbox(md, line, done) {
        record["markdown"] = json!(changed);
    }
    let note = finished(&world, clock, Some(&old), record)?;
    let task = match b.task_id.as_deref().and_then(|tid| world.task_index(tid)) {
        Some(i) if world.tasks[i].done != done => {
            let mut t = world.raw_tasks[i].clone();
            t["done"] = json!(done);
            t["doneAt"] = if done { num(clock.now_ms) } else { Value::Null };
            Some(t)
        }
        _ => None,
    };
    let label = if done { "check box" } else { "uncheck box" };
    let c = commit(store, label, Actor::You, |txn| {
        if let Some(t) = &task {
            put(txn, kind::TASK, t)?;
        }
        put(txn, kind::NOTE, &note)
    })?;
    Ok(Outcome::new(json!({"note": note, "task": task}), c))
}

/// `heat.note.search {q, limit?}`: notes that hold every word, best first.
pub fn search(store: &Store, q: &str, limit: usize) -> Result<Value> {
    let list: Vec<(String, String, String)> = notes(store)?
        .iter()
        .map(|n| {
            (
                text(n, "id").to_string(),
                title_of(n),
                text(n, "markdown").to_string(),
            )
        })
        .collect();
    Ok(json!({"hits": rules::search(&list, q, limit.clamp(1, 100))}))
}

// ----- the daily note -----

/// `heat.note.daily {date, markdown?}`: the note titled with the day. With
/// `markdown` it is written (made if it isn't there); without, it is read.
pub fn daily(
    store: &mut Store,
    clock: &Clock,
    date: &str,
    markdown: Option<&str>,
) -> Result<Outcome> {
    if !is_day(date) {
        return refused("A daily note needs a day, as YYYY-MM-DD.");
    }
    let existing = find(store, date).ok();
    match (existing, markdown) {
        (found, None) => Ok(Outcome::unchanged(json!({"note": found}))),
        (Some(old), Some(md)) => {
            let id = text(&old, "id").to_string();
            save(store, clock, &id, None, Some(md))
        }
        (None, Some(md)) if md.trim().is_empty() => Ok(Outcome::unchanged(json!({"note": null}))),
        (None, Some(md)) => {
            let world = World::load(store)?;
            let note = finished(&world, clock, None, json!({"title": date, "markdown": md}))?;
            let c = commit(store, "add daily note", Actor::You, |txn| {
                put(txn, kind::NOTE, &note)
            })?;
            Ok(Outcome::new(json!({"note": note}), c))
        }
    }
}

/// Daily notes from before Notes become notes titled with their day, once,
/// as one entry that one undo puts back. Whether there was anything to do.
pub fn bring_daily_notes(store: &mut Store, clock: &Clock) -> Result<Outcome> {
    const DONE: &str = "notes.daily";
    if crate::setting(store, DONE)?.is_some() {
        return Ok(Outcome::unchanged(json!({"moved": 0})));
    }
    // The usual case, and every new library's: there are none, and nothing
    // is written but that this was looked at.
    let old = all(store, kind::DAILY_NOTE)?;
    if old.is_empty() {
        crate::set_setting(store, DONE, &json!(true))?;
        return Ok(Outcome::unchanged(json!({"moved": 0})));
    }
    let world = World::load(store)?;
    let mut taken = titles(&notes(store)?, None);
    let mut made = Vec::new();
    let mut gone = Vec::new();
    for d in old {
        let date = text(&d, "date").to_string();
        if !is_day(&date) {
            continue;
        }
        gone.push(date.clone());
        if text(&d, "markdown").trim().is_empty() || taken.contains(&date) {
            continue;
        }
        taken.insert(date.clone());
        let mut note = finished(
            &world,
            clock,
            None,
            json!({"title": date, "markdown": d["markdown"]}),
        )?;
        // One that was public stays public.
        note["public"] = json!(d["public"] == true);
        made.push(note);
    }
    let c = commit(store, "move daily notes into Notes", Actor::You, |txn| {
        for n in &made {
            put(txn, kind::NOTE, n)?;
        }
        for date in &gone {
            txn.delete_doc(kind::DAILY_NOTE, date)?;
        }
        Ok(())
    })?;
    crate::set_setting(store, DONE, &json!(true))?;
    Ok(Outcome::new(json!({"moved": made.len()}), c))
}

// ----- a captured page -----

/// What a capture becomes: its pages' markdown, when it was taken, and the
/// files kept for it in the attachments folder.
#[derive(Clone, Debug, Default)]
pub struct Captured {
    pub markdown: String,
    pub captured_at: f64,
    pub attachments: Vec<String>,
}

/// "Capture · Oct 7, 10:22 AM": what a page is called until it is filed.
pub fn capture_title(clock: &Clock, at: f64) -> String {
    let day = clock.date_of(at);
    format!(
        "Capture · {}, {}",
        short_month_day(&day),
        fmt::clock(zone::minute_of_day(at, &clock.zone))
    )
}

/// A captured page as a note in the Notes inbox: one entry, "capture".
pub fn capture_add(store: &mut Store, clock: &Clock, captured: &Captured) -> Result<Outcome> {
    let world = World::load(store)?;
    let taken = titles(&notes(store)?, None);
    let title = rules::free_name(&capture_title(clock, captured.captured_at), &taken);
    let note = finished(
        &world,
        clock,
        None,
        json!({
            "title": title, "markdown": captured.markdown, "capturedAt": num(captured.captured_at),
            "attachments": captured.attachments, "inbox": true, "source": "capture",
        }),
    )?;
    let c = commit(store, "capture", Actor::You, |txn| {
        put(txn, kind::NOTE, &note)
    })?;
    Ok(Outcome::new(json!({"note": note}), c))
}

/// What Claude suggested for a page, kept beside it. Nothing is applied.
pub fn suggestions_set(
    store: &mut Store,
    note_id: &str,
    tasks: &[SuggestedTask],
    terms: &[String],
) -> Result<()> {
    if tasks.is_empty() && terms.is_empty() {
        return Ok(());
    }
    let record = json!({"noteId": note_id, "tasks": tasks, "terms": terms});
    store.set_doc(kind::NOTE_SUGGESTION, note_id, &record, "")?;
    Ok(())
}

/// `heat.note.suggestion.accept {noteId, index}`: one suggested task
/// becomes a task, by the person.
pub fn suggestion_accept(
    store: &mut Store,
    clock: &Clock,
    note_id: &str,
    index: usize,
) -> Result<Outcome> {
    let Some(mut s) = one(store, kind::NOTE_SUGGESTION, note_id)? else {
        return refused("That suggestion isn't there any more.");
    };
    let note = find(store, note_id)?;
    let Some(sug) = s["tasks"].get(index).cloned() else {
        return refused("That suggestion isn't there any more.");
    };
    if sug["taskId"].is_string() {
        return refused("That one is a task already.");
    }
    let world = World::load(store)?;
    let due = sug["due"]
        .as_str()
        .filter(|d| is_day(d))
        .map(|d| zone::at_minute(d, 23.0 * 60.0 + 59.0, &clock.zone));
    let task = new_task(&world, clock, &note, text(&sug, "title"), due)?;
    let c = commit(store, "add task", Actor::You, |txn| {
        put(txn, kind::TASK, &task)
    })?;
    s["tasks"][index]["taskId"] = task["id"].clone();
    store.set_doc(kind::NOTE_SUGGESTION, note_id, &s, "")?;
    Ok(Outcome::new(json!({"task": task}), c).also(&[kind::NOTE_SUGGESTION]))
}

/// `heat.note.suggestion.dismiss {noteId}`
pub fn suggestion_dismiss(store: &mut Store, note_id: &str) -> Result<Outcome> {
    store.remove_doc(kind::NOTE_SUGGESTION, note_id)?;
    Ok(Outcome::outside(json!({}), &[kind::NOTE_SUGGESTION]))
}

// ----- quiet notices -----

/// One line Learn shows quietly, with what can be done about it: an undo
/// (`txnId`), or one act (`{label, cmd, args}`). `key` makes a notice that
/// would say the same thing twice say it once.
pub fn notice_add(
    store: &mut Store,
    clock: &Clock,
    key: &str,
    kind_of: &str,
    text: &str,
    more: Value,
) -> Result<Value> {
    let mut record = json!({"id": key, "kind": kind_of, "text": text, "at": num(clock.now_ms)});
    if let (Some(r), Some(m)) = (record.as_object_mut(), more.as_object()) {
        for (k, v) in m {
            r.insert(k.clone(), v.clone());
        }
    }
    store.set_doc(kind::NOTICE, key, &record, "")?;
    Ok(record)
}

/// `heat.notice.dismiss {id}`
pub fn notice_dismiss(store: &mut Store, id: &str) -> Result<Outcome> {
    store.remove_doc(kind::NOTICE, id)?;
    Ok(Outcome::outside(json!({}), &[kind::NOTICE]))
}

/// Whether a notice with this key was ever made today: a "time to leave"
/// that was dismissed isn't said again.
pub fn notice_said(store: &Store, key: &str) -> Result<bool> {
    Ok(crate::setting(store, "notices.said")?
        .and_then(|v| v.get(key).cloned())
        .is_some())
}

/// Remembers that `key` was said, and forgets what was said before `today`.
pub fn notice_remember(store: &mut Store, key: &str, today: &str) -> Result<()> {
    let mut said: Map<String, Value> = crate::setting(store, "notices.said")?
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    said.retain(|_, day| day.as_str().is_some_and(|d| d >= today));
    said.insert(key.to_string(), json!(today));
    crate::set_setting(store, "notices.said", &Value::Object(said))
}

/// The notices to show, newest first; ones over a day old are left out.
pub fn notices(store: &Store, clock: &Clock) -> Result<Vec<Value>> {
    let mut list: Vec<Value> = all(store, kind::NOTICE)?
        .into_iter()
        .filter(|n| {
            n["at"]
                .as_f64()
                .is_some_and(|at| clock.now_ms - at < NOTICE_LASTS_MS)
        })
        .collect();
    list.sort_by(|a, b| {
        b["at"]
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&a["at"].as_f64().unwrap_or(0.0))
    });
    list.truncate(NOTICE_LIMIT);
    Ok(list)
}

// ----- the files on disk -----

/// What the core last saw of each note's file: `{path, hash, size, mtime}`
/// by note id. Outside the journal: it is bookkeeping, not a change.
pub fn file_states(store: &Store) -> Result<BTreeMap<String, Value>> {
    Ok(store
        .docs(kind::NOTE_FILE)?
        .into_iter()
        .map(|d| (d.key, d.json))
        .collect())
}

pub fn file_state_set(store: &mut Store, id: &str, state: &Value) -> Result<()> {
    store.set_doc(kind::NOTE_FILE, id, state, "")?;
    Ok(())
}

pub fn file_state_remove(store: &mut Store, id: &str) -> Result<()> {
    store.remove_doc(kind::NOTE_FILE, id)?;
    Ok(())
}

/// One thing that happened to a note's file outside the app.
#[derive(Clone, Debug)]
pub enum OnDisk {
    /// A file Learn has no note for.
    New {
        title: String,
        markdown: String,
        meta: rules::Meta,
    },
    /// A note's file was edited, or renamed.
    Changed {
        id: String,
        title: String,
        markdown: String,
        meta: rules::Meta,
    },
    /// A note's file is gone.
    Gone { id: String },
}

/// Changes made to the files outside the app, written into the library as
/// one entry, "notes from disk". Answers the ids of the notes made for new
/// files, in the order they came.
pub fn apply_disk(
    store: &mut Store,
    clock: &Clock,
    changes: &[OnDisk],
) -> Result<(Outcome, Vec<String>)> {
    let world = World::load(store)?;
    let all_notes = notes(store)?;
    let mut taken = titles(&all_notes, None);
    let mut writes: Vec<Value> = Vec::new();
    let mut gone: Vec<String> = Vec::new();
    let mut made: Vec<String> = Vec::new();
    let filed = |record: &mut Value, meta: &rules::Meta| {
        let Some(m) = record.as_object_mut() else {
            return;
        };
        m.remove("courseId");
        m.remove("spaceId");
        if let Some(c) = meta
            .course
            .as_deref()
            .and_then(|code| world.course(code).ok())
        {
            m.insert("courseId".into(), json!(c.id));
        }
        if let Some(s) = meta
            .space
            .as_deref()
            .and_then(|name| world.space(name).ok())
        {
            m.insert("spaceId".into(), json!(s.id));
        }
        if meta.inbox {
            m.insert("inbox".into(), json!(true));
        } else {
            m.remove("inbox");
        }
    };
    for change in changes {
        match change {
            OnDisk::New {
                title,
                markdown,
                meta,
            } => {
                let title = rules::free_name(title, &taken);
                taken.insert(title.to_lowercase());
                let mut record = json!({"title": title, "markdown": markdown});
                // A file that carries an id no note has keeps it: a note
                // deleted here and put back from a backup is the same note.
                if let Some(id) = meta
                    .id
                    .as_deref()
                    .filter(|id| !all_notes.iter().any(|n| n["id"] == *id))
                {
                    record["id"] = json!(id);
                }
                filed(&mut record, meta);
                let note = finished(&world, clock, None, record)?;
                made.push(text(&note, "id").to_string());
                writes.push(note);
            }
            OnDisk::Changed {
                id,
                title,
                markdown,
                meta,
            } => {
                let Some(old) = all_notes.iter().find(|n| n["id"] == id.as_str()) else {
                    continue;
                };
                let mut record = old.clone();
                record["markdown"] = json!(markdown);
                if title.to_lowercase() != title_of(old).to_lowercase() {
                    let others = titles(&all_notes, Some(id));
                    record["title"] = json!(rules::free_name(title, &others));
                }
                filed(&mut record, meta);
                let note = finished(&world, clock, Some(old), record)?;
                if &note != old {
                    writes.push(note);
                }
            }
            OnDisk::Gone { id } => gone.push(id.clone()),
        }
    }
    let c = commit(store, "notes from disk", Actor::You, |txn| {
        for n in &writes {
            put(txn, kind::NOTE, n)?;
        }
        for id in &gone {
            txn.delete_doc(kind::NOTE, id)?;
        }
        Ok(())
    })?;
    Ok((
        Outcome::new(json!({"changed": writes.len() + gone.len()}), c),
        made,
    ))
}

// ----- what the views read -----

/// The share of `heat.snapshot` that is Notes: for each note what it links
/// to and what links to it, its tags and checkboxes; the notes newest first;
/// the tags; what waits in the inbox; what Claude suggested; the notices.
pub(crate) fn for_snapshot(
    store: &Store,
    world: &World,
    clock: &Clock,
    snap: &mut Value,
) -> Result<()> {
    let all_notes = notes(store)?;
    let names = Names::new(&all_notes, world);
    let mut index = Map::new();
    let mut backlinks: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut tag_counts: BTreeMap<String, usize> = BTreeMap::new();
    for n in &all_notes {
        let (id, md) = (text(n, "id"), text(n, "markdown"));
        let lines: Vec<&str> = md.lines().collect();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut links = Vec::new();
        for l in rules::links(md) {
            let (kind_of, target_id) = match names.resolve(&l.target) {
                Target::Note(i) => ("note", Some(i)),
                Target::Course(i) => ("course", Some(i)),
                Target::Task(i) => ("task", Some(i)),
                Target::Missing => ("missing", None),
            };
            if kind_of == "note" {
                if let Some(to) = target_id
                    .as_deref()
                    .filter(|to| *to != id && seen.insert(to.to_string()))
                {
                    backlinks.entry(to.to_string()).or_default().push(json!({
                        "id": id, "title": title_of(n),
                        "line": rules::excerpt(lines.get(l.line).copied().unwrap_or(""), 160),
                    }));
                }
            }
            links.push(json!({"target": l.target, "shown": l.shown, "kind": kind_of, "id": target_id, "line": l.line}));
        }
        let tags = rules::tags(md);
        for t in &tags {
            *tag_counts.entry(t.clone()).or_default() += 1;
        }
        let boxes: Vec<Value> = rules::checkboxes(md)
            .into_iter()
            .map(|b| {
                let task_done = b
                    .task_id
                    .as_deref()
                    .and_then(|t| world.task_index(t))
                    .map(|i| world.tasks[i].done);
                // A line whose task is gone is a plain checkbox again.
                let linked = task_done.is_some();
                json!({
                    "line": b.line, "text": b.text, "done": task_done.unwrap_or(b.done),
                    "taskId": if linked { json!(b.task_id) } else { Value::Null },
                })
            })
            .collect();
        let course = n["courseId"]
            .as_str()
            .and_then(|c| world.courses.iter().find(|k| k.id == c));
        let space = n["spaceId"]
            .as_str()
            .and_then(|s| world.spaces.iter().find(|k| k.id == s));
        index.insert(
            id.to_string(),
            json!({
                "title": title_of(n), "excerpt": rules::excerpt(md, 140), "tags": tags, "links": links,
                "backlinks": [], "boxes": boxes,
                "updatedAt": n.get("updatedAt").cloned().unwrap_or(Value::Null),
                "label": course.map(|c| json!(course_label(&c.code, &c.name)))
                    .or_else(|| space.map(|s| json!(s.name)))
                    .unwrap_or(Value::Null),
                "hue": space.map_or(Value::Null, |s| num(s.hue)),
            }),
        );
    }
    for (to, from) in backlinks {
        if let Some(entry) = index.get_mut(&to) {
            entry["backlinks"] = Value::Array(from);
        }
    }
    // Newest first; a note from before notes kept their time goes by its id.
    let mut order: Vec<&Value> = all_notes.iter().collect();
    order.sort_by(|a, b| {
        let at = |n: &Value| n["updatedAt"].as_f64().unwrap_or(0.0);
        at(b)
            .total_cmp(&at(a))
            .then_with(|| text(b, "id").cmp(text(a, "id")))
    });
    let mut tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
    tags.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut suggestions = Map::new();
    for s in all(store, kind::NOTE_SUGGESTION)? {
        if let Some(id) = s["noteId"].as_str().filter(|id| index.contains_key(*id)) {
            suggestions.insert(
                id.to_string(),
                json!({"tasks": s["tasks"], "terms": s["terms"]}),
            );
        }
    }
    let today = snap["date"].as_str().unwrap_or_default().to_string();
    let daily = all_notes
        .iter()
        .find(|n| title_of(n) == today)
        .map(|n| n["id"].clone());
    snap["notes"] = json!({
        "index": index,
        "order": order.iter().map(|n| n["id"].clone()).collect::<Vec<_>>(),
        "tags": tags.iter().map(|(t, n)| json!({"tag": t, "count": n})).collect::<Vec<_>>(),
        "inbox": order.iter().filter(|n| n["inbox"] == true).map(|n| n["id"].clone()).collect::<Vec<_>>(),
        "suggestions": suggestions,
        "daily": daily.unwrap_or(Value::Null),
    });
    snap["notices"] = Value::Array(notices(store, clock)?);
    Ok(())
}
