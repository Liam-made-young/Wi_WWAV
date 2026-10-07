//! An independent review's adversarial tests for S2.5, S2.7 and 2.11/3.12.
//! A test marked #[ignore] fails against the code as reviewed; its comment
//! names the finding it exposes. Run them with `cargo test -- --ignored`.

use jiff::tz::TimeZone;
use jiff::Timestamp;
use serde_json::json;
use wi_heat::assist::{check_review, read_mail_body};
use wi_heat::brightspace::{
    classes_types, feed_items, is_kept, sync_feed, School, DEFAULT_COURSE_PATTERN,
};
use wi_heat::ical::{self, Event, When};
use wi_heat::mail::{self, MailState, Message};
use wi_heat::sync::{Change, Page, Replica, Server};

fn ny() -> TimeZone {
    TimeZone::get("America/New_York").unwrap()
}

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

fn school() -> School {
    School::new("brightspace.uri.edu", DEFAULT_COURSE_PATTERN, ny()).unwrap()
}

fn calendar(body: &str) -> String {
    format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{body}END:VCALENDAR\r\n")
}

fn due_event(summary: &str, description: &str) -> Event {
    Event {
        uid: Some("1300001-130211@brightspace.uri.edu".into()),
        summary: summary.into(),
        description: description.into(),
        location: "MTH 142 Sec 0004 - Fall 2026".into(),
        start: Some(When::At(ts("2026-10-09T03:59:00Z"))),
        ..Event::default()
    }
}

// ----- S2.5: a due item is missed -----

/// Finding: `is_kept` drops any item whose title holds the word "cancelled",
/// a rule the spec doesn't have (3.11 skips cancelled items, which the feed
/// marks with STATUS). A graded make-up for a cancelled class is a due item.
#[test]
#[ignore = "finding: the title-word cancellation rule drops graded due items"]
fn a_make_up_for_a_cancelled_class_is_still_due() {
    let e = due_event("Make-up quiz for the cancelled lab - Due", "");
    assert!(is_kept(&e), "a graded make-up quiz was dropped");
}

/// Finding: /non-graded/i is also tested against the description, so a
/// graded item that points at a non-graded practice run is dropped.
#[test]
#[ignore = "finding: /non-graded/i on the description drops graded due items"]
fn a_graded_quiz_that_mentions_non_graded_practice_is_still_due() {
    let e = due_event(
        "Quiz 5 - Due",
        "Graded, 10 points. Try the non-graded practice quiz first.",
    );
    assert!(is_kept(&e), "a graded quiz was dropped");
}

/// Finding: the reader promises "a missing END ... loses no event", but a
/// VEVENT without its END swallows every VEVENT after it as a child, so
/// they never reach the calendar's list.
#[test]
#[ignore = "finding: one missing END:VEVENT hides every later event"]
fn one_missing_end_vevent_loses_no_later_event() {
    let ics = calendar(
        "BEGIN:VEVENT\r\nUID:a\r\nSUMMARY:Homework 5 - Due\r\nDTSTART:20261009T035900Z\r\n\
         BEGIN:VEVENT\r\nUID:b\r\nSUMMARY:Homework 6 - Due\r\nDTSTART:20261016T035900Z\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:c\r\nSUMMARY:Lab 5a - Due\r\nDTSTART:20261012T130000Z\r\nEND:VEVENT\r\n",
    );
    let events = ical::parse(ics.as_bytes(), &ny()).unwrap();
    let uids: Vec<&str> = events.iter().filter_map(|e| e.uid.as_deref()).collect();
    assert_eq!(uids, ["a", "b", "c"]);
}

/// Finding: RFC 5545 makes the file's VTIMEZONE the definition of its TZID,
/// but a TZID the bundled database also knows is read from the database. A
/// feed that names its Eastern zone "EST" and defines it with DST gets every
/// summer deadline an hour late.
#[test]
#[ignore = "finding: the database overrides the file's own VTIMEZONE"]
fn the_files_vtimezone_defines_its_tzid() {
    let ics = calendar(
        "BEGIN:VTIMEZONE\r\nTZID:EST\r\n\
         BEGIN:STANDARD\r\nDTSTART:16011104T020000\r\nRRULE:FREQ=YEARLY;BYDAY=1SU;BYMONTH=11\r\n\
         TZOFFSETFROM:-0400\r\nTZOFFSETTO:-0500\r\nEND:STANDARD\r\n\
         BEGIN:DAYLIGHT\r\nDTSTART:16010311T020000\r\nRRULE:FREQ=YEARLY;BYDAY=2SU;BYMONTH=3\r\n\
         TZOFFSETFROM:-0500\r\nTZOFFSETTO:-0400\r\nEND:DAYLIGHT\r\nEND:VTIMEZONE\r\n\
         BEGIN:VEVENT\r\nUID:x\r\nSUMMARY:Lab 5a - Due\r\nDTSTART;TZID=EST:20261012T090000\r\nEND:VEVENT\r\n",
    );
    let e = &ical::parse(ics.as_bytes(), &ny()).unwrap()[0];
    assert_eq!(e.start, Some(When::At(ts("2026-10-12T13:00:00Z"))));
}

