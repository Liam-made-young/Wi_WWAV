//! Recurrence: the RRULE subset Heat needs, as the PKM stores it
//! (`docs/SPEC.md` 3.6). FREQ is DAILY, WEEKLY, MONTHLY or YEARLY, with
//! INTERVAL, BYDAY (plain weekdays), BYMONTHDAY, COUNT and UNTIL, read as RFC
//! 5545 reads them. Anything else is refused, never guessed at.
//!
//! The series starts at the task's due date, at its wall-clock time in the
//! person's zone, and every occurrence keeps that wall-clock time across DST.
//! A task with a rule but no due date runs from its scheduled date; setting a
//! rule on a task with neither pins today as its scheduled date, as the PKM's
//! task panel does. Each finished occurrence is a [`TaskOccurrence`] row; the
//! series never flips to done. A port of `recurrence.ts` (its preset menu's
//! labels stay in TypeScript).

use super::js;
use super::records::{ser, Id, Task, TaskOccurrence};
use super::zone::{
    add_days, at_minute, day_key, days_between, days_in_month, key_of, key_parts, minute_of_day,
    weekday_of, DayKey,
};
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// Where a series ends: an instant, or the end of a day.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Until {
    Instant {
        #[serde(serialize_with = "ser::num")]
        instant: f64,
    },
    Day {
        day: DayKey,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub freq: Freq,
    #[serde(serialize_with = "ser::num")]
    pub interval: f64,
    /// Weekdays, 0 for Sunday.
    pub by_day: Vec<i32>,
    /// Days of the month; -1 is the last.
    pub by_month_day: Vec<i32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser::opt_num"
    )]
    pub count: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<Until>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Occurrence {
    pub date: DayKey,
    #[serde(serialize_with = "ser::num")]
    pub at: f64,
}

/// Where a series starts: a day and the minute of that day.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeriesStart {
    pub day: DayKey,
    #[serde(serialize_with = "ser::num")]
    pub minute: f64,
}

const WEEKDAY_CODES: [&str; 7] = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];
// A rule that never matches (the 31st, every 12 months, from February) would
// otherwise search forever; a hundred years is past any plan.
const HORIZON_DAYS: f64 = 36_525.0;

fn parse_until(v: &str) -> Option<Until> {
    let b = v.as_bytes();
    let all_digits = |s: &[u8]| s.iter().all(u8::is_ascii_digit);
    let num = |s: &str| js::to_number(s);
    match b.len() {
        8 if all_digits(b) => Some(Until::Day {
            day: format!("{}-{}-{}", &v[0..4], &v[4..6], &v[6..8]),
        }),
        16 if all_digits(&b[0..8]) && b[8] == b'T' && all_digits(&b[9..15]) && b[15] == b'Z' => {
            Some(Until::Instant {
                instant: js::date_utc(
                    num(&v[0..4]),
                    num(&v[4..6]) - 1.0,
                    num(&v[6..8]),
                    num(&v[9..11]),
                    num(&v[11..13]),
                    num(&v[13..15]),
                ),
            })
        }
        _ => None,
    }
}

