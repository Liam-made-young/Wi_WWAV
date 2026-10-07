//! What the views write (docs/HEAT.md, the `heat.*` commands): each function
//! is one change in the journal, by the person, with the words the Edit menu
//! shows. The rules (what a record may hold, clamps, limits) are
//! `schema.rs`'s; the maths is `wi_heat::model`'s.

use serde_json::{json, Map, Value};
use wi_heat::model::focus::{check_off, set_took, CheckOff};
use wi_heat::model::import_artifact::{import_artifact, parse_heat_export, Existing};
use wi_heat::model::plan;
use wi_heat::model::records::Space;
use wi_heat::model::recurrence::recurs;
use wi_heat::model::spaces::default_spaces;
use wi_store::{Actor, Store};

use crate::derive::World;
use crate::schema::{self, edit_label, spec_of, words};
use crate::{all, commit, kind, num, one, put, refused, search_text, set_setting, setting, ulid, Clock, Committed, Outcome, Result};

/// 3.15's sentence for a grade's switch.
pub const GRADE_SWITCH: &str = "Grades are private by default. Your school keeps them as education records. Turning this on shows this grade to anyone who opens your sun.";

/// A Now making line clears itself after this long (3.15).
pub const NOW_LINES_LAST_MS: f64 = 7.0 * 86_400_000.0;

const NOW_LINE_MAX: usize = 200;

fn map_of(v: &Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn the_person() -> Actor {
    Actor::You
}

/// Puts a record the way the journal keeps it: the key is the record's own.
fn put_value(txn: &mut wi_store::Txn<'_>, k: &str, record: &Value) -> Result<()> {
    if k == kind::DAILY_NOTE {
        let date = record.get("date").and_then(Value::as_str).ok_or_else(|| crate::Error::Refused("A daily note needs its day.".into()))?;
        txn.put_doc(k, date, record, &search_text(k, record))?;
        return Ok(());
    }
    put(txn, k, record)
}

fn key_of<'a>(k: &str, record: &'a Value) -> Option<&'a str> {
    let field = if k == kind::DAILY_NOTE { "date" } else { "id" };
    record.get(field).and_then(Value::as_str)
}

/// Writes one record after `schema::finish`, as one entry.
fn write_record(
    store: &mut Store,
    clock: &Clock,
    sp: &schema::Spec,
    existing: Option<&Value>,
    candidate: Map<String, Value>,
    world: &World,
    label: Option<&str>,
) -> Result<(Committed, Value, bool)> {
    let done = schema::finish(sp, existing, candidate, world, clock, &mut ulid)?;
    let record = Value::Object(done.record);
    let label = match (label, existing) {
        (Some(l), _) => l.to_string(),
        (None, None) if sp.name == kind::BLOCK => "add block".to_string(),
        (None, None) => format!("add {}", words(sp.name)),
        (None, Some(old)) => edit_label(sp.name, old, &record),
    };
    let c = commit(store, &label, the_person(), |txn| put_value(txn, sp.name, &record))?;
    Ok((c, record, done.clamped))
}

/// `heat.put`: a record made or replaced, checked against its kind.
pub fn put_record(store: &mut Store, clock: &Clock, k: &str, record: &Value) -> Result<Outcome> {
    let sp = schema::writable(k)?;
    let Some(input) = record.as_object() else {
        return refused("A record is an object of its fields.");
    };
    let world = World::load(store)?;
    let key = key_of(k, record).map(str::to_string);
    let existing = match &key {
        Some(key) => one(store, k, key)?,
        None => None,
    };
    let (c, record, _) = write_record(store, clock, sp, existing.as_ref(), input.clone(), &world, None)?;
    Ok(Outcome::new(json!({ "record": record }), c))
}