/// Passes: a UID with escapes, folding and padding reads the same every
/// sync, and the id never moves when the date, title or course change.
#[test]
fn a_uid_survives_refolding_and_edits() {
    let one = calendar(
        "BEGIN:VEVENT\r\nUID:1290912-130211@brightspace.uri.edu\r\nSUMMARY:Homework 5 - Due\r\n\
         LOCATION:MTH 142\r\nDTSTART:20261009T035900Z\r\nEND:VEVENT\r\n",
    );
    let two = calendar(
        "BEGIN:VEVENT\r\nUID: 1290912-130211@brigh\r\n tspace.uri.edu \r\nSUMMARY:Homework 5 (revised) - Due\r\n\
         LOCATION:MTH 143\r\nDTSTART:20261011T035900Z\r\nEND:VEVENT\r\n",
    );
    let s = school();
    let mut known = Vec::new();
    for (n, ics) in [one, two].iter().enumerate() {
        let events = ical::parse(ics.as_bytes(), &s.zone).unwrap();
        let items = feed_items(&events, &s, &classes_types());
        let report = sync_feed(&mut known, &items, ts("2026-10-06T12:40:00Z"), &s.zone);
        assert_eq!(report.new.len(), usize::from(n == 0));
    }
    assert_eq!(known.len(), 1);
    assert_eq!(known[0].id, "1290912-130211@brightspace.uri.edu");
    assert_eq!(known[0].due, Some(ts("2026-10-11T03:59:00Z")));
}

// ----- 2.11/3.12 and the mail path -----

fn long_message(n: usize, text: &str) -> Message {
    Message {
        id: format!("18c3{n:04}"),
        thread_id: format!("18c3{n:04}"),
        from: "Brightspace <noreply@brightspace.uri.edu>".into(),
        subject: format!("JPN 201 お知らせ {n}"),
        text: text.into(),
        received: ts("2026-10-05T14:00:00Z"),
    }
}

/// Finding: MAIL_TEXT_CHARS cuts each message at 6000 characters, not bytes,
/// "so eight long emails stay inside the server's 100 KB body limit". Eight
/// long Japanese announcements are 144 KB. The server refuses the body
/// (413), the client neither retries nor remembers the batch, and the next
/// sync sends the same eight oldest messages: mail reading stalls until they
/// fall out of the 14-day window.
#[test]
#[ignore = "finding: eight long non-ASCII emails exceed the 100 KB body limit every sync"]
fn eight_long_japanese_announcements_fit_the_body_limit() {
    let text = "来週の小テストは第九課です。".repeat(500); // 7000 characters
    let inbox: Vec<Message> = (0..11).map(|n| long_message(n, &text)).collect();
    let mut state = MailState::default();
    let titles: Vec<String> = (0..80).map(|n| format!("Task {n}")).collect();
    for sync in 0..2 {
        let plan = state.plan(&inbox, &school(), true);
        let body = read_mail_body(&plan.for_claude, ts("2026-10-06T12:40:00Z"), &ny(), &titles);
        let bytes = serde_json::to_vec(&body).unwrap().len();
        assert!(
            bytes <= 100 * 1024,
            "sync {sync}: a {bytes}-byte body is over express.json()'s 100 KB"
        );
        state.finish(&plan, false, ts("2026-10-06T12:40:00Z"));
    }
}

/// Finding: "new grades?" matches anywhere in a subject, so an announcement
/// about a new grading scale becomes a pending grade and its deadline never
/// reaches Claude.
#[test]
#[ignore = "finding: the grade-notice pattern takes announcements for grades"]
fn an_announcement_about_a_new_grade_scale_is_not_a_grade() {
    let m = long_message(1, "The final project is due Friday, Oct 16.");
    let m = Message {
        subject: "New grade scale for the final project, due Friday".into(),
        ..m
    };
    assert!(!mail::is_grade_notice(&m));
}

