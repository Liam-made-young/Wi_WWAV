//! Commitments (docs/COMMITMENTS.md). What a fail looks like:
//! - a class is missing from a week it meets in, or shows in a break, after
//!   the term, on a skipped day, or twice on a moved one;
//! - free time counts minutes that are in a class, its travel, or sleep, or
//!   says it fits when more is planned than there is room for;
//! - a schedule line Claude got wrong becomes a commitment at a made-up
//!   time, or a week's shifts land in another week;
//! - a mail that doesn't cancel a class offers to skip one, or one that does
//!   names the wrong day.

mod common;

use common::{ny, ny_zone};
use serde_json::json;
use wi_heat::commitments::*;
use wi_heat::ical;

fn jpn() -> Commitment {
    Commitment {
        id: "jpn".into(),
        title: "JPN 101".into(),
        kind: Kind::Class,
        location: "Swan Hall 201".into(),
        start: 600.0,
        end: 650.0,
        rrule: Some("FREQ=WEEKLY;BYDAY=MO,WE,FR".into()),
        from: "2026-09-09".into(),
        buffer_before: 25.0,
        buffer_after: 10.0,
        course_id: Some("c-jpn".into()),
        source: "you".into(),
        ..Commitment::default()
    }
}

fn shift(day: &str) -> Commitment {
    Commitment {
        id: format!("shift-{day}"),
        title: "Bookstore".into(),
        kind: Kind::Work,
        start: 540.0,
        end: 1020.0,
        from: day.into(),
        source: "paste".into(),
        ..Commitment::default()
    }
}

fn days(c: &Commitment, academic: &Academic, from: &str, to: &str) -> Vec<String> {
    occurrences(c, academic, &ny_zone(), from, to)
        .into_iter()
        .map(|o| o.date)
        .collect()
}

#[test]
fn a_class_meets_every_week_of_the_term_and_never_in_a_break() {
    let breaks = [
        Break {
            id: "b1".into(),
            title: "Columbus Day".into(),
            from: "2026-10-12".into(),
            to: "2026-10-12".into(),
        },
        Break {
            id: "b2".into(),
            title: "Thanksgiving".into(),
            from: "2026-11-25".into(),
            to: "2026-11-29".into(),
        },
    ];
    let academic = Academic {
        breaks: &breaks,
        term_start: Some("2026-09-09"),
        term_end: Some("2026-12-11"),
    };
    // Three a week, Monday, Wednesday and Friday.
    assert_eq!(
        days(&jpn(), &academic, "2026-10-04", "2026-10-17"),
        [
            "2026-10-05",
            "2026-10-07",
            "2026-10-09",
            "2026-10-14",
            "2026-10-16"
        ],
        "Monday the 12th is a holiday"
    );
    assert_eq!(
        days(&jpn(), &academic, "2026-11-22", "2026-11-30"),
        ["2026-11-23", "2026-11-30"]
    );
    // It starts on the day it says, not on the Monday before.
    assert_eq!(
        days(&jpn(), &academic, "2026-09-06", "2026-09-12"),
        ["2026-09-09", "2026-09-11"]
    );
    // The term's last day is the last class, with no end of its own written.
    assert_eq!(
        days(&jpn(), &academic, "2026-12-07", "2026-12-31"),
        ["2026-12-07", "2026-12-09", "2026-12-11"]
    );
    // Every week between, counted: 14 weeks less the days off.
    let all = days(&jpn(), &academic, "2026-09-01", "2026-12-31");
    assert_eq!(all.len(), 2 + 13 * 3 - 3);
    // A shift is not a class: a break doesn't take it away, nor does the term.
    let work = Commitment {
        rrule: Some("FREQ=WEEKLY;BYDAY=SA".into()),
        ..shift("2026-09-12")
    };
    assert_eq!(days(&work, &academic, "2026-11-22", "2026-12-20").len(), 4);
}

