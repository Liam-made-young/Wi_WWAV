//! Calendars and Brightspace (docs/SPEC.md 3.11): what a feed the core
//! fetched becomes in Heat. Brightspace's due items become tasks by
//! `wi_heat::brightspace`'s rules, the VEVENT UID as the task id; the
//! other calendars' events become `calendarEvent` records, read-only, which
//! never make tasks. The network and the Keychain are the core's: this crate
//! sees only the parsed feed.
//!
//! A sync that changes a task is one journal entry, "calendar sync", so ⌘Z
//! takes it back; a sync that changes nothing leaves none. Events and each
//! feed's bookkeeping sit outside the journal.

use jiff::tz::TimeZone;
use jiff::Timestamp;
use serde_json::{json, Map, Value};
use wi_heat::brightspace::{
    sync_feed, FeedItem, Known, School, DEFAULT_COURSE_PATTERN, NO_LONGER_IN_BRIGHTSPACE,
};
use wi_heat::ical::{Event, When};
use wi_heat::model::records::TaskSource;
use wi_heat::model::zone;
use wi_store::{Actor, Store};

use crate::derive::World;
use crate::schema::is_day;
use crate::{
    all, commit, kind, num, one, refused, search_text, set_setting, setting, ulid, Clock, Result,
};

/// The tag a task gets when its item has been missing from two syncs.
pub const GONE_TAG: &str = NO_LONGER_IN_BRIGHTSPACE;

/// Events are kept from a week back to this many days ahead.
const EVENTS_AHEAD_DAYS: f64 = 90.0;
const EVENTS_BACK_DAYS: f64 = 7.0;

/// The School sheet (3.11): what Settings → Heat holds beyond the links.
/// Nothing in it is a secret: the Brightspace address is in the Keychain.
pub fn school_sheet(store: &Store) -> Result<Value> {
    Ok(setting(store, "school")?.unwrap_or_else(|| {
        json!({"name": "", "host": "", "codePattern": DEFAULT_COURSE_PATTERN, "termStart": null, "termEnd": null})
    }))
}

/// `heat.school.set` for the part that isn't the address: checked, kept.
pub fn set_school(store: &mut Store, args: &Map<String, Value>) -> Result<()> {
    let text = |k: &str| {
        args.get(k)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let pattern = match text("codePattern") {
        p if p.is_empty() => DEFAULT_COURSE_PATTERN.to_string(),
        p => p,
    };
    if School::new(&text("host"), &pattern, TimeZone::UTC).is_err() {
        return refused(
            "The course pattern isn't one Learn can read. The default is ^([A-Z]{3})\\s?(\\d{3}).",
        );
    }
    let day = |k: &str| -> Result<Value> {
        match args
            .get(k)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            None => Ok(Value::Null),
            Some(d) if is_day(d) => Ok(json!(d)),
            Some(_) => refused("The term's days are written YYYY-MM-DD."),
        }
    };
    let (start, end) = (day("termStart")?, day("termEnd")?);
    if let (Some(a), Some(b)) = (start.as_str(), end.as_str()) {
        if a > b {
            return refused("The term can't end before it starts.");
        }
    }
    set_setting(
        store,
        "school",
        &json!({"name": text("name"), "host": text("host"), "codePattern": pattern, "termStart": start, "termEnd": end}),
    )
}

/// The School as the feed rules read it, in the person's zone.
pub fn school(store: &Store, zone: &TimeZone) -> Result<School> {
    let sheet = school_sheet(store)?;
    let host = sheet["host"].as_str().unwrap_or("");
    let pattern = sheet["codePattern"]
        .as_str()
        .filter(|p| !p.is_empty())
        .unwrap_or(DEFAULT_COURSE_PATTERN);
    match School::new(host, pattern, zone.clone()) {
        Ok(s) => Ok(s),
        Err(_) => refused("The course pattern in Settings → Learn isn't one Learn can read."),
    }
}

/// Every calendar, as `calendar` records, in the order they were added.
pub fn calendars(store: &Store) -> Result<Vec<Value>> {
    all(store, kind::CALENDAR)
}

