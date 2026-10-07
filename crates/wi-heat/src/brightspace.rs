//! Brightspace in Heat (`docs/SPEC.md` 3.1, 3.11): the feed's items become
//! tasks by Heat's rules, matched to what Heat already holds.
//!
//! - Keep titles ending " - Due"; skip /non-graded/i and cancelled items.
//! - The course comes from the school's pattern, tried on the location and
//!   then the title; the type comes from the space's keywords.
//! - The VEVENT UID becomes the task id. A task that arrived through Google
//!   Calendar before (the artifact's import) takes the UID on when Heat's
//!   duplicate test matches it: same due day, same course, titles sharing at
//!   least 60% of their words.
//! - An item missing from two syncs in a row is tagged "No longer in
//!   Brightspace" and never deleted.

use crate::ical::{Event, When};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use regex::Regex;
use std::collections::{BTreeSet, HashSet};

/// `/^([A-Z]{3})\s?(\d{3})/`: "MTH 142", "JPN102".
pub const DEFAULT_COURSE_PATTERN: &str = r"^([A-Z]{3})\s?(\d{3})";
pub const NO_LONGER_IN_BRIGHTSPACE: &str = "No longer in Brightspace";
const DUE_SUFFIX: &str = " - Due";

/// Heat reads from 2 hours ago to 70 days ahead. Items outside that range
/// wait: the feed holds the whole term, and last month's homework shouldn't
/// arrive as a page of Overdue tasks.
pub const NEW_FROM: SignedDuration = SignedDuration::from_hours(2);
pub const NEW_UNTIL: SignedDuration = SignedDuration::from_hours(70 * 24);

/// Sync on open if the last sync was over 15 minutes ago, then hourly.
pub const SYNC_ON_OPEN_AFTER: SignedDuration = SignedDuration::from_mins(15);
pub const SYNC_EVERY: SignedDuration = SignedDuration::from_hours(1);

/// What the School sheet holds that these rules read.
#[derive(Clone, Debug)]
pub struct School {
    /// The Brightspace host, e.g. `brightspace.uri.edu`.
    pub host: String,
    pub course_pattern: Regex,
    /// The system's zone, which is the school's for a student at the school.
    pub zone: TimeZone,
}

impl School {
    pub fn new(host: &str, course_pattern: &str, zone: TimeZone) -> Result<School, regex::Error> {
        Ok(School {
            host: host.trim().to_string(),
            course_pattern: Regex::new(course_pattern)?,
            zone,
        })
    }
}

/// The course code `pattern` finds at the start of `text`. Its capture groups
/// are joined with a space, so "MTH142" and "MTH 142" are one course; a
/// pattern without groups gives its whole match.
pub fn course_code(pattern: &Regex, text: &str) -> Option<String> {
    let caps = pattern.captures(text.trim())?;
    let groups: Vec<&str> = caps
        .iter()
        .skip(1)
        .flatten()
        .map(|m| m.as_str().trim())
        .collect();
    let code = if groups.is_empty() {
        caps[0].trim().to_string()
    } else {
        groups.join(" ")
    };
    (!code.is_empty()).then_some(code)
}

/// A space's type and the words that name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeRule {
    pub kind: String,
    pub keywords: Vec<String>,
}

/// The Classes space's types (3.1), each with the words that pick it. The
/// first rule with a match wins, so "Lab 5a report" is a Lab and "Kanji quiz:
/// reading and writing" a Quiz.
pub fn classes_types() -> Vec<TypeRule> {
    let rule = |kind: &str, keywords: &[&str]| TypeRule {
        kind: kind.to_string(),
        keywords: keywords.iter().map(|k| k.to_string()).collect(),
    };
    vec![
        rule("Quiz", &["quiz", "quizzes"]),
        rule("Exam prep", &["exam", "midterm", "final exam", "test"]),
        rule("Lab", &["lab", "labs"]),
        rule("Listening", &["listening"]),
        rule("Reading", &["reading", "readings", "chapter", "chapters"]),
        rule(
            "Project",
            &[
                "project",
                "essay",
                "paper",
                "presentation",
                "proposal",
                "draft",
            ],
        ),
        rule(
            "Homework",
            &[
                "homework",
                "hw",
                "assignment",
                "problem set",
                "worksheet",
                "exercises",
            ],
        ),
    ]
}

fn words(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
}