#[test]
fn an_end_of_its_own_holds_when_it_is_sooner_than_the_term() {
    let academic = Academic {
        breaks: &[],
        term_start: None,
        term_end: Some("2026-12-11"),
    };
    let c = Commitment {
        until: Some("2026-10-09".into()),
        ..jpn()
    };
    assert_eq!(
        days(&c, &academic, "2026-10-05", "2026-10-31"),
        ["2026-10-05", "2026-10-07", "2026-10-09"]
    );
    let late = Commitment {
        until: Some("2027-01-15".into()),
        ..jpn()
    };
    assert_eq!(
        days(&late, &academic, "2026-12-09", "2027-01-31"),
        ["2026-12-09", "2026-12-11"]
    );
    // Every other week.
    let fortnightly = Commitment {
        rrule: Some("FREQ=WEEKLY;INTERVAL=2;BYDAY=WE".into()),
        ..jpn()
    };
    assert_eq!(
        days(&fortnightly, &academic, "2026-09-09", "2026-10-10"),
        ["2026-09-09", "2026-09-23", "2026-10-07"]
    );
}

#[test]
fn a_skipped_day_is_gone_and_a_moved_one_is_where_it_went() {
    let academic = Academic::default();
    let mut c = jpn();
    c.exceptions.push(Exception {
        date: "2026-10-07".into(),
        kind: ExceptionKind::Skip,
        ..Exception::default()
    });
    c.exceptions.push(Exception {
        date: "2026-10-09".into(),
        kind: ExceptionKind::Move,
        to_date: Some("2026-10-08".into()),
        start: Some(840.0),
        ..Exception::default()
    });
    let week = occurrences(&c, &academic, &ny_zone(), "2026-10-04", "2026-10-10");
    let seen: Vec<(&str, f64, f64)> = week
        .iter()
        .map(|o| (o.date.as_str(), o.start, o.end))
        .collect();
    assert_eq!(
        seen,
        [("2026-10-05", 600.0, 650.0), ("2026-10-08", 840.0, 890.0)]
    );
    assert_eq!(week[1].moved_from.as_deref(), Some("2026-10-09"));
    // A move within the day changes its time and nothing else.
    let mut same_day = jpn();
    same_day.exceptions.push(Exception {
        date: "2026-10-07".into(),
        kind: ExceptionKind::Move,
        start: Some(900.0),
        end: Some(960.0),
        ..Exception::default()
    });
    let o = &occurrences(&same_day, &academic, &ny_zone(), "2026-10-07", "2026-10-07")[0];
    assert_eq!((o.start, o.end), (900.0, 960.0));
    // An exception for a day it never met on moves nothing.
    let mut stray = jpn();
    stray.exceptions.push(Exception {
        date: "2026-10-06".into(),
        kind: ExceptionKind::Move,
        to_date: Some("2026-10-10".into()),
        ..Exception::default()
    });
    assert!(days(&stray, &academic, "2026-10-10", "2026-10-10").is_empty());
    // One off: on its day, and no other.
    assert_eq!(
        days(&shift("2026-10-10"), &academic, "2026-10-01", "2026-10-31"),
        ["2026-10-10"]
    );
}

#[test]
fn free_time_is_the_day_less_commitments_buffers_and_sleep() {
    let academic = Academic::default();
    let all = all_occurrences(&[jpn()], &academic, &ny_zone(), "2026-10-07", "2026-10-07");
    let taken = busy(&all, "2026-10-07");
    // 10:00 to 10:50, with 25 minutes to get there and 10 after.
    assert_eq!(taken, [(575.0, 660.0)]);
    let sleep = Sleep::default();
    // A whole day: 7 AM to 11 PM is 16 hours, less 85 minutes.
    let day = free_time("2026-10-07", 0.0, &taken, &sleep, &[], "Wednesday");
    assert_eq!(day.free_min, 16.0 * 60.0 - 85.0);
    assert_eq!(day.spans, [(420.0, 575.0), (660.0, 1380.0)]);
    assert_eq!(day.line, "14h 35m free Wednesday.");
    // From 7:40 PM there are 3h 20m before bed; four hours of blocks don't fit.
    let evening = free_time(
        "2026-10-07",
        1180.0,
        &taken,
        &sleep,
        &[(1200.0, 120.0), (1320.0, 120.0)],
        "today",
    );
    assert_eq!(evening.free_min, 200.0);
    assert_eq!(evening.planned_min, 240.0);
    assert_eq!(evening.over_min, 40.0);
    assert_eq!(evening.line, "3h 20m free today. Planned 4h. Move 40m?");
    // What fits is said plainly, with nothing to move.
    let fits = free_time(
        "2026-10-07",
        1180.0,
        &taken,
        &sleep,
        &[(1200.0, 60.0)],
        "today",
    );
    assert_eq!(fits.line, "3h 20m free today. Planned 1h.");
    // A block that began before now counts only for what is left of it.
    let part = free_time(
        "2026-10-07",
        1230.0,
        &taken,
        &sleep,
        &[(1200.0, 60.0)],
        "today",
    );
    assert_eq!(part.planned_min, 30.0);
    // Someone who sleeps 1 AM to 9 AM has the late evening and not the early morning.
    let owl = Sleep {
        from: 60.0,
        to: 540.0,
    };
    assert_eq!(owl.spans(), [(60.0, 540.0)]);
    assert_eq!(
        free_time("2026-10-07", 0.0, &[], &owl, &[], "today").free_min,
        1440.0 - 480.0
    );
}

