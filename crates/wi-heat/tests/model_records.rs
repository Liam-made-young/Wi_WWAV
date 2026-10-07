//! Heat's records as `docs/SPEC.md` 3.16 gives them. The TypeScript model
//! predates 3.16, so these have no TypeScript twin. They hold the two promises
//! the new fields make: a record the TypeScript wrote still reads and writes
//! back as it was, and what 3.16 added reads and writes once it is set.

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use wi_heat::model::focus::Phase;
use wi_heat::model::records::{
    Calendar, CalendarKind, Course, DailyNote, EstBy, FocusSession, FocusSource, Grade,
    GradeSource, Habit, HeatState, MailState, MailThread, Milestone, Note, ProfileShare, Project,
    RecordedBy, Room, ShareKind, Task, TaskSource,
};

/// Reads `wire` as a `T`, and checks that writing it gives `wire` again.
fn reads_and_writes_as_it_was<T: DeserializeOwned + Serialize>(wire: &Value) -> T {
    let record: T = serde_json::from_value(wire.clone()).expect("reads");
    assert_eq!(&serde_json::to_value(&record).expect("writes"), wire);
    record
}

/// What a record that can go public does: it reads as private when `public` is
/// absent, writes nothing for private, and writes `"public": true` once on.
fn goes_public<T: DeserializeOwned + Serialize>(
    wire: Value,
    is_public: impl Fn(&T) -> bool,
    switch_on: impl Fn(&mut T),
) {
    let mut record: T = reads_and_writes_as_it_was(&wire);
    assert!(!is_public(&record));
    switch_on(&mut record);
    let mut on = wire;
    on["public"] = json!(true);
    assert_eq!(serde_json::to_value(&record).expect("writes"), on);
    assert!(is_public(&reads_and_writes_as_it_was::<T>(&on)));
}

// --- a record from before 3.16 -----------------------------------------------------------------

#[test]
fn a_task_from_before_3_16_reads_with_the_new_fields_at_their_defaults_and_writes_back_as_it_was() {
    let wire = json!({
        "id": "task-1", "spaceId": "classes", "title": "Quiz 3", "type": "Quiz",
        "courseId": "jpn201", "due": 1791324000000i64, "difficulty": 3, "estMin": 45,
        "adjustMin": 0, "notes": "", "done": false, "doneAt": null, "source": "calendar"
    });
    let t: Task = reads_and_writes_as_it_was(&wire);
    assert_eq!(t.est_by, None);
    assert_eq!(t.est_reason, None);
    assert_eq!(t.source_id, None);
    assert!(!t.public);
    assert_eq!(t.source, TaskSource::Calendar);
}

#[test]
fn a_session_from_before_3_16_is_a_timer_session_that_is_not_public() {
    let wire = json!({
        "id": "session-1", "taskId": "task-1", "startedAt": 1791291600000i64,
        "endedAt": 1791293100000i64, "focusMin": 25, "interruptions": 1, "room": "space"
    });
    let s: FocusSession = reads_and_writes_as_it_was(&wire);
    assert_eq!(s.view, Room::Space);
    assert_eq!(s.source, FocusSource::Timer);
    assert!(!s.public);
}

#[test]
fn a_course_a_grade_a_habit_a_project_and_a_milestone_from_before_3_16_write_back_as_they_were() {
    let course: Course = reads_and_writes_as_it_was(&json!({
        "id": "jpn201", "termId": "fall26", "code": "JPN 201", "name": "Japanese",
        "categories": [{ "id": "hw", "name": "Homework", "weight": 20, "keywords": ["hw"] }],
        "notes": ""
    }));
    assert!(!course.public);
    let grade: Grade = reads_and_writes_as_it_was(&json!({
        "id": "g1", "courseId": "jpn201", "categoryId": "hw", "title": "Quiz 3", "score": 88,
        "outOf": 100, "dropped": false, "pending": false, "source": "mail"
    }));
    assert!(!grade.public);
    assert_eq!(grade.source, GradeSource::Mail);
    let habit: Habit = reads_and_writes_as_it_was(&json!({
        "id": "h1", "title": "Practise kanji", "minutes": 20,
        "log": { "2026-10-05": true }, "showCounter": false
    }));
    assert!(!habit.public);
    let project: Project = reads_and_writes_as_it_was(&json!({
        "id": "p1", "spaceId": "wwav", "title": "Album", "status": "on_hold"
    }));
    assert!(!project.public);
    let bead: Milestone = reads_and_writes_as_it_was(&json!({
        "id": "m1", "spaceId": "wwav", "title": "Enclosure v2", "date": "2026-10-20",
        "done": false, "order": 1
    }));
    assert!(!bead.public);
}

