//! `review.findings.test.ts`, case for case: the adversarial review of the
//! first build of the model. Each test states what the spec asks for and
//! failed against the model as first built; every finding is fixed, so none is
//! ignored.

mod common;

use common::*;
use serde_json::{json, Value};
use wi_heat::model::estimate::{estimate_context, estimate_min, weekly_load, Load};
use wi_heat::model::focus::{
    check_off, focus_step, initial_focus, CheckOff, FocusEffect, FocusEvent, FocusSettings,
    FocusState, FocusTarget, TargetKind,
};
use wi_heat::model::grades::{
    current_pct, default_scale, letter_for, letter_for_default, what_it_would_take,
};
use wi_heat::model::habits::mark_habit_done;
use wi_heat::model::heat::by_heat;
use wi_heat::model::import_artifact::{
    import_artifact, parse_heat_export, Existing, ExportError, HeatExport,
};
use wi_heat::model::js;
use wi_heat::model::plan::{plan_my_day, plan_next, BlockTarget, PlanData, PlanOptions};
use wi_heat::model::records::{Course, Grade, GradeCategory, GradeSource, LetterStep, Room, Task};
use wi_heat::model::recurrence::{open_tasks, recurs, set_repeat};
use wi_heat::model::spaces::default_spaces;

fn cat(id: &str, weight: f64) -> GradeCategory {
    GradeCategory {
        id: id.into(),
        name: id.into(),
        weight,
        keywords: vec![],
        drop_lowest: None,
    }
}

fn course(categories: Vec<GradeCategory>) -> Course {
    Course {
        id: "c".into(),
        term_id: "t".into(),
        code: "JPN 201".into(),
        name: "Intermediate Japanese".into(),
        categories,
        scale: None,
        notes: String::new(),
        ..Default::default()
    }
}

fn grade(category_id: &str, score: f64, out_of: f64) -> Grade {
    Grade {
        id: format!("{category_id}-{score}-{out_of}"),
        course_id: "c".into(),
        category_id: Some(category_id.into()),
        title: category_id.into(),
        score: Some(score),
        out_of,
        dropped: false,
        pending: false,
        link: None,
        source: GradeSource::You,
        ..Default::default()
    }
}

// --- S2.1: grade letters at exact edges (3.1) ---------------------------------------------------------

// Finding: currentPct sums weight × category % in floating point, so a course
// that stands at exactly 60% (0.2 × 68 + 0.8 × 58) comes out as
// 59.99999999999999 and letterFor gives F instead of D. Same for 67% → D.
#[test]
fn gives_d_at_exactly_60_and_d_plus_at_exactly_67() {
    let sixty = course(vec![cat("hw", 20.0), cat("exams", 80.0)]);
    assert_eq!(
        letter_for_default(
            current_pct(
                &sixty,
                &[grade("hw", 68.0, 100.0), grade("exams", 58.0, 100.0)]
            )
            .unwrap()
        ),
        "D"
    );
    let sixty_seven = course(vec![cat("hw", 25.0), cat("exams", 75.0)]);
    assert_eq!(
        letter_for_default(
            current_pct(
                &sixty_seven,
                &[grade("hw", 94.0, 100.0), grade("exams", 58.0, 100.0)]
            )
            .unwrap()
        ),
        "D+"
    );
}

// --- S2.1: what it would take, when 100% on the rest is exactly enough (3.8) ---------------------------

// Finding: whatItWouldTake tests need > 100 on a floating-point need, so a
// target reachable with exactly 100% is called out of reach, and the line
// contradicts itself: "A B- is out of reach; the highest possible is 80% (B-)."
#[test]
fn says_you_need_100_not_that_the_letter_is_out_of_reach() {
    // 2/3 on a 60% category: 0.6 × 66.67 + 40 = 80 exactly.
    let c = course(vec![cat("hw", 60.0), cat("final", 40.0)]);
    assert_eq!(
        what_it_would_take(&c, &[grade("hw", 2.0, 3.0)], "B-").as_deref(),
        Some("To finish with a B- (80%), you need 100% on the remaining 40%.")
    );
    // 50% on a 34% category: 17 + 66 = 83 exactly.
    let d = course(vec![cat("mid", 34.0), cat("final", 66.0)]);
    assert_eq!(
        what_it_would_take(&d, &[grade("mid", 50.0, 100.0)], "B").as_deref(),
        Some("To finish with a B (83%), you need 100% on the remaining 66%.")
    );
}