#[test]
fn a_block_on_a_class_or_its_travel_is_flagged() {
    let all = all_occurrences(
        &[jpn()],
        &Academic::default(),
        &ny_zone(),
        "2026-10-07",
        "2026-10-07",
    );
    let blocks = [
        ("clear".to_string(), 420.0, 90.0),
        ("travel".to_string(), 540.0, 45.0),
        ("class".to_string(), 615.0, 30.0),
        ("after".to_string(), 660.0, 60.0),
    ];
    let found = conflicts(&blocks, &all, "2026-10-07");
    let ids: Vec<(&str, bool)> = found
        .iter()
        .map(|c| (c.block_id.as_str(), c.buffer_only))
        .collect();
    assert_eq!(ids, [("travel", true), ("class", false)]);
    assert_eq!(found[1].line, "Overlaps JPN 101 (10:00 AM to 10:50 AM).");
    assert_eq!(found[0].line, "Overlaps the travel time for JPN 101.");
}

#[test]
fn the_readout_says_what_is_next_and_when_to_leave() {
    let all = all_occurrences(
        &[jpn()],
        &Academic::default(),
        &ny_zone(),
        "2026-10-07",
        "2026-10-07",
    );
    let next = next_up(&all, "2026-10-07", 9.0 * 60.0).unwrap();
    assert_eq!(next.line, "NEXT JPN 101 10:00 · LEAVE 9:35");
    assert_eq!(next.leave_at, Some(575.0));
    // Once it has started it isn't next any more, and nothing else is today.
    assert!(next_up(&all, "2026-10-07", 600.0).is_none());
    // With no travel time there is nothing to leave for.
    let near = Commitment {
        buffer_before: 0.0,
        ..jpn()
    };
    let all = all_occurrences(
        &[near],
        &Academic::default(),
        &ny_zone(),
        "2026-10-07",
        "2026-10-07",
    );
    assert_eq!(
        next_up(&all, "2026-10-07", 540.0).unwrap().line,
        "NEXT JPN 101 10:00"
    );
    assert!(leaving(&all, "2026-10-07", 590.0).is_empty());
    // Time to leave is from the buffer's start until the class starts.
    let all = all_occurrences(
        &[jpn()],
        &Academic::default(),
        &ny_zone(),
        "2026-10-07",
        "2026-10-07",
    );
    assert!(leaving(&all, "2026-10-07", 574.0).is_empty());
    assert_eq!(leaving(&all, "2026-10-07", 575.0).len(), 1);
    assert!(leaving(&all, "2026-10-07", 600.0).is_empty());
    assert_eq!(
        leave_line(&all[0]),
        "Time to leave for JPN 101 (10:00 AM, Swan Hall 201)."
    );
}