// --- what 3.16 added to them -----------------------------------------------------------------------

#[test]
fn a_task_reads_and_writes_who_estimated_it_where_it_came_from_and_whether_it_is_public() {
    let wire = json!({
        "id": "task-2", "spaceId": "classes", "title": "Read chapter 4", "type": "Reading",
        "due": null, "difficulty": 2, "estMin": 50, "estBy": "claude",
        "estReason": "Twelve pages, and you read about five a minute in JPN.",
        "adjustMin": 0, "notes": "", "done": false, "doneAt": null, "source": "claude",
        "sourceId": "18f3a9c2d1", "public": true
    });
    let t: Task = reads_and_writes_as_it_was(&wire);
    assert_eq!(t.est_by, Some(EstBy::Claude));
    assert_eq!(
        t.est_reason.as_deref(),
        Some("Twelve pages, and you read about five a minute in JPN.")
    );
    assert_eq!(t.source, TaskSource::Claude);
    assert_eq!(t.source_id.as_deref(), Some("18f3a9c2d1"));
    assert!(t.public);
}

#[test]
fn a_task_names_its_estimate_maker_as_you_claude_or_default() {
    for (word, by) in [
        ("you", EstBy::You),
        ("claude", EstBy::Claude),
        ("default", EstBy::Default),
    ] {
        let wire = json!({
            "id": "t", "spaceId": "s", "title": "x", "type": "y", "due": null, "difficulty": 1,
            "estMin": null, "estBy": word, "adjustMin": 0, "notes": "", "done": false,
            "doneAt": null, "source": "ical"
        });
        let t: Task = reads_and_writes_as_it_was(&wire);
        assert_eq!(t.est_by, Some(by));
        assert_eq!(t.source, TaskSource::Ical);
    }
}

#[test]
fn keeps_the_sources_the_artifact_import_makes_and_adds_3_16s() {
    for (word, source) in [
        ("you", TaskSource::You),
        ("calendar", TaskSource::Calendar),
        ("ical", TaskSource::Ical),
        ("mail", TaskSource::Mail),
        ("capture", TaskSource::Capture),
        ("claude", TaskSource::Claude),
    ] {
        assert_eq!(serde_json::to_value(source).unwrap(), json!(word));
        assert_eq!(
            serde_json::from_value::<TaskSource>(json!(word)).unwrap(),
            source
        );
    }
    for (word, source) in [
        ("you", GradeSource::You),
        ("mail", GradeSource::Mail),
        ("valence", GradeSource::Valence),
        ("claude", GradeSource::Claude),
    ] {
        assert_eq!(serde_json::to_value(source).unwrap(), json!(word));
        assert_eq!(
            serde_json::from_value::<GradeSource>(json!(word)).unwrap(),
            source
        );
    }
}

#[test]
fn a_session_says_who_logged_it_and_reads_its_view_under_either_name() {
    let by_claude = json!({
        "id": "session-2", "habitId": "h1", "startedAt": 0, "endedAt": 1500000, "focusMin": 25,
        "interruptions": 0, "room": "heat", "source": "claude", "public": true
    });
    let s: FocusSession = reads_and_writes_as_it_was(&by_claude);
    assert_eq!(s.source, FocusSource::Claude);
    assert_eq!(s.view, Room::Heat);

    // 3.16 calls it `view`; it is read, and written as `room` until the app is rewired.
    let mut spec_words = by_claude.clone();
    spec_words["view"] = spec_words["room"].take();
    spec_words.as_object_mut().unwrap().remove("room");
    let s: FocusSession = serde_json::from_value(spec_words).expect("reads view");
    assert_eq!(s.view, Room::Heat);
    assert_eq!(serde_json::to_value(&s).unwrap(), by_claude);
}

