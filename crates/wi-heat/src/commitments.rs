//! Commitments: the fixed things in a week (docs/COMMITMENTS.md). A class,
//! a work shift, a commute. Learn plans tasks around them, so it has to know
//! when they are, and this file is where that is worked out, with no I/O.
//!
//! - A commitment holds a time of day, a repeat rule (the RRULE subset of
//!   [`recurrence`]), the days it is active between, its exceptions (one day
//!   skipped or moved) and the minutes of travel before and after it.
//! - [`occurrences`] turns one into the days it really happens on: inside
//!   its range, never on a skipped day, and for a class never during a break
//!   and never past the term's last day.
//! - [`busy`] and [`free_time`] say what a day has left once commitments,
//!   their buffers and sleep are taken out. Plan my day reads only that.
//! - A schedule Claude read out of pasted text or a photo comes back as one
//!   JSON object ([`schedule_schema`]), checked here field by field
//!   ([`parse_schedule`]); an `.ics` file is read with [`from_ical`].
//! - Mail that says a class is canceled or moved is found by written rules
//!   ([`exception_in_mail`]), never applied: it waits for one tap.

use std::collections::BTreeSet;

use jiff::tz::TimeZone;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;

use crate::ical;
use crate::model::estimate::format_minutes;
use crate::model::format::{short_month_day, weekday_name};
use crate::model::records::ser;
use crate::model::recurrence::{self, SeriesStart};
use crate::model::zone::{add_days, day_key, days_between, minute_of_day, weekday_of, DayKey};

pub const DAY_MIN: f64 = 1440.0;

/// A capture taken this long after a class ends is still that class's.
pub const AFTER_CLASS_MIN: f64 = 30.0;

/// The longest travel buffer a commitment keeps on either side.
pub const BUFFER_MAX: f64 = 240.0;

const WEEKDAY_CODES: [&str; 7] = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Class,
    Work,
    Commute,
    #[default]
    Other,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Class => "class",
            Kind::Work => "work",
            Kind::Commute => "commute",
            Kind::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        match s.trim().to_ascii_lowercase().as_str() {
            "class" | "lecture" | "lab" | "course" => Some(Kind::Class),
            "work" | "shift" | "job" => Some(Kind::Work),
            "commute" | "travel" => Some(Kind::Commute),
            "other" => Some(Kind::Other),
            _ => None,
        }
    }
}

/// Fixed: it happens whatever the plan says. Flexible: it can give way, but
/// a plan still doesn't put work on top of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hardness {
    #[default]
    Fixed,
    Flexible,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExceptionKind {
    #[default]
    Skip,
    Move,
}

/// One day of a commitment that doesn't go as usual: skipped, or moved to
/// another time or day.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Exception {
    /// The day it would have happened.
    pub date: DayKey,
    pub kind: ExceptionKind,
    /// A move: the day it happens instead (the same day when absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_date: Option<DayKey>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser::opt_num"
    )]
    pub start: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser::opt_num"
    )]
    pub end: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// `you`, `mail` or `claude`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Commitment {
    pub id: String,
    pub title: String,
    pub kind: Kind,
    pub location: String,
    /// Minutes after the day's local midnight.
    #[serde(serialize_with = "ser::num")]
    pub start: f64,
    #[serde(serialize_with = "ser::num")]
    pub end: f64,
    /// None: it happens once, on `from`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rrule: Option<String>,
    /// The first day it can happen on.
    pub from: DayKey,
    /// The last day. A class with none ends with the term.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<DayKey>,
    pub exceptions: Vec<Exception>,
    #[serde(serialize_with = "ser::num")]
    pub buffer_before: f64,
    #[serde(serialize_with = "ser::num")]
    pub buffer_after: f64,
    pub hardness: Hardness,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub course_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
    /// `you`, `paste`, `photo`, `ics` or `claude`.
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// The Monday of the week a shift was pasted or photographed for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub week_of: Option<DayKey>,
    /// The subscribed `.ics` address it came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_id: Option<String>,
}

/// A stretch of days classes don't meet: a break, a holiday.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Break {
    pub id: String,
    pub title: String,
    pub from: DayKey,
    pub to: DayKey,
}

/// What classes follow besides their own rule: the breaks, and the term.
#[derive(Clone, Debug, Default)]
pub struct Academic<'a> {
    pub breaks: &'a [Break],
    pub term_start: Option<&'a str>,
    pub term_end: Option<&'a str>,
}

/// One day a commitment happens on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Occurrence {
    pub commitment_id: String,
    pub date: DayKey,
    #[serde(serialize_with = "ser::num")]
    pub start: f64,
    #[serde(serialize_with = "ser::num")]
    pub end: f64,
    pub title: String,
    pub kind: Kind,
    pub location: String,
    #[serde(serialize_with = "ser::num")]
    pub buffer_before: f64,
    #[serde(serialize_with = "ser::num")]
    pub buffer_after: f64,
    pub hardness: Hardness,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub course_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
    /// The day it was moved from, when an exception moved it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moved_from: Option<DayKey>,
}

impl Occurrence {
    /// Where its travel starts and ends: the stretch a plan keeps clear.
    pub fn span(&self) -> (f64, f64) {
        (
            (self.start - self.buffer_before).max(0.0),
            (self.end + self.buffer_after).min(DAY_MIN),
        )
    }
}

/// `YYYY-MM-DD`, and a real day.
pub fn is_day(s: &str) -> bool {
    s.len() == 10 && s.parse::<jiff::civil::Date>().is_ok()
}