#[test]
fn a_page_photographed_in_class_or_just_after_is_that_classs() {
    let egr = Commitment {
        id: "egr".into(),
        title: "EGR 101".into(),
        start: 660.0,
        end: 710.0,
        course_id: Some("c-egr".into()),
        ..jpn()
    };
    let all = all_occurrences(
        &[jpn(), egr, shift("2026-10-07")],
        &Academic::default(),
        &ny_zone(),
        "2026-10-07",
        "2026-10-07",
    );
    let at = |minute: f64| class_at(&all, "2026-10-07", minute).map(|o| o.title.as_str());
    assert_eq!(at(599.0), None, "a work shift is on, but it is not a class");
    assert_eq!(at(600.0), Some("JPN 101"));
    assert_eq!(at(650.0), Some("JPN 101"));
    // Ten minutes after Japanese ends, Engineering has started: it is on.
    assert_eq!(at(665.0), Some("EGR 101"));
    assert_eq!(
        at(655.0),
        Some("JPN 101"),
        "between the two, the one just over"
    );
    assert_eq!(at(740.0), Some("EGR 101"), "thirty minutes after");
    assert_eq!(at(741.0), None);
    assert_eq!(capture_title("JPN 101", "2026-10-07"), "JPN 101 · Oct 7");
}

#[test]
fn a_timetable_from_claude_is_checked_line_by_line() {
    let answer = json!({
        "items": [
            {"title": "JPN 101", "kind": "class", "days": ["MO", "WE", "FR"], "date": null, "start": "10:00", "end": "10:50", "location": "Swan 201", "course": "JPN 101", "from": null, "until": null},
            {"title": "ELE 209 Lab", "kind": "class", "days": ["TH"], "date": null, "start": "14:00", "end": "16:45", "location": null, "course": "ELE 209", "from": "2026-09-10", "until": "2026-12-03"},
            {"title": "Broken", "kind": "class", "days": ["MO"], "date": null, "start": "25:00", "end": "26:00", "location": null, "course": null, "from": null, "until": null},
            {"title": "Backwards", "kind": "class", "days": ["MO"], "date": null, "start": "11:00", "end": "10:00", "location": null, "course": null, "from": null, "until": null},
            {"title": "Nowhen", "kind": "other", "days": null, "date": null, "start": "11:00", "end": "12:00", "location": null, "course": null, "from": null, "until": null},
        ],
        "breaks": []
    });
    let s = parse_schedule(&answer, Mode::Schedule, "2026-10-05");
    assert_eq!(s.items.len(), 2);
    assert_eq!(s.unread, 3);
    assert_eq!((s.items[0].start, s.items[0].end), (600.0, 650.0));
    assert_eq!(s.items[0].days, ["MO", "WE", "FR"]);
    assert_eq!(
        weekly_rule(&s.items[0].days, 1.0).as_deref(),
        Some("FREQ=WEEKLY;BYDAY=MO,WE,FR")
    );
    assert_eq!(s.items[1].until.as_deref(), Some("2026-12-03"));
    assert_eq!(count_line(&s.items, 0), "2 classes");
    // The schema asks for exactly these fields, and nothing loose.
    let schema = schedule_schema();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["properties"]["items"]["items"]["additionalProperties"],
        false
    );
}

#[test]
fn a_weeks_shifts_are_each_on_a_day_of_that_week() {
    let answer = json!({
        "items": [
            {"title": "Bookstore", "kind": "work", "days": ["SA"], "date": null, "start": "09:00", "end": "17:00", "location": null, "course": null, "from": null, "until": null},
            {"title": "Bookstore", "kind": "work", "days": null, "date": "2026-10-06", "start": "16:00", "end": "20:00", "location": null, "course": null, "from": null, "until": null},
            {"title": "Bookstore", "kind": "work", "days": ["MO", "TU"], "date": null, "start": "16:00", "end": "20:00", "location": null, "course": null, "from": null, "until": null},
        ],
        "breaks": []
    });
    let s = parse_schedule(&answer, Mode::Week, "2026-10-05");
    let seen: Vec<(Option<&str>, bool)> = s
        .items
        .iter()
        .map(|i| (i.date.as_deref(), i.days.is_empty()))
        .collect();
    assert_eq!(
        seen,
        [(Some("2026-10-10"), true), (Some("2026-10-06"), true)]
    );
    assert_eq!(
        s.unread, 1,
        "a shift on two weekdays with no date isn't one shift"
    );
    assert_eq!(count_line(&s.items, 0), "2 shifts");
    assert_eq!(monday_of("2026-10-07"), "2026-10-05");
    assert_eq!(
        monday_of("2026-10-11"),
        "2026-10-05",
        "Sunday closes the week"
    );
    assert_eq!(monday_of("2026-10-05"), "2026-10-05");
}