// --- 3.6: a recurring task never vanishes -------------------------------------------------------------

// Finding: openTasks drops a task whose rule is readable but which has no due
// date and no scheduled date (seriesStart is null, so there is no next
// occurrence). It then shows in no list, count, plan or strip, and not in Done
// either. The PKM sets scheduledDate to today in this case.
#[test]
fn keeps_every_weekday_with_no_dates_in_the_open_tasks() {
    let t = task(|t| {
        t.title = "Practise scales".into();
        t.rrule = Some("FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR".into());
        t.due = None;
    });
    let open = open_tasks(
        std::slice::from_ref(&t),
        &[],
        ny("2026-10-06 08:40"),
        &ny_zone(),
    );
    assert_eq!(
        open.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
        [t.id.as_str()]
    );
}

// --- 3.6: the series never flips to done ------------------------------------------------------------------

// Finding: checkOff completes any task with logged time by returning it with
// done: true, recurring or not. Checking a recurring task that has focus
// sessions therefore ends the whole series instead of writing a
// TaskOccurrence row (checkOccurrence).
#[test]
fn does_not_mark_a_recurring_task_done_when_it_is_checked_off() {
    let t = task(|t| {
        t.rrule = Some("FREQ=DAILY".into());
        t.due = Some(ny("2026-10-06 17:00"));
    });
    let now = ny("2026-10-06 18:00");
    let sessions = vec![session(|s| s.task_id = Some(t.id.clone()))];
    let result = check_off(&t, &sessions, &[], now, &ny_zone(), &mut ids("id"));
    assert!(!matches!(&result, CheckOff::Done { task, .. } if task.done));
    assert_eq!(
        result,
        CheckOff::Occurrence {
            occurrence: wi_heat::model::records::TaskOccurrence {
                id: "id-1".into(),
                task_id: t.id.clone(),
                date: "2026-10-06".into(),
                done_at: now
            },
            message: Some("Done. Took 25m across 1 focus session.".into()),
        }
    );
}

// --- S2.2: drafts and P land in the time column (3.5) -----------------------------------------------------

// Finding: planMyDay and planNext start from now with no floor at the column's
// 7 AM, so planning after midnight drafts blocks at 1:15 AM, which the 7 AM to
// midnight column can't show or drag.
#[test]
fn never_places_a_block_before_7_am() {
    let now = ny("2026-10-06 01:10");
    let t = task(|t| {
        t.est_min = Some(45.0);
        t.due = Some(ny("2026-10-07 23:59"));
    });
    let data = PlanData {
        tasks: vec![t.clone()],
        ..PlanData::default()
    };
    for d in plan_my_day(&data, now, &ny_zone(), &PlanOptions::default()) {
        assert!(d.start >= 7.0 * 60.0);
    }
    let p = plan_next(
        &BlockTarget::Task {
            task_id: t.id.clone(),
        },
        &data,
        now,
        &ny_zone(),
        &mut ids("id"),
    );
    assert!(p.map_or(true, |p| p.start >= 7.0 * 60.0));
}

// --- S2.3: minutes go to the current task (3.5) ----------------------------------------------------------------

fn task_target(id: &str, title: &str) -> FocusTarget {
    FocusTarget {
        kind: TargetKind::Task,
        id: id.into(),
        title: title.into(),
        minutes: None,
    }
}