/// The first type whose keyword appears in `title` as whole words, else
/// "Other". A keyword can be a phrase ("problem set").
pub fn task_type(title: &str, rules: &[TypeRule]) -> String {
    let title: Vec<String> = words(title).collect();
    for rule in rules {
        for keyword in &rule.keywords {
            let k: Vec<String> = words(keyword).collect();
            if !k.is_empty() && title.windows(k.len()).any(|w| w == k.as_slice()) {
                return rule.kind.clone();
            }
        }
    }
    "Other".to_string()
}

/// A graded due item read from the feed.
#[derive(Clone, Debug, PartialEq)]
pub struct FeedItem {
    pub uid: String,
    /// The summary without " - Due".
    pub title: String,
    pub course: Option<String>,
    pub kind: String,
    pub due: Timestamp,
    pub notes: String,
    pub url: Option<String>,
}

/// Heat's filters: a title ending " - Due", nothing non-graded, nothing
/// cancelled (by STATUS, or by an instructor's "Cancelled" in the title).
pub fn is_kept(e: &Event) -> bool {
    let non_graded = |s: &str| s.to_lowercase().contains("non-graded");
    let cancelled = e.status.as_deref() == Some("CANCELLED")
        || words(&e.summary).any(|w| w == "cancelled" || w == "canceled");
    e.summary.trim_end().ends_with(DUE_SUFFIX)
        && !non_graded(&e.summary)
        && !non_graded(&e.description)
        && !cancelled
}

/// A whole-day deadline is due at 11:59 PM that day, as a task added on the
/// Calendar is (3.3).
pub fn due_on(date: Date, zone: &TimeZone) -> Option<Timestamp> {
    zone.to_ambiguous_timestamp(date.at(23, 59, 0, 0))
        .compatible()
        .ok()
}

/// The kept items of a feed. An item needs a UID, which becomes its task id,
/// and a time: DUE if the event has one, else DTSTART, which is when D2L
/// puts the deadline (DTEND of a whole day is the next day).
pub fn feed_items(events: &[Event], school: &School, types: &[TypeRule]) -> Vec<FeedItem> {
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for e in events.iter().filter(|e| is_kept(e)) {
        let (Some(uid), Some(when)) = (&e.uid, e.due.or(e.start)) else {
            continue;
        };
        let due = match when {
            When::At(t) => t,
            When::Date(d) => match due_on(d, &school.zone) {
                Some(t) => t,
                None => continue,
            },
        };
        if !seen.insert(uid.as_str()) {
            continue; // the same item twice; the first one stands
        }
        let summary = e.summary.trim_end();
        let title = summary
            .strip_suffix(DUE_SUFFIX)
            .unwrap_or(summary)
            .trim()
            .to_string();
        let course = course_code(&school.course_pattern, &e.location)
            .or_else(|| course_code(&school.course_pattern, &title));
        items.push(FeedItem {
            uid: uid.clone(),
            kind: task_type(&title, types),
            title,
            course,
            due,
            notes: e.description.clone(),
            url: e.url.clone(),
        });
    }
    items
}

/// True when the titles share at least 60% of their words, counted against
/// the longer title, with " - Due" set aside.
pub fn same_title(a: &str, b: &str) -> bool {
    let set = |t: &str| -> BTreeSet<String> {
        let t = t.trim_end();
        words(t.strip_suffix(DUE_SUFFIX).unwrap_or(t)).collect()
    };
    let (a, b) = (set(a), set(b));
    let shared = a.intersection(&b).count();
    shared > 0 && shared * 10 >= a.len().max(b.len()) * 6
}

/// What the feed sync needs to know of a task: one made from the feed, or
/// one that came through Google Calendar and has no UID yet.
#[derive(Clone, Debug, PartialEq)]
pub struct Known {
    pub id: String,
    /// The VEVENT UID this task follows.
    pub uid: Option<String>,
    pub title: String,
    pub course: Option<String>,
    pub due: Option<Timestamp>,
    pub done: bool,
    /// Syncs in a row whose feed didn't hold this task's UID.
    pub missed: u8,
    /// Tagged "No longer in Brightspace".
    pub gone: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DateChange {
    pub task_id: String,
    pub from: Option<Timestamp>,
    pub to: Timestamp,
}

/// What one sync changed in `known`, for the caller to save and count.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeedReport {
    /// Items that became tasks; each is in `known` under its UID.
    pub new: Vec<FeedItem>,
    /// Google Calendar tasks that took on a UID.
    pub took_uid: Vec<String>,
    pub date_changes: Vec<DateChange>,
    /// Tasks tagged "No longer in Brightspace" by this sync.
    pub gone: Vec<String>,
    /// Tagged tasks whose item came back; the tag is cleared.
    pub back: Vec<String>,
}