/// A weekly rule for the days named ("MO", "WE", "FR"), every `interval`
/// weeks. None when a day isn't one.
pub fn weekly_rule(days: &[String], interval: f64) -> Option<String> {
    let mut codes: Vec<usize> = Vec::new();
    for d in days {
        let code = day_code(d)?;
        if !codes.contains(&code) {
            codes.push(code);
        }
    }
    if codes.is_empty() {
        return None;
    }
    // Monday first, as a timetable reads.
    codes.sort_by_key(|c| (c + 6) % 7);
    let by_day: Vec<&str> = codes.iter().map(|c| WEEKDAY_CODES[*c]).collect();
    let every = if interval > 1.0 {
        format!(";INTERVAL={}", interval as i64)
    } else {
        String::new()
    };
    Some(format!("FREQ=WEEKLY{every};BYDAY={}", by_day.join(",")))
}

/// A weekday however it is written: "MO", "Mon", "monday", "M", "Th", "R".
/// 0 is Sunday.
pub fn day_code(s: &str) -> Option<usize> {
    let d = s.trim().trim_end_matches('.').to_ascii_lowercase();
    Some(match d.as_str() {
        "su" | "sun" | "sunday" | "u" => 0,
        "mo" | "mon" | "monday" | "m" => 1,
        "tu" | "tue" | "tues" | "tuesday" | "t" => 2,
        "we" | "wed" | "weds" | "wednesday" | "w" => 3,
        "th" | "thu" | "thur" | "thurs" | "thursday" | "r" => 4,
        "fr" | "fri" | "friday" | "f" => 5,
        "sa" | "sat" | "saturday" | "s" => 6,
        _ => return None,
    })
}

/// The weekdays a stored rule names, as codes, Monday first; empty for a
/// rule that isn't weekly by day.
pub fn rule_days(rule: &str) -> Vec<&'static str> {
    let Some(r) = recurrence::parse_rule(rule) else {
        return Vec::new();
    };
    let mut days: Vec<usize> = r.by_day.iter().map(|d| *d as usize).collect();
    days.sort_by_key(|c| (c + 6) % 7);
    days.into_iter().map(|d| WEEKDAY_CODES[d]).collect()
}

/// A rule as RFC 5545 writes it, cut down to the parts Learn follows: what a
/// calendar adds beside them (WKST, a BYDAY with a position) is dropped or
/// refused rather than guessed at.
pub fn kept_rule(rule: &str) -> Option<String> {
    let body = rule.trim();
    let body = match body.get(..6) {
        Some(head) if head.eq_ignore_ascii_case("RRULE:") => &body[6..],
        _ => body,
    };
    let parts: Vec<&str> = body
        .split(';')
        .filter(|p| {
            let key = p.split('=').next().unwrap_or("").to_ascii_uppercase();
            matches!(
                key.as_str(),
                "FREQ" | "INTERVAL" | "BYDAY" | "BYMONTHDAY" | "COUNT" | "UNTIL"
            )
        })
        .collect();
    let kept = parts.join(";");
    recurrence::parse_rule(&kept).map(|_| kept)
}

fn in_break(day: &str, breaks: &[Break]) -> bool {
    breaks
        .iter()
        .any(|b| b.from.as_str() <= day && day <= b.to.as_str())
}

/// The last day a commitment can happen on: its own, and for a class the
/// term's when that is sooner or it has none.
pub fn last_day<'a>(c: &'a Commitment, academic: &Academic<'a>) -> Option<&'a str> {
    let own = c.until.as_deref().filter(|d| !d.is_empty());
    match (own, academic.term_end.filter(|_| c.kind == Kind::Class)) {
        (Some(a), Some(b)) => Some(if a <= b { a } else { b }),
        (Some(a), None) => Some(a),
        (None, b) => b,
    }
}

/// The days a commitment's own rule gives in `[from, to]`, before exceptions
/// and breaks.
fn rule_days_between(
    c: &Commitment,
    academic: &Academic,
    tz: &TimeZone,
    from: &str,
    to: &str,
) -> Vec<DayKey> {
    if !is_day(&c.from) {
        return Vec::new();
    }
    let lo = if c.from.as_str() > from {
        c.from.as_str()
    } else {
        from
    };
    let hi = match last_day(c, academic) {
        Some(last) if last < to => last,
        _ => to,
    };
    if lo > hi {
        return Vec::new();
    }
    let Some(text) = c.rrule.as_deref().filter(|r| !r.is_empty()) else {
        return if c.from.as_str() >= lo && c.from.as_str() <= hi {
            vec![c.from.clone()]
        } else {
            Vec::new()
        };
    };
    let Some(rule) = recurrence::parse_rule(text) else {
        return Vec::new();
    };
    let start = SeriesStart {
        day: c.from.clone(),
        minute: c.start,
    };
    recurrence::occurrences_between(&rule, &start, tz, lo, hi)
        .into_iter()
        .map(|o| o.date)
        .collect()
}

fn occurrence(c: &Commitment, date: &str, moved: Option<&Exception>) -> Occurrence {
    let length = c.end - c.start;
    let (start, end) = match moved {
        Some(e) => {
            let start = e.start.unwrap_or(c.start);
            (start, e.end.unwrap_or(start + length).min(DAY_MIN))
        }
        None => (c.start, c.end),
    };
    Occurrence {
        commitment_id: c.id.clone(),
        date: date.to_string(),
        start,
        end,
        title: c.title.clone(),
        kind: c.kind,
        location: moved
            .and_then(|e| e.location.clone())
            .unwrap_or_else(|| c.location.clone()),
        buffer_before: c.buffer_before,
        buffer_after: c.buffer_after,
        hardness: c.hardness,
        course_id: c.course_id.clone(),
        space_id: c.space_id.clone(),
        moved_from: moved.map(|e| e.date.clone()),
    }
}