// Finding: closeSession rounds each part of a split round to the minute on its
// own, so a 25-minute round split at 12:30 logs 13 + 13 = 26 minutes to
// actualMin (and three parts of 8:20 log 24).
#[test]
fn logs_exactly_the_rounds_25_minutes_when_the_current_task_changes_halfway() {
    let t0 = ny("2026-10-06 09:00");
    let mut s: FocusState = initial_focus();
    let mut effects: Vec<FocusEffect> = vec![];
    let mut step = |e: FocusEvent, at: f64| {
        let r = focus_step(&s, &e, at, FocusSettings::default());
        s = r.state;
        effects.extend(r.effects);
    };
    step(
        FocusEvent::Press {
            target: Some(task_target("a", "Grammar quiz 4")),
            room: None,
        },
        t0,
    );
    step(
        FocusEvent::SetTarget {
            target: Some(task_target("b", "Mix the second verse")),
        },
        t0 + 12.5 * MIN,
    );
    step(FocusEvent::Tick, t0 + 25.0 * MIN);
    let logged: f64 = effects
        .iter()
        .map(|e| match e {
            FocusEffect::Log { session } => session.focus_min,
            _ => 0.0,
        })
        .sum();
    assert_eq!(logged, 25.0);
}

// --- 3.15: moving in refuses a malformed export instead of half-reading it -------------------------------------

// Finding: parseHeatExport checks only that the lists exist. A row that is
// null throws a TypeError out of the parser, and a due that isn't a date is
// accepted and imported as NaN. A NaN due makes heatOf's v NaN, and byHeat's
// comparator then misorders every other task (a task due in 2 hours sorts
// after one due tomorrow), so Plan my day drafts in the wrong order.
fn base() -> Value {
    json!({
        "format": "heat-export",
        "version": 1,
        "exportedAt": "2026-10-06T12:40:00.000Z",
        "timeZone": "America/New_York",
        "workspaces": [{"key": "classes", "name": "Classes", "groupLabel": "Course", "types": ["Quiz"], "persona": ""}],
        "tasks": [],
        "milestones": [],
        "habits": [],
        "term": "Fall 2026",
        "courses": [],
        "grades": [],
        "processedMailIds": [],
        "lastSyncAt": null,
    })
}

fn with(base: Value, over: Value) -> Value {
    let mut out = base;
    for (k, v) in over.as_object().expect("an object").iter() {
        out[k] = v.clone();
    }
    out
}

fn row(due: &str) -> Value {
    json!({
        "id": "em-1", "workspace": "classes", "title": "Grammar quiz 4", "group": null, "type": "Quiz",
        "due": due, "difficulty": 2, "estMin": null, "actualMin": null, "notes": "", "done": false,
        "doneAt": null, "source": "calendar",
    })
}

fn not_export() -> Result<HeatExport, ExportError> {
    Err(ExportError {
        error: "This file isn’t a Learn export.".into(),
    })
}

#[test]
fn returns_an_error_for_a_null_row_rather_than_throwing() {
    assert!(parse_heat_export(&with(base(), json!({"tasks": [null]}))).is_err());
}

#[test]
fn never_imports_a_due_that_is_not_a_number() {
    let Ok(parsed) = parse_heat_export(&with(base(), json!({"tasks": [row("next Wednesday")]})))
    else {
        return;
    };
    let due = import_artifact(&parsed, &Existing::default(), &mut ids("id")).tasks[0].due;
    assert!(due.map_or(true, f64::is_finite));
}

#[test]
fn keeps_the_other_tasks_in_heat_order_when_one_due_is_nan() {
    let now = ny("2026-10-06 08:00");
    let soon = task(|t| t.due = Some(ny("2026-10-06 10:00")));
    let tomorrow = task(|t| t.due = Some(ny("2026-10-07 10:00")));
    let broken = task(|t| t.due = Some(f64::NAN));
    let items = vec![tomorrow.clone(), broken, soon.clone()];
    let order: Vec<&str> = by_heat(&items, now).iter().map(|t| t.id.as_str()).collect();
    let at = |id: &str| order.iter().position(|x| *x == id).unwrap();
    assert!(at(&soon.id) < at(&tomorrow.id));
}

// --- S2.1: every letter edge, and no sentence that contradicts itself (3.1, 3.8) ---------------------

