//! Calendars by iCal (docs/SPEC.md 3.11, 8.7; PLAN S2.5 and the first sync of
//! S2.6). Against a stored raw feed, what a fail looks like:
//! - a due item is missed, or a non-graded or cancelled one is kept;
//! - a UID changes, or an item missing from two syncs in a row is deleted;
//! - an iCal address is logged, or kept anywhere but the Keychain;
//! - a calendar's failure says anything but which calendar and when it
//!   will try again, or a page that isn't a calendar tags every task gone;
//! - the feed is read at other times than on open after 15 minutes, and hourly;
//! - moving in leaves the first sync finding the moved tasks new.
//!
//! The feed is `crates/wi-heat/tests/fixtures/brightspace.ics`, synthesised in
//! the shape D2L emits, served from a port on this machine.

use std::time::Duration;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::SecretStore;

/// As D2L writes it, 75-octet folds splitting a UTF-8 character and all: bytes, not text.
const FEED: &[u8] = include_bytes!("../../../wi-heat/tests/fixtures/brightspace.ics");

/// What the rules keep of the fixture on Tuesday, October 6 at 8:40 AM, from
/// two hours ago to 70 days ahead: (uid, title, course, type, due).
const KEPT: [(&str, &str, &str, &str, &str); 9] = [
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
        "1291102-131002@brightspace.uri.edu",
        "Reading response 3",
        "WRT 104",
        "Reading",
        "2026-10-10T03:59:00Z",
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
        "1291410-130587@brightspace.uri.edu",
        "Problem set 4",
        "PHY 203",
        "Homework",
        "2026-10-06T11:00:00Z",
    ),
];

/// Due items the window leaves for later (73 days ahead) or for never (last month).
const OUT_OF_WINDOW: [&str; 2] = [
    "1291301-130211@brightspace.uri.edu",
    "1290500-130211@brightspace.uri.edu",
];

/// The availability windows, a workshop, the non-graded item, the cancelled.
const DROPPED: [&str; 6] = [
    "1290878-128934@brightspace.uri.edu",
    "1290801-128934@brightspace.uri.edu",
    "1291045-130587@brightspace.uri.edu",
    "1291200-130587@brightspace.uri.edu",
    "1291230-132440@brightspace.uri.edu",
    "1291400-131002@brightspace.uri.edu",
];

const TOKEN: &str = "SECRET-TOKEN-4f9c2d71";
const NOW: &str = "2026-10-06 08:40";