/// Whether a commitment's own rule puts it on `day` (before exceptions; a
/// class's breaks and term count).
pub fn meets_on(c: &Commitment, academic: &Academic, tz: &TimeZone, day: &str) -> bool {
    !(c.kind == Kind::Class && in_break(day, academic.breaks))
        && rule_days_between(c, academic, tz, day, day)
            .iter()
            .any(|d| d == day)
}

/// The days one commitment happens on in `[from, to]`, in order.
pub fn occurrences(
    c: &Commitment,
    academic: &Academic,
    tz: &TimeZone,
    from: &str,
    to: &str,
) -> Vec<Occurrence> {
    let mut out = Vec::new();
    for day in rule_days_between(c, academic, tz, from, to) {
        if c.kind == Kind::Class && in_break(&day, academic.breaks) {
            continue;
        }
        // The last exception written for a day is the one that holds.
        if c.exceptions.iter().any(|e| e.date == day) {
            continue;
        }
        out.push(occurrence(c, &day, None));
    }
    // A moved day lands where it was moved to, which may be outside the
    // rule's own days, so it is looked for by where it goes.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for e in c.exceptions.iter().rev() {
        if !seen.insert(e.date.as_str()) || e.kind != ExceptionKind::Move {
            continue;
        }
        let lands = e.to_date.as_deref().unwrap_or(&e.date);
        if lands < from || lands > to {
            continue;
        }
        if meets_on(c, academic, tz, &e.date) {
            out.push(occurrence(c, lands, Some(e)));
        }
    }
    out.sort_by(|a, b| a.date.cmp(&b.date).then(a.start.total_cmp(&b.start)));
    out
}

/// Every commitment's days in `[from, to]`, by day and then by start.
pub fn all_occurrences(
    commitments: &[Commitment],
    academic: &Academic,
    tz: &TimeZone,
    from: &str,
    to: &str,
) -> Vec<Occurrence> {
    let mut out: Vec<Occurrence> = commitments
        .iter()
        .flat_map(|c| occurrences(c, academic, tz, from, to))
        .collect();
    out.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then(a.start.total_cmp(&b.start))
            .then(a.commitment_id.cmp(&b.commitment_id))
    });
    out
}

/// When the person sleeps, in minutes after midnight: to bed at `from`, up
/// at `to`. 11 PM to 7 AM unless they say otherwise, which is the stretch
/// Today's column never showed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sleep {
    #[serde(serialize_with = "ser::num")]
    pub from: f64,
    #[serde(serialize_with = "ser::num")]
    pub to: f64,
}

impl Default for Sleep {
    fn default() -> Sleep {
        Sleep {
            from: 23.0 * 60.0,
            to: 7.0 * 60.0,
        }
    }
}

impl Sleep {
    /// The minutes of one day spent asleep.
    pub fn spans(&self) -> Vec<(f64, f64)> {
        let (from, to) = (self.from.clamp(0.0, DAY_MIN), self.to.clamp(0.0, DAY_MIN));
        if from == to {
            Vec::new()
        } else if from > to {
            // Over midnight: the small hours, and the end of the evening.
            vec![(0.0, to), (from, DAY_MIN)]
                .into_iter()
                .filter(|(a, b)| b > a)
                .collect()
        } else {
            vec![(from, to)]
        }
    }
}

/// Spans sorted and joined where they touch or overlap.
pub fn merged(mut spans: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    spans.retain(|(a, b)| b > a);
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (a, b) in spans {
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// The minutes of `date` that commitments and their buffers take.
pub fn busy(occurrences: &[Occurrence], date: &str) -> Vec<(f64, f64)> {
    merged(
        occurrences
            .iter()
            .filter(|o| o.date == date)
            .map(Occurrence::span)
            .collect(),
    )
}

/// What a day has left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeTime {
    pub date: DayKey,
    /// Minutes awake and uncommitted, from `from` to the day's end.
    #[serde(serialize_with = "ser::num")]
    pub free_min: f64,
    /// Minutes of task and habit blocks in the same stretch.
    #[serde(serialize_with = "ser::num")]
    pub planned_min: f64,
    /// How much more is planned than there is room for; 0 when it fits.
    #[serde(serialize_with = "ser::num")]
    pub over_min: f64,
    /// The free stretches, in order.
    pub spans: Vec<(f64, f64)>,
    /// "3h 20m free today. Planned 4h. Move 40m?"
    pub line: String,
}

/// `[from, 1440)` with `taken` cut out.
fn left_over(from: f64, taken: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut at = from;
    for (a, b) in merged(taken.to_vec()) {
        if b <= at {
            continue;
        }
        if a > at {
            out.push((at, a));
        }
        at = at.max(b);
    }
    if at < DAY_MIN {
        out.push((at, DAY_MIN));
    }
    out
}

/// A day's free time: the day from `from_min` on (now, today; 0 on a day
/// ahead), minus `taken` (commitments with their buffers, timed events from
/// other calendars) and minus sleep. `blocks` are the task and habit blocks
/// of the day as `(start, minutes)`; `day_word` is "today" or a weekday.
pub fn free_time(
    date: &str,
    from_min: f64,
    taken: &[(f64, f64)],
    sleep: &Sleep,
    blocks: &[(f64, f64)],
    day_word: &str,
) -> FreeTime {
    let from = from_min.clamp(0.0, DAY_MIN);
    let mut all = taken.to_vec();
    all.extend(sleep.spans());
    let spans = left_over(from, &all);
    let free: f64 = spans.iter().map(|(a, b)| b - a).sum();
    let planned: f64 = blocks
        .iter()
        .map(|(start, minutes)| ((start + minutes).min(DAY_MIN) - start.max(from)).max(0.0))
        .sum();
    let over = (planned - free).max(0.0);
    let mut line = format!("{} free {day_word}.", format_minutes(free));
    if planned > 0.0 {
        line.push_str(&format!(" Planned {}.", format_minutes(planned)));
    }
    if over > 0.0 {
        line.push_str(&format!(" Move {}?", format_minutes(over)));
    }
    FreeTime {
        date: date.to_string(),
        free_min: free,
        planned_min: planned,
        over_min: over,
        spans,
        line,
    }
}

/// A task block that sits on a commitment, or on the travel around one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub block_id: String,
    pub commitment_id: String,
    pub date: DayKey,
    /// True when it only reaches into the travel time, not the thing itself.
    pub buffer_only: bool,
    /// "Overlaps JPN 101 (10:00 AM to 10:50 AM)."
    pub line: String,
}

