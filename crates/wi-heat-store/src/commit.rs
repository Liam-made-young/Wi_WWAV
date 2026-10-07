//! Commitments in the library (docs/COMMITMENTS.md): the fixed things in a
//! week, the breaks classes skip, a schedule read and waiting to be applied,
//! and an exception a mail asked for and nobody has confirmed yet.
//!
//! - A `commitment` and a `termBreak` are journaled records, so ⌘Z takes
//!   back an import, an edit or a skipped class. Neither ever syncs.
//! - A `commitmentDraft` is a schedule read out of pasted text, a photo or a
//!   calendar file. Nothing is a commitment until it is accepted, and an
//!   accepted draft is one entry: one ⌘Z takes all of it back.
//! - A `pendingException` is what a mail asked for ("class canceled
//!   Thursday"). It is offered, never applied.
//! - [`Fixed`] is what a plan steps around: every commitment with its
//!   travel time, and sleep. Plan my day reads the day through it.
//!
//! The rules (when a class really meets, what a day has left) are
//! `wi_heat::commitments`'s.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};
use wi_heat::commitments::{
    self as rules, Academic, Break, Commitment, ExceptionKind, Item, Kind, Mode, Occurrence,
    Schedule, Sleep, BUFFER_MAX, DAY_MIN,
};
use wi_heat::homes::{course_label, offering_in, squash};
use wi_heat::model::format::{short_month_day, weekday_name};
use wi_heat::model::records::CalendarEvent;
use wi_heat::model::zone;
use wi_store::{Actor, Store};

use crate::derive::World;
use crate::homes::{courses_from, school_of, Sighting};
use crate::schema::is_day;
use crate::{
    all, commit, kind, num, one, put, refused, set_setting, setting, ulid, Clock, Outcome, Result,
};

/// The `heatSetting` that holds when the person sleeps.
const SLEEP: &str = "sleep";

/// How long a mail is looked at for a canceled class after it came.
const MAIL_DAYS: f64 = 14.0;

fn typed<T: serde::de::DeserializeOwned>(raw: &[Value]) -> Vec<T> {
    raw.iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect()
}

/// Everything fixed about the person's time: read once per call.
#[derive(Clone, Debug, Default)]
pub struct Fixed {
    pub commitments: Vec<Commitment>,
    pub breaks: Vec<Break>,
    pub sleep: Sleep,
    pub term_start: Option<String>,
    pub term_end: Option<String>,
}

impl Fixed {
    pub fn load(store: &Store) -> Result<Fixed> {
        let school = setting(store, "school")?.unwrap_or(Value::Null);
        let day = |k: &str| school[k].as_str().filter(|d| is_day(d)).map(String::from);
        Ok(Fixed {
            commitments: typed(&all(store, kind::COMMITMENT)?),
            breaks: typed(&all(store, kind::BREAK)?),
            sleep: sleep(store)?,
            term_start: day("termStart"),
            term_end: day("termEnd"),
        })
    }

    pub fn academic(&self) -> Academic<'_> {
        Academic {
            breaks: &self.breaks,
            term_start: self.term_start.as_deref(),
            term_end: self.term_end.as_deref(),
        }
    }

    /// Every commitment's days in `[from, to]`.
    pub fn occurrences(&self, clock: &Clock, from: &str, to: &str) -> Vec<Occurrence> {
        rules::all_occurrences(&self.commitments, &self.academic(), &clock.zone, from, to)
    }

    /// The minutes of `date` a plan can't use: commitments with their
    /// travel time, and sleep.
    pub fn taken(&self, clock: &Clock, date: &str) -> Vec<(f64, f64)> {
        let mut spans = rules::busy(&self.occurrences(clock, date, date), date);
        spans.extend(self.sleep.spans());
        rules::merged(spans)
    }

    /// The same, as events of that day: what Plan my day already steps
    /// around for other calendars, so it steps around these too.
    pub fn busy_events(&self, clock: &Clock, date: &str) -> Vec<CalendarEvent> {
        self.taken(clock, date)
            .into_iter()
            .enumerate()
            .map(|(n, (a, b))| CalendarEvent {
                id: format!("fixed:{date}:{n}"),
                title: String::new(),
                start: zone::at_minute(date, a, &clock.zone),
                end: zone::at_minute(date, b, &clock.zone),
                all_day: false,
            })
            .collect()
    }
}

/// The day's busy time for a plan: other calendars' events, and what is
/// fixed. Any planner that reads this never plans into a class, a shift, a
/// buffer or sleep.
pub fn busy(
    store: &Store,
    clock: &Clock,
    events: &[CalendarEvent],
    date: &str,
) -> Result<Vec<CalendarEvent>> {
    let mut out = events.to_vec();
    out.extend(Fixed::load(store)?.busy_events(clock, date));
    Ok(out)
}

/// When the person sleeps: 11 PM to 7 AM until they say.
pub fn sleep(store: &Store) -> Result<Sleep> {
    Ok(setting(store, SLEEP)?
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

/// `heat.sleep.set {from, to}`: minutes after midnight, to bed and up.
pub fn set_sleep(store: &mut Store, from: f64, to: f64) -> Result<Outcome> {
    let ok = |m: f64| m.is_finite() && (0.0..=DAY_MIN).contains(&m);
    if !ok(from) || !ok(to) {
        return refused("Sleep runs between two times of day.");
    }
    let asleep = if from > to {
        DAY_MIN - from + to
    } else {
        to - from
    };
    if asleep > 16.0 * 60.0 {
        return refused("That leaves under eight hours awake. Check the two times.");
    }
    set_setting(store, SLEEP, &json!({"from": num(from), "to": num(to)}))?;
    Ok(Outcome::outside(json!({}), &[kind::SETTING]))
}

// ----- what a commitment may hold -----

fn text<'a>(m: &'a Map<String, Value>, k: &str) -> &'a str {
    m.get(k).and_then(Value::as_str).unwrap_or("").trim()
}

/// A time of day as minutes: a number, or "10:00", "5pm".
fn minutes(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64().filter(|m| m.is_finite()),
        Value::String(s) => rules::minutes_of(s),
        _ => None,
    }
}

fn kind_word(k: &str) -> &'static str {
    match k {
        "class" => "class",
        "work" => "shift",
        "commute" => "commute",
        _ => "commitment",
    }
}