/// A calendar record for a new address: checked, with an id and the name of
/// the Keychain item that will hold the address. Not stored yet: the core
/// puts the address in the Keychain first.
pub fn new_calendar(store: &Store, name: &str, kind_of: &str) -> Result<Value> {
    let name = name.trim();
    if name.is_empty() {
        return refused("Give the calendar a name first.");
    }
    if !matches!(kind_of, "brightspace" | "ical") {
        return refused("A calendar is a Brightspace calendar or another iCal address.");
    }
    if kind_of == "brightspace" && calendars(store)?.iter().any(|c| c["kind"] == "brightspace") {
        return refused(
            "There is a Brightspace calendar already. Remove it first, then add the new link.",
        );
    }
    let id = ulid();
    Ok(
        json!({"id": id, "name": name, "kind": kind_of, "keychainRef": format!("Heat calendar {id}"), "lastSyncedAt": null}),
    )
}

/// Keeps a calendar record. Outside the journal: its address isn't an edit,
/// and its `lastSyncedAt` moves each sync.
pub fn save_calendar(store: &mut Store, record: &Value) -> Result<()> {
    let id = record["id"].as_str().unwrap_or_default();
    store.set_doc(
        kind::CALENDAR,
        id,
        record,
        record["name"].as_str().unwrap_or(""),
    )?;
    Ok(())
}

/// A calendar and its events and bookkeeping gone. The tasks Brightspace made
/// stay: nothing a feed made is ever deleted for the person.
pub fn remove_calendar(store: &mut Store, id: &str) -> Result<Value> {
    let Some(record) = one(store, kind::CALENDAR, id)? else {
        return refused("That calendar isn't in Learn any more.");
    };
    let prefix = format!("{id}/");
    for d in store.docs(kind::EVENT)? {
        if d.key.starts_with(&prefix) {
            store.remove_doc(kind::EVENT, &d.key)?;
        }
    }
    store.remove_doc(kind::CALENDAR, id)?;
    store.remove_doc(kind::SETTING, &format!("feed.{id}"))?;
    Ok(record)
}

/// A sync that read the feed: when, so Heat syncs again an hour on.
pub fn mark_synced(store: &mut Store, id: &str, at_ms: f64) -> Result<()> {
    if let Some(mut record) = one(store, kind::CALENDAR, id)? {
        record["lastSyncedAt"] = num(at_ms);
        save_calendar(store, &record)?;
    }
    Ok(())
}

/// A feed item the person deleted: the next sync doesn't make it again.
pub fn dismiss(store: &mut Store, uid: &str) -> Result<()> {
    let mut map = setting(store, "feedDismissed")?
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    map.insert(uid.to_string(), json!(true));
    set_setting(store, "feedDismissed", &Value::Object(map))
}

fn ms_of(t: Timestamp) -> f64 {
    t.as_millisecond() as f64
}

fn stamp(ms: f64) -> Option<Timestamp> {
    Timestamp::from_millisecond(ms as i64).ok()
}

fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase()
}

/// "Fall 2026", from a day: the term a first sync makes when there is none.
fn term_name(day: &str) -> String {
    let (year, month, _) = zone::key_parts(day);
    let season = match month as i32 {
        1..=5 => "Spring",
        6..=7 => "Summer",
        _ => "Fall",
    };
    format!("{season} {}", year as i64)
}

/// What a Brightspace sync changed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeedChanges {
    pub new_tasks: usize,
    pub date_changes: usize,
    /// Tasks tagged "No longer in Brightspace" by this sync, and tags cleared.
    pub gone: usize,
    pub back: usize,
    /// The journal entry, when the sync changed a task.
    pub txn: Option<String>,
}