/// `heat.patch`: some fields of a record changed, the rest untouched.
pub fn patch_record(store: &mut Store, clock: &Clock, k: &str, id: &str, set: &Map<String, Value>) -> Result<Outcome> {
    let sp = schema::writable(k)?;
    if set.contains_key("public") {
        return refused("The Public switch has its own command.");
    }
    if set.get(sp.key).is_some_and(|v| v.as_str() != Some(id)) {
        return refused("A record's id can't change.");
    }
    let Some(old) = one(store, k, id)? else {
        return refused(format!("No {} has that id.", words(k)));
    };
    let world = World::load(store)?;
    let mut candidate = map_of(&old);
    for (field, value) in set {
        candidate.insert(field.clone(), value.clone());
    }
    let (c, record, _) = write_record(store, clock, sp, Some(&old), candidate, &world, None)?;
    Ok(Outcome::new(json!({ "record": record }), c))
}

/// `heat.delete`: a record gone, with what belongs to it, as one entry.
pub fn delete_record(store: &mut Store, _clock: &Clock, k: &str, id: &str) -> Result<Outcome> {
    let sp = schema::writable(k)?;
    let Some(old) = one(store, k, id)? else {
        return refused(format!("No {} has that id.", words(k)));
    };
    let world = World::load(store)?;
    let mut gone: Vec<(&'static str, String)> = vec![(sp.name, id.to_string())];
    // Records that pointed at this one, rewritten without the pointer.
    let mut rewritten: Vec<(&'static str, Value)> = Vec::new();
    let shares = all(store, kind::SHARE)?;
    let without = |records: &[Value], field: &str, kind_of: &'static str, id: &str, out: &mut Vec<(&'static str, Value)>| {
        for r in records.iter().filter(|r| r.get(field).and_then(Value::as_str) == Some(id)) {
            let mut r = r.clone();
            if let Some(m) = r.as_object_mut() {
                m.remove(field);
            }
            out.push((kind_of, r));
        }
    };
    match k {
        kind::TASK => {
            gone.extend(world.blocks.iter().filter(|b| b.task_id.as_deref() == Some(id)).map(|b| (kind::BLOCK, b.id.clone())));
            gone.extend(world.occurrences.iter().filter(|o| o.task_id == id).map(|o| (kind::OCCURRENCE, o.id.clone())));
            gone.extend(
                shares
                    .iter()
                    .filter(|s| s["kind"] == "now" && s["sourceId"] == id)
                    .filter_map(|s| s["id"].as_str().map(|i| (kind::SHARE, i.to_string()))),
            );
            without(&world.raw_tasks, "parentTaskId", kind::TASK, id, &mut rewritten);
        }
        kind::HABIT => {
            gone.extend(world.blocks.iter().filter(|b| b.habit_id.as_deref() == Some(id)).map(|b| (kind::BLOCK, b.id.clone())));
        }
        kind::COURSE => {
            gone.extend(world.grades.iter().filter(|g| g.course_id == id).map(|g| (kind::GRADE, g.id.clone())));
            without(&world.raw_tasks, "courseId", kind::TASK, id, &mut rewritten);
        }
        kind::PROJECT => {
            gone.extend(world.milestones.iter().filter(|m| m.project_id.as_deref() == Some(id)).map(|m| (kind::MILESTONE, m.id.clone())));
            gone.extend(
                shares
                    .iter()
                    .filter(|s| s["kind"] == "timeline" && s["sourceId"] == id)
                    .filter_map(|s| s["id"].as_str().map(|i| (kind::SHARE, i.to_string()))),
            );
            without(&world.raw_tasks, "projectId", kind::TASK, id, &mut rewritten);
            without(&all(store, kind::NOTE)?, "projectId", kind::NOTE, id, &mut rewritten);
        }
        kind::MILESTONE => {
            without(&world.raw_tasks, "milestoneId", kind::TASK, id, &mut rewritten);
        }
        kind::SPACE => {
            let busy = world.tasks.iter().any(|t| t.space_id == id)
                || world.projects.iter().any(|p| p.space_id == id)
                || world.milestones.iter().any(|m| m.space_id == id);
            if busy {
                return refused("This space still holds tasks, projects or milestones. Move or delete them first.");
            }
        }
        kind::TERM if world.courses.iter().any(|c| c.term_id == id) => {
            return refused("This term still holds courses. Delete them first.");
        }
        _ => {}
    }
    let label = if k == kind::BLOCK { "remove block".to_string() } else { format!("delete {}", words(k)) };
    let c = commit(store, &label, the_person(), |txn| {
        for (kind_of, record) in &rewritten {
            put_value(txn, kind_of, record)?;
        }
        for (kind_of, key) in &gone {
            txn.delete_doc(kind_of, key)?;
        }
        Ok(())
    })?;
    let mut out = Outcome::new(json!({}), c);
    // A task feed item the person deletes stays deleted: the next sync
    // doesn't make it again.
    if k == kind::TASK && old["source"] == "ical" {
        if let Some(uid) = old.get("sourceId").and_then(Value::as_str) {
            crate::feed::dismiss(store, uid)?;
        }
    }
    // The current task and the plan's drafts forget a task that is gone.
    if k == kind::TASK {
        let mut state = crate::state(store)?;
        let mut changed = false;
        if state["currentTaskId"] == id {
            state["currentTaskId"] = Value::Null;
            changed = true;
        }
        if let Some(drafts) = state.get_mut("planDrafts").and_then(Value::as_array_mut) {
            let before = drafts.len();
            drafts.retain(|d| d["taskId"] != id);
            changed |= drafts.len() != before;
        }
        if changed {
            crate::set_state(store, &state)?;
            out = out.also(&[kind::STATE]);
        }
    }
    Ok(out)
}

/// `heat.done`: a task checked off, or back. A recurring task gets or loses
/// the tick of one day instead; its series never flips to done (3.6).
pub fn done(store: &mut Store, clock: &Clock, task_id: &str, make_done: bool, date: Option<&str>) -> Result<Outcome> {
    let world = World::load(store)?;
    let Some(i) = world.task_index(task_id) else {
        return refused("No task has that id.");
    };
    let task = world.tasks[i].clone();
    let raw = world.raw_tasks[i].clone();
    let (now, tz) = (clock.now_ms, &clock.zone);
    if let Some(day) = date {
        if !schema::is_day(day) {
            return refused("A day is written YYYY-MM-DD.");
        }
    }
    if recurs(&task) {
        let mine: Vec<_> = world.occurrences.iter().filter(|o| o.task_id == task_id).collect();
        if make_done {
            let (occurrence, message) = match date {
                Some(day) if mine.iter().any(|o| o.date == day) => return Ok(Outcome::unchanged(json!({ "task": raw }))),
                Some(day) => (
                    json!({"id": ulid(), "taskId": task_id, "date": day, "doneAt": num(now)}),
                    None,
                ),
                None => match check_off(&task, &world.sessions, &world.occurrences, now, tz, &mut ulid) {
                    CheckOff::Occurrence { occurrence, message } => (serde_json::to_value(&occurrence).unwrap_or(Value::Null), message),
                    _ => return refused("This series has ended. Nothing is left to check."),
                },
            };
            let c = commit(store, "mark done", the_person(), |txn| put(txn, kind::OCCURRENCE, &occurrence))?;
            let mut result = json!({ "task": raw });
            if let Some(m) = message {
                result["took"] = json!(m);
            }
            return Ok(Outcome::new(result, c));
        }
        // Not done: the tick on `date`, or the newest tick there is.
        let target = match date {
            Some(day) => mine.iter().find(|o| o.date == day).map(|o| o.id.clone()),
            None => mine.iter().max_by(|a, b| a.date.cmp(&b.date)).map(|o| o.id.clone()),
        };
        let Some(target) = target else {
            return Ok(Outcome::unchanged(json!({ "task": raw })));
        };
        let c = commit(store, "mark not done", the_person(), |txn| {
            txn.delete_doc(kind::OCCURRENCE, &target)?;
            Ok(())
        })?;
        return Ok(Outcome::new(json!({ "task": raw }), c));
    }

    let mut record = map_of(&raw);
    let mut took = None;
    if make_done {
        if task.done {
            return Ok(Outcome::unchanged(json!({ "task": raw })));
        }
        if let CheckOff::Done { message, .. } = check_off(&task, &world.sessions, &world.occurrences, now, tz, &mut ulid) {
            took = Some(message);
        }
        record.insert("done".into(), json!(true));
        record.insert("doneAt".into(), num(now));
    } else {
        if !task.done {
            return Ok(Outcome::unchanged(json!({ "task": raw })));
        }
        record.insert("done".into(), json!(false));
        record.insert("doneAt".into(), Value::Null);
    }
    let record = Value::Object(record);
    // A Now making line clears quietly when its task is done (3.15).
    let lines: Vec<String> = if make_done {
        all(store, kind::SHARE)?
            .iter()
            .filter(|s| s["kind"] == "now" && s["sourceId"] == task_id)
            .filter_map(|s| s["id"].as_str().map(String::from))
            .collect()
    } else {
        Vec::new()
    };
    let label = if make_done { "mark done" } else { "mark not done" };
    let c = commit(store, label, the_person(), |txn| {
        put(txn, kind::TASK, &record)?;
        for id in &lines {
            txn.delete_doc(kind::SHARE, id)?;
        }
        Ok(())
    })?;
    let mut result = json!({ "task": record });
    if let Some(m) = took {
        result["took"] = json!(m);
    }
    let mut out = Outcome::new(result, c);
    // The current task, once done, is no longer current.
    if make_done {
        let mut st = crate::state(store)?;
        if st["currentTaskId"] == task_id {
            st["currentTaskId"] = Value::Null;
            crate::set_state(store, &st)?;
            out = out.also(&[kind::STATE]);
        }
    }
    Ok(out)
}

/// `heat.estimate`: the person's difficulty or minutes, the same rules as
/// Claude's `update_task`, with the person as the actor.
pub fn estimate(
    store: &mut Store,
    clock: &Clock,
    task_id: &str,
    difficulty: Option<f64>,
    est_min: Option<Option<f64>>,
) -> Result<Outcome> {
    if difficulty.is_none() && est_min.is_none() {
        return refused("Give a difficulty, an estimate in minutes, or both.");
    }
    let sp = schema::writable(kind::TASK)?;
    let Some(old) = one(store, kind::TASK, task_id)? else {
        return refused("No task has that id.");
    };
    let world = World::load(store)?;
    let mut candidate = map_of(&old);
    if let Some(d) = difficulty {
        candidate.insert("difficulty".into(), num(d));
    }
    if let Some(m) = est_min {
        candidate.insert("estMin".into(), m.map_or(Value::Null, num));
    }
    let (c, record, clamped) = write_record(store, clock, sp, Some(&old), candidate, &world, Some("estimate"))?;
    let clamped = clamped || difficulty.is_some_and(|d| d.round().clamp(1.0, 5.0) != d);
    Ok(Outcome::new(json!({ "task": record, "clamped": clamped }), c))
}

/// `heat.tookTime`: Get Info's "Took", so the total reads `minutes`.
pub fn took_time(store: &mut Store, _clock: &Clock, task_id: &str, minutes: f64) -> Result<Outcome> {
    if !minutes.is_finite() || minutes < 0.0 {
        return refused("Minutes can't be less than 0.");
    }
    let world = World::load(store)?;
    let Some(i) = world.task_index(task_id) else {
        return refused("No task has that id.");
    };
    let set = set_took(&world.tasks[i], &world.sessions, minutes);
    let mut record = world.raw_tasks[i].clone();
    record["adjustMin"] = num(set.adjust_min);
    let c = commit(store, "change time taken", the_person(), |txn| put(txn, kind::TASK, &record))?;
    Ok(Outcome::new(json!({ "task": record }), c))
}

/// `heat.block.put`: a block made, moved or resized on the 15-minute grid.
pub fn put_block(store: &mut Store, clock: &Clock, args: &Map<String, Value>) -> Result<Outcome> {
    let sp = schema::writable(kind::BLOCK)?;
    let world = World::load(store)?;
    let id = args.get("id").and_then(Value::as_str).map(str::to_string);
    let old = match &id {
        Some(id) => one(store, kind::BLOCK, id)?,
        None => None,
    };
    let mut m = old.as_ref().map(map_of).unwrap_or_default();
    for field in ["id", "taskId", "habitId", "date"] {
        if let Some(v) = args.get(field).filter(|v| !v.is_null()) {
            m.insert(field.into(), v.clone());
        }
    }
    let number = |field: &str| args.get(field).and_then(Value::as_f64).or_else(|| m.get(field).and_then(Value::as_f64));
    let (Some(start), Some(minutes)) = (number("start"), number("minutes")) else {
        return refused("A block needs a start and a length in minutes.");
    };
    // The column runs from 7 AM to midnight in steps of 15 minutes (3.5).
    let (first, last) = (7.0 * 60.0, 24.0 * 60.0);
    let minutes = plan::snap(minutes).max(plan::SNAP_MIN);
    let start = plan::snap(start).clamp(first, (last - minutes).max(first));
    let minutes = minutes.min(last - start);
    m.insert("start".into(), num(start));
    m.insert("minutes".into(), num(minutes));
    let (c, record, _) = write_record(store, clock, sp, old.as_ref(), m, &world, None)?;
    Ok(Outcome::new(json!({ "block": record }), c))
}

/// "4 in inbox · captured ✓"
fn inbox_line(count: usize) -> String {
    format!("{count} in inbox · captured ✓")
}

/// `heat.capture.add`: a line kept for the Inbox, and the count line.
pub fn capture_add(store: &mut Store, _clock: &Clock, text: &str, link: Option<&Value>) -> Result<Outcome> {
    let text = text.trim();
    if text.is_empty() {
        return refused("Type something to capture.");
    }
    let mut record = json!({"id": ulid(), "text": text});
    if let Some(link) = link.filter(|l| !l.is_null()) {
        record["link"] = link.clone();
    }
    let waiting = all(store, kind::CAPTURE)?.iter().filter(|c| c.get("triagedAt").map_or(true, Value::is_null)).count() + 1;
    let c = commit(store, "capture", the_person(), |txn| put(txn, kind::CAPTURE, &record))?;
    Ok(Outcome::new(json!({ "capture": record, "inbox": inbox_line(waiting) }), c))
}

/// `heat.capture.triage`: a capture made into a task, note or project (or
/// marked as an upload), and out of the Inbox.
pub fn capture_triage(store: &mut Store, clock: &Clock, id: &str, to: &str, record: Option<&Map<String, Value>>) -> Result<Outcome> {
    let Some(capture) = one(store, kind::CAPTURE, id)? else {
        return refused("No capture has that id.");
    };
    if capture.get("triagedAt").is_some_and(|t| !t.is_null()) {
        return refused("That capture was triaged already.");
    }
    let world = World::load(store)?;
    let text = capture["text"].as_str().unwrap_or("").trim().to_string();
    let (first, rest) = match text.split_once('\n') {
        Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
        None => (text.clone(), String::new()),
    };
    let mut fields = record.cloned().unwrap_or_default();
    let first_space = world.spaces.first().map(|s| s.id.clone());
    let made: Option<(&'static str, Value)> = match to {
        "task" => {
            fields.entry("title").or_insert(json!(first));
            if !rest.is_empty() {
                fields.entry("notes").or_insert(json!(rest));
            }
            if let (Some(space), true) = (first_space, !fields.contains_key("spaceId")) {
                fields.insert("spaceId".into(), json!(space));
            }
            if let Some(link) = capture.get("link") {
                fields.entry("link").or_insert(link.clone());
            }
            let done = schema::finish(spec_of(kind::TASK).expect("task"), None, fields, &world, clock, &mut ulid)?;
            Some((kind::TASK, Value::Object(done.record)))
        }
        "note" => {
            fields.entry("markdown").or_insert(json!(text));
            if let Some(link) = capture.get("link") {
                fields.entry("link").or_insert(link.clone());
            }
            let done = schema::finish(spec_of(kind::NOTE).expect("note"), None, fields, &world, clock, &mut ulid)?;
            Some((kind::NOTE, Value::Object(done.record)))
        }
        "project" => {
            fields.entry("title").or_insert(json!(first));
            if let (Some(space), true) = (first_space, !fields.contains_key("spaceId")) {
                fields.insert("spaceId".into(), json!(space));
            }
            let done = schema::finish(spec_of(kind::PROJECT).expect("project"), None, fields, &world, clock, &mut ulid)?;
            Some((kind::PROJECT, Value::Object(done.record)))
        }
        // The file came into the library when it was dropped; this only
        // records that the capture is dealt with.
        "upload" => None,
        _ => return refused("Triage a capture to a task, note, project or upload."),
    };
    let mut triaged = capture.clone();
    triaged["triagedAt"] = num(clock.now_ms);
    triaged["resultType"] = json!(to);
    let result_id = match &made {
        Some((_, r)) => r["id"].as_str().map(str::to_string),
        None => record.and_then(|r| r.get("id")).and_then(Value::as_str).map(str::to_string),
    };
    if let Some(rid) = &result_id {
        triaged["resultId"] = json!(rid);
    }
    let c = commit(store, "triage capture", the_person(), |txn| {
        if let Some((k, r)) = &made {
            put(txn, k, r)?;
        }
        put(txn, kind::CAPTURE, &triaged)
    })?;
    let result = match made {
        Some((k, r)) => json!({"type": to, "kind": k, "id": result_id, "record": r}),
        None => json!({"type": to, "id": result_id}),
    };
    Ok(Outcome::new(json!({ "result": result }), c))
}

/// `heat.score`: the person types a score into a grade, pending or not. Only
/// the person does: no tool of Claude's has an argument for it (3.8).
pub fn score(store: &mut Store, _clock: &Clock, grade_id: &str, score: f64, out_of: Option<f64>) -> Result<Outcome> {
    if !score.is_finite() || score < 0.0 {
        return refused("A score is a number, 0 or more.");
    }
    let Some(old) = one(store, kind::GRADE, grade_id)? else {
        return refused("No grade has that id.");
    };
    let mut record = old.clone();
    record["score"] = num(score);
    record["pending"] = json!(false);
    if let Some(total) = out_of {
        if !total.is_finite() || total <= 0.0 {
            return refused("A grade is out of more than 0.");
        }
        record["outOf"] = num(total);
    }
    let c = commit(store, "enter score", the_person(), |txn| put(txn, kind::GRADE, &record))?;
    Ok(Outcome::new(json!({ "grade": record }), c))
}

/// `heat.public.set`: the Public switch of one record (3.15). Nothing else
/// turns it on: not put, not patch, not any tool of Claude's.
pub fn set_public(store: &mut Store, _clock: &Clock, k: &str, id: &str, public: bool) -> Result<Outcome> {
    if k == kind::MAIL {
        return refused("Mail is never public.");
    }
    let Some(sp) = spec_of(k).filter(|s| s.switch) else {
        return refused(format!("A {} has no Public switch.", words(k)));
    };
    let Some(old) = one(store, sp.name, id)? else {
        return refused(format!("No {} has that id.", words(k)));
    };
    let mut record = old.clone();
    record["public"] = json!(public);
    let label = if public { "make public" } else { "make private" };
    let c = commit(store, label, the_person(), |txn| put_value(txn, sp.name, &record))?;
    let mut result = json!({ "record": record });
    if k == kind::GRADE {
        result["sentence"] = json!(GRADE_SWITCH);
    }
    Ok(Outcome::new(result, c))
}

/// `heat.share.now`: the Now making line, after Show. One line at a time; it
/// clears when its task is done, or after 7 days.
pub fn share_now(store: &mut Store, clock: &Clock, task_id: &str, text: &str) -> Result<Outcome> {
    let world = World::load(store)?;
    if !world.tasks.iter().any(|t| t.id == task_id && !t.done) {
        return refused("Pick an open task to show.");
    }
    let text = text.trim();
    if text.is_empty() {
        return refused("Write the line first.");
    }
    if text.chars().count() > NOW_LINE_MAX {
        return refused(format!("Keep the line to {NOW_LINE_MAX} characters."));
    }
    let shares = all(store, kind::SHARE)?;
    // Showing a new line replaces the one on show.
    let id = shares.iter().find(|s| s["kind"] == "now").and_then(|s| s["id"].as_str()).map_or_else(ulid, str::to_string);
    let share = json!({
        "id": id, "kind": "now", "sourceId": task_id, "text": text, "targetId": "sun",
        "clearsAt": num(clock.now_ms + NOW_LINES_LAST_MS),
    });
    let c = commit(store, "show Now making", the_person(), |txn| put(txn, kind::SHARE, &share))?;
    Ok(Outcome::new(json!({ "share": share }), c))
}

/// `heat.share.timeline`: a project's timeline on its solar system.
pub fn share_timeline(store: &mut Store, _clock: &Clock, project_id: &str, target_id: &str) -> Result<Outcome> {
    let world = World::load(store)?;
    if !world.projects.iter().any(|p| p.id == project_id) {
        return refused("Pick a project to show.");
    }
    let target = target_id.trim();
    if target.is_empty() {
        return refused("Drop the project on one of your solar systems.");
    }
    let shares = all(store, kind::SHARE)?;
    let id = shares
        .iter()
        .find(|s| s["kind"] == "timeline" && s["sourceId"] == project_id)
        .and_then(|s| s["id"].as_str())
        .map_or_else(ulid, str::to_string);
    let share = json!({"id": id, "kind": "timeline", "sourceId": project_id, "targetId": target});
    let c = commit(store, "show timeline", the_person(), |txn| put(txn, kind::SHARE, &share))?;
    Ok(Outcome::new(json!({ "share": share }), c))
}

/// `heat.share.hide`: a line or a timeline back off the public view.
pub fn share_hide(store: &mut Store, _clock: &Clock, id: &str) -> Result<Outcome> {
    let Some(share) = one(store, kind::SHARE, id)? else {
        return refused("That isn't on your public Heat view.");
    };
    let label = if share["kind"] == "now" { "hide Now making" } else { "hide timeline" };
    let c = commit(store, label, the_person(), |txn| {
        txn.delete_doc(kind::SHARE, id)?;
        Ok(())
    })?;
    Ok(Outcome::new(json!({}), c))
}

/// `heat.review.complete`: the note the person wrote, kept. Heat never writes
/// it for them (3.14).
pub fn review_complete(store: &mut Store, _clock: &Clock, week_start: &str, note: &str) -> Result<Outcome> {
    if !schema::is_day(week_start) {
        return refused("A week starts on a day, written YYYY-MM-DD.");
    }
    let markdown = note.trim();
    if markdown.is_empty() {
        return refused("Write your note first.");
    }
    // One note for each week: pressing the button again changes it.
    let id = format!("review-{week_start}");
    let old = one(store, kind::NOTE, &id)?;
    let mut record = old.clone().unwrap_or_else(|| json!({"id": id, "public": false}));
    record["title"] = json!(format!("Weekly review, week of {}", wi_heat::model::format::short_month_day(week_start)));
    record["markdown"] = json!(markdown);
    let c = commit(store, "weekly review", the_person(), |txn| put(txn, kind::NOTE, &record))?;
    Ok(Outcome::new(json!({ "note": record }), c))
}

fn values<T: serde::Serialize>(v: &[T]) -> Vec<Value> {
    v.iter().filter_map(|r| serde_json::to_value(r).ok()).collect()
}

/// With a Public switch off, for a record built here.
fn private(k: &str, mut v: Value) -> Value {
    if spec_of(k).is_some_and(|s| s.switch) {
        if let Some(m) = v.as_object_mut() {
            m.entry("public").or_insert(json!(false));
        }
    }
    v
}

/// `heat.import`: moving in from the artifact's JSON (3.16). Every id is
/// kept, so the first sync finds nothing new; everything comes in private;
/// a record already here is left as it is.
pub fn import(store: &mut Store, _clock: &Clock, json_in: &Value) -> Result<Outcome> {
    let dump = parse_heat_export(json_in).map_err(|e| crate::Error::Refused(e.error))?;
    let world = World::load(store)?;
    let existing = Existing {
        spaces: Some(world.spaces.clone()),
        terms: Some(world.terms.clone()),
        courses: Some(world.courses.clone()),
    };
    let imported = import_artifact(&dump, &existing, &mut ulid);
    let mut counts = Map::new();
    let mut writes: Vec<(&'static str, Value)> = Vec::new();
    let mut queue = |k: &'static str, list: Vec<Value>| -> Result<()> {
        let mut n = 0;
        for record in list {
            let key = key_of(k, &record).unwrap_or_default().to_string();
            if one(store, k, &key)?.is_some() {
                continue; // moved in already
            }
            writes.push((k, private(k, record)));
            n += 1;
        }
        counts.insert(k.to_string(), json!(n));
        Ok(())
    };
    queue(kind::SPACE, values(&imported.spaces))?;
    queue(kind::TERM, values(&imported.terms))?;
    queue(kind::COURSE, values(&imported.courses))?;
    // The ids of tasks that came through mail or the calendar are what Claude
    // and the feed match against: they become `sourceId`s too (3.16).
    let tasks: Vec<Value> = imported
        .tasks
        .iter()
        .filter_map(|t| serde_json::to_value(t).ok())
        .map(|mut t| {
            if matches!(t["source"].as_str(), Some("mail" | "calendar")) && t.get("sourceId").is_none() {
                t["sourceId"] = t["id"].clone();
            }
            t
        })
        .collect();
    queue(kind::TASK, tasks)?;
    queue(kind::MILESTONE, values(&imported.milestones))?;
    queue(kind::HABIT, values(&imported.habits))?;
    queue(kind::GRADE, values(&imported.grades))?;
    let c = commit(store, "move in", the_person(), |txn| {
        for (k, r) in &writes {
            put(txn, k, r)?;
        }
        Ok(())
    })?;
    if !imported.sync.processed_mail_ids.is_empty() {
        set_setting(store, "mailIds", &json!(imported.sync.processed_mail_ids))?;
    }
    Ok(Outcome::new(json!({ "counts": counts }), c))
}

/// Makes Classes, WWAV and Personal the first time Heat opens, once, so a
/// task has a space to be in (3.4). They are made outside the journal, so
/// ⌘Z never takes the person's own spaces away, and with the same ids on
/// every Mac, so two Macs signing in to one account don't each bring their own.
/// Returns the records made, for sync to carry.
pub fn ensure_defaults(store: &mut Store) -> Result<Vec<(&'static str, Value)>> {
    if setting(store, "seeded")?.is_some() {
        return Ok(Vec::new());
    }
    let mut made = Vec::new();
    if all(store, kind::SPACE)?.is_empty() {
        let mut ids = ["classes", "wwav", "personal"].into_iter();
        let spaces: Vec<Space> = default_spaces(&mut || ids.next().unwrap_or("space").to_string());
        for space in spaces {
            let record = private(kind::SPACE, serde_json::to_value(&space).unwrap_or(Value::Null));
            store.set_doc(kind::SPACE, &space.id, &record, &search_text(kind::SPACE, &record))?;
            made.push((kind::SPACE, record));
        }
    }
    set_setting(store, "seeded", &json!(true))?;
    Ok(made)
}
