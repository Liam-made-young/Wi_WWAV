//! docs/SPEC.md 2.11 and 3.12: the Claude client rules for
//! `/api/assist/:task`. Fails if a body sends more than the consent sentence
//! names or more than its limits, a failure reads anything but 2.11's words,
//! a non-retryable error is retried, a retryable one is retried more than once
//! or outside 1500-2300 ms, or an answer outside the clamps is accepted.

use jiff::tz::TimeZone;
use jiff::Timestamp;
use proptest::prelude::*;
use rand::rngs::mock::StepRng;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Duration;
use wi_heat::assist::{
    self, check_review, fallback_minutes, gate, parse_batch, parse_mail_tasks, parse_release_plan,
    parse_score, parse_syllabus, read_mail_body, release_milestones, step, BatchTask, Consent,
    Failure, Gate, Outcome, ScoreBatchBody, ScoreBody, Step, Task, Tier,
};
use wi_heat::mail::{Message, MORE_EMAILS_LEFT};

fn ts(s: &str) -> Timestamp {
    s.parse().unwrap()
}

fn zone() -> TimeZone {
    TimeZone::get("America/New_York").unwrap()
}

fn keys(v: &Value) -> Vec<&str> {
    v.as_object().unwrap().keys().map(String::as_str).collect()
}

#[test]
fn the_consent_sentence_is_exact_and_the_body_sends_nothing_else() {
    assert_eq!(
        assist::SCORING_CONSENT,
        "Learn will send this task's title, type and notes, and your average minutes per type. Nothing else."
    );
    assert_eq!(assist::TURN_ON_SCORING, "Turn on scoring");
    assert_eq!(assist::NOT_NOW, "Not now");

    let body = ScoreBody {
        title: "Grammar quiz 4".into(),
        kind: "Quiz".into(),
        notes: "Covers lesson 9.".into(),
        averages: BTreeMap::from([("Quiz".to_string(), 35), ("Homework".to_string(), 75)]),
    };
    let v = serde_json::to_value(&body).unwrap();
    assert_eq!(keys(&v), ["title", "type", "notes", "averages"]);
    assert_eq!(v["averages"], json!({"Homework": 75, "Quiz": 35}));

    let batch = ScoreBatchBody {
        tasks: vec![BatchTask {
            i: 0,
            title: "Lab 5a".into(),
            kind: "Lab".into(),
            notes: String::new(),
        }],
        averages: BTreeMap::new(),
    };
    let v = serde_json::to_value(&batch).unwrap();
    assert_eq!(keys(&v), ["tasks", "averages"]);
    assert_eq!(keys(&v["tasks"][0]), ["i", "title", "type", "notes"]);
}

#[test]
fn each_feature_is_off_until_its_first_use() {
    assert_eq!(Consent::default(), Consent::Unasked);
    assert_eq!(gate(Consent::Unasked, true), Gate::Ask);
    assert_eq!(gate(Consent::On, true), Gate::Go);
    assert_eq!(gate(Consent::Off, true), Gate::Hidden);
    assert_eq!(gate(Consent::Off, false), Gate::Hidden);
    assert_eq!(gate(Consent::On, false), Gate::NeedsConnection);
    assert_eq!(gate(Consent::Unasked, false), Gate::NeedsConnection);
}

#[test]
fn failures_read_exactly_as_written() {
    assert_eq!(
        Failure::NotGranted.sentence(),
        "Claude scoring is off. Set difficulty yourself."
    );
    assert_eq!(
        Failure::RateLimited.sentence(),
        "Too many requests. Wait a minute, then try again."
    );
    assert_eq!(Failure::Offline.sentence(), "Needs a connection");
    assert_eq!(
        Failure::DailyLimit { limit: 50 }.sentence(),
        "Claude's 50 calls for today are used. They come back at midnight."
    );
}