#[test]
fn an_academic_calendar_gives_breaks_and_nothing_else() {
    let answer = json!({
        "items": [],
        "breaks": [
            {"title": "Thanksgiving recess", "from": "2026-11-25", "to": "2026-11-29"},
            {"title": "Backwards", "from": "2026-11-29", "to": "2026-11-25"},
            {"title": "Veterans Day", "from": "2026-11-11", "to": "2026-11-11"},
        ]
    });
    let s = parse_schedule(&answer, Mode::Breaks, "2026-10-05");
    assert_eq!(s.breaks.len(), 2);
    assert_eq!(s.unread, 1);
    assert_eq!(count_line(&[], 2), "2 breaks");
}

#[test]
fn times_are_read_however_they_are_written() {
    for (text, minutes) in [
        ("10:00", 600.0),
        ("9:05", 545.0),
        ("17:00", 1020.0),
        ("5pm", 1020.0),
        ("5:30 PM", 1050.0),
        ("12am", 0.0),
        ("12 pm", 720.0),
        ("0900", 540.0),
        ("24:00", 1440.0),
    ] {
        assert_eq!(minutes_of(text), Some(minutes), "{text}");
    }
    for bad in ["", "25:00", "10:75", "13pm", "noon", "ten"] {
        assert_eq!(minutes_of(bad), None, "{bad}");
    }
    for (word, n) in [
        ("MO", 1),
        ("Mon", 1),
        ("thursday", 4),
        ("R", 4),
        ("Sat.", 6),
        ("su", 0),
    ] {
        assert_eq!(day_code(word), Some(n), "{word}");
    }
    assert_eq!(day_code("someday"), None);
}

const TIMETABLE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\n\
BEGIN:VEVENT\r\nUID:jpn-101@uri\r\nSUMMARY:JPN 101\r\nLOCATION:Swan Hall 201\r\n\
DTSTART;TZID=America/New_York:20260909T100000\r\nDTEND;TZID=America/New_York:20260909T105000\r\n\
RRULE:FREQ=WEEKLY;WKST=SU;UNTIL=20261212T045959Z;BYDAY=MO,WE,FR\r\n\
EXDATE;TZID=America/New_York:20261012T100000,20261125T100000\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:shift-1\r\nSUMMARY:Bookstore\r\nDTSTART:20261010T130000Z\r\nDTEND:20261010T210000Z\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:odd\r\nSUMMARY:First Tuesday\r\nDTSTART:20261006T130000Z\r\nDTEND:20261006T140000Z\r\nRRULE:FREQ=MONTHLY;BYDAY=1TU\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:gone\r\nSUMMARY:Canceled thing\r\nSTATUS:CANCELLED\r\nDTSTART:20261006T130000Z\r\nDTEND:20261006T140000Z\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:break\r\nSUMMARY:Thanksgiving recess\r\nDTSTART;VALUE=DATE:20261125\r\nDTEND;VALUE=DATE:20261130\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:holiday\r\nSUMMARY:Veterans Day\r\nDTSTART;VALUE=DATE:20261111\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n";