fn ms(iso: &str) -> f64 {
    iso.parse::<jiff::Timestamp>().unwrap().as_millisecond() as f64
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// The feed without one event.
fn without(feed: &[u8], uid: &str) -> Vec<u8> {
    let at = find(feed, format!("UID:{uid}").as_bytes(), 0).expect("the fixture holds the event");
    let start = (0..at)
        .rev()
        .find(|i| feed[*i..].starts_with(b"BEGIN:VEVENT"))
        .unwrap();
    let end = find(feed, b"END:VEVENT", at).unwrap() + "END:VEVENT".len();
    let end = end
        + feed[end..]
            .iter()
            .take_while(|c| **c == b'\r' || **c == b'\n')
            .count();
    [&feed[..start], &feed[end..]].concat()
}

/// The feed with every `from` written as `to`.
fn replaced(feed: &[u8], from: &str, to: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = find(feed, from.as_bytes(), at) {
        out.extend_from_slice(&feed[at..i]);
        out.extend_from_slice(to.as_bytes());
        at = i + from.len();
    }
    out.extend_from_slice(&feed[at..]);
    out
}

fn tasks(core: &wi_core::Core) -> Vec<Value> {
    records(&snap(core, "2026-10-06"), "task")
}

fn brightspace(core: &wi_core::Core, feed: &FeedServer) -> Value {
    ok(
        core,
        "heat.calendars.add",
        json!({"name": "Brightspace", "kind": "brightspace", "url": feed.address(TOKEN)}),
    )["calendar"]
        .clone()
}

fn sync_line(core: &wi_core::Core) -> String {
    ok(core, "heat.calendars.sync", json!({}))["line"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn s2_5_the_feed_becomes_tasks_by_heats_rules_with_the_uid_as_the_id() {
    let feed = FeedServer::start(FEED);
    let setup = Setup::new();
    let core = heat_core_on(&setup, NOW, NO_SERVER);
    brightspace(&core, &feed);
    assert_eq!(sync_line(&core), "Synced 8:40 AM: 9 new tasks");

    let made = tasks(&core);
    assert_eq!(made.len(), 9);
    let shown = snap(&core, "2026-10-06");
    let course_code = |id: &str| {
        records(&shown, "course")
            .into_iter()
            .find(|c| c["id"] == id)
            .unwrap()["code"]
            .as_str()
            .unwrap()
            .to_string()
    };
    for (uid, title, course, kind, due) in KEPT {
        let t = made
            .iter()
            .find(|t| t["id"] == uid)
            .unwrap_or_else(|| panic!("{title} wasn't made"));
        assert_eq!(
            (
                t["title"].as_str(),
                t["type"].as_str(),
                t["source"].as_str(),
                t["sourceId"].as_str()
            ),
            (Some(title), Some(kind), Some("ical"), Some(uid))
        );
        assert_eq!(t["due"].as_f64(), Some(ms(due)), "{title}");
        assert_eq!(
            course_code(t["courseId"].as_str().unwrap()),
            course,
            "{title}"
        );
        assert_eq!(
            (
                t["done"].clone(),
                t["public"].clone(),
                t["difficulty"].clone()
            ),
            (json!(false), json!(false), json!(3))
        );
    }
    for uid in OUT_OF_WINDOW.into_iter().chain(DROPPED) {
        assert!(
            made.iter().all(|t| t["id"] != uid),
            "{uid} shouldn't be a task"
        );
    }
    // The courses and the term the feed names are made once.
    assert_eq!(records(&shown, "course").len(), 5);
    assert_eq!(
        records(&shown, "term")
            .iter()
            .map(|t| t["name"].clone())
            .collect::<Vec<_>>(),
        [json!("Fall 2026")]
    );
    // Brightspace items are tasks, never grey events (3.7).
    assert!(shown["events"].as_array().unwrap().is_empty());
    // The sync is one entry, and ⌘Z takes it back.
    assert_eq!(
        ok(&core, "history.get", json!({"room": "heat"}))["undo"],
        "Undo calendar sync"
    );
    ok(&core, "history.undo", json!({"room": "heat"}));
    assert!(tasks(&core).is_empty());
    ok(&core, "history.redo", json!({"room": "heat"}));
    assert_eq!(tasks(&core).len(), 9);

    // Reading it again changes nothing, and no id moves.
    let ids: Vec<Value> = tasks(&core).iter().map(|t| t["id"].clone()).collect();
    assert_eq!(sync_line(&core), "Synced 8:40 AM: nothing new");
    assert_eq!(
        tasks(&core)
            .iter()
            .map(|t| t["id"].clone())
            .collect::<Vec<_>>(),
        ids,
        "a UID never changes"
    );
    assert_eq!(
        ok(&core, "history.get", json!({"room": "heat"}))["undo"],
        "Undo calendar sync",
        "a sync that changed nothing leaves no entry"
    );
}

#[test]
fn s2_5_a_new_date_is_followed_and_a_done_task_keeps_the_date_it_was_done_against() {
    let feed = FeedServer::start(FEED);
    let setup = Setup::new();
    let core = heat_core_on(&setup, NOW, NO_SERVER);
    brightspace(&core, &feed);
    sync_line(&core);
    ok(
        &core,
        "heat.done",
        json!({"taskId": "1290913-130211@brightspace.uri.edu", "done": true}),
    );
    let moved = replaced(
        &replaced(FEED, "20261009T035900Z", "20261010T035900Z"),
        "20261016T035900Z",
        "20261017T035900Z",
    );
    feed.serve(200, &moved);
    assert_eq!(sync_line(&core), "Synced 8:40 AM: 1 date change");
    let by = |uid: &str| tasks(&core).into_iter().find(|t| t["id"] == uid).unwrap();
    assert_eq!(
        by("1290912-130211@brightspace.uri.edu")["due"].as_f64(),
        Some(ms("2026-10-10T03:59:00Z"))
    );
    assert_eq!(
        by("1290913-130211@brightspace.uri.edu")["due"].as_f64(),
        Some(ms("2026-10-16T03:59:00Z")),
        "done keeps its date"
    );
}

#[test]
fn s2_5_an_item_missing_from_two_syncs_is_tagged_and_never_deleted() {
    let feed = FeedServer::start(FEED);
    let setup = Setup::new();
    let core = heat_core_on(&setup, NOW, NO_SERVER);
    brightspace(&core, &feed);
    sync_line(&core);
    let lab = "1291044-130587@brightspace.uri.edu";
    let lab_task = |core: &wi_core::Core| tasks(core).into_iter().find(|t| t["id"] == lab);
    let gone = without(FEED, lab);
    feed.serve(200, &gone);
    sync_line(&core);
    assert!(
        lab_task(&core).unwrap().get("tag").is_none(),
        "missing once is not yet gone"
    );
    sync_line(&core);
    assert_eq!(lab_task(&core).unwrap()["tag"], "No longer in Brightspace");
    // It stays a task, with everything the person had put on it.
    assert_eq!(tasks(&core).len(), 9);
    sync_line(&core);
    sync_line(&core);
    assert!(lab_task(&core).is_some(), "never deleted on its own");
    // Back in the feed, the tag goes.
    feed.serve(200, FEED);
    sync_line(&core);
    assert!(lab_task(&core).unwrap().get("tag").is_none());
    // A task the person deleted stays deleted, though the feed still holds it.
    ok(&core, "heat.delete", json!({"kind": "task", "id": lab}));
    assert_eq!(sync_line(&core), "Synced 8:40 AM: nothing new");
    assert!(lab_task(&core).is_none());
}

#[test]
fn s2_5_a_calendar_that_cant_be_read_says_which_and_changes_nothing() {
    let feed = FeedServer::start(FEED);
    let setup = Setup::new();
    let core = heat_core_on(&setup, NOW, NO_SERVER);
    brightspace(&core, &feed);
    sync_line(&core);
    let sentence = "Couldn't read the Brightspace calendar. Heat will try again in an hour.";
    // The login page an expired link returns; an error; nothing listening.
    for (status, body) in [
        (200, "<html><body>Sign in to Brightspace</body></html>"),
        (500, "oops"),
        (404, ""),
    ] {
        feed.serve(status, body.as_bytes());
        for _ in 0..3 {
            assert_eq!(sync_line(&core), sentence);
        }
    }
    let t = tasks(&core);
    assert_eq!(t.len(), 9);
    assert!(
        t.iter().all(|t| t.get("tag").is_none()),
        "a feed that couldn't be read tags nothing gone"
    );
    drop(feed);
    assert_eq!(sync_line(&core), sentence);
    // No calendars, no line to speak of.
    let other = Setup::new();
    let empty = heat_core_on(&other, NOW, NO_SERVER);
    assert_eq!(
        sync_line(&empty),
        "No calendars yet. Add one in Settings → Heat."
    );
    // A calendar named for what it is.
    let work = FeedServer::start(b"not a calendar");
    ok(
        &empty,
        "heat.calendars.add",
        json!({"name": "Work", "kind": "ical", "url": work.address("w")}),
    );
    assert_eq!(
        sync_line(&empty),
        "Couldn't read the Work calendar. Heat will try again in an hour."
    );
}

#[test]
fn s2_5_an_address_lives_in_the_keychain_and_nowhere_else() {
    let feed = FeedServer::start(FEED);
    let setup = Setup::new();
    let core = heat_core_on(&setup, NOW, NO_SERVER);
    let events = core.events();
    let added = brightspace(&core, &feed);
    let item = added["keychainRef"].as_str().unwrap().to_string();
    assert_eq!(
        setup.secrets.get(&item).unwrap().as_deref(),
        Some(feed.address(TOKEN).as_str())
    );
    sync_line(&core);
    // A refusal says what is wrong with an address without repeating it.
    let e = refused(
        &core,
        "heat.calendars.add",
        json!({"name": "Bad", "kind": "ical", "url": format!("ftp://example.org/{TOKEN}")}),
    );
    assert_eq!(e.0, "bad_address");
    assert!(!e.1.contains(TOKEN));
    let second = refused(
        &core,
        "heat.calendars.add",
        json!({"name": "Again", "kind": "brightspace", "url": feed.address(TOKEN)}),
    );
    assert_eq!(
        second.1,
        "There is a Brightspace calendar already. Remove it first, then add the new link."
    );
    feed.serve(500, b"");
    sync_line(&core);

    // Every file the library holds, and every event and answer the core gave.
    let export = setup.dir.path().join("Export");
    ok(
        &core,
        "export.everything",
        json!({"to": export, "zip": false}),
    );
    let mut seen = Vec::new();
    for dir in [core.library().to_path_buf(), export.clone()] {
        let mut todo = vec![dir];
        while let Some(d) = todo.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    todo.push(p);
                } else if std::fs::metadata(&p)
                    .map(|m| m.len() < 64 << 20)
                    .unwrap_or(false)
                {
                    seen.push((p.clone(), std::fs::read(&p).unwrap_or_default()));
                }
            }
        }
    }
    assert!(
        seen.iter().any(|(p, _)| p.ends_with("library.sqlite"))
            && seen.iter().any(|(p, _)| p.ends_with("heat.json"))
    );
    for (path, bytes) in &seen {
        assert!(
            !bytes.windows(TOKEN.len()).any(|w| w == TOKEN.as_bytes()),
            "the address is in {}",
            path.display()
        );
    }
    let text: String = events.try_iter().map(|e| e.payload.to_string()).collect();
    assert!(!text.contains(TOKEN));
    // The record names the Keychain item, never the address; removing it forgets both.
    let shown = snap(&core, "2026-10-06");
    assert_eq!(records(&shown, "calendar")[0]["keychainRef"], item.as_str());
    ok(&core, "heat.calendars.remove", json!({"id": added["id"]}));
    assert_eq!(setup.secrets.get(&item).unwrap(), None);
    assert!(records(&snap(&core, "2026-10-06"), "calendar").is_empty());
    assert_eq!(tasks(&core).len(), 9, "the tasks it made stay");
}