// Every two-category course (weights w and 100 - w, whole scores out of 100)
// that stands exactly on a threshold gets that threshold's letter.
#[test]
fn gives_the_thresholds_letter_to_every_course_that_stands_exactly_on_it() {
    let mut misses: Vec<String> = vec![];
    for step in default_scale().iter().filter(|x| x.min > 0.0) {
        for w in 1..100 {
            let w = f64::from(w);
            let c = course(vec![cat("a", w), cat("b", 100.0 - w)]);
            for a in 0..=100 {
                let a = f64::from(a);
                let rest = 100.0 * step.min - w * a;
                if rest % (100.0 - w) != 0.0 {
                    continue;
                }
                let b = rest / (100.0 - w);
                if !(0.0..=100.0).contains(&b) {
                    continue;
                }
                let pct = current_pct(&c, &[grade("a", a, 100.0), grade("b", b, 100.0)]).unwrap();
                let letter = letter_for_default(pct);
                if letter != step.letter {
                    misses.push(format!("{w}% at {a}, {}% at {b}: {letter}", 100.0 - w));
                }
            }
        }
    }
    assert_eq!(misses, Vec::<String>::new());
}

/// `/you need ([\d.]+)%/`: the number the sentence asks for.
fn need_in(line: &str) -> Option<f64> {
    let rest = &line[line.find("you need ")? + "you need ".len()..];
    let digits: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!digits.is_empty() && rest[digits.len()..].starts_with('%')).then(|| js::to_number(&digits))
}

/// `/\((\S+)\)\.$/`: the letter in the closing brackets, if the sentence ends in one.
fn best_in(line: &str) -> Option<&str> {
    let rest = line.strip_suffix(").")?;
    let open = rest.rfind('(')?;
    let inside = &rest[open + 1..];
    (!inside.is_empty() && !inside.contains(char::is_whitespace)).then_some(inside)
}

// Over ordinary weights and scores, "what it would take" never asks for more
// than 100%, and never names the target as the best possible.
#[test]
fn never_asks_for_more_than_100_and_never_calls_the_target_its_own_best() {
    let mut bad: Vec<String> = vec![];
    let mut pairs: Vec<(f64, f64)> = vec![
        (0.0, 1.0),
        (1.0, 3.0),
        (2.0, 3.0),
        (1.0, 6.0),
        (5.0, 6.0),
        (1.0, 7.0),
    ];
    pairs.extend((0..=100).map(|i| (f64::from(i), 100.0)));
    for w in 5..100 {
        let w = f64::from(w);
        let c = course(vec![cat("done", w), cat("left", 100.0 - w)]);
        for (score, out_of) in &pairs {
            for step in default_scale() {
                let line = what_it_would_take(&c, &[grade("done", *score, *out_of)], &step.letter)
                    .unwrap();
                if need_in(&line).is_some_and(|n| n > 100.0) {
                    bad.push(line.clone());
                }
                if line.contains("out of reach") && best_in(&line) == Some(step.letter.as_str()) {
                    bad.push(line);
                }
            }
        }
    }
    assert_eq!(bad, Vec::<String>::new());
}

#[test]
fn still_reads_a_course_with_no_categories_yet_as_all_to_play_for() {
    assert_eq!(
        what_it_would_take(&course(vec![]), &[], "B").as_deref(),
        Some("To finish with a B (83%), you need 83% on the remaining 100%.")
    );
}

// Finding: letterFor fell back to the scale's last step, so a course scale
// without a 0 floor gave its lowest letter to every score below it.
#[test]
fn gives_f_below_a_course_scale_that_has_no_0_floor() {
    let scale: Vec<LetterStep> = [("A", 90.0), ("B", 80.0), ("C", 70.0), ("D", 60.0)]
        .iter()
        .map(|(letter, min)| LetterStep {
            letter: (*letter).into(),
            min: *min,
        })
        .collect();
    assert_eq!(letter_for(40.0, &scale), "F");
    assert_eq!(letter_for(60.0, &scale), "D");
}

// --- 3.6: a rule is pinned to a day when it is set, as in the PKM ------------------------------------------------

