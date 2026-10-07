//! S2.5 (docs/PLAN.md): against a stored raw feed, a due item is missed, a
//! non-graded or cancelled item is kept, a UID changes, or an item missing
//! from two syncs is deleted.
//!
//! `fixtures/brightspace.ics` is synthesised in the shape D2L Brightspace
//! emits (CRLF, 75-octet folding that splits a UTF-8 character, a Windows
//! TZID defined by its own VTIMEZONE, all-day dates, floating times, escaped
//! text). It stands in until the founder's real feed is stored (docs/SPEC.md
//! 11.2, Open: "the iCal parser against a stored raw feed"); when it is, it
//! goes beside this one and these tests run on both.

use jiff::tz::TimeZone;
use jiff::Timestamp;
use wi_heat::brightspace::{
    self, classes_types, feed_items, sync_feed, FeedItem, Known, School, DEFAULT_COURSE_PATTERN,
    NO_LONGER_IN_BRIGHTSPACE,
};
use wi_heat::ical;

const FEED: &[u8] = include_bytes!("fixtures/brightspace.ics");

fn school() -> School {
    let zone = TimeZone::get("America/New_York").unwrap();
    School::new("brightspace.uri.edu", DEFAULT_COURSE_PATTERN, zone).unwrap()
}

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

/// Tuesday, October 6, 8:40 AM in Kingston: the morning in docs/SPEC.md 1.6.
fn now() -> Timestamp {
    ts("2026-10-06T12:40:00Z")
}

fn items() -> Vec<FeedItem> {
    let school = school();
    let events = ical::parse(FEED, &school.zone).expect("the fixture is a calendar");
    feed_items(&events, &school, &classes_types())
}

/// Every VEVENT in the fixture that is a graded " - Due" item, written out by
/// hand from the file: (uid, title, course, type, due).
const DUE_ITEMS: [(&str, &str, &str, &str, &str); 10] = [
    (
        "1290877-128934@brightspace.uri.edu",
        "Grammar quiz 4",
        "JPN 201",
        "Quiz",
        "2026-10-08T03:59:00Z",
    ),
    (
        "1290912-130211@brightspace.uri.edu",
        "Homework 5",
        "MTH 142",
        "Homework",
        "2026-10-09T03:59:00Z",
    ),
    (
        "1290913-130211@brightspace.uri.edu",
        "Problem set 6",
        "MTH 142",
        "Homework",
        "2026-10-16T03:59:00Z",
    ),
    (
        "1291044-130587@brightspace.uri.edu",
        "Lab 5a",
        "PHY 203",
        "Lab",
        "2026-10-12T13:00:00Z",
    ),
    (
        "1291150-131002@brightspace.uri.edu",
        "Essay draft",
        "WRT 104",
        "Project",
        "2026-10-21T03:59:00Z",
    ),
    (
        "1291231-132440@brightspace.uri.edu",
        "MUS 110 Listening journal 3",
        "MUS 110",
        "Listening",
        "2026-10-13T03:59:00Z",
    ),
    (
        "1291300-128934@brightspace.uri.edu",
        "Kanji quiz 3 - lessons 7 and 8: reading and writing (漢字テスト 第三回)",
        "JPN 201",
        "Quiz",
        "2026-10-10T03:59:00Z",
    ),
    (
        "1291301-130211@brightspace.uri.edu",
        "Final project",
        "MTH 142",
        "Project",
        "2026-12-18T04:59:00Z",
    ),
    (
        "1290500-130211@brightspace.uri.edu",
        "Homework 1",
        "MTH 142",
        "Homework",
        "2026-09-11T03:59:00Z",
    ),
    (
        "1291410-130587@brightspace.uri.edu",
        "Problem set 4",
        "PHY 203",
        "Homework",
        "2026-10-06T11:00:00Z",
    ),
];

/// Every VEVENT the rules must drop: availability windows, a workshop, the
/// non-graded pair and the two cancelled items.
const DROPPED: [&str; 7] = [
    "1290878-128934@brightspace.uri.edu", // "- Availability Ends"
    "1290801-128934@brightspace.uri.edu", // "- Availability Starts"
    "1291045-130587@brightspace.uri.edu", // "(non-graded)" in the title
    "1291102-131002@brightspace.uri.edu", // "Non-Graded" in the description
    "1291200-130587@brightspace.uri.edu", // STATUS:CANCELLED
    "1291230-132440@brightspace.uri.edu", // "CANCELLED - " in the title
    "1291400-131002@brightspace.uri.edu", // no " - Due"
];