#[test]
fn a_calendar_file_gives_what_repeats_with_its_rule_and_its_skipped_days() {
    let tz = ny_zone();
    let events = ical::parse(TIMETABLE.as_bytes(), &tz).unwrap();
    let s = from_ical(&events, Mode::Schedule, &tz);
    assert_eq!(s.items.len(), 2);
    assert_eq!(
        s.unread, 1,
        "a rule Learn can't follow is said, not guessed at"
    );
    let class = &s.items[0];
    assert_eq!(
        (class.title.as_str(), class.start, class.end),
        ("JPN 101", 600.0, 650.0)
    );
    assert_eq!(
        class.rrule.as_deref(),
        Some("FREQ=WEEKLY;UNTIL=20261212T045959Z;BYDAY=MO,WE,FR")
    );
    assert_eq!(class.days, ["MO", "WE", "FR"]);
    assert_eq!(class.skip, ["2026-10-12", "2026-11-25"]);
    assert_eq!(class.source_id.as_deref(), Some("jpn-101@uri"));
    assert_eq!(class.from.as_deref(), Some("2026-09-09"));
    // A one-off, in the person's own time: 13:00Z is 9 AM in New York.
    let once = &s.items[1];
    assert_eq!(
        (once.date.as_deref(), once.start, once.end),
        (Some("2026-10-10"), 540.0, 1020.0)
    );
    assert!(s.breaks.is_empty());
    // The same file as an academic calendar: only the whole days.
    let b = from_ical(&events, Mode::Breaks, &tz);
    assert!(b.items.is_empty());
    let seen: Vec<(&str, &str, &str)> = b
        .breaks
        .iter()
        .map(|x| (x.title.as_str(), x.from.as_str(), x.to.as_str()))
        .collect();
    assert_eq!(
        seen,
        [
            ("Thanksgiving recess", "2026-11-25", "2026-11-29"),
            ("Veterans Day", "2026-11-11", "2026-11-11")
        ]
    );
    // And the rule, expanded, stops where the file says.
    let c = Commitment {
        rrule: class.rrule.clone(),
        from: "2026-09-09".into(),
        ..jpn()
    };
    assert_eq!(
        days(&c, &Academic::default(), "2026-12-07", "2026-12-31"),
        ["2026-12-07", "2026-12-09", "2026-12-11"]
    );
}

fn mail<'a>(subject: &'a str, text: &'a str, course: Option<&'a str>, at: &str) -> MailSeen<'a> {
    MailSeen {
        subject,
        text,
        course,
        received_at: ny(at),
    }
}

fn code(id: &str) -> Option<String> {
    match id {
        "c-jpn" => Some("JPN 101".into()),
        "c-egr" => Some("EGR 101".into()),
        _ => None,
    }
}

#[test]
fn mail_that_cancels_a_class_offers_to_skip_that_day() {
    let tz = ny_zone();
    let thursday = Commitment {
        id: "egr".into(),
        title: "EGR 101".into(),
        rrule: Some("FREQ=WEEKLY;BYDAY=TU,TH".into()),
        course_id: Some("c-egr".into()),
        ..jpn()
    };
    let all = [jpn(), thursday];
    let academic = Academic::default();
    let find = |m: &MailSeen| exception_in_mail(m, &all, &academic, &tz, &code);

    // Wednesday's mail about Thursday's class.
    let f = find(&mail(
        "EGR 101: Class canceled Thursday",
        "Hi all, I am out sick. Class is canceled Thursday. See you next week.",
        Some("EGR 101"),
        "2026-10-07 08:15",
    ))
    .unwrap();
    assert_eq!(f.commitment_id, "egr");
    assert_eq!(
        (f.exception.date.as_str(), f.exception.kind),
        ("2026-10-08", ExceptionKind::Skip)
    );
    assert_eq!(f.line, "EGR 101 is canceled Thursday, Oct 8. Skip it?");
    assert_eq!(f.act, "Skip it");
    assert_eq!(f.exception.source.as_deref(), Some("mail"));

    // No course on the thread: the one class the mail writes out.
    let f = find(&mail(
        "No class tomorrow",
        "JPN 101 will not meet tomorrow.",
        None,
        "2026-10-08 20:00",
    ))
    .unwrap();
    assert_eq!(
        (f.commitment_id.as_str(), f.exception.date.as_str()),
        ("jpn", "2026-10-09")
    );

    // "Today", before it starts.
    let f = find(&mail(
        "Class cancelled today",
        "Sorry for the late notice.",
        Some("JPN 101"),
        "2026-10-07 07:30",
    ))
    .unwrap();
    assert_eq!(f.exception.date, "2026-10-07");

    // No day named: the next time it meets. After today's class, that is Friday.
    let f = find(&mail(
        "JPN 101 cancelled",
        "The next class is cancelled.",
        Some("JPN 101"),
        "2026-10-07 13:00",
    ))
    .unwrap();
    assert_eq!(f.exception.date, "2026-10-09");

    // A date written out.
    let f = find(&mail(
        "Schedule change",
        "JPN 101 is canceled on Oct 16 for the career fair.",
        None,
        "2026-10-07 13:00",
    ))
    .unwrap();
    assert_eq!(f.exception.date, "2026-10-16");
    let f = find(&mail(
        "Schedule change",
        "No class 10/19.",
        Some("JPN 101"),
        "2026-10-07 13:00",
    ))
    .unwrap();
    assert_eq!(f.exception.date, "2026-10-19");
}