#[test]
fn each_feature_says_its_own_failure() {
    // Batch scoring of synced items fails silently and keeps the defaults.
    assert_eq!(
        assist::failure_line(Task::ScoreBatch, &Failure::RateLimited),
        None
    );
    // Reading mail carries the rest to the next sync.
    assert_eq!(
        assist::failure_line(Task::ReadMail, &Failure::Offline).as_deref(),
        Some(MORE_EMAILS_LEFT)
    );
    assert_eq!(
        MORE_EMAILS_LEFT,
        "More emails left, they'll come in on the next sync."
    );
    assert_eq!(
        assist::failure_line(Task::Score, &Failure::RateLimited).as_deref(),
        Some("Too many requests. Wait a minute, then try again.")
    );
}

#[test]
fn tasks_have_one_endpoint_and_a_tier() {
    assert_eq!(Task::Score.path(), "/api/assist/score");
    assert_eq!(Task::ReadMail.path(), "/api/assist/read-mail");
    assert_eq!(Task::Score.tier(), Tier::Quick);
    assert_eq!(Task::ScoreBatch.tier(), Tier::Quick);
    for t in [
        Task::ReadMail,
        Task::Syllabus,
        Task::WeeklyReview,
        Task::ReleasePlan,
    ] {
        assert_eq!(t.tier(), Tier::Default);
    }
}

fn answered(status: u16, body: &str) -> Outcome {
    Outcome::Answered {
        status,
        body: body.into(),
    }
}