/// Checks and completes a commitment (`schema::finish` comes through here).
/// `new_courses` are courses this same write makes.
pub(crate) fn check(
    m: &mut Map<String, Value>,
    world: &World,
    clock: &Clock,
    new_courses: &[String],
) -> Result<()> {
    let title = text(m, "title").to_string();
    if title.is_empty() {
        return refused("Give the commitment a name first.");
    }
    m.insert("title".into(), json!(title));
    let kind_of = match m.get("kind").and_then(Value::as_str) {
        None => Kind::Other,
        Some(k) => match Kind::parse(k) {
            Some(k) => k,
            None => return refused("A commitment is a class, work, a commute or other."),
        },
    };
    m.insert("kind".into(), json!(kind_of.as_str()));
    let (Some(start), Some(end)) = (minutes(m.get("start")), minutes(m.get("end"))) else {
        return refused("A commitment needs a start and an end, such as 10:00 and 10:50.");
    };
    if !(0.0..DAY_MIN).contains(&start) || end > DAY_MIN {
        return refused("A commitment's times are within one day.");
    }
    if end <= start {
        return refused("A commitment ends after it starts. One that runs past midnight is two.");
    }
    m.insert("start".into(), num(start.round()));
    m.insert("end".into(), num(end.round()));
    if text(m, "from").is_empty() {
        m.insert("from".into(), json!(clock.today()));
    }
    if !is_day(text(m, "from")) {
        return refused("A commitment's first day is written YYYY-MM-DD.");
    }
    match m.get("until").and_then(Value::as_str).map(str::trim) {
        None | Some("") => {
            m.remove("until");
        }
        Some(d) if !is_day(d) => return refused("A commitment's last day is written YYYY-MM-DD."),
        Some(d) if d < text(m, "from") => {
            return refused("A commitment's last day is on or after its first.")
        }
        Some(_) => {}
    }
    match m.get("rrule").and_then(Value::as_str).map(str::trim) {
        None | Some("") => {
            m.remove("rrule");
        }
        Some(rule) => match rules::kept_rule(rule) {
            Some(kept) => {
                m.insert("rrule".into(), json!(kept));
            }
            None => return refused("Learn can't read that repeat rule."),
        },
    }
    for k in ["bufferBefore", "bufferAfter"] {
        let v = match m.get(k) {
            None | Some(Value::Null) => 0.0,
            Some(v) => match v.as_f64().filter(|n| n.is_finite()) {
                Some(n) => n.round().clamp(0.0, BUFFER_MAX),
                None => return refused("Travel time is a number of minutes."),
            },
        };
        m.insert(k.into(), num(v));
    }
    match m.get("hardness").and_then(Value::as_str) {
        None => {
            m.insert("hardness".into(), json!("fixed"));
        }
        Some("fixed" | "flexible") => {}
        Some(_) => return refused("A commitment is fixed or flexible."),
    }
    let location = text(m, "location").to_string();
    m.insert("location".into(), json!(location));
    if let Some(id) = m.get("courseId").and_then(Value::as_str) {
        if !world.courses.iter().any(|c| c.id == id) && !new_courses.iter().any(|c| c == id) {
            return refused("That course isn't in Learn any more.");
        }
    }
    if let Some(id) = m.get("spaceId").and_then(Value::as_str) {
        if !world.spaces.iter().any(|s| s.id == id) {
            return refused("That space isn't in Learn any more.");
        }
    }
    let mut kept = Vec::new();
    match m.get("exceptions") {
        None | Some(Value::Null) => {}
        Some(Value::Array(list)) => {
            for e in list {
                kept.push(exception(e)?);
            }
        }
        Some(_) => return refused("A commitment's exceptions are a list."),
    }
    kept.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    m.insert("exceptions".into(), Value::Array(kept));
    if text(m, "source").is_empty() {
        m.insert("source".into(), json!("you"));
    }
    if !["you", "paste", "photo", "ics", "claude"].contains(&text(m, "source")) {
        return refused(
            "A commitment comes from you, a paste, a photo, a calendar file or Claude.",
        );
    }
    if let Some(week) = m.get("weekOf").and_then(Value::as_str) {
        if !is_day(week) {
            return refused("A week is named by its Monday, YYYY-MM-DD.");
        }
    }
    Ok(())
}

/// One exception, checked: a day, and for a move where it goes.
fn exception(e: &Value) -> Result<Value> {
    let Some(m) = e.as_object() else {
        return refused("An exception names a day.");
    };
    let date = text(m, "date");
    if !is_day(date) {
        return refused("An exception names a day, as YYYY-MM-DD.");
    }
    let mut out = Map::new();
    out.insert("date".into(), json!(date));
    let moves = match m.get("kind").and_then(Value::as_str) {
        None | Some("skip") => false,
        Some("move") => true,
        Some(_) => return refused("An exception skips a day or moves it."),
    };
    out.insert("kind".into(), json!(if moves { "move" } else { "skip" }));
    if moves {
        let to = text(m, "toDate");
        if !to.is_empty() {
            if !is_day(to) {
                return refused("A moved day is written YYYY-MM-DD.");
            }
            out.insert("toDate".into(), json!(to));
        }
        let start = minutes(m.get("start"));
        let end = minutes(m.get("end"));
        if m.get("start").is_some_and(|v| !v.is_null()) && start.is_none() {
            return refused("A moved time is a time of day, such as 14:00.");
        }
        if let Some(s) = start {
            if !(0.0..DAY_MIN).contains(&s) {
                return refused("A moved time is within the day.");
            }
            out.insert("start".into(), num(s.round()));
            if let Some(e) = end.filter(|e| *e > s && *e <= DAY_MIN) {
                out.insert("end".into(), num(e.round()));
            }
        }
        if to.is_empty() && start.is_none() {
            return refused("Say where it moves to: a day, a time, or both.");
        }
        let location = text(m, "location");
        if !location.is_empty() {
            out.insert("location".into(), json!(location));
        }
    }
    for k in ["note", "source"] {
        let v = text(m, k);
        if !v.is_empty() {
            out.insert(k.into(), json!(v));
        }
    }
    Ok(Value::Object(out))
}

/// A break, checked: a name and its days.
pub(crate) fn check_break(m: &mut Map<String, Value>) -> Result<()> {
    if text(m, "title").is_empty() {
        return refused("Give the break a name first.");
    }
    let from = text(m, "from").to_string();
    if !is_day(&from) {
        return refused("A break's first day is written YYYY-MM-DD.");
    }
    if text(m, "to").is_empty() {
        m.insert("to".into(), json!(from));
    }
    if !is_day(text(m, "to")) {
        return refused("A break's last day is written YYYY-MM-DD.");
    }
    if text(m, "to") < from.as_str() {
        return refused("A break's last day is on or after its first.");
    }
    if text(m, "source").is_empty() {
        m.insert("source".into(), json!("you"));
    }
    Ok(())
}

// ----- writing one commitment -----