#[test]
fn every_record_the_spec_lets_go_public_starts_private_and_writes_nothing_until_it_is_on() {
    goes_public::<Task>(
        json!({
            "id": "t", "spaceId": "s", "title": "x", "type": "y", "due": null, "difficulty": 1,
            "estMin": null, "adjustMin": 0, "notes": "", "done": false, "doneAt": null,
            "source": "you"
        }),
        |t| t.public,
        |t| t.public = true,
    );
    goes_public::<FocusSession>(
        json!({
            "id": "f", "startedAt": 0, "endedAt": 1, "focusMin": 25, "interruptions": 0,
            "room": "heat"
        }),
        |s| s.public,
        |s| s.public = true,
    );
    goes_public::<Project>(
        json!({ "id": "p", "spaceId": "s", "title": "x", "status": "active" }),
        |p| p.public,
        |p| p.public = true,
    );
    goes_public::<Milestone>(
        json!({
            "id": "m", "spaceId": "s", "title": "x", "date": "2026-10-20", "done": false,
            "order": 1
        }),
        |m| m.public,
        |m| m.public = true,
    );
    goes_public::<Habit>(
        json!({ "id": "h", "title": "x", "log": {}, "showCounter": false }),
        |h| h.public,
        |h| h.public = true,
    );
    goes_public::<Course>(
        json!({
            "id": "c", "termId": "t", "code": "JPN 201", "name": "x", "categories": [],
            "notes": ""
        }),
        |c| c.public,
        |c| c.public = true,
    );
    goes_public::<Grade>(
        json!({
            "id": "g", "courseId": "c", "categoryId": null, "title": "x", "score": null,
            "outOf": 100, "dropped": false, "pending": true, "source": "claude"
        }),
        |g| g.public,
        |g| g.public = true,
    );
    goes_public::<DailyNote>(
        json!({ "date": "2026-10-06", "markdown": "Quiz tomorrow." }),
        |n| n.public,
        |n| n.public = true,
    );
    goes_public::<Note>(
        json!({ "id": "n", "markdown": "Liner notes." }),
        |n| n.public,
        |n| n.public = true,
    );
}

// --- the records 3.16 made -------------------------------------------------------------------------

#[test]
fn a_mail_thread_holds_what_claude_decided_and_no_body() {
    let wire = json!({
        "id": "mt1", "gmailThreadId": "18f3a9c2d1", "subject": "Quiz 3 grade posted",
        "from": "sato@school.edu", "receivedAt": 1791291600000i64, "course": "JPN 201",
        "state": "grade", "reason": "A grade notice for Quiz 3.", "taskId": "task-9",
        "recordedBy": "claude"
    });
    let m: MailThread = reads_and_writes_as_it_was(&wire);
    assert_eq!(m.state, MailState::Grade);
    assert_eq!(m.recorded_by, RecordedBy::Claude);
    assert_eq!(m.course.as_deref(), Some("JPN 201"));
    assert_eq!(m.task_id.as_deref(), Some("task-9"));

    // Nothing to act on: no course, no task. Claude is the only maker, written even if absent.
    let quiet = json!({
        "id": "mt2", "gmailThreadId": "18f3a9c2d2", "subject": "Campus newsletter",
        "from": "news@school.edu", "receivedAt": 0, "state": "nothing", "reason": "A newsletter."
    });
    let m: MailThread = serde_json::from_value(quiet.clone()).expect("reads");
    assert_eq!(m.state, MailState::Nothing);
    let mut written = quiet;
    written["recordedBy"] = json!("claude");
    assert_eq!(serde_json::to_value(&m).unwrap(), written);
    for (word, state) in [("grade", MailState::Grade), ("task", MailState::Task)] {
        assert_eq!(
            serde_json::from_value::<MailState>(json!(word)).unwrap(),
            state
        );
    }
}