#[test]
fn no_due_item_is_missed() {
    let items = items();
    for (uid, title, course, kind, due) in DUE_ITEMS {
        let item = items
            .iter()
            .find(|i| i.uid == uid)
            .unwrap_or_else(|| panic!("missed {uid}"));
        assert_eq!(item.title, title, "{uid}");
        assert_eq!(item.course.as_deref(), Some(course), "{uid}");
        assert_eq!(item.kind, kind, "{uid}");
        assert_eq!(item.due, ts(due), "{uid}");
    }
    assert_eq!(items.len(), DUE_ITEMS.len());
}

#[test]
fn non_graded_availability_and_cancelled_items_are_dropped() {
    let items = items();
    for uid in DROPPED {
        assert!(items.iter().all(|i| i.uid != uid), "kept {uid}");
    }
}

#[test]
fn escaped_text_and_links_come_through() {
    let items = items();
    let hw = items.iter().find(|i| i.title == "Homework 5").unwrap();
    assert_eq!(
        hw.notes,
        "Section 3.4, problems 1-23 odd; show your work.\nUpload one PDF to the Assignments folder."
    );
    assert_eq!(
        hw.url.as_deref(),
        Some("https://brightspace.uri.edu/d2l/le/calendar/130211/event/1290912/detailsview")
    );
    let ps = items.iter().find(|i| i.title == "Problem set 4").unwrap();
    assert_eq!(
        ps.notes,
        "Chapters 5 and 6. Use g = 9.81 m/s^2; a backslash \\ marks a vector in the scanned key."
    );
}

#[test]
fn the_uid_becomes_the_task_id_and_never_changes() {
    let items = items();
    let mut known = Vec::new();
    let first = sync_feed(&mut known, &items, now(), &school().zone);

    // Items due from 2 hours ago to 70 days ahead become tasks; the rest wait.
    let mut made: Vec<&str> = first.new.iter().map(|i| i.uid.as_str()).collect();
    made.sort();
    let mut expected: Vec<&str> = DUE_ITEMS
        .iter()
        .map(|d| d.0)
        .filter(|uid| !uid.starts_with("1291301") && !uid.starts_with("1290500"))
        .collect();
    expected.sort();
    assert_eq!(made, expected);
    for task in &known {
        assert_eq!(
            Some(&task.id),
            task.uid.as_ref(),
            "a task made from the feed is named by its UID"
        );
    }

    let ids: Vec<String> = known.iter().map(|t| t.id.clone()).collect();
    let later = ts("2026-10-06T13:40:00Z");
    let second = sync_feed(&mut known, &items, later, &school().zone);
    assert!(second.new.is_empty(), "a second sync finds nothing new");
    assert!(second.date_changes.is_empty());
    let again: Vec<String> = known.iter().map(|t| t.id.clone()).collect();
    assert_eq!(ids, again, "no id changed between syncs");
}

#[test]
fn an_item_missing_from_two_syncs_is_tagged_and_never_deleted() {
    let all = items();
    let gone_uid = "1290913-130211@brightspace.uri.edu";
    let without: Vec<FeedItem> = all.iter().filter(|i| i.uid != gone_uid).cloned().collect();
    let zone = school().zone;
    let mut known = Vec::new();
    sync_feed(&mut known, &all, now(), &zone);
    let count = known.len();

    let one = sync_feed(&mut known, &without, ts("2026-10-06T13:40:00Z"), &zone);
    let task = known
        .iter()
        .find(|t| t.id == gone_uid)
        .expect("still there after one miss");
    assert!(!task.gone, "one miss is not enough");
    assert!(one.gone.is_empty());

    let two = sync_feed(&mut known, &without, ts("2026-10-06T14:40:00Z"), &zone);
    let task = known
        .iter()
        .find(|t| t.id == gone_uid)
        .expect("still there after two misses");
    assert!(task.gone);
    assert_eq!(two.gone, vec![gone_uid.to_string()]);
    assert_eq!(NO_LONGER_IN_BRIGHTSPACE, "No longer in Brightspace");

    for hour in 15..20 {
        sync_feed(
            &mut known,
            &without,
            ts(&format!("2026-10-06T{hour}:40:00Z")),
            &zone,
        );
    }
    assert_eq!(known.len(), count, "nothing is ever deleted on its own");
    assert!(known.iter().any(|t| t.id == gone_uid));

    let back = sync_feed(&mut known, &all, ts("2026-10-06T20:40:00Z"), &zone);
    assert_eq!(back.back, vec![gone_uid.to_string()]);
    assert!(!known.iter().find(|t| t.id == gone_uid).unwrap().gone);
}