#[test]
fn retryable_errors_get_one_retry_after_1500_ms_plus_jitter() {
    // A generator stuck at zero draws no jitter at all.
    let mut low = StepRng::new(0, 0);
    for outcome in [
        answered(500, ""),
        answered(502, ""),
        answered(503, ""),
        answered(529, ""),
        Outcome::TimedOut,
    ] {
        assert_eq!(
            step(1, outcome.clone(), &mut low),
            Step::RetryAfter(Duration::from_millis(1500))
        );
        assert_eq!(
            step(2, outcome, &mut low),
            Step::Done(Err(Failure::Unavailable)),
            "only one retry"
        );
    }
    // Over many draws the wait covers 1500 to 2300 ms and never leaves it.
    let mut rng = StdRng::seed_from_u64(7);
    let waits: Vec<u128> = (0..10_000)
        .map(|_| match step(1, Outcome::TimedOut, &mut rng) {
            Step::RetryAfter(d) => d.as_millis(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(waits.iter().min(), Some(&1500));
    assert_eq!(waits.iter().max(), Some(&2300));
}

#[test]
fn other_answers_are_never_retried() {
    let mut rng = StepRng::new(0, 0);
    let ok = r#"{"difficulty":2,"minutes":45,"reason":"A short quiz on one lesson."}"#;
    assert_eq!(
        step(1, answered(200, ok), &mut rng),
        Step::Done(Ok(ok.into()))
    );
    assert_eq!(
        step(
            1,
            answered(
                429,
                r#"{"error":"Too many requests","code":"rate_limited"}"#
            ),
            &mut rng
        ),
        Step::Done(Err(Failure::RateLimited))
    );
    assert_eq!(
        step(
            1,
            answered(
                429,
                r#"{"error":"Daily limit","code":"daily_limit","limit":50}"#
            ),
            &mut rng
        ),
        Step::Done(Err(Failure::DailyLimit { limit: 50 }))
    );
    assert_eq!(
        step(1, Outcome::Offline, &mut rng),
        Step::Done(Err(Failure::Offline))
    );
    // A body the server refuses is refused again if sent again.
    for status in [400, 413, 422] {
        assert_eq!(
            step(1, answered(status, "{}"), &mut rng),
            Step::Done(Err(Failure::Refused)),
            "{status}"
        );
    }
    for status in [401, 403, 404] {
        assert_eq!(
            step(1, answered(status, "{}"), &mut rng),
            Step::Done(Err(Failure::Unavailable)),
            "{status}"
        );
    }
    assert_eq!(
        Failure::Refused.sentence(),
        Failure::Unavailable.sentence(),
        "the spec gives a refusal no words of its own"
    );
}

#[test]
fn a_score_is_clamped_and_needs_its_reason() {
    let s = parse_score(r#"{"difficulty":2,"minutes":45,"reason":"A short quiz on one lesson."}"#)
        .unwrap();
    assert_eq!(
        (s.difficulty, s.minutes, s.reason.as_str()),
        (2, 45, "A short quiz on one lesson.")
    );

    let s = parse_score(r#"{"difficulty":9,"minutes":2,"reason":"Tiny."}"#).unwrap();
    assert_eq!((s.difficulty, s.minutes), (5, 5));
    let s = parse_score(r#"{"difficulty":0.4,"minutes":4000,"reason":"Huge."}"#).unwrap();
    assert_eq!((s.difficulty, s.minutes), (1, 600));

    // Claude sometimes wraps its JSON in a sentence or a fence.
    let s = parse_score(
        "Here it is:\n```json\n{\"difficulty\":3,\"minutes\":90,\"reason\":\"Two sections.\"}\n```",
    )
    .unwrap();
    assert_eq!(s.minutes, 90);

    assert!(
        parse_score(r#"{"difficulty":3,"minutes":90}"#).is_none(),
        "no reason, no suggestion"
    );
    assert!(parse_score("I can't help with that.").is_none());
}

#[test]
fn without_claude_the_estimate_is_the_type_average_else_difficulty_times_20() {
    assert_eq!(fallback_minutes(Some(75), 2), 75);
    assert_eq!(fallback_minutes(None, 2), 40);
    assert_eq!(fallback_minutes(None, 5), 100);
}

#[test]
fn batch_scores_line_up_by_index_and_clamp() {
    let body = r#"{"scores":[{"i":1,"difficulty":3,"minutes":900},{"i":0,"difficulty":1,"minutes":1},{"i":7,"difficulty":2,"minutes":30}]}"#;
    assert_eq!(
        parse_batch(body, 3),
        vec![Some((1, 5)), Some((3, 600)), None]
    );
    assert_eq!(parse_batch("not json", 2), vec![None, None]);
}

fn message(n: usize) -> Message {
    Message {
        id: format!("18c2f{n:03}"),
        thread_id: format!("18c2f{n:03}"),
        from: "Brightspace <noreply@brightspace.uri.edu>".into(),
        subject: format!("Announcement {n}"),
        text: "Problem set 7 is due Friday at 5 PM.".into(),
        received: ts("2026-10-05T14:00:00Z"),
    }
}

#[test]
fn reading_mail_sends_at_most_8_messages_and_80_titles() {
    let messages: Vec<Message> = (0..11).map(message).collect();
    let titles: Vec<String> = (0..95).map(|n| format!("Task {n}")).collect();
    let body = read_mail_body(&messages, ts("2026-10-06T12:40:00Z"), &zone(), &titles);
    let v = serde_json::to_value(&body).unwrap();
    assert_eq!(keys(&v), ["messages", "today", "zone", "titles"]);
    assert_eq!(v["messages"].as_array().unwrap().len(), 8);
    assert_eq!(v["titles"].as_array().unwrap().len(), 80);
    assert_eq!(v["today"], "2026-10-06");
    assert_eq!(v["zone"], "America/New_York");
    assert_eq!(
        keys(&v["messages"][0]),
        ["id", "from", "subject", "date", "text"]
    );
}

proptest! {
    /// Whatever the mail holds (long, non-ASCII, full of characters JSON
    /// escapes), the body stays under the server's limit, and every message
    /// keeps a share of it.
    #[test]
    fn a_read_mail_body_never_passes_its_byte_limit(
        texts in prop::collection::vec(("[a-z \n\"\\\u{1}\t漢字🎵é]{0,40}", 0..400usize), 1..12),
        line in "[a-z漢🎵\u{2}\"]{0,400}",
        titles in prop::collection::vec("[a-zA-Z漢🎵\"\u{3} ]{0,500}", 0..100),
    ) {
        let messages: Vec<Message> = texts
            .iter()
            .enumerate()
            .map(|(n, (piece, times))| Message {
                subject: format!("{line} {n}"),
                from: line.clone(),
                text: piece.repeat(*times),
                ..message(n)
            })
            .collect();
        let body = read_mail_body(&messages, ts("2026-10-06T12:40:00Z"), &zone(), &titles);
        let bytes = serde_json::to_vec(&body).unwrap().len();
        prop_assert!(bytes <= assist::MAIL_BODY_BYTES, "{} bytes", bytes);
        prop_assert!(body.titles.len() <= 80);
        // What a string weighs inside the JSON body.
        let weight = |s: &str| serde_json::to_string(s).unwrap().len() - 2;
        for (sent, m) in body.messages.iter().zip(&messages) {
            prop_assert!(m.text.starts_with(&sent.text));
            prop_assert!(sent.text.chars().count() <= assist::MAIL_TEXT_CHARS);
            let whole: String = m.text.chars().take(assist::MAIL_TEXT_CHARS).collect();
            prop_assert!(
                weight(&sent.text) >= weight(&whole).min(4 * 1024),
                "a long email crowded out another"
            );
        }
    }
}

#[test]
fn ascii_mail_goes_whole_up_to_6000_characters() {
    let mut m = message(1);
    m.text = "Problem set 7 is due Friday at 5 PM. ".repeat(500);
    let body = read_mail_body(
        std::slice::from_ref(&m),
        ts("2026-10-06T12:40:00Z"),
        &zone(),
        &[],
    );
    assert_eq!(body.messages[0].text.chars().count(), 6000);
    assert!(m.text.starts_with(&body.messages[0].text));
    let short = message(2);
    let body = read_mail_body(
        std::slice::from_ref(&short),
        ts("2026-10-06T12:40:00Z"),
        &zone(),
        &[],
    );
    assert_eq!(body.messages[0].text, short.text);
}

#[test]
fn mail_tasks_keep_only_future_deadlines() {
    let sent = [message(1)];
    let body = r#"{"tasks":[
        {"message":"18c2f001","title":"Problem set 7","due":"2026-10-09T17:00","course":"MTH 142","type":"Homework","notes":"Sections 4.1-4.3."},
        {"message":"18c2f001","title":"Reading quiz","due":"2026-10-12"},
        {"message":"18c2f001","title":"Last week's lab","due":"2026-10-01"},
        {"message":"18c2f001","title":"","due":"2026-10-12"},
        {"message":"18c2f001","title":"No date"}
    ]}"#;
    let tasks = parse_mail_tasks(body, &sent, ts("2026-10-06T12:40:00Z"), &zone()).unwrap();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].title, "Problem set 7");
    assert_eq!(tasks[0].due, ts("2026-10-09T21:00:00Z"));
    assert_eq!(tasks[0].course.as_deref(), Some("MTH 142"));
    assert_eq!(tasks[0].message_id.as_deref(), Some("18c2f001"));
    assert!(tasks[0].id.starts_with("em-"));
    assert_eq!(
        tasks[0].notes,
        "From mail: Announcement 1\n\nSections 4.1-4.3."
    );
    assert_eq!(tasks[1].notes, "From mail: Announcement 1");
    assert_eq!(
        tasks[1].due,
        ts("2026-10-13T03:59:00Z"),
        "a date alone is due 11:59 PM"
    );
    assert_ne!(tasks[0].id, tasks[1].id);
    assert!(parse_mail_tasks("{}", &sent, ts("2026-10-06T12:40:00Z"), &zone()).is_none());
}

#[test]
fn a_syllabus_draft_says_what_it_found() {
    let body = r#"{"categories":[
        {"name":"Homework","weight":20,"keywords":["homework","problem set"]},
        {"name":"Quizzes","weight":15,"keywords":["quiz"]},
        {"name":"Labs","weight":20,"keywords":["lab"]},
        {"name":"Midterm","weight":20,"keywords":["midterm"]},
        {"name":"Final","weight":25,"keywords":["final"]}
    ],"scale":[{"letter":"A","min":93},{"letter":"A-","min":90}]}"#;
    let draft = parse_syllabus(body).unwrap();
    assert_eq!(draft.categories.len(), 5);
    assert_eq!(draft.scale[1], ("A-".to_string(), 90.0));
    assert_eq!(
        draft.check_line(),
        "Claude found 5 categories adding to 100%. Check them against the syllabus."
    );
}

#[test]
fn a_review_draft_may_only_restate_numbers_heat_passed_in() {
    let facts = vec![
        "Tasks done: 14".to_string(),
        "Focus time in Classes: 6h 40m".to_string(),
        "Homework: estimated 1h 15m, took 1h 32m across 4".to_string(),
    ];
    let honest = r#"{"draft":"What moved: 14 tasks, 6h 40m of focus. What slipped: homework ran 1h 32m against 1h 15m. Next week's one thing: start problem sets early."}"#;
    assert!(check_review(honest, &facts).is_some());
    let invented = r#"{"draft":"What moved: 14 tasks, a 20% gain on last week."}"#;
    assert!(
        check_review(invented, &facts).is_none(),
        "20 is not a number Learn passed in"
    );
    // Numbers in words are held to the same rule, by value.
    for (draft, honest) in [
        ("Fourteen tasks done.", true),
        (
            "Homework ran fifty minutes over across four of them.",
            false,
        ),
        ("Forty minutes of focus on top of six hours.", true),
        ("Thirty-two minutes over on homework, across four.", true),
        ("Thirty two minutes over.", true),
        ("Thirty, two minutes over.", false),
        ("Fourteen hundred minutes of focus.", false),
        ("One thing to watch: homework.", true),
        ("Twenty-one tasks.", false),
        ("A dozen tasks.", false),
        ("Zero tasks slipped.", false),
    ] {
        let body = serde_json::to_string(&json!({ "draft": draft })).unwrap();
        assert_eq!(check_review(&body, &facts).is_some(), honest, "{draft}");
    }
    let big = vec!["Focus time: 1405 minutes, 2005 total".to_string()];
    for draft in [
        "One thousand four hundred and five minutes.",
        "Two thousand and five in all.",
    ] {
        let body = serde_json::to_string(&json!({ "draft": draft })).unwrap();
        assert!(check_review(&body, &big).is_some(), "{draft}");
    }
}

#[test]
fn a_release_plan_falls_back_to_three_empty_phases() {
    let body = r#"{"pre":["Finish the mix","Send to mastering"],"launch":["Publish to Space"],"post":["Write the devlog post"]}"#;
    let plan = parse_release_plan(body).unwrap();
    assert_eq!(plan.pre.len(), 2);
    assert_eq!(plan.launch, vec!["Publish to Space".to_string()]);

    let day = jiff::civil::date(2026, 12, 4);
    let phases = release_milestones(day);
    assert_eq!(phases[0], ("Pre-release", jiff::civil::date(2026, 11, 6)));
    assert_eq!(phases[1], ("Release day", day));
    assert_eq!(phases[2], ("Post-release", jiff::civil::date(2027, 1, 1)));
}

#[test]
fn drafting_bodies_send_what_they_name() {
    let v = serde_json::to_value(assist::SyllabusBody {
        text: "Homework 20%".into(),
    })
    .unwrap();
    assert_eq!(keys(&v), ["text"]);
    let v = serde_json::to_value(assist::ReviewBody {
        facts: vec!["Tasks done: 14".into()],
    })
    .unwrap();
    assert_eq!(keys(&v), ["facts"]);
    let plan = assist::ReleasePlanBody {
        title: "More Love".into(),
        release_date: "2026-12-04".into(),
        work: None,
    };
    assert_eq!(
        keys(&serde_json::to_value(&plan).unwrap()),
        ["title", "release_date"]
    );
    let linked = assist::ReleasePlanBody {
        work: Some("More Love (album)".into()),
        ..plan
    };
    assert_eq!(
        keys(&serde_json::to_value(&linked).unwrap()),
        ["title", "release_date", "work"]
    );
}