// The PKM's task panel gives a recurring task with no dates today as its
// DTSTART. setRepeat does the same, so "Every weekday" really repeats.
#[test]
fn pins_today_as_the_scheduled_date_when_a_rule_is_set_on_an_undated_task() {
    let now = ny("2026-10-06 08:40");
    let z = ny_zone();
    let t = set_repeat(
        &task(|t| t.title = "Practise scales".into()),
        Some("FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR"),
        now,
        &z,
    );
    assert_eq!(t.scheduled_date.as_deref(), Some("2026-10-06"));
    assert!(recurs(&t));
    let open = open_tasks(std::slice::from_ref(&t), &[], now, &z);
    assert_eq!(
        open.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
        [t.id.as_str()]
    );
    // A task with a due date keeps its dates; clearing the rule removes it.
    let dated = task(|t| t.due = Some(ny("2026-10-07 23:59")));
    assert_eq!(
        set_repeat(&dated, Some("FREQ=DAILY"), now, &z),
        Task {
            rrule: Some("FREQ=DAILY".into()),
            ..dated.clone()
        }
    );
    assert_eq!(set_repeat(&t, None, now, &z).rrule, None);
}

#[test]
fn treats_a_rule_with_nothing_to_start_from_as_a_plain_task_never_a_vanished_one() {
    let t = task(|t| t.rrule = Some("FREQ=DAILY".into()));
    assert!(!recurs(&t));
    let now = ny("2026-10-06 08:40");
    let sessions = vec![session(|s| s.task_id = Some(t.id.clone()))];
    assert!(matches!(
        check_off(&t, &sessions, &[], now, &ny_zone(), &mut ids("id")),
        CheckOff::Done { .. }
    ));
}

// --- 3.6: a checked occurrence says what it took since the last one -------------------------------------------------

#[test]
fn counts_only_the_focus_sessions_since_the_previous_tick() {
    let t = task(|t| {
        t.rrule = Some("FREQ=DAILY".into());
        t.due = Some(ny("2026-10-05 17:00"));
    });
    let monday = wi_heat::model::records::TaskOccurrence {
        id: "o-1".into(),
        task_id: t.id.clone(),
        date: "2026-10-05".into(),
        done_at: ny("2026-10-05 18:00"),
    };
    let sessions = vec![
        session(|s| {
            s.task_id = Some(t.id.clone());
            s.focus_min = 30.0;
            s.ended_at = ny("2026-10-05 17:30");
        }),
        session(|s| {
            s.task_id = Some(t.id.clone());
            s.focus_min = 20.0;
            s.ended_at = ny("2026-10-06 09:20");
        }),
    ];
    let now = ny("2026-10-06 10:00");
    let z = ny_zone();
    assert_eq!(
        check_off(
            &t,
            &sessions,
            std::slice::from_ref(&monday),
            now,
            &z,
            &mut ids("id")
        ),
        CheckOff::Occurrence {
            occurrence: wi_heat::model::records::TaskOccurrence {
                id: "id-1".into(),
                task_id: t.id.clone(),
                date: "2026-10-06".into(),
                done_at: now
            },
            message: Some("Done. Took 20m across 1 focus session.".into()),
        }
    );
    // Nothing logged since: it is ticked without asking, since an occurrence has no time of its own.
    assert!(matches!(
        check_off(
            &t,
            &sessions[..1],
            std::slice::from_ref(&monday),
            now,
            &z,
            &mut ids("id")
        ),
        CheckOff::Occurrence { message: None, .. }
    ));
}

// --- S2.3: three parts of one round ------------------------------------------------------------------------------------