fn is_duplicate(item: &FeedItem, task: &Known, zone: &TimeZone) -> bool {
    let Some(due) = task.due else { return false };
    zone.to_datetime(due).date() == zone.to_datetime(item.due).date()
        && task.course == item.course
        && same_title(&task.title, &item.title)
}

/// Brings `known` up to date with one read of the feed. It adds and changes
/// tasks and never removes one.
pub fn sync_feed(
    known: &mut Vec<Known>,
    items: &[FeedItem],
    now: Timestamp,
    zone: &TimeZone,
) -> FeedReport {
    let mut report = FeedReport::default();
    for item in items {
        if let Some(task) = known
            .iter_mut()
            .find(|t| t.uid.as_deref() == Some(item.uid.as_str()))
        {
            // A done task keeps the date it was done against.
            if !task.done && task.due != Some(item.due) {
                report.date_changes.push(DateChange {
                    task_id: task.id.clone(),
                    from: task.due,
                    to: item.due,
                });
                task.due = Some(item.due);
            }
        } else if let Some(task) = known
            .iter_mut()
            .find(|t| t.uid.is_none() && is_duplicate(item, t, zone))
        {
            // The same deadline, seen through Google Calendar before. Its time
            // may differ by minutes; the feed's is the one to keep.
            task.uid = Some(item.uid.clone());
            task.due = Some(item.due);
            report.took_uid.push(task.id.clone());
        } else {
            let ahead = item.due.duration_since(now);
            if ahead >= -NEW_FROM && ahead <= NEW_UNTIL {
                known.push(Known {
                    id: item.uid.clone(),
                    uid: Some(item.uid.clone()),
                    title: item.title.clone(),
                    course: item.course.clone(),
                    due: Some(item.due),
                    done: false,
                    missed: 0,
                    gone: false,
                });
                report.new.push(item.clone());
            }
        }
    }

    let present: HashSet<&str> = items.iter().map(|i| i.uid.as_str()).collect();
    for task in known.iter_mut() {
        let Some(uid) = task.uid.as_deref() else {
            continue;
        };
        if present.contains(uid) {
            if task.gone {
                report.back.push(task.id.clone());
            }
            task.missed = 0;
            task.gone = false;
        } else {
            task.missed = task.missed.saturating_add(1);
            if task.missed >= 2 && !task.gone {
                task.gone = true;
                report.gone.push(task.id.clone());
            }
        }
    }
    report
}

/// Whether opening Heat should sync now.
pub fn sync_on_open(last_sync: Option<Timestamp>, now: Timestamp) -> bool {
    match last_sync {
        None => true,
        Some(last) => now.duration_since(last) > SYNC_ON_OPEN_AFTER,
    }
}

/// When the next hourly sync is due.
pub fn next_sync(last_sync: Timestamp) -> Timestamp {
    last_sync.checked_add(SYNC_EVERY).unwrap_or(Timestamp::MAX)
}

/// What one sync brought, across the feed and mail.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub new_tasks: usize,
    pub date_changes: usize,
    pub new_grades: usize,
}

/// "3:41 PM" on the person's clock.
pub fn clock(at: Timestamp, zone: &TimeZone) -> String {
    let t = zone.to_datetime(at);
    let (h, half) = match t.hour() {
        0 => (12, "AM"),
        h @ 1..=11 => (h, "AM"),
        12 => (12, "PM"),
        h => (h - 12, "PM"),
    };
    format!("{h}:{:02} {half}", t.minute())
}