/// Each block of `date` (`(id, start, minutes)`) that overlaps a commitment.
pub fn conflicts(
    blocks: &[(String, f64, f64)],
    occurrences: &[Occurrence],
    date: &str,
) -> Vec<Conflict> {
    let mut out = Vec::new();
    for (id, start, minutes) in blocks {
        let (a, b) = (*start, start + minutes);
        for o in occurrences.iter().filter(|o| o.date == date) {
            let (lo, hi) = o.span();
            if a >= hi || b <= lo {
                continue;
            }
            let buffer_only = a >= o.end || b <= o.start;
            let line = if buffer_only {
                format!("Overlaps the travel time for {}.", o.title)
            } else {
                format!(
                    "Overlaps {} ({} to {}).",
                    o.title,
                    crate::model::format::clock(o.start),
                    crate::model::format::clock(o.end)
                )
            };
            out.push(Conflict {
                block_id: id.clone(),
                commitment_id: o.commitment_id.clone(),
                date: date.to_string(),
                buffer_only,
                line,
            });
            break;
        }
    }
    out
}

/// "10:00", "9:35": a clock time as the readout writes it, with no AM or PM.
pub fn short_clock(minutes: f64) -> String {
    let m = minutes.round().rem_euclid(DAY_MIN);
    let h = (m / 60.0).floor();
    let h12 = if h % 12.0 == 0.0 { 12.0 } else { h % 12.0 };
    format!("{}:{:02}", h12 as i64, (m % 60.0) as i64)
}

/// What is next today, for the readout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Next {
    pub commitment_id: String,
    pub title: String,
    pub date: DayKey,
    #[serde(serialize_with = "ser::num")]
    pub start: f64,
    /// When to leave: the start of its travel time. None with no buffer.
    #[serde(serialize_with = "ser::opt_num")]
    pub leave_at: Option<f64>,
    /// "NEXT JPN 101 10:00 · LEAVE 9:35"
    pub line: String,
}

/// The next commitment of `date` that hasn't started at `now_min`.
pub fn next_up(occurrences: &[Occurrence], date: &str, now_min: f64) -> Option<Next> {
    let o = occurrences
        .iter()
        .filter(|o| o.date == date && o.start > now_min)
        .min_by(|a, b| a.start.total_cmp(&b.start))?;
    let leave_at = (o.buffer_before > 0.0).then(|| (o.start - o.buffer_before).max(0.0));
    let mut line = format!("NEXT {} {}", o.title.to_uppercase(), short_clock(o.start));
    if let Some(leave) = leave_at {
        line.push_str(&format!(" · LEAVE {}", short_clock(leave)));
    }
    Some(Next {
        commitment_id: o.commitment_id.clone(),
        title: o.title.clone(),
        date: date.to_string(),
        start: o.start,
        leave_at,
        line,
    })
}

/// The commitments of `date` whose travel time has started at `now_min` and
/// that haven't started themselves: it is time to leave.
pub fn leaving<'a>(occurrences: &'a [Occurrence], date: &str, now_min: f64) -> Vec<&'a Occurrence> {
    occurrences
        .iter()
        .filter(|o| {
            o.date == date
                && o.buffer_before > 0.0
                && o.start - o.buffer_before <= now_min
                && now_min < o.start
        })
        .collect()
}

/// "Time to leave for JPN 101 (10:00 AM)."
pub fn leave_line(o: &Occurrence) -> String {
    let place = if o.location.trim().is_empty() {
        String::new()
    } else {
        format!(", {}", o.location.trim())
    };
    format!(
        "Time to leave for {} ({}{place}).",
        o.title,
        crate::model::format::clock(o.start)
    )
}

/// The class a moment falls in: during it, or within thirty minutes after.
/// A photo of a notebook page taken then is that class's.
pub fn class_at<'a>(
    occurrences: &'a [Occurrence],
    date: &str,
    minute: f64,
) -> Option<&'a Occurrence> {
    occurrences
        .iter()
        .filter(|o| {
            o.date == date
                && o.kind == Kind::Class
                && minute >= o.start
                && minute <= o.end + AFTER_CLASS_MIN
        })
        // Two classes back to back: the one that is on, else the one just over.
        .min_by(|a, b| {
            let on = |o: &Occurrence| if minute <= o.end { 0 } else { 1 };
            on(a).cmp(&on(b)).then(b.start.total_cmp(&a.start))
        })
}

/// "JPN 101 · Oct 7": what a page captured in a class is called.
pub fn capture_title(label: &str, date: &str) -> String {
    format!("{label} · {}", short_month_day(date))
}

/// "Thursday, Oct 8"
pub fn day_words(date: &str) -> String {
    format!(
        "{}, {}",
        weekday_name(weekday_of(date)),
        short_month_day(date)
    )
}