#[test]
fn log_25_minutes_between_them_not_24() {
    let t0 = ny("2026-10-06 09:00");
    let mut s: FocusState = initial_focus();
    let mut logged: Vec<f64> = vec![];
    let mut step = |e: FocusEvent, at: f64| {
        let r = focus_step(&s, &e, at, FocusSettings::default());
        s = r.state;
        for effect in r.effects {
            if let FocusEffect::Log { session } = effect {
                logged.push(session.focus_min);
            }
        }
    };
    let third = (25.0 * MIN) / 3.0;
    step(
        FocusEvent::Press {
            target: Some(task_target("a", "A")),
            room: Some(Room::Heat),
        },
        t0,
    );
    step(
        FocusEvent::SetTarget {
            target: Some(task_target("b", "B")),
        },
        t0 + third,
    );
    step(
        FocusEvent::SetTarget {
            target: Some(task_target("c", "C")),
        },
        t0 + 2.0 * third,
    );
    step(FocusEvent::Tick, t0 + 25.0 * MIN);
    assert_eq!(logged, [8.0, 9.0, 8.0]);
}

// --- 3.15: moving in twice defines each course once (3.4) --------------------------------------------------------------

// Finding: each import minted a new term and new courses, so a second import
// (a fresher download) made a second "JPN 201" and "Fall 2026".
fn twice() -> HeatExport {
    let tasks = json!([{
        "id": "em-1", "workspace": "classes", "title": "Grammar quiz 4", "group": "JPN 201", "type": "Quiz",
        "due": "2026-10-08T03:59:00.000Z", "difficulty": 2, "estMin": null, "actualMin": null, "notes": "",
        "done": false, "doneAt": null, "source": "calendar",
    }]);
    let courses = json!([{
        "code": "JPN 201", "name": "Intermediate Japanese",
        "categories": [{"name": "Quizzes", "weight": 40, "keywords": []}],
    }]);
    let grades = json!([{
        "id": "gr-1", "course": "JPN 201", "title": "Kanji quiz 6", "category": "Quizzes", "score": 18,
        "outOf": 20, "dropped": false, "pending": false,
    }]);
    parse_heat_export(&with(
        base(),
        json!({"tasks": tasks, "courses": courses, "grades": grades}),
    ))
    .expect("a Learn export")
}

#[test]
fn finds_the_term_and_the_course_already_there_and_their_categories() {
    let first = import_artifact(
        &twice(),
        &Existing {
            spaces: Some(default_spaces(&mut ids("space"))),
            ..Existing::default()
        },
        &mut ids("a"),
    );
    let second = import_artifact(
        &twice(),
        &Existing {
            spaces: Some(default_spaces(&mut ids("space"))),
            terms: Some(first.terms.clone()),
            courses: Some(first.courses.clone()),
        },
        &mut ids("b"),
    );
    assert_eq!(second.terms, vec![]);
    assert_eq!(second.courses, vec![]);
    assert_eq!(second.tasks[0].course_id, Some(first.courses[0].id.clone()));
    assert_eq!(second.grades[0].course_id, first.courses[0].id);
    assert_eq!(
        second.grades[0].category_id,
        Some(first.courses[0].categories[0].id.clone())
    );
}

// --- 3.15: the export's fields are checked, not only its lists -----------------------------------------------------------

#[test]
fn reads_version_1_only_as_the_integer_1() {
    assert_eq!(
        serde_json::to_value(parse_heat_export(&base()).unwrap()).unwrap(),
        base()
    );
    assert_eq!(
        parse_heat_export(&with(base(), json!({"version": 0.5}))),
        not_export()
    );
    assert_eq!(
        parse_heat_export(&with(base(), json!({"version": "1"}))),
        not_export()
    );
}