/// The course a commitment names by its code: its id, and what has to be
/// written for it to exist. A class naming a course Learn doesn't hold makes
/// a stub, as a sync does.
fn course_for(
    world: &World,
    clock: &Clock,
    code: &str,
) -> (Option<String>, Vec<(&'static str, Value)>) {
    if let Ok(c) = world.course(code) {
        return (Some(c.id.clone()), Vec::new());
    }
    let pattern = &school_of(world).course_pattern;
    let Some(o) =
        offering_in(pattern, &code.to_uppercase()).filter(|o| squash(&o.code) == squash(code))
    else {
        return (None, Vec::new());
    };
    let found = courses_from(
        world,
        clock,
        &[Sighting {
            code: o.code.clone(),
            name: None,
            term: None,
            sure: true,
        }],
    );
    (found.id_of(world, &o.code), found.writes())
}

/// The space a class sits in when none is named: the one grouped by course.
fn class_space(world: &World) -> Option<String> {
    world
        .raw_spaces
        .iter()
        .find(|s| s["groupKind"] == "course")
        .and_then(|s| s["id"].as_str().map(String::from))
}

/// The record a set of args asks for, laid over `base`, and the courses to
/// write with it. Args name a course by `courseId` or `course` (its code), a
/// space by `spaceId` or `space` (its name), and the days by `days` (weekday
/// codes), `date` (one day) or `rrule`.
fn record_from(
    world: &World,
    clock: &Clock,
    base: Map<String, Value>,
    args: &Map<String, Value>,
) -> Result<(Map<String, Value>, Vec<(&'static str, Value)>)> {
    let mut m = base;
    let mut writes = Vec::new();
    const PLAIN: [&str; 14] = [
        "title",
        "kind",
        "location",
        "start",
        "end",
        "rrule",
        "from",
        "until",
        "bufferBefore",
        "bufferAfter",
        "hardness",
        "courseId",
        "spaceId",
        "exceptions",
    ];
    for (k, v) in args {
        match k.as_str() {
            k if PLAIN.contains(&k) => {
                m.insert(k.to_string(), v.clone());
            }
            "id" | "course" | "space" | "days" | "date" | "interval" | "source" | "sourceId"
            | "weekOf" | "feedId" => {}
            other => return refused(format!("A commitment has no field called '{other}'.")),
        }
    }
    for k in ["source", "sourceId", "weekOf", "feedId"] {
        if let Some(v) = args.get(k).filter(|v| !v.is_null()) {
            m.insert(k.into(), v.clone());
        }
    }
    if let Some(code) = args
        .get("course")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        let (id, more) = course_for(world, clock, code);
        match id {
            Some(id) => {
                m.insert("courseId".into(), json!(id));
                writes = more;
            }
            None => return refused(format!("No course has the code {code}.")),
        }
        if !m.contains_key("kind") {
            m.insert("kind".into(), json!("class"));
        }
        if text(&m, "title").is_empty() {
            m.insert("title".into(), json!(code));
        }
    }
    if let Some(name) = args
        .get("space")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        m.insert("spaceId".into(), json!(world.space(name)?.id));
    }
    if let Some(days) = args.get("days").filter(|v| !v.is_null()) {
        let Some(list) = days.as_array() else {
            return refused("Days are a list of weekdays, such as MO, WE, FR.");
        };
        if list.is_empty() {
            m.remove("rrule");
        } else {
            let names: Vec<String> = list
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect();
            let interval = args.get("interval").and_then(Value::as_f64).unwrap_or(1.0);
            match rules::weekly_rule(&names, interval).filter(|_| names.len() == list.len()) {
                Some(rule) => {
                    m.insert("rrule".into(), json!(rule));
                }
                None => return refused("Days are weekdays, such as MO, WE, FR."),
            }
        }
    }
    if let Some(date) = args.get("date").and_then(Value::as_str) {
        m.insert("from".into(), json!(date));
        if !args.contains_key("days") && !args.contains_key("rrule") {
            m.remove("rrule");
        }
    }
    // A class with no first day starts with the term, and sits with the courses.
    if m.get("kind").and_then(Value::as_str).and_then(Kind::parse) == Some(Kind::Class) {
        if text(&m, "from").is_empty() {
            let start = world.fixed.term_start.clone();
            if let Some(start) = start.filter(|s| s.as_str() <= clock.today().as_str()) {
                m.insert("from".into(), json!(start));
            }
        }
        if !m.contains_key("spaceId") {
            if let Some(space) = class_space(world) {
                m.insert("spaceId".into(), json!(space));
            }
        }
    }
    Ok((m, writes))
}