#[test]
fn a_calendar_has_not_synced_until_it_has_a_time() {
    let fresh = json!({
        "id": "cal1", "name": "Brightspace", "kind": "brightspace",
        "keychainRef": "heat.calendar.cal1", "lastSyncedAt": null
    });
    let c: Calendar = reads_and_writes_as_it_was(&fresh);
    assert_eq!(c.kind, CalendarKind::Brightspace);
    assert_eq!(c.last_synced_at, None);

    let synced = json!({
        "id": "cal2", "name": "Club", "kind": "ical", "keychainRef": "heat.calendar.cal2",
        "lastSyncedAt": 1791291600000i64
    });
    let c: Calendar = reads_and_writes_as_it_was(&synced);
    assert_eq!(c.kind, CalendarKind::Ical);
    assert_eq!(c.last_synced_at, Some(1791291600000.0));

    // Absent reads the same as null.
    let mut absent = fresh.clone();
    absent.as_object_mut().unwrap().remove("lastSyncedAt");
    let c: Calendar = serde_json::from_value(absent).expect("reads");
    assert_eq!(serde_json::to_value(&c).unwrap(), fresh);
}

#[test]
fn a_note_and_a_daily_note_write_what_they_hold() {
    let daily: DailyNote = reads_and_writes_as_it_was(&json!({
        "date": "2026-10-06", "markdown": "Quiz tomorrow.", "public": true
    }));
    assert!(daily.public);
    let note: Note = reads_and_writes_as_it_was(&json!({
        "id": "n1", "title": "Liner notes", "markdown": "Recorded in one take.",
        "projectId": "p1", "link": { "kind": "work", "id": "w1" }, "public": true
    }));
    assert_eq!(note.title.as_deref(), Some("Liner notes"));
    assert_eq!(note.project_id.as_deref(), Some("p1"));
    let bare: Note = reads_and_writes_as_it_was(&json!({ "id": "n2", "markdown": "" }));
    assert_eq!(bare.title, None);
    assert_eq!(bare.link, None);
}

#[test]
fn a_profile_share_is_a_now_line_that_clears_or_a_timeline() {
    let now: ProfileShare = reads_and_writes_as_it_was(&json!({
        "id": "s1", "kind": "now", "sourceId": "task-1", "text": "Mixing the second side.",
        "targetId": "sun", "clearsAt": 1791896400000i64
    }));
    assert_eq!(now.kind, ShareKind::Now);
    assert_eq!(now.clears_at, Some(1791896400000.0));
    let timeline: ProfileShare = reads_and_writes_as_it_was(&json!({
        "id": "s2", "kind": "timeline", "sourceId": "p1", "targetId": "p1"
    }));
    assert_eq!(timeline.kind, ShareKind::Timeline);
    assert_eq!(timeline.text, None);
    assert_eq!(timeline.clears_at, None);
}

#[test]
fn heat_state_keeps_the_current_task_the_timer_and_the_drafts() {
    let busy = json!({
        "currentTaskId": "task-1",
        "timer": { "phase": "focus", "round": 2, "endsAt": 1791293100000i64 },
        "planDrafts": [{
            "taskId": "task-1", "date": "2026-10-06", "start": 540, "minutes": 45, "leftMin": 0,
            "reason": "Due today 4:00 PM, Hot.", "leftLine": null
        }]
    });
    let s: HeatState = reads_and_writes_as_it_was(&busy);
    assert_eq!(s.current_task_id.as_deref(), Some("task-1"));
    assert_eq!(s.timer.phase, Phase::Focus);
    assert_eq!(s.timer.round, 2.0);
    assert_eq!(s.plan_drafts.len(), 1);

    // A fresh install has none of it: the timer is idle on round 1.
    let fresh: HeatState = serde_json::from_value(json!({})).expect("reads");
    assert_eq!(fresh, HeatState::default());
    assert_eq!(fresh.timer.phase, Phase::Idle);
    assert_eq!(fresh.timer.round, 1.0);
    assert_eq!(fresh.timer.ends_at, None);
    assert_eq!(
        serde_json::to_value(&fresh).unwrap(),
        json!({
            "timer": { "phase": "idle", "round": 1, "endsAt": null },
            "planDrafts": []
        })
    );
}