#[test]
fn mail_that_moves_a_class_offers_the_new_time() {
    let tz = ny_zone();
    let all = [jpn()];
    let academic = Academic::default();
    let find = |m: &MailSeen| exception_in_mail(m, &all, &academic, &tz, &code);
    let f = find(&mail(
        "Friday's class moved",
        "Friday's class is moved to 2 PM in the same room.",
        Some("JPN 101"),
        "2026-10-07 13:00",
    ))
    .unwrap();
    assert_eq!(f.exception.kind, ExceptionKind::Move);
    assert_eq!(
        (
            f.exception.date.as_str(),
            f.exception.start,
            f.exception.end
        ),
        ("2026-10-09", Some(840.0), Some(890.0))
    );
    assert_eq!(
        f.line,
        "JPN 101 on Friday, Oct 9 moves to 2:00 PM. Move it?"
    );
    assert_eq!(f.act, "Move it");
    // Moved with nowhere to go: only the usual time can be offered up.
    let f = find(&mail(
        "Class postponed",
        "Friday's class is postponed. I will write with a new time.",
        Some("JPN 101"),
        "2026-10-07 13:00",
    ))
    .unwrap();
    assert_eq!(f.exception.kind, ExceptionKind::Skip);
    assert_eq!(f.line, "JPN 101 on Friday, Oct 9 has moved, and the mail doesn't say where to. Skip the usual time?");
}

#[test]
fn mail_that_cancels_nothing_offers_nothing() {
    let tz = ny_zone();
    let all = [jpn()];
    let academic = Academic::default();
    let find = |m: &MailSeen| exception_in_mail(m, &all, &academic, &tz, &code);
    for (subject, text, course) in [
        ("Homework 3 posted", "It is due Friday.", Some("JPN 101")),
        (
            "Class is NOT canceled",
            "Despite the snow, class is not canceled Friday.",
            Some("JPN 101"),
        ),
        (
            "MTH 142 canceled Friday",
            "No class Friday.",
            Some("MTH 142"),
        ),
        (
            "Office hours canceled",
            "My Tuesday office hours are canceled.",
            Some("JPN 101"),
        ),
        (
            "Your order was canceled",
            "Order 4411 was canceled Friday.",
            None,
        ),
    ] {
        assert_eq!(
            find(&mail(subject, text, course, "2026-10-07 13:00")),
            None,
            "{subject}"
        );
    }
}

#[test]
fn a_commitment_reads_back_what_it_wrote() {
    let mut c = jpn();
    c.exceptions.push(Exception {
        date: "2026-10-07".into(),
        ..Exception::default()
    });
    let v = serde_json::to_value(&c).unwrap();
    assert_eq!(v["start"], 600);
    assert_eq!(v["bufferBefore"], 25);
    assert_eq!(v["kind"], "class");
    assert_eq!(
        v["exceptions"][0],
        json!({"date": "2026-10-07", "kind": "skip"})
    );
    assert!(v.get("until").is_none());
    assert_eq!(serde_json::from_value::<Commitment>(v).unwrap(), c);
    // A record from before a field existed still reads.
    let old: Commitment = serde_json::from_value(
        json!({"id": "x", "title": "Gym", "start": 420, "end": 480, "from": "2026-10-07"}),
    )
    .unwrap();
    assert_eq!(
        (old.kind, old.hardness, old.buffer_before),
        (Kind::Other, Hardness::Fixed, 0.0)
    );
    assert_eq!(when_line(&jpn()), "MWF 10:00 AM to 10:50 AM");
    assert_eq!(
        when_line(&shift("2026-10-10")),
        "Sat Oct 10, 9:00 AM to 5:00 PM"
    );
}