fn new_ids(writes: &[(&'static str, Value)]) -> Vec<String> {
    writes
        .iter()
        .filter(|(k, _)| *k == kind::COURSE)
        .filter_map(|(_, c)| c["id"].as_str().map(String::from))
        .collect()
}

/// `heat.commitment.create`: one commitment, by the sheet or by Claude.
pub fn create(
    store: &mut Store,
    clock: &Clock,
    args: &Map<String, Value>,
    actor: Actor,
) -> Result<Outcome> {
    let world = World::load(store)?;
    let (mut m, writes) = record_from(&world, clock, Map::new(), args)?;
    m.insert("id".into(), json!(ulid()));
    check(&mut m, &world, clock, &new_ids(&writes))?;
    let record = Value::Object(m);
    let c = commit(store, "add commitment", actor, |txn| {
        for (k, r) in &writes {
            put(txn, k, r)?;
        }
        put(txn, kind::COMMITMENT, &record)
    })?;
    let course = writes
        .iter()
        .find(|(k, _)| *k == kind::COURSE)
        .map(|(_, c)| c.clone());
    Ok(Outcome::new(
        json!({"commitment": record, "course": course, "line": said(&record)}),
        c,
    ))
}

/// "JPN 101, MWF 10:00 AM to 10:50 AM."
fn said(record: &Value) -> String {
    match serde_json::from_value::<Commitment>(record.clone()) {
        Ok(c) => format!("{}, {}.", c.title, rules::when_line(&c)),
        Err(_) => String::new(),
    }
}

/// A commitment by its id, or by its title when one has it.
pub fn find(store: &Store, q: &str) -> Result<Value> {
    if let Some(found) = one(store, kind::COMMITMENT, q)? {
        return Ok(found);
    }
    let want = squash(q);
    let named: Vec<Value> = all(store, kind::COMMITMENT)?
        .into_iter()
        .filter(|c| squash(c["title"].as_str().unwrap_or("")) == want)
        .collect();
    match named.len() {
        0 => refused(format!("No commitment is called {q}.")),
        1 => Ok(named.into_iter().next().expect("one")),
        _ => refused(format!(
            "More than one commitment is called {q}. Name it by its id."
        )),
    }
}

/// `heat.commitment.update {id, set}`
pub fn update(
    store: &mut Store,
    clock: &Clock,
    id: &str,
    set: &Map<String, Value>,
    actor: Actor,
) -> Result<Outcome> {
    let old = find(store, id)?;
    let world = World::load(store)?;
    let base = old.as_object().cloned().unwrap_or_default();
    let (mut m, writes) = record_from(&world, clock, base, set)?;
    m.retain(|_, v| !v.is_null());
    check(&mut m, &world, clock, &new_ids(&writes))?;
    let record = Value::Object(m);
    let c = commit(store, "edit commitment", actor, |txn| {
        for (k, r) in &writes {
            put(txn, k, r)?;
        }
        put(txn, kind::COMMITMENT, &record)
    })?;
    Ok(Outcome::new(
        json!({"commitment": record, "line": said(&record)}),
        c,
    ))
}

/// `heat.commitment.addException`: one day skipped, or moved. The day has
/// to be one it meets on.
pub fn add_exception(
    store: &mut Store,
    clock: &Clock,
    id: &str,
    ex: &Value,
    actor: Actor,
) -> Result<Outcome> {
    let old = find(store, id)?;
    let checked = exception(ex)?;
    let fixed = Fixed::load(store)?;
    let Ok(c) = serde_json::from_value::<Commitment>(old.clone()) else {
        return refused("That commitment doesn't read. Delete it and add it again.");
    };
    let date = checked["date"].as_str().unwrap_or_default().to_string();
    if !rules::meets_on(&c, &fixed.academic(), &clock.zone, &date) {
        return refused(format!(
            "{} doesn't meet on {}.",
            c.title,
            rules::day_words(&date)
        ));
    }
    let mut record = old.clone();
    let mut list: Vec<Value> = record["exceptions"].as_array().cloned().unwrap_or_default();
    list.retain(|e| e["date"] != checked["date"]);
    list.push(checked.clone());
    list.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    record["exceptions"] = Value::Array(list);
    let moves = checked["kind"] == "move";
    let label = format!(
        "{} {}",
        if moves { "move" } else { "skip" },
        kind_word(c.kind.as_str())
    );
    let done = commit(store, &label, actor, |txn| {
        put(txn, kind::COMMITMENT, &record)
    })?;
    let line = if moves {
        format!("{} on {} is moved.", c.title, rules::day_words(&date))
    } else {
        format!("{} on {} is skipped.", c.title, rules::day_words(&date))
    };
    Ok(Outcome::new(
        json!({"commitment": record, "line": line}),
        done,
    ))
}

/// `heat.commitment.removeException {id, date}`: the day goes as usual again.
pub fn remove_exception(store: &mut Store, id: &str, date: &str) -> Result<Outcome> {
    let mut record = find(store, id)?;
    let mut list: Vec<Value> = record["exceptions"].as_array().cloned().unwrap_or_default();
    let before = list.len();
    list.retain(|e| e["date"] != date);
    if list.len() == before {
        return refused("That day has no exception.");
    }
    record["exceptions"] = Value::Array(list);
    let word = kind_word(record["kind"].as_str().unwrap_or("other"));
    let done = commit(store, &format!("restore {word}"), Actor::You, |txn| {
        put(txn, kind::COMMITMENT, &record)
    })?;
    Ok(Outcome::new(json!({"commitment": record}), done))
}

// ----- free time -----

fn day_word(clock: &Clock, date: &str) -> String {
    if date == clock.today() {
        "today".to_string()
    } else if date == zone::add_days(&clock.today(), 1.0) {
        "tomorrow".to_string()
    } else {
        weekday_name(zone::weekday_of(date)).to_string()
    }
}

/// One day's free time: the day (from now, today) less commitments, their
/// travel, other calendars' timed events and sleep, beside what is planned.
pub(crate) fn free_of(fixed: &Fixed, world: &World, clock: &Clock, date: &str) -> rules::FreeTime {
    let mut taken = fixed.taken(clock, date);
    taken.extend(wi_heat::model::plan::busy_spans(
        &[],
        &world.events,
        date,
        &clock.zone,
    ));
    let from = if date == clock.today() {
        zone::minute_of_day(clock.now_ms, &clock.zone)
    } else {
        0.0
    };
    let blocks: Vec<(f64, f64)> = world
        .blocks
        .iter()
        .filter(|b| b.date == date)
        .map(|b| (b.start, b.minutes))
        .collect();
    rules::free_time(
        date,
        from,
        &taken,
        &fixed.sleep,
        &blocks,
        &day_word(clock, date),
    )
}

/// `heat.planner.freeTime {date}` or `{from, to}`: what each day has left,
/// and what is fixed in it.
pub fn free_time(store: &Store, clock: &Clock, from: &str, to: &str) -> Result<Value> {
    if !is_day(from) || !is_day(to) {
        return refused("A day is written YYYY-MM-DD.");
    }
    if to < from || zone::days_between(from, to) > 62.0 {
        return refused("Ask for up to two months of days, first to last.");
    }
    let world = World::load(store)?;
    let fixed = &world.fixed;
    let occurrences = fixed.occurrences(clock, from, to);
    let mut days = Vec::new();
    let mut day = from.to_string();
    while day.as_str() <= to {
        let free = free_of(fixed, &world, clock, &day);
        let mut v = serde_json::to_value(&free).unwrap_or(Value::Null);
        v["commitments"] = json!(occurrences
            .iter()
            .filter(|o| o.date == day)
            .collect::<Vec<_>>());
        days.push(v);
        day = zone::add_days(&day, 1.0);
    }
    Ok(json!({ "days": days }))
}

// ----- a schedule waiting to be applied -----

fn draft_put(store: &mut Store, draft: &Value) -> Result<()> {
    let id = draft["id"].as_str().unwrap_or_default().to_string();
    store.set_doc(kind::COMMITMENT_DRAFT, &id, draft, "")?;
    Ok(())
}

fn draft_get(store: &Store, id: &str) -> Result<Value> {
    match one(store, kind::COMMITMENT_DRAFT, id)? {
        Some(d) => Ok(d),
        None => refused("That schedule isn't waiting any more."),
    }
}

/// The week a draft of shifts is for: the one named, else this one.
fn week_for(clock: &Clock, week_of: Option<&str>) -> Result<String> {
    match week_of {
        Some(d) if is_day(d) => Ok(rules::monday_of(d)),
        Some(_) => refused("A week is named by a day in it, YYYY-MM-DD."),
        None => Ok(rules::monday_of(&clock.today())),
    }
}

/// A draft that is `reading`: pasted text or a photo, for the worker to
/// hand to Claude. Nothing of the schedule changes.
pub fn draft_begin(
    store: &mut Store,
    clock: &Clock,
    mode: Mode,
    week_of: Option<&str>,
    text: Option<&str>,
    path: Option<&str>,
    file_name: &str,
) -> Result<Value> {
    let source = if path.is_some() { "photo" } else { "paste" };
    let mut draft = json!({
        "id": ulid(), "mode": mode.as_str(), "source": source, "fileName": file_name,
        "weekOf": week_for(clock, week_of)?, "createdAt": num(clock.now_ms), "state": "reading",
    });
    if let Some(t) = text {
        draft["text"] = json!(t);
    }
    if let Some(p) = path {
        draft["path"] = json!(p);
    }
    draft_put(store, &draft)?;
    Ok(draft)
}

/// The drafts still to be read, oldest first.
pub fn drafts_to_read(store: &Store) -> Result<Vec<Value>> {
    Ok(all(store, kind::COMMITMENT_DRAFT)?
        .into_iter()
        .filter(|d| d["state"] == "reading")
        .collect())
}

/// A draft that couldn't be read says why, in one sentence.
pub fn draft_fail(store: &mut Store, id: &str, sentence: &str) -> Result<()> {
    let mut draft = draft_get(store, id)?;
    draft["state"] = json!("failed");
    draft["error"] = json!(sentence);
    draft_put(store, &draft)
}

fn nothing_found(mode: Mode) -> &'static str {
    match mode {
        Mode::Schedule => {
            "Nothing in that reads as a schedule. Try a clearer copy, or add it by hand."
        }
        Mode::Week => "No shifts could be read from that. Try a clearer copy, or add them by hand.",
        Mode::Breaks => {
            "No breaks could be read from that. Try a clearer copy, or add them by hand."
        }
    }
}

/// A read draft takes its schedule and is `ready`, or `failed` when there
/// is nothing in it.
pub fn draft_ready(store: &mut Store, id: &str, schedule: &Schedule) -> Result<Value> {
    let mut draft = draft_get(store, id)?;
    let mode = draft["mode"]
        .as_str()
        .and_then(Mode::parse)
        .unwrap_or(Mode::Schedule);
    if schedule.items.is_empty() && schedule.breaks.is_empty() {
        draft_fail(store, id, nothing_found(mode))?;
        return refused(nothing_found(mode));
    }
    draft["state"] = json!("ready");
    draft["items"] = json!(schedule.items);
    draft["breaks"] = json!(schedule.breaks);
    draft["unread"] = json!(schedule.unread);
    // The text and the photo have been read: the draft keeps neither.
    if let Some(m) = draft.as_object_mut() {
        m.remove("text");
        m.remove("path");
    }
    draft_put(store, &draft)?;
    Ok(draft)
}

/// A draft that is ready at once: a calendar file, or a schedule the caller
/// read itself.
pub fn draft_from(
    store: &mut Store,
    clock: &Clock,
    mode: Mode,
    source: &str,
    week_of: Option<&str>,
    file_name: &str,
    schedule: &Schedule,
) -> Result<Value> {
    if schedule.items.is_empty() && schedule.breaks.is_empty() {
        return refused(nothing_found(mode));
    }
    let draft = json!({
        "id": ulid(), "mode": mode.as_str(), "source": source, "fileName": file_name,
        "weekOf": week_for(clock, week_of)?, "createdAt": num(clock.now_ms), "state": "ready",
        "items": schedule.items, "breaks": schedule.breaks, "unread": schedule.unread,
    });
    draft_put(store, &draft)?;
    Ok(draft)
}

/// `heat.commitment.draft.discard`
pub fn draft_discard(store: &mut Store, id: &str) -> Result<Outcome> {
    store.remove_doc(kind::COMMITMENT_DRAFT, id)?;
    Ok(Outcome::outside(json!({}), &[kind::COMMITMENT_DRAFT]))
}

/// The course an item names: by its own `course`, else a class whose title
/// is a course code.
fn item_course(world: &World, item: &Item) -> Option<String> {
    if let Some(c) = item.course.as_deref().filter(|c| !c.trim().is_empty()) {
        return Some(c.trim().to_string());
    }
    if item.kind != Kind::Class {
        return None;
    }
    let pattern = &school_of(world).course_pattern;
    offering_in(pattern, &item.title.to_uppercase()).map(|o| o.code)
}

/// The same thing at the same time: a schedule applied twice adds nothing.
fn same(a: &Value, b: &Value) -> bool {
    squash(a["title"].as_str().unwrap_or("")) == squash(b["title"].as_str().unwrap_or(""))
        && a["start"] == b["start"]
        && a["end"] == b["end"]
        && a.get("rrule") == b.get("rrule")
        && (a.get("rrule").is_some() || a["from"] == b["from"])
}

/// What accepting a draft would write, worked out against the library as it
/// stands. Nothing here is saved.
struct Preview {
    /// Each item's record, whether it is new, and whether its course is.
    records: Vec<(Value, bool, bool)>,
    breaks: Vec<(Value, bool)>,
    courses: Vec<(&'static str, Value)>,
    /// Commitments of the same week this replaces.
    replaced: Vec<String>,
}

fn preview(store: &Store, world: &World, clock: &Clock, draft: &Value) -> Result<Preview> {
    let mode = draft["mode"]
        .as_str()
        .and_then(Mode::parse)
        .unwrap_or(Mode::Schedule);
    let source = draft["source"].as_str().unwrap_or("paste");
    let week = draft["weekOf"].as_str().unwrap_or_default();
    let items: Vec<Item> = typed(draft["items"].as_array().map_or(&[][..], Vec::as_slice));
    let existing = all(store, kind::COMMITMENT)?;
    let replaced: Vec<String> = if mode == Mode::Week {
        existing
            .iter()
            .filter(|c| c["weekOf"] == week && c["kind"] == "work")
            .filter_map(|c| c["id"].as_str().map(String::from))
            .collect()
    } else {
        Vec::new()
    };
    let mut courses: Vec<(&'static str, Value)> = Vec::new();
    let mut records = Vec::new();
    for item in &items {
        let mut args = Map::new();
        args.insert("title".into(), json!(item.title));
        args.insert("kind".into(), json!(item.kind.as_str()));
        args.insert("start".into(), num(item.start));
        args.insert("end".into(), num(item.end));
        args.insert("location".into(), json!(item.location));
        args.insert("source".into(), json!(source));
        if let Some(rule) = &item.rrule {
            args.insert("rrule".into(), json!(rule));
        } else if !item.days.is_empty() {
            args.insert("days".into(), json!(item.days));
        }
        if let Some(d) = item.date.as_ref().or(item.from.as_ref()) {
            args.insert("from".into(), json!(d));
        }
        if let Some(d) = &item.until {
            args.insert("until".into(), json!(d));
        }
        if let Some(id) = &item.source_id {
            args.insert("sourceId".into(), json!(id));
        }
        if mode == Mode::Week {
            args.insert("weekOf".into(), json!(week));
        }
        if !item.skip.is_empty() {
            args.insert(
                "exceptions".into(),
                json!(item
                    .skip
                    .iter()
                    .map(|d| json!({"date": d, "kind": "skip"}))
                    .collect::<Vec<_>>()),
            );
        }
        // A course made for one item is there for the next.
        let mut new_course = false;
        let code = item_course(world, item);
        if let Some(code) = &code {
            let made = courses
                .iter()
                .find(|(k, c)| {
                    *k == kind::COURSE && squash(c["code"].as_str().unwrap_or("")) == squash(code)
                })
                .and_then(|(_, c)| c["id"].as_str().map(String::from));
            match made {
                Some(id) => {
                    args.insert("courseId".into(), json!(id));
                    new_course = true;
                }
                None => {
                    let (id, writes) = course_for(world, clock, code);
                    if let Some(id) = id {
                        args.insert("courseId".into(), json!(id));
                        new_course = !writes.is_empty();
                        // One term for every course the draft makes.
                        for (k, w) in writes {
                            if k == kind::TERM
                                && courses
                                    .iter()
                                    .any(|(kk, t)| *kk == kind::TERM && t["name"] == w["name"])
                            {
                                continue;
                            }
                            courses.push((k, w));
                        }
                    }
                }
            }
        }
        let (mut m, _) = record_from(world, clock, Map::new(), &args)?;
        m.insert("id".into(), json!(ulid()));
        check(&mut m, world, clock, &new_ids(&courses))?;
        let record = Value::Object(m);
        let is_new = mode == Mode::Week
            || !existing.iter().any(|c| same(c, &record))
                && !records
                    .iter()
                    .any(|(r, _, _): &(Value, bool, bool)| same(r, &record));
        records.push((record, is_new, new_course && is_new));
    }
    // A course no kept item needs isn't made.
    let needed: Vec<&str> = records
        .iter()
        .filter(|(_, is_new, _)| *is_new)
        .filter_map(|(r, _, _)| r["courseId"].as_str())
        .collect();
    courses.retain(|(k, c)| {
        *k != kind::COURSE || c["id"].as_str().is_some_and(|id| needed.contains(&id))
    });
    if !courses.iter().any(|(k, _)| *k == kind::COURSE) {
        courses.clear();
    }
    let have = all(store, kind::BREAK)?;
    let mut breaks = Vec::new();
    for b in typed::<Break>(draft["breaks"].as_array().map_or(&[][..], Vec::as_slice)) {
        let is_new = !have
            .iter()
            .any(|h| h["from"] == b.from.as_str() && h["to"] == b.to.as_str())
            && !breaks.iter().any(|(r, _): &(Value, bool)| {
                r["from"] == b.from.as_str() && r["to"] == b.to.as_str()
            });
        breaks.push((
            json!({"id": ulid(), "title": b.title, "from": b.from, "to": b.to, "source": source}),
            is_new,
        ));
    }
    Ok(Preview {
        records,
        breaks,
        courses,
        replaced,
    })
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A draft as the views show it: with its preview, when it is ready.
fn draft_shown(store: &Store, world: &World, clock: &Clock, draft: &Value) -> Result<Value> {
    let mut out = draft.clone();
    if let Some(m) = out.as_object_mut() {
        m.remove("text");
        m.remove("path");
    }
    if draft["state"] != "ready" {
        return Ok(out);
    }
    let p = preview(store, world, clock, draft)?;
    let mode = draft["mode"]
        .as_str()
        .and_then(Mode::parse)
        .unwrap_or(Mode::Schedule);
    let label = |r: &Value| -> Value {
        let Some(id) = r["courseId"].as_str() else {
            return Value::Null;
        };
        world
            .courses
            .iter()
            .find(|c| c.id == id)
            .map(|c| json!(course_label(&c.code, &c.name)))
            .or_else(|| {
                p.courses
                    .iter()
                    .find(|(_, c)| c["id"] == id)
                    .map(|(_, c)| c["code"].clone())
            })
            .unwrap_or(Value::Null)
    };
    let shown: Vec<Value> = p
        .records
        .iter()
        .map(|(r, is_new, new_course)| {
            let c: Option<Commitment> = serde_json::from_value(r.clone()).ok();
            json!({
                "title": r["title"], "kind": r["kind"], "start": r["start"], "end": r["end"],
                "location": r["location"], "from": r["from"], "until": r.get("until").cloned().unwrap_or(Value::Null),
                "days": r["rrule"].as_str().map(rules::rule_days).unwrap_or_default(),
                "when": c.as_ref().map(rules::when_line).unwrap_or_default(),
                "course": label(r), "isNew": is_new, "newCourse": new_course,
            })
        })
        .collect();
    let new_items: Vec<Item> =
        typed::<Item>(draft["items"].as_array().map_or(&[][..], Vec::as_slice))
            .into_iter()
            .zip(&p.records)
            .filter(|(_, (_, is_new, _))| *is_new)
            .map(|(i, _)| i)
            .collect();
    let new_breaks = p.breaks.iter().filter(|(_, n)| *n).count();
    let mut line = format!("{}.", rules::count_line(&new_items, new_breaks));
    let already = p.records.iter().filter(|(_, n, _)| !*n).count() + p.breaks.len() - new_breaks;
    if already > 0 {
        line.push_str(&format!(" {} already there.", already));
    }
    let made = p.courses.iter().filter(|(k, _)| *k == kind::COURSE).count();
    if made > 0 {
        line.push_str(&format!(" {}.", plural(made, "new course", "new courses")));
    }
    if mode == Mode::Week {
        let week = draft["weekOf"].as_str().unwrap_or_default();
        if p.replaced.is_empty() {
            line.push_str(&format!(" For the week of {}.", short_month_day(week)));
        } else {
            line.push_str(&format!(
                " Replaces {} in the week of {}.",
                plural(p.replaced.len(), "shift", "shifts"),
                short_month_day(week)
            ));
        }
    }
    let unread = draft["unread"].as_u64().unwrap_or(0) as usize;
    if unread > 0 {
        line.push_str(&format!(
            " {} couldn't be read.",
            plural(unread, "line", "lines")
        ));
    }
    out["items"] = Value::Array(shown);
    out["breaks"] = Value::Array(
        p.breaks
            .iter()
            .map(|(b, is_new)| json!({"title": b["title"], "from": b["from"], "to": b["to"], "isNew": is_new}))
            .collect(),
    );
    out["replaces"] = json!(p.replaced.len());
    out["line"] = json!(line);
    Ok(out)
}

/// One draft as the views show it, or None.
pub fn draft(store: &Store, clock: &Clock, id: &str) -> Result<Option<Value>> {
    let Some(d) = one(store, kind::COMMITMENT_DRAFT, id)? else {
        return Ok(None);
    };
    let world = World::load(store)?;
    draft_shown(store, &world, clock, &d).map(Some)
}

/// `heat.commitment.draft.accept {draftId, skip?}`: the draft's commitments
/// (but the items named in `skip`, by their place in the list), its breaks
/// and the courses they need, as one entry. A week's shifts replace that
/// week's.
pub fn draft_accept(
    store: &mut Store,
    clock: &Clock,
    id: &str,
    skip: &[usize],
    actor: Actor,
) -> Result<Outcome> {
    let d = draft_get(store, id)?;
    if d["state"] != "ready" {
        return refused("That schedule is still being read.");
    }
    let world = World::load(store)?;
    let p = preview(store, &world, clock, &d)?;
    let mode = d["mode"]
        .as_str()
        .and_then(Mode::parse)
        .unwrap_or(Mode::Schedule);
    let records: Vec<&Value> = p
        .records
        .iter()
        .enumerate()
        .filter(|(n, (_, is_new, _))| *is_new && !skip.contains(n))
        .map(|(_, (r, _, _))| r)
        .collect();
    let breaks: Vec<&Value> = p
        .breaks
        .iter()
        .filter(|(_, n)| *n)
        .map(|(b, _)| b)
        .collect();
    let needed: Vec<&str> = records
        .iter()
        .filter_map(|r| r["courseId"].as_str())
        .collect();
    let courses: Vec<&(&'static str, Value)> = p
        .courses
        .iter()
        .filter(|(k, c)| {
            *k == kind::TERM || c["id"].as_str().is_some_and(|id| needed.contains(&id))
        })
        .collect();
    let makes_course = courses.iter().any(|(k, _)| *k == kind::COURSE);
    let label = match mode {
        Mode::Schedule => "import schedule",
        Mode::Week => "this week's shifts",
        Mode::Breaks => "import breaks",
    };
    let c = commit(store, label, actor, |txn| {
        for (k, r) in courses
            .iter()
            .filter(|(k, _)| makes_course || *k != kind::TERM)
        {
            put(txn, k, r)?;
        }
        for gone in &p.replaced {
            txn.delete_doc(kind::COMMITMENT, gone)?;
        }
        for r in &records {
            put(txn, kind::COMMITMENT, r)?;
        }
        for b in &breaks {
            put(txn, kind::BREAK, b)?;
        }
        Ok(())
    })?;
    store.remove_doc(kind::COMMITMENT_DRAFT, id)?;
    let out = Outcome::new(
        json!({
            "commitments": records.len(), "breaks": breaks.len(), "replaced": p.replaced.len(),
            "courses": courses.iter().filter(|(k, _)| *k == kind::COURSE).map(|(_, c)| c.clone()).collect::<Vec<_>>(),
        }),
        c,
    );
    Ok(out.also(&[kind::COMMITMENT_DRAFT]))
}

// ----- a calendar address that is read again -----

/// The subscribed calendars, as stored.
pub fn feeds(store: &Store) -> Result<Vec<Value>> {
    all(store, kind::COMMITMENT_FEED)
}

/// A new subscription's record. Its address goes to the Keychain under
/// `keychainRef`, and nowhere else.
pub fn feed_new(store: &mut Store, name: &str) -> Result<Value> {
    let id = ulid();
    let name = match name.trim() {
        "" => "Schedule",
        n => n,
    };
    let record = json!({"id": id, "name": name, "keychainRef": format!("wi-wwav.commitments.{id}"), "lastSyncedAt": null});
    store.set_doc(kind::COMMITMENT_FEED, &id, &record, "")?;
    Ok(record)
}

/// `heat.commitment.feed.remove {id}`: the subscription goes; what it made
/// stays until it is deleted.
pub fn feed_remove(store: &mut Store, id: &str) -> Result<Value> {
    let Some(record) = one(store, kind::COMMITMENT_FEED, id)? else {
        return refused("That calendar isn't subscribed any more.");
    };
    store.remove_doc(kind::COMMITMENT_FEED, id)?;
    Ok(record)
}

/// A subscribed calendar read again: what it holds now replaces what it
/// made before, as one entry, "schedule sync", and only when something
/// changed. An exception the person added to one of its commitments is kept.
pub fn feed_apply(
    store: &mut Store,
    clock: &Clock,
    feed_id: &str,
    schedule: &Schedule,
) -> Result<Outcome> {
    let Some(mut feed) = one(store, kind::COMMITMENT_FEED, feed_id)? else {
        return refused("That calendar isn't subscribed any more.");
    };
    let world = World::load(store)?;
    let draft = json!({"mode": "schedule", "source": "ics", "weekOf": rules::monday_of(&clock.today()), "items": schedule.items, "breaks": [], "state": "ready"});
    let have: Vec<Value> = all(store, kind::COMMITMENT)?
        .into_iter()
        .filter(|c| c["feedId"] == feed_id)
        .collect();
    let items: Vec<Item> = schedule.items.clone();
    let mut writes: Vec<Value> = Vec::new();
    let mut kept: Vec<&str> = Vec::new();
    let mut courses: Vec<(&'static str, Value)> = Vec::new();
    for (n, item) in items.iter().enumerate() {
        let one_draft = json!({"mode": "schedule", "source": "ics", "weekOf": draft["weekOf"], "items": [item], "breaks": []});
        let mut p = preview(store, &world, clock, &one_draft)?;
        let Some((mut record, _, _)) = p.records.pop() else {
            continue;
        };
        record["feedId"] = json!(feed_id);
        let key = item
            .source_id
            .clone()
            .unwrap_or_else(|| format!("{}#{n}", item.title));
        record["sourceId"] = json!(key);
        match have.iter().find(|c| c["sourceId"] == key.as_str()) {
            Some(old) => {
                kept.push(old["id"].as_str().unwrap_or_default());
                record["id"] = old["id"].clone();
                // What the person set on it stays theirs.
                for k in [
                    "bufferBefore",
                    "bufferAfter",
                    "hardness",
                    "spaceId",
                    "courseId",
                    "kind",
                ] {
                    if let Some(v) = old.get(k) {
                        record[k] = v.clone();
                    }
                }
                let mut ex: Vec<Value> =
                    record["exceptions"].as_array().cloned().unwrap_or_default();
                for e in old["exceptions"].as_array().map_or(&[][..], Vec::as_slice) {
                    if e["source"].is_string() && !ex.iter().any(|x| x["date"] == e["date"]) {
                        ex.push(e.clone());
                    }
                }
                ex.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
                record["exceptions"] = Value::Array(ex);
                if &record != old {
                    writes.push(record);
                }
            }
            None => {
                courses.extend(p.courses);
                writes.push(record);
            }
        }
    }
    let gone: Vec<&str> = have
        .iter()
        .filter_map(|c| c["id"].as_str())
        .filter(|id| !kept.contains(id))
        .collect();
    let c = commit(store, "schedule sync", Actor::You, |txn| {
        for (k, r) in &courses {
            put(txn, k, r)?;
        }
        for id in &gone {
            txn.delete_doc(kind::COMMITMENT, id)?;
        }
        for r in &writes {
            put(txn, kind::COMMITMENT, r)?;
        }
        Ok(())
    })?;
    feed["lastSyncedAt"] = num(clock.now_ms);
    store.set_doc(kind::COMMITMENT_FEED, feed_id, &feed, "")?;
    Ok(
        Outcome::new(json!({"changed": writes.len() + gone.len()}), c)
            .also(&[kind::COMMITMENT_FEED]),
    )
}

// ----- what a mail asked for -----

fn pending_key(thread: &str) -> String {
    format!("mail-{thread}")
}

/// Reads the mail Claude recorded for a class that is canceled or moved,
/// and keeps what it finds as pending: one per thread, never twice, never
/// for a day that is over. Returns the ones that are new, for the notice.
pub fn pending_from_mail(store: &mut Store, clock: &Clock) -> Result<Vec<Value>> {
    let fixed = Fixed::load(store)?;
    if fixed.commitments.is_empty() {
        return Ok(Vec::new());
    }
    let courses = all(store, kind::COURSE)?;
    let code = |id: &str| -> Option<String> {
        courses
            .iter()
            .find(|c| c["id"] == id)
            .and_then(|c| c["code"].as_str().map(String::from))
    };
    let today = clock.today();
    let since = clock.now_ms - MAIL_DAYS * 86_400_000.0;
    let mut made = Vec::new();
    for thread in all(store, kind::MAIL)? {
        let received = thread["receivedAt"].as_f64().unwrap_or(0.0);
        let Some(gmail) = thread["gmailThreadId"].as_str().or(thread["id"].as_str()) else {
            continue;
        };
        if received < since || one(store, kind::PENDING_EXCEPTION, &pending_key(gmail))?.is_some() {
            continue;
        }
        // The newest message is the one that says what is happening.
        let saved = one(store, kind::MAIL_TEXT, gmail)?;
        let body = saved
            .as_ref()
            .and_then(|d| d["messages"].as_array())
            .and_then(|m| m.last())
            .and_then(|m| m["text"].as_str())
            .unwrap_or("");
        let text = format!("{}\n{}", thread["reason"].as_str().unwrap_or(""), body);
        let seen = rules::MailSeen {
            subject: thread["subject"].as_str().unwrap_or(""),
            text: &text,
            course: thread["course"].as_str(),
            received_at: received,
        };
        let Some(found) = rules::exception_in_mail(
            &seen,
            &fixed.commitments,
            &fixed.academic(),
            &clock.zone,
            &code,
        ) else {
            continue;
        };
        let lands = found
            .exception
            .to_date
            .as_deref()
            .unwrap_or(&found.exception.date);
        if found.exception.date < today && lands < today.as_str() {
            continue;
        }
        let Some(c) = fixed
            .commitments
            .iter()
            .find(|c| c.id == found.commitment_id)
        else {
            continue;
        };
        if c.exceptions.iter().any(|e| e.date == found.exception.date) {
            continue;
        }
        let id = pending_key(gmail);
        let record = json!({
            "id": id, "commitmentId": found.commitment_id, "title": c.title,
            "exception": found.exception, "line": found.line, "act": found.act,
            "mailThreadId": gmail, "subject": seen.subject, "createdAt": num(clock.now_ms), "state": "pending",
            "kind": if found.exception.kind == ExceptionKind::Move { "move" } else { "skip" },
        });
        store.set_doc(kind::PENDING_EXCEPTION, &id, &record, "")?;
        made.push(record);
    }
    Ok(made)
}

/// The exceptions waiting for a tap.
pub fn pending(store: &Store) -> Result<Vec<Value>> {
    Ok(all(store, kind::PENDING_EXCEPTION)?
        .into_iter()
        .filter(|p| p["state"] == "pending")
        .collect())
}

/// `heat.commitment.exception.confirm {id}`: the one tap. The exception is
/// written, as one entry by the person.
pub fn pending_confirm(store: &mut Store, clock: &Clock, id: &str) -> Result<Outcome> {
    let Some(mut p) = one(store, kind::PENDING_EXCEPTION, id)?.filter(|p| p["state"] == "pending")
    else {
        return refused("That isn't waiting any more.");
    };
    let commitment = p["commitmentId"].as_str().unwrap_or_default().to_string();
    let out = add_exception(store, clock, &commitment, &p["exception"], Actor::You)?;
    p["state"] = json!("confirmed");
    store.set_doc(kind::PENDING_EXCEPTION, id, &p, "")?;
    Ok(out.also(&[kind::PENDING_EXCEPTION]))
}

/// `heat.commitment.exception.dismiss {id}`: it isn't offered again.
pub fn pending_dismiss(store: &mut Store, id: &str) -> Result<Outcome> {
    let Some(mut p) = one(store, kind::PENDING_EXCEPTION, id)? else {
        return refused("That isn't waiting any more.");
    };
    p["state"] = json!("dismissed");
    store.set_doc(kind::PENDING_EXCEPTION, id, &p, "")?;
    Ok(Outcome::outside(json!({}), &[kind::PENDING_EXCEPTION]))
}

// ----- what the views read -----

fn hue_of(world: &World, c: &Commitment) -> Value {
    let space = c.space_id.clone().or_else(|| {
        (c.kind == Kind::Class || c.course_id.is_some())
            .then(|| class_space(world))
            .flatten()
    });
    space
        .and_then(|id| world.spaces.iter().find(|s| s.id == id))
        .map_or(Value::Null, |s| num(s.hue))
}

/// "Sep 9 to Dec 11", "from Sep 9", or nothing for a single day.
fn range_line(c: &Commitment, fixed: &Fixed) -> String {
    if c.rrule.as_deref().map_or(true, str::is_empty) {
        return String::new();
    }
    match rules::last_day(c, &fixed.academic()) {
        Some(last) => format!("{} to {}", short_month_day(&c.from), short_month_day(last)),
        None => format!("from {}", short_month_day(&c.from)),
    }
}

/// The share of `heat.snapshot` that is commitments: the records, each day
/// of the window with what is fixed in it, what is next, what today has
/// left, the blocks that sit on something, and what waits for a tap.
pub fn for_snapshot(
    store: &Store,
    clock: &Clock,
    snap: &mut Value,
    from: &str,
    to: &str,
) -> Result<()> {
    let world = World::load(store)?;
    let fixed = world.fixed.clone();
    let date = snap["date"].as_str().unwrap_or_default().to_string();
    let today = clock.today();
    let lo = if date.as_str() < from {
        date.as_str()
    } else {
        from
    };
    let hi = if date.as_str() > to {
        date.as_str()
    } else {
        to
    };
    let occurrences = fixed.occurrences(clock, lo, hi);
    let labels: BTreeMap<&str, Value> = fixed
        .commitments
        .iter()
        .map(|c| {
            let label = c
                .course_id
                .as_deref()
                .and_then(|id| world.courses.iter().find(|k| k.id == id))
                .map_or(Value::Null, |k| json!(course_label(&k.code, &k.name)));
            (
                c.id.as_str(),
                json!({"hue": hue_of(&world, c), "label": label}),
            )
        })
        .collect();
    let mut days: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for o in &occurrences {
        let mut v = serde_json::to_value(o).unwrap_or(Value::Null);
        if let Some(extra) = labels.get(o.commitment_id.as_str()) {
            v["hue"] = extra["hue"].clone();
            v["label"] = extra["label"].clone();
        }
        days.entry(o.date.clone()).or_default().push(v);
    }
    let list: Vec<Value> = fixed
        .commitments
        .iter()
        .map(|c| {
            json!({
                "id": c.id, "when": rules::when_line(c), "range": range_line(c, &fixed),
                "days": c.rrule.as_deref().map(rules::rule_days).unwrap_or_default(),
                "hue": labels.get(c.id.as_str()).map_or(Value::Null, |l| l["hue"].clone()),
                "label": labels.get(c.id.as_str()).map_or(Value::Null, |l| l["label"].clone()),
            })
        })
        .collect();
    let now_min = zone::minute_of_day(clock.now_ms, &clock.zone);
    let next = rules::next_up(&occurrences, &today, now_min);
    let mut conflicts = Vec::new();
    let mut day = lo.to_string();
    while day.as_str() <= hi {
        let blocks: Vec<(String, f64, f64)> = world
            .blocks
            .iter()
            .filter(|b| b.date == day)
            .map(|b| (b.id.clone(), b.start, b.minutes))
            .collect();
        if !blocks.is_empty() {
            conflicts.extend(rules::conflicts(&blocks, &occurrences, &day));
        }
        day = zone::add_days(&day, 1.0);
    }
    let mut drafts = Vec::new();
    for d in all(store, kind::COMMITMENT_DRAFT)? {
        drafts.push(draft_shown(store, &world, clock, &d)?);
    }
    snap["records"][kind::COMMITMENT] = Value::Array(all(store, kind::COMMITMENT)?);
    snap["records"][kind::BREAK] = Value::Array(all(store, kind::BREAK)?);
    snap["commitments"] = json!({
        "days": days,
        "list": list,
        "next": next,
        "free": free_of(&fixed, &world, clock, &date),
        "conflicts": conflicts,
        "sleep": fixed.sleep,
        "term": {"start": fixed.term_start, "end": fixed.term_end},
        "drafts": drafts,
        "pending": pending(store)?,
        "feeds": feeds(store)?.iter().map(|f| json!({"id": f["id"], "name": f["name"], "lastSyncedAt": f["lastSyncedAt"]})).collect::<Vec<_>>(),
    });
    Ok(())
}