/// "Synced 3:41 PM: 2 new tasks, 1 date change, 1 new grade posted". Counts
/// of zero are left out; a sync that brought nothing says so.
pub fn sync_line(at: Timestamp, zone: &TimeZone, counts: Counts) -> String {
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut parts = Vec::new();
    if counts.new_tasks > 0 {
        parts.push(plural(counts.new_tasks, "new task", "new tasks"));
    }
    if counts.date_changes > 0 {
        parts.push(plural(counts.date_changes, "date change", "date changes"));
    }
    if counts.new_grades > 0 {
        parts.push(plural(
            counts.new_grades,
            "new grade posted",
            "new grades posted",
        ));
    }
    let news = if parts.is_empty() {
        "nothing new".to_string()
    } else {
        parts.join(", ")
    };
    format!("Synced {}: {news}", clock(at, zone))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ny() -> TimeZone {
        TimeZone::get("America/New_York").unwrap()
    }

    fn ts(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    #[test]
    fn the_course_comes_from_the_pattern() {
        let re = Regex::new(DEFAULT_COURSE_PATTERN).unwrap();
        assert_eq!(
            course_code(&re, "MTH 142 Sec 0004 - Fall 2026").as_deref(),
            Some("MTH 142")
        );
        assert_eq!(
            course_code(&re, "MTH142 Sec 0004").as_deref(),
            Some("MTH 142")
        );
        assert_eq!(
            course_code(&re, "Fall 2026 MTH 142"),
            None,
            "the default pattern is anchored"
        );
        assert_eq!(course_code(&re, ""), None);
        let whole = Regex::new(r"^[A-Z]{2,4}-\d{4}").unwrap();
        assert_eq!(
            course_code(&whole, "CHEM-1000 Lecture").as_deref(),
            Some("CHEM-1000")
        );
    }

    #[test]
    fn the_type_comes_from_whole_keywords() {
        let rules = classes_types();
        assert_eq!(task_type("Grammar quiz 4", &rules), "Quiz");
        assert_eq!(task_type("Problem set 6", &rules), "Homework");
        assert_eq!(task_type("Lab 5a report", &rules), "Lab");
        assert_eq!(task_type("Midterm exam", &rules), "Exam prep");
        assert_eq!(
            task_type("Syllabus check", &rules),
            "Other",
            "\"lab\" inside a word doesn't count"
        );
        assert_eq!(
            task_type("Set problem", &rules),
            "Other",
            "a phrase keeps its order"
        );
    }

    #[test]
    fn a_whole_day_is_due_at_11_59_pm() {
        assert_eq!(
            due_on(jiff::civil::date(2026, 10, 20), &ny()),
            Some(ts("2026-10-21T03:59:00Z"))
        );
        assert_eq!(
            due_on(jiff::civil::date(2026, 12, 1), &ny()),
            Some(ts("2026-12-02T04:59:00Z"))
        );
    }

    #[test]
    fn heat_syncs_on_open_after_15_minutes_then_hourly() {
        let now = ts("2026-10-06T12:40:00Z");
        assert!(sync_on_open(None, now));
        assert!(!sync_on_open(Some(ts("2026-10-06T12:26:00Z")), now));
        assert!(
            !sync_on_open(Some(ts("2026-10-06T12:25:00Z")), now),
            "exactly 15 minutes isn't over"
        );
        assert!(sync_on_open(Some(ts("2026-10-06T12:24:59Z")), now));
        assert_eq!(next_sync(now), ts("2026-10-06T13:40:00Z"));
    }

    #[test]
    fn the_sync_line_reads_as_written() {
        let at = ts("2026-10-06T19:41:00Z");
        let all = Counts {
            new_tasks: 2,
            date_changes: 1,
            new_grades: 1,
        };
        assert_eq!(
            sync_line(at, &ny(), all),
            "Synced 3:41 PM: 2 new tasks, 1 date change, 1 new grade posted"
        );
        let morning = Counts {
            new_tasks: 2,
            date_changes: 1,
            new_grades: 0,
        };
        assert_eq!(
            sync_line(ts("2026-10-06T12:41:00Z"), &ny(), morning),
            "Synced 8:41 AM: 2 new tasks, 1 date change"
        );
        let more = Counts {
            new_tasks: 1,
            date_changes: 3,
            new_grades: 2,
        };
        assert_eq!(
            sync_line(at, &ny(), more),
            "Synced 3:41 PM: 1 new task, 3 date changes, 2 new grades posted"
        );
        assert_eq!(
            sync_line(at, &ny(), Counts::default()),
            "Synced 3:41 PM: nothing new"
        );
        assert_eq!(clock(ts("2026-10-06T04:05:00Z"), &ny()), "12:05 AM");
        assert_eq!(clock(ts("2026-10-06T16:30:00Z"), &ny()), "12:30 PM");
    }
}