/// One read of the Brightspace feed, brought into the tasks: new items made,
/// dates followed, a Google Calendar duplicate taking the UID, and an item
/// missing from two syncs in a row tagged and never deleted (3.11).
pub fn apply_brightspace(
    store: &mut Store,
    clock: &Clock,
    calendar_id: &str,
    items: &[FeedItem],
) -> Result<FeedChanges> {
    let world = World::load(store)?;
    let state_key = format!("feed.{calendar_id}");
    let saved = setting(store, &state_key)?.unwrap_or_else(|| json!({}));
    let missed: Map<String, Value> = saved
        .get("missed")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let dismissed: Map<String, Value> = setting(store, "feedDismissed")?
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let code_of = |course_id: &Option<String>| -> Option<String> {
        course_id
            .as_ref()
            .and_then(|id| world.courses.iter().find(|c| &c.id == id))
            .map(|c| c.code.clone())
    };

    // The tasks the feed follows: its own (by UID), and tasks that came through
    // Google Calendar before and have no UID yet, for the duplicate test.
    let mut known: Vec<Known> = Vec::new();
    let mut at: Vec<usize> = Vec::new();
    for (i, t) in world.tasks.iter().enumerate() {
        let raw = &world.raw_tasks[i];
        let uid = match t.source {
            TaskSource::Ical => Some(t.source_id.clone().unwrap_or_else(|| t.id.clone())),
            // Through Google Calendar before (the artifact's own path): no UID yet.
            TaskSource::Calendar => None,
            _ => continue,
        };
        known.push(Known {
            id: t.id.clone(),
            uid: uid.clone(),
            title: t.title.clone(),
            course: code_of(&t.course_id),
            due: t.due.and_then(stamp),
            done: t.done,
            missed: uid
                .as_deref()
                .and_then(|u| missed.get(u))
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .min(255) as u8,
            gone: raw["tag"] == GONE_TAG,
        });
        at.push(i);
    }
    let followed = |uid: &str, known: &[Known]| known.iter().any(|k| k.uid.as_deref() == Some(uid));
    let items: Vec<FeedItem> = items
        .iter()
        .filter(|it| !dismissed.contains_key(&it.uid) || followed(&it.uid, &known))
        .cloned()
        .collect();
    let Some(now) = stamp(clock.now_ms) else {
        return refused("Learn can't read the clock.");
    };
    let report = sync_feed(&mut known, &items, now, &clock.zone);

    // What changes in tasks, as records to write.
    let mut writes: Vec<(&'static str, Value)> = Vec::new();
    let patch = |id: &str, f: &dyn Fn(&mut Value)| -> Option<Value> {
        let i = world.task_index(id)?;
        let mut r = world.raw_tasks[i].clone();
        f(&mut r);
        Some(r)
    };
    for change in &report.date_changes {
        if let Some(r) = patch(&change.task_id, &|r| r["due"] = num(ms_of(change.to))) {
            writes.push((kind::TASK, r));
        }
    }
    for id in &report.took_uid {
        let Some(k) = known.iter().find(|k| &k.id == id) else {
            continue;
        };
        let (uid, due) = (k.uid.clone(), k.due);
        if let Some(r) = patch(id, &|r| {
            r["sourceId"] = json!(uid);
            r["source"] = json!("ical");
            if let Some(d) = due {
                r["due"] = num(ms_of(d));
            }
        }) {
            writes.push((kind::TASK, r));
        }
    }
    for id in &report.gone {
        if let Some(r) = patch(id, &|r| r["tag"] = json!(GONE_TAG)) {
            writes.push((kind::TASK, r));
        }
    }
    for id in &report.back {
        if let Some(r) = patch(id, &|r| {
            r.as_object_mut().map(|m| m.remove("tag"));
        }) {
            writes.push((kind::TASK, r));
        }
    }

    // New items become tasks in the space that groups by course, each in its
    // course, which is made (once) if no course of that code is there.
    let space = world
        .spaces
        .iter()
        .find(|s| s.group_kind == wi_heat::model::records::GroupKind::Course)
        .or_else(|| world.spaces.first());
    let mut made_courses: Vec<Value> = Vec::new();
    let mut made_term: Option<Value> = None;
    if !report.new.is_empty() && space.is_none() {
        return refused("There's no space yet. Open Learn in Wi_WWAV once.");
    }
    for item in &report.new {
        let space = space.expect("checked above");
        let mut course_id: Option<String> = None;
        if let Some(code) = &item.course {
            let want = squash(code);
            let found = world
                .courses
                .iter()
                .map(|c| (c.id.as_str(), c.code.as_str()))
                .chain(made_courses.iter().map(|c| {
                    (
                        c["id"].as_str().unwrap_or(""),
                        c["code"].as_str().unwrap_or(""),
                    )
                }))
                .find(|(_, c)| squash(c) == want)
                .map(|(id, _)| id.to_string());
            course_id = match found {
                Some(id) => Some(id),
                None => {
                    let term = match world.terms.last() {
                        Some(t) => t.id.clone(),
                        None => {
                            let t = made_term.get_or_insert_with(
                                || json!({"id": ulid(), "name": term_name(&clock.today())}),
                            );
                            t["id"].as_str().unwrap_or_default().to_string()
                        }
                    };
                    let course = json!({"id": ulid(), "termId": term, "code": code, "name": code, "categories": [], "notes": "", "public": false});
                    let id = course["id"].as_str().map(str::to_string);
                    made_courses.push(course);
                    id
                }
            };
        }
        let mut notes = item.notes.clone();
        if let Some(url) = &item.url {
            if !notes.contains(url.as_str()) {
                if !notes.is_empty() {
                    notes.push('\n');
                }
                notes.push_str(url);
            }
        }
        let mut task = json!({
            "id": item.uid, "spaceId": space.id, "title": item.title, "type": item.kind,
            "due": num(ms_of(item.due)), "difficulty": 3, "estMin": null, "estBy": "default", "adjustMin": 0,
            "notes": notes, "done": false, "doneAt": null, "source": "ical", "sourceId": item.uid, "public": false,
        });
        if let Some(id) = course_id {
            task["courseId"] = json!(id);
        }
        writes.push((kind::TASK, task));
    }
    let mut first: Vec<(&'static str, Value)> = Vec::new();
    if let Some(t) = made_term {
        first.push((kind::TERM, t));
    }
    first.extend(made_courses.into_iter().map(|c| (kind::COURSE, c)));
    first.extend(writes);
    let writes = first;
    let c = commit(store, "calendar sync", Actor::You, |txn| {
        for (k, r) in &writes {
            let key = r["id"].as_str().unwrap_or_default();
            txn.put_doc(k, key, r, &search_text(k, r))?;
        }
        Ok(())
    })?;

    // The count of syncs an item has been missing from, kept outside the journal.
    let mut now_missed = Map::new();
    for k in &known {
        if let (Some(uid), true) = (&k.uid, k.missed > 0) {
            now_missed.insert(uid.clone(), json!(k.missed));
        }
    }
    set_setting(store, &state_key, &json!({ "missed": now_missed }))?;
    Ok(FeedChanges {
        new_tasks: report.new.len(),
        date_changes: report.date_changes.len(),
        gone: report.gone.len(),
        back: report.back.len(),
        txn: c.txn,
    })
}

fn when_ms(w: When, clock: &Clock) -> (f64, bool) {
    match w {
        When::At(t) => (ms_of(t), false),
        When::Date(d) => {
            let key = format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day());
            (zone::start_of_day(&key, &clock.zone), true)
        }
    }
}

/// The events of one other calendar, replacing what it held: cancelled ones
/// left out, and only a stretch around today kept. Returns how many are kept.
pub fn replace_events(
    store: &mut Store,
    clock: &Clock,
    calendar_id: &str,
    events: &[Event],
) -> Result<usize> {
    let from = clock.now_ms - EVENTS_BACK_DAYS * 86_400_000.0;
    let to = clock.now_ms + EVENTS_AHEAD_DAYS * 86_400_000.0;
    let mut keep: Vec<(String, Value)> = Vec::new();
    for e in events {
        if e.status.as_deref() == Some("CANCELLED") {
            continue;
        }
        let Some(start) = e.start.or(e.due) else {
            continue;
        };
        let (start_ms, all_day) = when_ms(start, clock);
        let end_ms = match e.end {
            Some(end) => when_ms(end, clock).0,
            None if all_day => start_ms + 86_400_000.0,
            None => start_ms + 3_600_000.0,
        }
        .max(start_ms);
        if end_ms < from || start_ms > to {
            continue;
        }
        let uid = e
            .uid
            .clone()
            .unwrap_or_else(|| format!("{}-{}", e.summary, start_ms));
        let key = format!("{calendar_id}/{uid}");
        let title = if e.summary.trim().is_empty() {
            "(No title)".to_string()
        } else {
            e.summary.trim().to_string()
        };
        keep.push((
            key.clone(),
            json!({"id": key, "calendarId": calendar_id, "title": title, "start": num(start_ms), "end": num(end_ms), "allDay": all_day}),
        ));
    }
    let prefix = format!("{calendar_id}/");
    for d in store.docs(kind::EVENT)? {
        if d.key.starts_with(&prefix) && !keep.iter().any(|(k, _)| *k == d.key) {
            store.remove_doc(kind::EVENT, &d.key)?;
        }
    }
    for (key, record) in &keep {
        store.set_doc(
            kind::EVENT,
            key,
            record,
            record["title"].as_str().unwrap_or(""),
        )?;
    }
    Ok(keep.len())
}