/// `/^-?\d+$/`
fn is_int(v: &str) -> bool {
    let digits = v.strip_prefix('-').unwrap_or(v);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Reads a stored rule, with or without "RRULE:"; None when it is outside the subset.
pub fn parse_rule(text: &str) -> Option<Rule> {
    let trimmed = js::trim(text);
    // `.replace(/^RRULE:/i, '')`
    let body = match trimmed.get(..6) {
        Some(head) if head.eq_ignore_ascii_case("RRULE:") => &trimmed[6..],
        _ => trimmed,
    };
    if body.is_empty() {
        return None;
    }
    let mut freq = None;
    let mut interval = 1.0;
    let mut by_day: Vec<i32> = vec![];
    let mut by_month_day: Vec<i32> = vec![];
    let mut count = None;
    let mut until = None;
    for part in body.split(';') {
        let mut pieces = part.split('=');
        let key = pieces.next().unwrap_or("");
        let value = pieces.next().unwrap_or("");
        match key.to_uppercase().as_str() {
            "FREQ" => {
                freq = Some(match value.to_uppercase().as_str() {
                    "DAILY" => Freq::Daily,
                    "WEEKLY" => Freq::Weekly,
                    "MONTHLY" => Freq::Monthly,
                    "YEARLY" => Freq::Yearly,
                    _ => return None,
                });
            }
            "INTERVAL" => {
                if !is_int(value) || js::to_number(value) < 1.0 {
                    return None;
                }
                interval = js::to_number(value);
            }
            "BYDAY" => {
                let mut days = vec![];
                for code in value.to_uppercase().split(',') {
                    days.push(WEEKDAY_CODES.iter().position(|c| *c == code)? as i32);
                }
                days.sort_unstable();
                days.dedup();
                by_day = days;
            }
            "BYMONTHDAY" => {
                let parts: Vec<&str> = value.split(',').collect();
                if parts
                    .iter()
                    .any(|d| !is_int(d) || js::to_number(d) == 0.0 || js::to_number(d).abs() > 31.0)
                {
                    return None;
                }
                by_month_day = parts.iter().map(|d| js::to_number(d) as i32).collect();
            }
            "COUNT" => {
                if !is_int(value) || js::to_number(value) < 1.0 {
                    return None;
                }
                count = Some(js::to_number(value));
            }
            "UNTIL" => until = Some(parse_until(value)?),
            _ => return None,
        }
    }
    let freq = freq?;
    // RFC 5545 forbids both of these.
    if count.is_some() && until.is_some() {
        return None;
    }
    if freq == Freq::Weekly && !by_month_day.is_empty() {
        return None;
    }
    Some(Rule {
        freq,
        interval,
        by_day,
        by_month_day,
        count,
        until,
    })
}

// The days of the k-th period of the series, in order. None once the period
// is past the calendar altogether (a year beyond the years a Date holds): the
// series has ended there. The TypeScript gives an empty period instead, and
// keeps asking for the next one for ever (docs/QUESTIONS.md #159).
fn period_days(rule: &Rule, start: &str, k: f64) -> Option<Vec<DayKey>> {
    let (sy, sm, sd) = key_parts(start);
    let step = k * rule.interval;
    let from_one = |year: f64, month: f64, n: f64| -> Option<Vec<DayKey>> {
        if n.is_nan() {
            return None;
        }
        Some(
            (1..=(n as usize))
                .map(|i| key_of(year, month, i as f64))
                .collect(),
        )
    };
    match rule.freq {
        Freq::Daily => Some(vec![add_days(start, step)]),
        Freq::Weekly => {
            // Weeks start on Monday, RFC 5545's default WKST.
            let monday = add_days(start, -((weekday_of(start) + 6.0) % 7.0) + 7.0 * step);
            Some((0..7).map(|i| add_days(&monday, f64::from(i))).collect())
        }
        Freq::Monthly => {
            let m = sm - 1.0 + step;
            let year = sy + (m / 12.0).floor();
            let month = (m % 12.0) + 1.0;
            from_one(year, month, days_in_month(year, month))
        }
        Freq::Yearly => {
            let year = sy + step;
            let feb = days_in_month(year, 2.0);
            if feb.is_nan() {
                return None;
            }
            // With no BYDAY or BYMONTHDAY, a yearly rule falls on the start's date.
            if rule.by_day.is_empty() && rule.by_month_day.is_empty() {
                return Some(if sd <= days_in_month(year, sm) {
                    vec![key_of(year, sm, sd)]
                } else {
                    vec![]
                });
            }
            let first = key_of(year, 1.0, 1.0);
            let length = if feb == 29.0 { 366 } else { 365 };
            Some(
                (0..length)
                    .map(|i| add_days(&first, f64::from(i)))
                    .collect(),
            )
        }
    }
}

fn matches(rule: &Rule, start: &str, day: &str) -> bool {
    let by_day: Vec<f64> = if rule.freq == Freq::Weekly && rule.by_day.is_empty() {
        vec![weekday_of(start)]
    } else {
        rule.by_day.iter().map(|d| f64::from(*d)).collect()
    };
    if !by_day.is_empty() && !by_day.contains(&weekday_of(day)) {
        return false;
    }
    let by_month_day: Vec<f64> =
        if rule.freq == Freq::Monthly && rule.by_day.is_empty() && rule.by_month_day.is_empty() {
            vec![key_parts(start).2]
        } else {
            rule.by_month_day.iter().map(|d| f64::from(*d)).collect()
        };
    if by_month_day.is_empty() {
        return true;
    }
    let (year, month, d) = key_parts(day);
    let last = days_in_month(year, month);
    by_month_day.iter().any(|md| {
        if *md > 0.0 {
            *md == d
        } else {
            last + md + 1.0 == d
        }
    })
}

// The period to start walking from. Without COUNT nothing before `from`
// can matter, so a long-running series skips straight to the period that
// holds it; with COUNT every occurrence from the start has to be counted.
fn first_period(rule: &Rule, start: &str, from: &str) -> f64 {
    if rule.count.is_some() || js::cmp(from, start) != Ordering::Greater {
        return 0.0;
    }
    let (sy, sm, _) = key_parts(start);
    let (fy, fm, _) = key_parts(from);
    let periods = match rule.freq {
        Freq::Daily => days_between(start, from),
        Freq::Weekly => {
            (days_between(&add_days(start, -((weekday_of(start) + 6.0) % 7.0)), from) / 7.0).floor()
        }
        Freq::Monthly => (fy - sy) * 12.0 + (fm - sm),
        Freq::Yearly => fy - sy,
    };
    (periods / rule.interval).floor()
}

/// The series' dates, in order, until COUNT, UNTIL or the horizon, each handed
/// to `visit`, which says whether to go on. Only dates: turning a wall clock
/// into an instant is the costly step, so it is done only for the occurrences
/// a caller keeps, and for the one day an UNTIL instant can cut either way.
fn series_days(
    rule: &Rule,
    start: &SeriesStart,
    tz: &TimeZone,
    from: &str,
    mut visit: impl FnMut(DayKey) -> bool,
) {
    let end = add_days(&start.day, HORIZON_DAYS);
    let last_day: Option<DayKey> = match &rule.until {
        None => None,
        Some(Until::Day { day }) => Some(day.clone()),
        Some(Until::Instant { instant }) => Some(day_key(*instant, tz)),
    };
    let mut emitted = 0.0;
    let mut k = first_period(rule, &start.day, from);
    loop {
        let Some(days) = period_days(rule, &start.day, k) else {
            return;
        };
        if days
            .first()
            .is_some_and(|d| js::cmp(d, &end) == Ordering::Greater)
        {
            return;
        }
        for date in days {
            if js::cmp(&date, &start.day) == Ordering::Less || !matches(rule, &start.day, &date) {
                continue;
            }
            if let Some(last) = &last_day {
                if js::cmp(&date, last) == Ordering::Greater {
                    return;
                }
                if date == *last {
                    if let Some(Until::Instant { instant }) = &rule.until {
                        if at_minute(&date, start.minute, tz) > *instant {
                            return;
                        }
                    }
                }
            }
            if !visit(date) {
                return;
            }
            emitted += 1.0;
            if rule.count.is_some_and(|c| emitted >= c) {
                return;
            }
        }
        k += 1.0;
    }
}

/// The occurrences whose dates fall in [from, to], both inclusive.
pub fn occurrences_between(
    rule: &Rule,
    start: &SeriesStart,
    tz: &TimeZone,
    from: &str,
    to: &str,
) -> Vec<Occurrence> {
    let mut out = vec![];
    series_days(rule, start, tz, from, |date| {
        if js::cmp(&date, to) == Ordering::Greater {
            return false;
        }
        if js::cmp(&date, from) != Ordering::Less {
            let at = at_minute(&date, start.minute, tz);
            out.push(Occurrence { date, at });
        }
        true
    });
    out
}

/// Where a task's series starts: its due date and time, else its scheduled date.
pub fn series_start(task: &Task, tz: &TimeZone) -> Option<SeriesStart> {
    if let Some(due) = task.due {
        return Some(SeriesStart {
            day: day_key(due, tz),
            minute: minute_of_day(due, tz),
        });
    }
    match &task.scheduled_date {
        Some(day) if !day.is_empty() => Some(SeriesStart {
            day: day.clone(),
            minute: 0.0,
        }),
        _ => None,
    }
}

fn rule_of(task: &Task) -> Option<Rule> {
    task.rrule
        .as_deref()
        .filter(|r| !r.is_empty())
        .and_then(parse_rule)
}

/// A recurring task's occurrences in [from, to]; empty for a task without a readable rule.
pub fn task_occurrences(task: &Task, tz: &TimeZone, from: &str, to: &str) -> Vec<Occurrence> {
    let rule = rule_of(task);
    let start = rule.as_ref().and_then(|_| series_start(task, tz));
    match (rule, start) {
        (Some(rule), Some(start)) => occurrences_between(&rule, &start, tz, from, to),
        _ => vec![],
    }
}

/// The next occurrence still to do: the earliest one from today on that has no
/// [`TaskOccurrence`] row. Today's stays until midnight, so it can read "1h
/// overdue"; an earlier day's that went by unticked is left behind rather
/// than staying overdue for ever.
pub fn next_open_occurrence(
    task: &Task,
    occurrences: &[TaskOccurrence],
    now: f64,
    tz: &TimeZone,
) -> Option<Occurrence> {
    let rule = rule_of(task)?;
    let start = series_start(task, tz)?;
    let today = day_key(now, tz);
    let done: HashSet<&str> = occurrences
        .iter()
        .filter(|o| o.task_id == task.id)
        .map(|o| o.date.as_str())
        .collect();
    let mut found = None;
    series_days(&rule, &start, tz, &today, |date| {
        if js::cmp(&date, &today) != Ordering::Less && !done.contains(date.as_str()) {
            let at = at_minute(&date, start.minute, tz);
            found = Some(Occurrence { date, at });
            return false;
        }
        true
    });
    found
}

/// Whether a task repeats. A rule Heat can't read, or one with no date to
/// start from, leaves it a plain task, so it never vanishes.
pub fn recurs(task: &Task) -> bool {
    task.rrule
        .as_deref()
        .is_some_and(|r| parse_rule(r).is_some())
        && (task.due.is_some()
            || task
                .scheduled_date
                .as_deref()
                .is_some_and(|d| !d.is_empty()))
}

/// Sets or clears a task's rule (the Repeat menu). A rule needs a day to start
/// from, so a task with no due or scheduled date is scheduled for today.
pub fn set_repeat(task: &Task, rule: Option<&str>, now: f64, tz: &TimeZone) -> Task {
    let mut out = task.clone();
    out.rrule = None;
    let Some(rule) = rule else { return out };
    if task.due.is_none() && task.scheduled_date.as_deref().map_or(true, str::is_empty) {
        out.scheduled_date = Some(day_key(now, tz));
    }
    out.rrule = Some(rule.to_string());
    out
}

/// The task as heat sees it: a recurring task's due becomes its next open
/// occurrence (None once the series has ended). Any other task comes back
/// unchanged.
pub fn with_effective_due(
    task: &Task,
    occurrences: &[TaskOccurrence],
    now: f64,
    tz: &TimeZone,
) -> Task {
    if !recurs(task) {
        return task.clone();
    }
    let next = if task.due.is_none() {
        None
    } else {
        next_open_occurrence(task, occurrences, now, tz)
    };
    Task {
        due: next.map(|n| n.at),
        ..task.clone()
    }
}

/// Checking a recurring task ticks its next open occurrence as a row of its own.
pub fn check_occurrence(
    task: &Task,
    occurrences: &[TaskOccurrence],
    now: f64,
    tz: &TimeZone,
    new_id: &mut dyn FnMut() -> Id,
) -> Option<TaskOccurrence> {
    let next = next_open_occurrence(task, occurrences, now, tz)?;
    Some(TaskOccurrence {
        id: new_id(),
        task_id: task.id.clone(),
        date: next.date,
        done_at: now,
    })
}

/// Unticking an occurrence removes its row.
pub fn reopen_occurrence(
    occurrences: &[TaskOccurrence],
    task_id: &str,
    date: &str,
) -> Vec<TaskOccurrence> {
    occurrences
        .iter()
        .filter(|o| !(o.task_id == task_id && o.date == date))
        .cloned()
        .collect()
}

/// The tasks still to do, as heat sees them: not done, and for a recurring
/// task, a series that hasn't ended, with its due moved to its next open
/// occurrence.
pub fn open_tasks(
    tasks: &[Task],
    occurrences: &[TaskOccurrence],
    now: f64,
    tz: &TimeZone,
) -> Vec<Task> {
    let mut out = vec![];
    for t in tasks {
        if t.done {
            continue;
        }
        if !recurs(t) {
            out.push(t.clone());
            continue;
        }
        let Some(next) = next_open_occurrence(t, occurrences, now, tz) else {
            continue;
        };
        out.push(Task {
            due: t.due.map(|_| next.at),
            ..t.clone()
        });
    }
    out
}