#[test]
fn s2_5_another_calendars_events_are_grey_and_never_make_tasks() {
    let band = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Google Inc//Google Calendar 70.9054//EN\r\n\
BEGIN:VEVENT\r\nUID:practice-1@google.com\r\nDTSTART:20261006T220000Z\r\nDTEND:20261007T000000Z\r\nSUMMARY:Band practice\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:holiday-1@google.com\r\nDTSTART;VALUE=DATE:20261012\r\nDTEND;VALUE=DATE:20261013\r\nSUMMARY:Indigenous Peoples' Day\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:cancelled-1@google.com\r\nDTSTART:20261007T150000Z\r\nDTEND:20261007T160000Z\r\nSUMMARY:Called off\r\nSTATUS:CANCELLED\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:old-1@google.com\r\nDTSTART:20250101T150000Z\r\nDTEND:20250101T160000Z\r\nSUMMARY:Long ago\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:due-1@google.com\r\nDTSTART:20261008T150000Z\r\nDTEND:20261008T160000Z\r\nSUMMARY:Essay - Due\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n";
    let feed = FeedServer::start(band.as_bytes());
    let setup = Setup::new();
    let core = heat_core_on(&setup, NOW, NO_SERVER);
    let cal = ok(
        &core,
        "heat.calendars.add",
        json!({"name": "Band", "kind": "ical", "url": feed.address("band")}),
    )["calendar"]
        .clone();
    assert_eq!(sync_line(&core), "Synced 8:40 AM: nothing new");
    let shown = snap(&core, "2026-10-06");
    let mut titles: Vec<String> = shown["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["title"].as_str().unwrap().to_string())
        .collect();
    titles.sort();
    assert_eq!(
        titles,
        ["Band practice", "Essay - Due", "Indigenous Peoples' Day"]
    );
    let practice = shown["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["title"] == "Band practice")
        .unwrap()
        .clone();
    assert_eq!(
        (practice["calendarId"].clone(), practice["allDay"].clone()),
        (cal["id"].clone(), json!(false))
    );
    assert_eq!(practice["start"].as_f64(), Some(ms("2026-10-06T22:00:00Z")));
    let holiday = shown["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["title"] == "Indigenous Peoples' Day")
        .unwrap()
        .clone();
    assert_eq!(holiday["allDay"], true);
    assert!(
        tasks(&core).is_empty(),
        "other calendars' events never make tasks, even one that says Due"
    );
    // Plan my day goes around an event.
    let space = records(&shown, "space")[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    add_task(
        &core,
        &space,
        "Mix",
        json!({"estMin": 120, "due": ny("2026-10-06 23:00")}),
    );
    pin(&core, "2026-10-06 17:50");
    let plan = ok(&core, "heat.plan.make", json!({"date": "2026-10-06"}));
    let start = plan["drafts"][0]["start"].as_f64().unwrap();
    // Band practice is 6 to 8 PM in New York.
    assert!(
        start >= 20.0 * 60.0 || start + 90.0 <= 18.0 * 60.0,
        "a draft starting at {start} runs into band practice"
    );
    // Gone when the calendar is.
    ok(&core, "heat.calendars.remove", json!({"id": cal["id"]}));
    assert!(snap(&core, "2026-10-06")["events"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn s2_5_calendars_are_read_on_open_after_fifteen_minutes_then_hourly() {
    let feed = FeedServer::start(FEED);
    let setup = Setup::new();
    let core = heat_core_on(&setup, NOW, NO_SERVER);
    brightspace(&core, &feed);
    sync_line(&core);
    assert_eq!(feed.hits(), 1);
    drop(core);

    // Opened ten minutes on: not read.
    let core = heat_core_on(&setup, "2026-10-06 08:50", NO_SERVER);
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(feed.hits(), 1);
    drop(core);

    // Opened twenty-one minutes on: read as it opens.
    let core = heat_core_on(&setup, "2026-10-06 09:01", NO_SERVER);
    assert!(
        eventually(Duration::from_secs(8), || feed.hits() == 2),
        "not read on open"
    );
    // Then hourly: an hour and a bit on, with no one asking, it reads again.
    core.set_now(Some(ny("2026-10-06 10:05")));
    ok(
        &core,
        "heat.capture.add",
        json!({"text": "a write wakes the worker"}),
    );
    assert!(
        eventually(Duration::from_secs(8), || feed.hits() == 3),
        "not read after the hour"
    );
    // And not before the next one.
    core.set_now(Some(ny("2026-10-06 10:30")));
    ok(&core, "heat.capture.add", json!({"text": "again"}));
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(feed.hits(), 3);
}

#[test]
fn the_school_sheet_is_kept_checked_and_its_link_goes_to_the_keychain() {
    let feed = FeedServer::start(FEED);
    let setup = Setup::new();
    let core = heat_core_on(&setup, NOW, NO_SERVER);
    let bad = refused(
        &core,
        "heat.school.set",
        json!({"name": "URI", "host": "brightspace.uri.edu", "codePattern": "([", "termStart": "2026-08-31", "termEnd": "2026-12-18"}),
    );
    assert_eq!(
        bad.1,
        "The course pattern isn't one Heat can read. The default is ^([A-Z]{3})\\s?(\\d{3})."
    );
    let backwards = refused(
        &core,
        "heat.school.set",
        json!({"name": "URI", "host": "h", "codePattern": "", "termStart": "2026-12-18", "termEnd": "2026-08-31"}),
    );
    assert_eq!(backwards.1, "The term can't end before it starts.");
    let set = ok(
        &core,
        "heat.school.set",
        json!({"name": "URI", "host": "brightspace.uri.edu", "icalUrl": feed.address(TOKEN), "codePattern": "^([A-Z]{3})\\s?(\\d{3})", "termStart": "2026-08-31", "termEnd": "2026-12-18"}),
    );
    assert_eq!(set, json!({}));
    let calendars = records(&snap(&core, "2026-10-06"), "calendar");
    assert_eq!(
        (
            calendars.len(),
            calendars[0]["kind"].as_str(),
            calendars[0]["name"].as_str()
        ),
        (1, Some("brightspace"), Some("Brightspace"))
    );
    let item = calendars[0]["keychainRef"].as_str().unwrap();
    assert_eq!(
        setup.secrets.get(item).unwrap().as_deref(),
        Some(feed.address(TOKEN).as_str())
    );
    assert_eq!(sync_line(&core), "Synced 8:40 AM: 9 new tasks");
    // A pattern of the school's own picks the course out of the feed.
    ok(
        &core,
        "heat.school.set",
        json!({"name": "URI", "host": "brightspace.uri.edu", "codePattern": "^([A-Z]{3}) ?(\\d{3})"}),
    );
    // A new link for the calendar already there is the same calendar, read again.
    let second = FeedServer::start(FEED);
    ok(
        &core,
        "heat.school.set",
        json!({"name": "URI", "host": "brightspace.uri.edu", "icalUrl": second.address("another"), "codePattern": ""}),
    );
    assert_eq!(records(&snap(&core, "2026-10-06"), "calendar").len(), 1);
    assert_eq!(
        setup.secrets.get(item).unwrap().as_deref(),
        Some(second.address("another").as_str())
    );
}

#[test]
fn s2_6_the_first_sync_after_moving_in_finds_the_moved_tasks_already_there() {
    let feed = FeedServer::start(FEED);
    let setup = Setup::new();
    let core = heat_core_on(&setup, NOW, NO_SERVER);
    ok(
        &core,
        "heat.import",
        json!({"json": crate::heat_commands::artifact_export()}),
    );
    assert_eq!(tasks(&core).len(), 3);
    brightspace(&core, &feed);
    // Nine items in the feed; the one that came through Google Calendar before
    // (same day, same course, the same words) is that task, which takes its UID on.
    assert_eq!(sync_line(&core), "Synced 8:40 AM: 8 new tasks");
    let all = tasks(&core);
    assert_eq!(all.len(), 11);
    let google = all.iter().find(|t| t["id"] == "evt-google-1").unwrap();
    assert_eq!(
        (google["sourceId"].as_str(), google["source"].as_str()),
        (Some("1290877-128934@brightspace.uri.edu"), Some("ical"))
    );
    assert_eq!(
        all.iter()
            .filter(|t| t["title"] == "Grammar quiz 4")
            .count(),
        1,
        "no second copy of the same quiz"
    );
    // Every moved id is still there.
    for id in ["em-7c1f9a", "evt-google-1", "t-own-1"] {
        assert!(all.iter().any(|t| t["id"] == id), "{id}");
    }
    assert_eq!(sync_line(&core), "Synced 8:40 AM: nothing new");
}