/// "MWF 10:00 AM to 10:50 AM", "Sat, Oct 10, 9:00 AM to 5:00 PM".
pub fn when_line(c: &Commitment) -> String {
    let times = format!(
        "{} to {}",
        crate::model::format::clock(c.start),
        crate::model::format::clock(c.end)
    );
    match c.rrule.as_deref().filter(|r| !r.is_empty()) {
        None => format!("{}, {times}", {
            let name = weekday_name(weekday_of(&c.from));
            format!(
                "{} {}",
                &name[..3.min(name.len())],
                short_month_day(&c.from)
            )
        }),
        Some(rule) => {
            let days = rule_days(rule);
            if days.is_empty() {
                return format!("Repeats, {times}");
            }
            let letters: String = days
                .iter()
                .map(|d| match *d {
                    "MO" => "M",
                    "TU" => "Tu",
                    "WE" => "W",
                    "TH" => "Th",
                    "FR" => "F",
                    "SA" => "Sa",
                    _ => "Su",
                })
                .collect();
            format!("{letters} {times}")
        }
    }
}

// ----- a schedule from Claude -----

/// How a pasted or photographed schedule is to be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// A timetable: what repeats every week of the term.
    Schedule,
    /// This week's shifts: each on its own day, replacing that week only.
    Week,
    /// The academic calendar: breaks and holidays.
    Breaks,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "schedule" => Some(Mode::Schedule),
            "week" => Some(Mode::Week),
            "breaks" => Some(Mode::Breaks),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Schedule => "schedule",
            Mode::Week => "week",
            Mode::Breaks => "breaks",
        }
    }
}

/// The Monday of the week `day` is in.
pub fn monday_of(day: &str) -> DayKey {
    add_days(day, -((weekday_of(day) + 6.0) % 7.0))
}

/// One line of a schedule, as it will be previewed: nothing is a commitment
/// until the person applies it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Item {
    pub title: String,
    pub kind: Kind,
    /// Weekday codes for something weekly; empty for a single day.
    pub days: Vec<String>,
    /// The one day, for something that happens once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<DayKey>,
    #[serde(serialize_with = "ser::num")]
    pub start: f64,
    #[serde(serialize_with = "ser::num")]
    pub end: f64,
    pub location: String,
    /// A course code, when it is a class.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub course: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<DayKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<DayKey>,
    /// A rule kept whole, when it came from a calendar file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rrule: Option<String>,
    /// Days the rule skips, from a calendar file's EXDATEs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<DayKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
}

/// A schedule read and checked: what it holds, and how many of its lines
/// couldn't be read.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    pub items: Vec<Item>,
    pub breaks: Vec<Break>,
    pub unread: usize,
}

/// The JSON Claude answers with for any of the three jobs.
pub fn schedule_schema() -> Value {
    let day = json!({"type": ["string", "null"], "description": "YYYY-MM-DD, or null."});
    json!({
        "type": "object",
        "properties": {
            "items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string", "description": "A class by its course code (JPN 101); a shift by the place or the job."},
                        "kind": {"type": "string", "enum": ["class", "work", "commute", "other"]},
                        "days": {"type": ["array", "null"], "items": {"type": "string", "enum": ["MO", "TU", "WE", "TH", "FR", "SA", "SU"]}, "description": "The weekdays, for something that repeats every week. Null for something on one day."},
                        "date": day,
                        "start": {"type": "string", "description": "24-hour HH:MM."},
                        "end": {"type": "string", "description": "24-hour HH:MM."},
                        "location": {"type": ["string", "null"]},
                        "course": {"type": ["string", "null"], "description": "The course code, for a class."},
                        "from": day,
                        "until": day
                    },
                    "required": ["title", "kind", "days", "date", "start", "end", "location", "course", "from", "until"],
                    "additionalProperties": false
                }
            },
            "breaks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"},
                        "from": {"type": "string", "description": "The first day off, YYYY-MM-DD."},
                        "to": {"type": "string", "description": "The last day off, YYYY-MM-DD. The same day for a single holiday."}
                    },
                    "required": ["title", "from", "to"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["items", "breaks"],
        "additionalProperties": false
    })
}

/// What Claude is told. `source` is the pasted text, or for a photo the
/// name of the file it is to open.
pub fn schedule_prompt(
    mode: Mode,
    source: &str,
    is_image: bool,
    today: &str,
    week_of: &str,
    courses: &[String],
    term: (Option<&str>, Option<&str>),
) -> String {
    let what = if is_image {
        format!("Open the image file {source} in this folder and read it.")
    } else {
        "Read the text between the lines of dashes below.".to_string()
    };
    let job = match mode {
        Mode::Schedule => "It is a timetable: classes, work or anything that happens at the same time every week. Give one item for each thing with the weekdays it meets on in `days`, and `date` null. Two meeting patterns of one class (a lecture MWF and a lab on Thursday) are two items. Leave `breaks` empty.".to_string(),
        Mode::Week => format!(
            "It is one week's work shifts. Give one item for each shift, on its own day: `date` is that day and `days` is null. The week is Monday {week_of} to Sunday {}. A shift with a weekday and no date is on that weekday of this week. Use kind \"work\" unless it plainly isn't. Leave `breaks` empty.",
            add_days(week_of, 6.0)
        ),
        Mode::Breaks => "It is an academic calendar. Give every stretch of days with no classes (a break, a holiday, a reading day) in `breaks`, and leave `items` empty. Days when classes still meet (the first day of classes, an add/drop deadline, exams) are not breaks.".to_string(),
    };
    let weekday = weekday_name(weekday_of(today));
    let known = if courses.is_empty() {
        String::new()
    } else {
        format!(
            " The person's courses: {}. Write a class's code the way it is written there.",
            courses.join(", ")
        )
    };
    let term_line = match term {
        (Some(a), Some(b)) => format!(" The term runs {a} to {b}."),
        _ => String::new(),
    };
    let mut prompt = format!(
        "{what} {job}\n\nToday is {weekday}, {today}.{term_line}{known}\n\nTimes are 24-hour HH:MM. A time with no AM or PM is read the way a timetable means it (1:00 for a class is 13:00). Give only what is written: a field the source doesn't give is null, and nothing is invented. If nothing in it is a schedule, answer with both lists empty."
    );
    if !is_image {
        prompt.push_str(&format!("\n\n----------\n{source}\n----------"));
    }
    prompt
}