#[test]
fn refuses_rows_whose_fields_have_the_wrong_type() {
    let ok = row("2026-10-06T12:40:00.000Z");
    assert!(parse_heat_export(&with(base(), json!({"tasks": [row_with(&ok, json!({}))]}))).is_ok());
    for bad in [
        json!({"difficulty": "hard"}),
        json!({"difficulty": 9}),
        json!({"source": "fax"}),
        json!({"id": 7}),
        json!({"done": "no"}),
        json!({"doneAt": "yesterday"}),
        json!({"estMin": -5}),
    ] {
        assert_eq!(
            parse_heat_export(&with(base(), json!({"tasks": [row_with(&ok, bad)]}))),
            not_export()
        );
    }
    assert_eq!(
        parse_heat_export(&with(base(), json!({"milestones": [null]}))),
        not_export()
    );
    assert_eq!(
        parse_heat_export(&with(
            base(),
            json!({"habits": [{"id": "h", "title": "Kanji", "log": {"yesterday": true}}]})
        )),
        not_export()
    );
    assert_eq!(
        parse_heat_export(&with(
            base(),
            json!({"grades": [{
                "id": "g", "course": "X", "title": "Quiz", "category": null, "score": "A", "outOf": 10,
                "dropped": false, "pending": false,
            }]})
        )),
        not_export()
    );
    assert_eq!(
        parse_heat_export(&with(
            base(),
            json!({"courses": [{"code": "X", "name": "X", "categories": [{"name": "Q", "weight": "forty"}]}]})
        )),
        not_export()
    );
    assert_eq!(
        parse_heat_export(&with(base(), json!({"processedMailIds": [42]}))),
        not_export()
    );
    assert_eq!(
        parse_heat_export(&with(base(), json!({"lastSyncAt": "soon"}))),
        not_export()
    );
}

fn row_with(row: &Value, over: Value) -> Value {
    with(row.clone(), over)
}

// --- parent links that loop ---------------------------------------------------------------------------------------------------

// Finding: estimateMin recursed through children and weeklyLoad walked
// parentTaskId with no guard, so a task that is its own parent, or A → B → A,
// overflowed the stack or looped for ever.
#[test]
fn estimates_a_task_that_is_its_own_parent_from_itself() {
    let t = task(|t| {
        t.id = "self".into();
        t.parent_task_id = Some("self".into());
        t.difficulty = 2.0;
    });
    assert_eq!(
        estimate_min(&t, &estimate_context(std::slice::from_ref(&t), &[])),
        40.0
    );
}

#[test]
fn ends_the_weekly_load_when_a_counted_task_sits_under_a_loop() {
    let now = ny("2026-10-06 08:00");
    let linked = |id: &str, parent: &str, est: f64, due: Option<f64>| {
        task(|t| {
            t.id = id.into();
            t.parent_task_id = Some(parent.into());
            t.est_min = Some(est);
            t.due = due;
        })
    };
    let c = linked("c", "x", 15.0, Some(ny("2026-10-08 23:59")));
    let x = linked("x", "y", 45.0, None);
    let y = linked("y", "x", 30.0, None);
    let tasks = vec![c, x, y];
    assert_eq!(
        weekly_load(&tasks, &estimate_context(&tasks, &[]), now),
        Load {
            minutes: 15.0,
            count: 1.0
        }
    );
}

#[test]
fn counts_a_loops_counted_task_once_with_its_open_children() {
    let now = ny("2026-10-06 08:00");
    let linked = |id: &str, parent: &str, est: f64, due: Option<f64>| {
        task(|t| {
            t.id = id.into();
            t.parent_task_id = Some(parent.into());
            t.est_min = Some(est);
            t.due = due;
        })
    };
    let a = linked("a", "b", 30.0, Some(ny("2026-10-07 23:59")));
    let b = linked("b", "a", 45.0, None);
    let c = linked("c", "a", 15.0, Some(ny("2026-10-08 23:59")));
    let tasks = vec![a, b, c];
    // a is counted; its children are b (45, its own child a already counted) and c (15).
    assert_eq!(
        weekly_load(&tasks, &estimate_context(&tasks, &[]), now),
        Load {
            minutes: 60.0,
            count: 1.0
        }
    );
}

// --- a habit ticked by a focus session (3.9) ---------------------------------------------------------------------------------------

// Finding: focus.ts emits tickHabit, but habits.ts offered only a toggle,
// which would untick a habit already done by hand today.
#[test]
fn stays_done_when_it_was_already_done_today() {
    let h = habit(|h| {
        h.log.insert("2026-10-06".into(), true);
    });
    assert_eq!(mark_habit_done(&h, "2026-10-06").log, h.log);
    assert_eq!(mark_habit_done(&habit(|_| {}), "2026-10-06").log, h.log);
}