#[test]
fn a_moved_due_date_is_one_date_change() {
    let mut items = items();
    let zone = school().zone;
    let mut known = Vec::new();
    sync_feed(&mut known, &items, now(), &zone);

    let uid = "1290912-130211@brightspace.uri.edu";
    let moved = ts("2026-10-11T03:59:00Z");
    items.iter_mut().find(|i| i.uid == uid).unwrap().due = moved;
    let report = sync_feed(&mut known, &items, ts("2026-10-06T13:40:00Z"), &zone);
    assert_eq!(report.date_changes.len(), 1);
    assert_eq!(report.date_changes[0].task_id, uid);
    assert_eq!(
        report.date_changes[0].from,
        Some(ts("2026-10-09T03:59:00Z"))
    );
    assert_eq!(report.date_changes[0].to, moved);
    assert_eq!(known.iter().find(|t| t.id == uid).unwrap().due, Some(moved));

    let quiet = sync_feed(&mut known, &items, ts("2026-10-06T14:40:00Z"), &zone);
    assert!(quiet.date_changes.is_empty());
}

fn from_google(id: &str, title: &str, course: &str, due: &str) -> Known {
    Known {
        id: id.into(),
        uid: None,
        title: title.into(),
        course: Some(course.into()),
        due: Some(ts(due)),
        done: false,
        missed: 0,
        gone: false,
    }
}

#[test]
fn a_task_from_google_calendar_takes_on_the_uid() {
    let items = items();
    let zone = school().zone;
    // As the artifact saved it: Google Calendar's event id, the title as the
    // calendar showed it, due the same evening at a different minute.
    let mut known = vec![
        from_google(
            "4b1q7f0ea3k8",
            "Homework 5 - Due",
            "MTH 142",
            "2026-10-09T03:00:00Z",
        ),
        // Same day and words, other course: not the same item.
        from_google(
            "9d2v0c5kq1pa",
            "Homework 5 - Due",
            "PHY 203",
            "2026-10-09T03:59:00Z",
        ),
        // Same day and course, too few words in common (1 of 3).
        from_google(
            "2m8s4h6tq0zn",
            "Exam 4 review",
            "JPN 201",
            "2026-10-08T03:59:00Z",
        ),
    ];
    let report = sync_feed(&mut known, &items, now(), &zone);

    assert_eq!(report.took_uid, vec!["4b1q7f0ea3k8".to_string()]);
    let task = &known[0];
    assert_eq!(task.id, "4b1q7f0ea3k8", "the task keeps its own id");
    assert_eq!(
        task.uid.as_deref(),
        Some("1290912-130211@brightspace.uri.edu")
    );
    assert_eq!(task.due, Some(ts("2026-10-09T03:59:00Z")));
    assert!(known[1].uid.is_none());
    assert!(known[2].uid.is_none());
    assert!(report
        .new
        .iter()
        .all(|i| i.uid != "1290912-130211@brightspace.uri.edu"));
    assert_eq!(report.new.len(), 7);
}

#[test]
fn titles_must_share_sixty_percent_of_their_words() {
    assert!(brightspace::same_title(
        "Grammar quiz 4",
        "Grammar Quiz 4 - Due"
    ));
    assert!(brightspace::same_title("Lab 5a report", "Lab 5a")); // 2 of 3
    assert!(!brightspace::same_title("Lab 5a prelab notes", "Lab 5a")); // 2 of 4
    assert!(!brightspace::same_title("", "Lab 5a"));
}

#[test]
fn an_expired_link_is_not_an_empty_calendar() {
    let page = b"<!DOCTYPE html><html><body>Login - Brightspace</body></html>";
    assert!(ical::parse(page, &school().zone).is_err());
}