/// "10:00", "9:5", "1700", "5pm", "5:30 PM" as minutes after midnight.
pub fn minutes_of(text: &str) -> Option<f64> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(\d{1,2})(?:[:.]?(\d{2}))?\s*(a\.?m\.?|p\.?m\.?)?\s*$").expect("valid")
    });
    let caps = re.captures(text)?;
    let mut hour: f64 = caps.get(1)?.as_str().parse().ok()?;
    let minute: f64 = caps.get(2).map_or(Some(0.0), |m| m.as_str().parse().ok())?;
    if minute >= 60.0 {
        return None;
    }
    match caps.get(3).map(|m| m.as_str().to_ascii_lowercase()) {
        Some(half) => {
            if !(1.0..=12.0).contains(&hour) {
                return None;
            }
            let pm = half.starts_with('p');
            hour = (hour % 12.0) + if pm { 12.0 } else { 0.0 };
        }
        None if hour > 24.0 => return None,
        None => {}
    }
    let total = hour * 60.0 + minute;
    (total <= DAY_MIN).then_some(total)
}

fn opt_day(v: &Value) -> Option<DayKey> {
    v.as_str()
        .map(str::trim)
        .filter(|d| is_day(d))
        .map(String::from)
}

/// Claude's answer, checked. A line that doesn't read is counted and left
/// out; it never becomes a commitment at a made-up time.
pub fn parse_schedule(answer: &Value, mode: Mode, week_of: &str) -> Schedule {
    let mut out = Schedule::default();
    for raw in answer["items"].as_array().map_or(&[][..], Vec::as_slice) {
        let title = raw["title"].as_str().unwrap_or("").trim().to_string();
        let (start, end) = (
            raw["start"].as_str().and_then(minutes_of),
            raw["end"].as_str().and_then(minutes_of),
        );
        let (Some(start), Some(end)) = (start, end) else {
            out.unread += 1;
            continue;
        };
        if title.is_empty() || end <= start {
            out.unread += 1;
            continue;
        }
        let mut days: Vec<String> = Vec::new();
        for d in raw["days"].as_array().map_or(&[][..], Vec::as_slice) {
            if let Some(code) = d.as_str().and_then(day_code) {
                let code = WEEKDAY_CODES[code].to_string();
                if !days.contains(&code) {
                    days.push(code);
                }
            }
        }
        let mut date = opt_day(&raw["date"]);
        if mode == Mode::Week {
            // A week's shifts are each on a day of that week: a weekday with
            // no date is that weekday of the week.
            if date.is_none() {
                if let [only] = days.as_slice() {
                    let n = day_code(only).unwrap_or(1);
                    date = Some(add_days(week_of, ((n + 6) % 7) as f64));
                }
            }
            days.clear();
        }
        if days.is_empty() && date.is_none() {
            out.unread += 1;
            continue;
        }
        if !days.is_empty() {
            date = None;
        }
        let course = raw["course"]
            .as_str()
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(String::from);
        let kind = raw["kind"]
            .as_str()
            .and_then(Kind::parse)
            .unwrap_or(if course.is_some() {
                Kind::Class
            } else {
                Kind::Other
            });
        out.items.push(Item {
            title,
            kind,
            days,
            date,
            start,
            end,
            location: raw["location"].as_str().unwrap_or("").trim().to_string(),
            course,
            from: opt_day(&raw["from"]),
            until: opt_day(&raw["until"]),
            rrule: None,
            skip: Vec::new(),
            source_id: None,
        });
    }
    for raw in answer["breaks"].as_array().map_or(&[][..], Vec::as_slice) {
        let title = raw["title"].as_str().unwrap_or("").trim().to_string();
        match (opt_day(&raw["from"]), opt_day(&raw["to"])) {
            (Some(from), Some(to)) if from <= to && !title.is_empty() => out.breaks.push(Break {
                id: String::new(),
                title,
                from,
                to,
            }),
            _ => out.unread += 1,
        }
    }
    out
}