/// Finding: `plan` removes repeated ids with `dedup_by` after sorting by
/// time, which only removes neighbours. A message listed twice (Gmail pages
/// shifting under new mail) with another in between at the same instant
/// makes two pending grades with one id.
#[test]
#[ignore = "finding: a repeated message id makes two pending grades"]
fn a_message_listed_twice_makes_one_pending_grade() {
    let at = ts("2026-10-05T14:00:00Z");
    let grade = Message {
        subject: "Grade released: Quiz 4 - JPN 201".into(),
        received: at,
        ..long_message(1, "")
    };
    let other = Message {
        id: "18c3other".into(),
        subject: "Office hours moved".into(),
        received: at,
        ..long_message(2, "")
    };
    let plan = MailState::default().plan(&[grade.clone(), other, grade], &school(), true);
    assert_eq!(plan.grades.len(), 1);
}

/// Finding: the "never invent metrics" check only looks at digits, so a
/// number written as a word passes.
#[test]
#[ignore = "finding: the review check lets numbers written as words through"]
fn a_review_draft_cannot_invent_a_number_in_words() {
    let facts = vec!["Tasks done: 14".to_string()];
    let draft = r#"{"draft":"What moved: 14 tasks, a twenty percent gain on last week."}"#;
    assert!(check_review(draft, &facts).is_none());
}

// ----- S2.7 -----

/// Finding (a spec question as much as a bug): stamps order concurrent writes
/// by how many writes a device has made, not by when. The studio edits a
/// title at 9:00 while offline, after a morning of other edits; the Air
/// edits the same title at 10:00. The Air's push lands first, the studio's
/// lands at 10:05 and replaces the 10:00 title with the 9:00 one: a slower
/// older write overwrites a newer one, as the person sees it.
#[test]
#[ignore = "finding: across devices, a busier device's earlier edit beats a later one"]
fn a_later_edit_on_a_quieter_device_is_not_overwritten_by_an_earlier_one() {
    let mut studio = Replica::new("studio");
    let mut air = Replica::new("air");
    let mut server = Server::default();
    for n in 0..10 {
        studio.write("task", &format!("t{n}"), "notes", json!("morning"));
    }
    studio.write("task", "title-me", "title", json!("9:00 title")); // 9:00, offline
    air.write("task", "title-me", "title", json!("10:00 title")); // 10:00
    let a = air.push_batch();
    server.push(&a); // lands 10:01
    air.acked(&a);
    while studio.has_pending() {
        let s = studio.push_batch();
        server.push(&s); // lands 10:05
        studio.acked(&s);
    }
    let page = server.pull(0, 100);
    let title = page.changes.iter().find(|c| c.id == "title-me").unwrap();
    assert_eq!(title.value, json!("10:00 title"));
}

/// Finding: a pulled sequence number at the top of u64 makes the next local
/// write overflow: a panic in debug builds, and a wrap to 0 in release, after
/// which every local write loses. Remote input shouldn't be able to do that.
#[test]
#[ignore = "finding: a pulled u64::MAX seq overflows the next local write"]
fn a_huge_pulled_number_does_not_break_local_writes() {
    let mut mac = Replica::new("mac");
    mac.pulled(Page {
        changes: vec![Change {
            table: "task".into(),
            id: "t1".into(),
            field: "title".into(),
            value: json!("from the server"),
            seq: u64::MAX,
            device: "air".into(),
        }],
        cursor: 1,
        more: false,
    });
    let wrote = std::panic::catch_unwind(move || {
        mac.write("task", "t2", "title", json!("local"));
        mac.push_batch()[0].seq
    });
    assert!(wrote.is_ok(), "a local write panicked");
}

/// Finding: a Replica can't be saved or restored (private fields, no
/// constructor from saved state), so after a restart a device starts again
/// at seq 0. Its first edit after an offline restart carries the same stamp
/// as its own older edit; the server keeps the older one and the device
/// keeps the newer one, and they never converge.
#[test]
#[ignore = "finding: the counter can't survive a restart, so a newer edit loses"]
fn a_newer_edit_after_a_restart_reaches_the_server() {
    let mut server = Server::default();
    let mut mac = Replica::new("mac");
    mac.write("task", "t1", "title", json!("before the restart"));
    let b = mac.push_batch();
    server.push(&b);
    mac.acked(&b);

    // The app restarts offline; the crate offers no way to restore `mac`.
    let mut mac = Replica::new("mac");
    mac.write("task", "t1", "title", json!("after the restart"));
    let b = mac.push_batch();
    server.push(&b);
    mac.acked(&b);
    loop {
        let page = server.pull(mac.cursor(), 10);
        let more = page.more;
        mac.pulled(page);
        if !more {
            break;
        }
    }
    let on_server = server.pull(0, 10).changes[0].value.clone();
    assert_eq!(on_server, json!("after the restart"));
    assert_eq!(mac.value("task", "t1", "title"), Some(&on_server));
}