/// An `.ics` file as a schedule. Timed events become items, with the rule
/// they repeat by; whole-day events are the days off of an academic calendar.
/// A canceled event is left out.
pub fn from_ical(events: &[ical::Event], mode: Mode, tz: &TimeZone) -> Schedule {
    let mut out = Schedule::default();
    let ms = |t: jiff::Timestamp| t.as_millisecond() as f64;
    for e in events {
        if e.status.as_deref() == Some("CANCELLED") {
            continue;
        }
        let title = e.summary.trim().to_string();
        match (e.start, e.end) {
            (Some(ical::When::Date(first)), end) => {
                if mode != Mode::Breaks || title.is_empty() {
                    continue;
                }
                let from = first.to_string();
                // A whole-day event ends on the day after its last (RFC 5545).
                let to = match end {
                    Some(ical::When::Date(d)) if d > first => add_days(&d.to_string(), -1.0),
                    _ => from.clone(),
                };
                out.breaks.push(Break {
                    id: String::new(),
                    title,
                    from,
                    to,
                });
            }
            (Some(ical::When::At(start)), end) => {
                if mode == Mode::Breaks {
                    continue;
                }
                let day = day_key(ms(start), tz);
                let start_min = minute_of_day(ms(start), tz);
                let end_min = match end {
                    Some(ical::When::At(t)) if t > start => {
                        // One that runs past midnight stops there.
                        if day_key(ms(t), tz) == day {
                            minute_of_day(ms(t), tz)
                        } else {
                            DAY_MIN
                        }
                    }
                    _ => (start_min + 60.0).min(DAY_MIN),
                };
                if title.is_empty() || end_min <= start_min {
                    out.unread += 1;
                    continue;
                }
                let rule = match e.rrule.as_deref() {
                    None => None,
                    Some(r) => match kept_rule(r) {
                        Some(kept) => Some(kept),
                        None => {
                            out.unread += 1;
                            continue;
                        }
                    },
                };
                let skip: Vec<DayKey> = e
                    .exdates
                    .iter()
                    .map(|w| match w {
                        ical::When::Date(d) => d.to_string(),
                        ical::When::At(t) => day_key(ms(*t), tz),
                    })
                    .collect();
                out.items.push(Item {
                    title,
                    kind: Kind::Other,
                    days: rule
                        .as_deref()
                        .map(rule_days)
                        .unwrap_or_default()
                        .iter()
                        .map(|d| d.to_string())
                        .collect(),
                    date: rule.is_none().then(|| day.clone()),
                    start: start_min,
                    end: end_min,
                    location: e.location.trim().to_string(),
                    course: None,
                    from: Some(day),
                    until: None,
                    rrule: rule,
                    skip,
                    source_id: e.uid.clone().filter(|u| !u.is_empty()),
                });
            }
            _ => {}
        }
    }
    out
}

/// "3 classes and 2 shifts", for the preview's line.
pub fn count_line(items: &[Item], breaks: usize) -> String {
    let count = |k: Kind| items.iter().filter(|i| i.kind == k).count();
    let mut parts = Vec::new();
    let mut say = |n: usize, one: &str, many: &str| {
        if n > 0 {
            parts.push(format!("{n} {}", if n == 1 { one } else { many }));
        }
    };
    say(count(Kind::Class), "class", "classes");
    say(count(Kind::Work), "shift", "shifts");
    say(count(Kind::Commute), "commute", "commutes");
    say(count(Kind::Other), "commitment", "commitments");
    say(breaks, "break", "breaks");
    match parts.len() {
        0 => "Nothing to add".to_string(),
        1 => parts.remove(0),
        _ => {
            let last = parts.pop().unwrap_or_default();
            format!("{} and {last}", parts.join(", "))
        }
    }
}

// ----- mail that says a class is canceled or moved -----

/// What Learn knows of a mail thread: no more than Mail shows.
#[derive(Clone, Debug, Default)]
pub struct MailSeen<'a> {
    pub subject: &'a str,
    pub text: &'a str,
    /// The course code Claude gave the thread, if any.
    pub course: Option<&'a str>,
    pub received_at: f64,
}

/// An exception a mail asks for. It is only ever offered.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub commitment_id: String,
    pub exception: Exception,
    /// "JPN 101 is canceled Thursday, Oct 8. Skip it?"
    pub line: String,
    /// What the one tap is called: "Skip it", "Move it".
    pub act: &'static str,
}

fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

fn cancel_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(cancel+ed|cancel+ing|cancel+ation|cancel|no class|no lecture|no lab|class will not meet|will not meet|won'?t meet|not meeting|will not be held|won'?t be held|is off|called off)\b").expect("valid")
    })
}

fn move_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(moved|moving|rescheduled|rescheduling|postponed|pushed back|pushed|will meet at|will start at|starts at|time change|room change|relocated)\b").expect("valid")
    })
}

fn negated_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(not|isn'?t|is not|no longer|never|won'?t be|will not be)\s+(be\s+)?(cancel+ed|moved|rescheduled|postponed)\b").expect("valid")
    })
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// The days a text names, in the order it names them, counted from the day
/// the mail came: "today", "tomorrow", a weekday, "Oct 9", "10/9".
fn days_named(text: &str, received: &str) -> Vec<DayKey> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)\b(today|tonight|this (?:morning|afternoon|evening)|tomorrow|sunday|monday|tuesday|wednesday|thursday|friday|saturday|sun|mon|tues?|wed|thurs?|fri|sat|(?:jan|feb|mar|apr|may|jun|jul|aug|sept?|oct|nov|dec)[a-z]*\.?\s+\d{1,2}|\d{1,2}/\d{1,2})\b").expect("valid")
    });
    let (year, _, _) = crate::model::zone::key_parts(received);
    let mut out: Vec<DayKey> = Vec::new();
    for m in re.find_iter(text) {
        let word = m.as_str().to_lowercase();
        let day = if word == "today" || word == "tonight" || word.starts_with("this ") {
            Some(received.to_string())
        } else if word == "tomorrow" {
            Some(add_days(received, 1.0))
        } else if let Some(n) = day_code(&word) {
            let ahead = (n as f64 - weekday_of(received)).rem_euclid(7.0);
            Some(add_days(received, ahead))
        } else if let Some((m, d)) = word.split_once('/') {
            month_day(year, m.parse().ok(), d.parse().ok(), received)
        } else {
            let mut parts = word.split_whitespace();
            let name = parts.next().unwrap_or("");
            let month = MONTHS
                .iter()
                .position(|mm| name.starts_with(mm))
                .map(|i| i as f64 + 1.0);
            month_day(
                year,
                month,
                parts.next().and_then(|d| d.parse().ok()),
                received,
            )
        };
        if let Some(day) = day {
            if !out.contains(&day) {
                out.push(day);
            }
        }
    }
    out
}

/// A month and day as the next such day on or after the mail's.
fn month_day(year: f64, month: Option<f64>, day: Option<f64>, received: &str) -> Option<DayKey> {
    let (month, day) = (month?, day?);
    let this = crate::model::zone::key_of(year, month, day);
    if !is_day(&this) {
        return None;
    }
    // A date a few days back is still this year's; one long past is next year's.
    if days_between(&this, received) > 60.0 {
        let next = crate::model::zone::key_of(year + 1.0, month, day);
        return is_day(&next).then_some(next);
    }
    Some(this)
}

/// A new time in what follows "moved": "to 2 PM", "at 3:30pm", "to 14:00".
fn time_after(text: &str) -> Option<f64> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)\b(?:to|at|for|until)\s+(\d{1,2}(?::\d{2})?\s*(?:a\.?m\.?|p\.?m\.?)|\d{1,2}:\d{2})").expect("valid")
    });
    re.captures_iter(text)
        .filter_map(|c| minutes_of(c.get(1)?.as_str()))
        .next()
}

/// Which commitment a mail is about: the one whose course Claude named, else
/// the only one whose title or course code the mail writes out.
fn about<'a>(
    mail: &MailSeen,
    commitments: &'a [Commitment],
    course_code: &dyn Fn(&str) -> Option<String>,
) -> Vec<&'a Commitment> {
    let code_of = |c: &Commitment| c.course_id.as_deref().and_then(course_code);
    if let Some(course) = mail.course.map(squash).filter(|c| !c.is_empty()) {
        let named: Vec<&Commitment> = commitments
            .iter()
            .filter(|c| {
                code_of(c).is_some_and(|code| squash(&code) == course) || squash(&c.title) == course
            })
            .collect();
        if !named.is_empty() {
            return named;
        }
    }
    let all = squash(&format!("{} {}", mail.subject, mail.text));
    commitments
        .iter()
        .filter(|c| {
            let title = squash(&c.title);
            let code = code_of(c).map(|code| squash(&code)).unwrap_or_default();
            (title.len() >= 4 && all.contains(&title)) || (code.len() >= 4 && all.contains(&code))
        })
        .collect()
}

/// Reads a mail for a class that is canceled or moved. `course_code` gives
/// a course's code for its id. None unless the mail says so plainly and
/// names something Learn holds, on a day it really meets.
pub fn exception_in_mail(
    mail: &MailSeen,
    commitments: &[Commitment],
    academic: &Academic,
    tz: &TimeZone,
    course_code: &dyn Fn(&str) -> Option<String>,
) -> Option<Found> {
    let text = format!("{}\n{}", mail.subject, mail.text);
    if negated_re().is_match(&text) {
        return None;
    }
    let canceled = cancel_re().find(&text);
    let moved = move_re().find(&text);
    if canceled.is_none() && moved.is_none() {
        return None;
    }
    let candidates = about(mail, commitments, course_code);
    if candidates.is_empty() {
        return None;
    }
    let received = day_key(mail.received_at, tz);
    let received_min = minute_of_day(mail.received_at, tz);
    let named = days_named(&text, &received);
    // The day: the first one named that it meets on; with none named, the
    // next time it meets, counting today if it hasn't started.
    let horizon = add_days(&received, 21.0);
    let mut best: Option<(&Commitment, DayKey)> = None;
    for c in &candidates {
        let meets: Vec<Occurrence> = occurrences(c, academic, tz, &received, &horizon);
        let day = if named.is_empty() {
            meets
                .iter()
                .find(|o| o.date > received || o.start > received_min)
                .map(|o| o.date.clone())
        } else {
            named
                .iter()
                .find(|d| meets.iter().any(|o| &o.date == *d))
                .cloned()
        };
        if let Some(day) = day {
            if best.as_ref().map_or(true, |(_, d)| day < *d) {
                best = Some((c, day));
            }
        }
    }
    let (c, day) = best?;
    let source = Some("mail".to_string());
    // A move needs somewhere to go: a new time, or another day it names.
    if canceled.is_none() {
        let after = &text[moved.map_or(0, |m| m.start())..];
        let to_time = time_after(after);
        let to_date = named.iter().find(|d| **d != day && **d > received).cloned();
        if to_time.is_some() || to_date.is_some() {
            let start = to_time.unwrap_or(c.start);
            let end = (start + (c.end - c.start)).min(DAY_MIN);
            let mut to = String::new();
            if to_time.is_some() {
                to.push_str(&crate::model::format::clock(start));
            }
            if let Some(d) = &to_date {
                if !to.is_empty() {
                    to.push_str(" on ");
                }
                to.push_str(&day_words(d));
            }
            return Some(Found {
                commitment_id: c.id.clone(),
                line: format!("{} on {} moves to {to}. Move it?", c.title, day_words(&day)),
                exception: Exception {
                    date: day,
                    kind: ExceptionKind::Move,
                    to_date,
                    start: to_time,
                    end: to_time.map(|_| end),
                    location: None,
                    note: Some(mail.subject.trim().to_string()).filter(|s| !s.is_empty()),
                    source,
                },
                act: "Move it",
            });
        }
        return Some(Found {
            commitment_id: c.id.clone(),
            line: format!(
                "{} on {} has moved, and the mail doesn't say where to. Skip the usual time?",
                c.title,
                day_words(&day)
            ),
            exception: Exception {
                date: day,
                kind: ExceptionKind::Skip,
                note: Some(mail.subject.trim().to_string()).filter(|s| !s.is_empty()),
                source,
                ..Exception::default()
            },
            act: "Skip it",
        });
    }
    Some(Found {
        commitment_id: c.id.clone(),
        line: format!("{} is canceled {}. Skip it?", c.title, day_words(&day)),
        exception: Exception {
            date: day,
            kind: ExceptionKind::Skip,
            note: Some(mail.subject.trim().to_string()).filter(|s| !s.is_empty()),
            source,
            ..Exception::default()
        },
        act: "Skip it",
    })
}
