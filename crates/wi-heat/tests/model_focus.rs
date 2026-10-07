//! `focus.test.ts`, case for case.

mod common;

use common::*;
use wi_heat::model::estimate::actual_min;
use wi_heat::model::focus::{
    check_off, finish_with_time, focus_lcd, focus_step, focus_strip, initial_focus,
    is_focus_length, set_took, CheckOff, FocusEffect, FocusEvent, FocusSettings, FocusState,
    FocusTarget, NewFocusSession, Phase, TargetKind, FOCUS_LENGTHS,
};
use wi_heat::model::records::{FocusSession, Room, Task};

fn t0() -> f64 {
    ny("2026-10-06 09:00")
}

fn target(kind: TargetKind, id: &str, title: &str, minutes: Option<f64>) -> FocusTarget {
    FocusTarget {
        kind,
        id: id.into(),
        title: title.into(),
        minutes,
    }
}

fn mix() -> FocusTarget {
    target(TargetKind::Task, "mix", "Mix the second verse", None)
}

fn press(target: Option<FocusTarget>) -> FocusEvent {
    FocusEvent::Press {
        target,
        room: Some(Room::Heat),
    }
}

fn step(state: &FocusState, event: &FocusEvent, at: f64) -> wi_heat::model::focus::Step {
    focus_step(state, event, at, FocusSettings::default())
}

// Runs events in order and gathers every effect.
fn run(
    events: &[(f64, FocusEvent)],
    state: FocusState,
    chime: bool,
) -> (FocusState, Vec<FocusEffect>) {
    let (mut state, mut effects) = (state, vec![]);
    for (at, e) in events {
        let r = focus_step(&state, e, *at, FocusSettings { chime });
        state = r.state;
        effects.extend(r.effects);
    }
    (state, effects)
}

fn logged(
    task_id: Option<&str>,
    habit_id: Option<&str>,
    started_at: f64,
    ended_at: f64,
    focus_min: f64,
) -> FocusEffect {
    FocusEffect::Log {
        session: NewFocusSession {
            task_id: task_id.map(str::to_string),
            habit_id: habit_id.map(str::to_string),
            started_at,
            ended_at,
            focus_min,
            interruptions: 0.0,
            room: Room::Heat,
        },
    }
}

// --- lengths ------------------------------------------------------------------------------------

#[test]
fn defaults_to_25_minutes_offers_50_and_takes_a_custom_10_to_90() {
    assert_eq!(initial_focus().focus_min, 25.0);
    assert_eq!(FOCUS_LENGTHS, [25.0, 50.0]);
    assert_eq!(
        [9.0, 10.0, 45.0, 90.0, 91.0, 12.5].map(is_focus_length),
        [false, true, true, true, false, false]
    );
    let set = |minutes: f64| {
        run(
            &[(t0(), FocusEvent::SetLength { minutes })],
            initial_focus(),
            false,
        )
        .0
    };
    assert_eq!(set(50.0).focus_min, 50.0);
    assert_eq!(set(95.0).focus_min, 25.0);
}

#[test]
fn keeps_a_round_in_progress_at_its_own_length() {
    let (state, _) = run(
        &[
            (t0(), press(Some(mix()))),
            (t0() + MIN, FocusEvent::SetLength { minutes: 50.0 }),
        ],
        initial_focus(),
        false,
    );
    assert_eq!(state.focus_min, 25.0);
}

#[test]
fn gives_a_5_minute_break_and_15_after_every_fourth_round() {
    let mut state = initial_focus();
    let mut breaks = vec![];
    let mut at = t0();
    for _ in 1..=5 {
        state = step(&state, &press(Some(mix())), at).state;
        at += 25.0 * MIN;
        state = step(&state, &FocusEvent::Tick, at).state;
        breaks.push(state.left_ms / MIN);
        state = step(&state, &press(None), at).state;
        at += state.left_ms;
        state = step(&state, &FocusEvent::Tick, at).state;
    }
    assert_eq!(breaks, [5.0, 5.0, 5.0, 15.0, 5.0]);
    assert_eq!(state.round, 2.0);
}

// --- a round, as 1.6 runs it ---------------------------------------------------------------------

#[test]
fn starts_on_f_reads_24_59_a_second_in_and_logs_25m_to_the_task_when_it_ends() {
    let mut s = step(&initial_focus(), &press(Some(mix())), t0()).state;
    let lcd = focus_lcd(&s, t0() + 1000.0);
    assert_eq!(lcd.digits, "24:59");
    assert_eq!(lcd.line, "Focus 1 of 4 · Mix the second verse");
    assert_eq!(lcd.note, None);
    assert_eq!(lcd.meter, (24.0 * MIN + 59_000.0) / (25.0 * MIN));
    assert!(!lcd.paused);
    let end = step(&s, &FocusEvent::Tick, t0() + 25.0 * MIN);
    s = end.state;
    assert_eq!(
        end.effects,
        vec![logged(Some("mix"), None, t0(), t0() + 25.0 * MIN, 25.0)]
    );
    let lcd = focus_lcd(&s, t0() + 25.0 * MIN);
    assert_eq!(lcd.digits, "5:00");
    assert_eq!(lcd.line, "Break 5:00. Press F to start it.");
    assert_eq!(
        lcd.note.as_deref(),
        Some("Focus done. 25m logged to Mix the second verse.")
    );
    assert_eq!(lcd.meter, 1.0);
    assert!(!lcd.paused);
}

#[test]
fn logs_to_the_end_of_the_round_even_when_the_tick_comes_late() {
    let s = step(&initial_focus(), &press(Some(mix())), t0()).state;
    let r = step(&s, &FocusEvent::Tick, t0() + 40.0 * MIN);
    match &r.effects[0] {
        FocusEffect::Log { session } => {
            assert_eq!(
                (session.ended_at, session.focus_min),
                (t0() + 25.0 * MIN, 25.0)
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn pauses_and_resumes_on_f_and_the_pause_is_not_focus_time() {
    let (state, effects) = run(
        &[
            (t0(), press(Some(mix()))),
            (t0() + 10.0 * MIN, press(None)),
            (t0() + 40.0 * MIN, press(None)),
            (t0() + 54.0 * MIN, FocusEvent::Tick),
            (t0() + 55.0 * MIN, FocusEvent::Tick),
        ],
        initial_focus(),
        false,
    );
    assert_eq!(effects.len(), 1);
    match &effects[0] {
        FocusEffect::Log { session } => assert_eq!(
            (session.focus_min, session.ended_at),
            (25.0, t0() + 55.0 * MIN)
        ),
        other => panic!("{other:?}"),
    }
    assert_eq!(state.phase, Phase::Break);
}

#[test]
fn stops_and_logs_on_shift_f_rounding_to_the_minute() {
    let (state, effects) = run(
        &[
            (t0(), press(Some(mix()))),
            (t0() + 18.0 * MIN + 40_000.0, FocusEvent::Stop),
        ],
        initial_focus(),
        false,
    );
    assert_eq!(
        effects,
        vec![logged(
            Some("mix"),
            None,
            t0(),
            t0() + 18.0 * MIN + 40_000.0,
            19.0
        )]
    );
    assert_eq!(state.phase, Phase::Idle);
    assert_eq!(state.round, 1.0);
    assert_eq!(
        focus_lcd(&state, t0() + 19.0 * MIN).note.as_deref(),
        Some("Focus stopped. 19m logged to Mix the second verse.")
    );
}

#[test]
fn logs_nothing_for_a_stop_under_half_a_minute() {
    let (_, effects) = run(
        &[
            (t0(), press(Some(mix()))),
            (t0() + 20_000.0, FocusEvent::Stop),
        ],
        initial_focus(),
        false,
    );
    assert_eq!(effects, vec![]);
}

#[test]
fn marks_pulled_away_with_i_it_pauses_and_records_an_interruption() {
    let (state, effects) = run(
        &[
            (t0(), press(Some(mix()))),
            (t0() + 5.0 * MIN, FocusEvent::PulledAway),
            (t0() + 6.0 * MIN, FocusEvent::PulledAway),
            (t0() + 9.0 * MIN, press(None)),
            (t0() + 10.0 * MIN, FocusEvent::PulledAway),
            (t0() + 12.0 * MIN, press(None)),
            (t0() + 32.0 * MIN, FocusEvent::Tick),
        ],
        initial_focus(),
        false,
    );
    assert_eq!(state.phase, Phase::Break);
    match &effects[0] {
        FocusEffect::Log { session } => {
            assert_eq!((session.interruptions, session.focus_min), (2.0, 25.0))
        }
        other => panic!("{other:?}"),
    }
}

// --- nothing starts without a press ---------------------------------------------------------------

#[test]
fn waits_after_a_round_and_after_a_break() {
    let (mut s, _) = run(
        &[
            (t0(), press(Some(mix()))),
            (t0() + 25.0 * MIN, FocusEvent::Tick),
        ],
        initial_focus(),
        false,
    );
    for m in 26..200 {
        s = step(&s, &FocusEvent::Tick, t0() + f64::from(m) * MIN).state;
    }
    assert_eq!((s.phase, s.running), (Phase::Break, false));
    s = step(&s, &press(None), t0() + 200.0 * MIN).state;
    s = step(&s, &FocusEvent::Tick, t0() + 205.0 * MIN).state;
    assert_eq!((s.phase, s.running, s.round), (Phase::Idle, false, 2.0));
    assert_eq!(
        focus_lcd(&s, t0() + 205.0 * MIN).note.as_deref(),
        Some("Break done. Press F to start focus 2 of 4.")
    );
    for m in 206..400 {
        s = step(&s, &FocusEvent::Tick, t0() + f64::from(m) * MIN).state;
    }
    assert_eq!((s.phase, s.running), (Phase::Idle, false));
}

#[test]
fn never_needs_a_target_to_stay_still_and_f_with_no_current_task_does_nothing() {
    assert_eq!(
        step(&initial_focus(), &press(None), t0()).state,
        initial_focus()
    );
}

// The generator the TypeScript tests use, in the arithmetic they use it: one number type.
struct Lcg(f64);

impl Lcg {
    fn random(&mut self) -> f64 {
        self.0 = (self.0 * 1_103_515_245.0 + 12_345.0) % 2_147_483_648.0;
        self.0 / 2_147_483_648.0
    }
}

#[test]
fn holds_over_thousands_of_random_events_only_f_ever_sets_the_timer_running() {
    let mut rng = Lcg(7.0);
    let kanji = target(TargetKind::Habit, "kanji", "Practise kanji", Some(20.0));
    let events = vec![
        press(Some(mix())),
        press(None),
        FocusEvent::Tick,
        FocusEvent::Tick,
        FocusEvent::Tick,
        FocusEvent::Stop,
        FocusEvent::PulledAway,
        FocusEvent::SetLength { minutes: 10.0 },
        FocusEvent::SetTarget {
            target: Some(kanji),
        },
        FocusEvent::SetTarget {
            target: Some(mix()),
        },
    ];
    let mut state = initial_focus();
    let mut at = t0();
    let mut starts = 0;
    for _ in 0..5000 {
        at += (rng.random() * 40.0 * MIN).floor();
        let e = &events[(rng.random() * events.len() as f64).floor() as usize];
        let next = step(&state, e, at).state;
        if !state.running && next.running {
            assert!(matches!(e, FocusEvent::Press { .. }));
            starts += 1;
        }
        state = next;
    }
    assert!(starts > 100, "{starts}");
}

// --- minutes go to the current task --------------------------------------------------------------------

#[test]
fn adds_a_finished_round_to_the_tasks_actual_min() {
    let t = task(|t| {
        t.id = "mix".into();
        t.adjust_min = 5.0;
    });
    let (_, effects) = run(
        &[
            (t0(), press(Some(mix()))),
            (t0() + 25.0 * MIN, FocusEvent::Tick),
        ],
        initial_focus(),
        false,
    );
    let logs: Vec<FocusSession> = effects
        .into_iter()
        .filter_map(|e| match e {
            FocusEffect::Log { session } => Some(session.with_id("s".into())),
            _ => None,
        })
        .collect();
    assert_eq!(actual_min(&t, &logs), 30.0);
}

#[test]
fn splits_a_round_when_the_current_task_changes_so_each_task_gets_its_own_minutes() {
    let quiz = target(TargetKind::Task, "quiz", "Grammar quiz 4", None);
    let (state, effects) = run(
        &[
            (t0(), press(Some(mix()))),
            (
                t0() + 10.0 * MIN,
                FocusEvent::SetTarget { target: Some(quiz) },
            ),
            (t0() + 25.0 * MIN, FocusEvent::Tick),
        ],
        initial_focus(),
        false,
    );
    assert_eq!(
        effects,
        vec![
            logged(Some("mix"), None, t0(), t0() + 10.0 * MIN, 10.0),
            logged(
                Some("quiz"),
                None,
                t0() + 10.0 * MIN,
                t0() + 25.0 * MIN,
                15.0
            ),
        ]
    );
    assert_eq!(
        focus_lcd(&state, t0() + 25.0 * MIN).note.as_deref(),
        Some("Focus done. 15m logged to Grammar quiz 4.")
    );
}

#[test]
fn says_only_focus_done_when_the_rounds_last_session_was_too_short_to_log() {
    let quiz = target(TargetKind::Task, "quiz", "Grammar quiz 4", None);
    let (state, effects) = run(
        &[
            (t0(), press(Some(mix()))),
            (
                t0() + 24.0 * MIN + 50_000.0,
                FocusEvent::SetTarget { target: Some(quiz) },
            ),
            (t0() + 25.0 * MIN, FocusEvent::Tick),
        ],
        initial_focus(),
        false,
    );
    assert_eq!(
        effects,
        vec![logged(
            Some("mix"),
            None,
            t0(),
            t0() + 24.0 * MIN + 50_000.0,
            25.0
        )]
    );
    assert_eq!(
        focus_lcd(&state, t0() + 25.0 * MIN).note.as_deref(),
        Some("Focus done.")
    );
}

#[test]
fn logs_a_habits_minutes_to_the_habit_and_ticks_it_once_the_session_reaches_its_length() {
    let kanji = target(TargetKind::Habit, "kanji", "Practise kanji", Some(20.0));
    let (state, effects) = run(
        &[
            (t0(), press(Some(kanji))),
            (t0() + 19.0 * MIN, FocusEvent::Tick),
        ],
        initial_focus(),
        false,
    );
    assert_eq!(effects, vec![]);
    let r2 = step(&state, &FocusEvent::Tick, t0() + 20.0 * MIN);
    assert_eq!(
        r2.effects,
        vec![FocusEffect::TickHabit {
            habit_id: "kanji".into()
        }]
    );
    let r3 = step(&r2.state, &FocusEvent::Tick, t0() + 25.0 * MIN);
    assert_eq!(
        r3.effects,
        vec![logged(None, Some("kanji"), t0(), t0() + 25.0 * MIN, 25.0)]
    );
}

// --- the chime ------------------------------------------------------------------------------------------

fn round_with_a_break() -> Vec<(f64, FocusEvent)> {
    vec![
        (t0(), press(Some(mix()))),
        (t0() + 25.0 * MIN, FocusEvent::Tick),
        (t0() + 26.0 * MIN, press(None)),
        (t0() + 31.0 * MIN, FocusEvent::Tick),
    ]
}

#[test]
fn the_chime_is_off_by_default_and_never_sounds_while_off() {
    let (_, effects) = run(&round_with_a_break(), initial_focus(), false);
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, FocusEffect::Chime))
            .count(),
        0
    );
}

#[test]
fn sounds_once_when_a_focus_round_ends_if_switched_on_and_never_to_end_a_break() {
    let (_, effects) = run(&round_with_a_break(), initial_focus(), true);
    let kinds: Vec<&str> = effects
        .iter()
        .map(|e| match e {
            FocusEffect::Log { .. } => "log",
            FocusEffect::TickHabit { .. } => "tickHabit",
            FocusEffect::Chime => "chime",
        })
        .collect();
    assert_eq!(kinds, ["log", "chime"]);
}

// --- the Now strip ---------------------------------------------------------------------------------------------

#[test]
fn reads_focus_18_40_left_during_focus_paused_or_not() {
    let s = step(&initial_focus(), &press(Some(mix())), t0()).state;
    assert_eq!(
        focus_strip(&s, t0() + 6.0 * MIN + 20_000.0).as_deref(),
        Some("focus 18:40 left")
    );
    let paused = step(&s, &press(None), t0() + 6.0 * MIN + 20_000.0).state;
    assert_eq!(
        focus_strip(&paused, t0() + 60.0 * MIN).as_deref(),
        Some("focus 18:40 left")
    );
    assert_eq!(focus_strip(&initial_focus(), t0()), None);
}

// --- checking a task off (3.5) ------------------------------------------------------------------------------------

#[test]
fn completes_at_once_when_it_has_logged_time_with_done_took_1h_15m_across_3_focus_sessions() {
    let now = ny("2026-10-06 15:00");
    let t = task(|_| {});
    let sessions: Vec<FocusSession> = [25.0, 25.0, 25.0]
        .iter()
        .map(|m| {
            session(|s| {
                s.task_id = Some(t.id.clone());
                s.focus_min = *m;
            })
        })
        .collect();
    let z = ny_zone();
    assert_eq!(
        check_off(&t, &sessions, &[], now, &z, &mut ids("id")),
        CheckOff::Done {
            task: Task {
                done: true,
                done_at: Some(now),
                ..t.clone()
            },
            message: "Done. Took 1h 15m across 3 focus sessions.".into(),
        }
    );
    match check_off(&t, &sessions[..1], &[], now, &z, &mut ids("id")) {
        CheckOff::Done { message, .. } => {
            assert_eq!(message, "Done. Took 25m across 1 focus session.")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn asks_time_it_took_only_when_nothing_is_logged() {
    let now = ny("2026-10-06 15:00");
    assert_eq!(
        check_off(&task(|_| {}), &[], &[], now, &ny_zone(), &mut ids("id")),
        CheckOff::Ask {
            title: "Time it took".into(),
            hint: "This trains your time averages for this type of task.".into()
        }
    );
}

#[test]
fn takes_the_answer_as_the_time_and_get_infos_took_adjusts_it_by_hand() {
    let now = ny("2026-10-06 15:00");
    let t = task(|_| {});
    let done = finish_with_time(&t, 40.0, now);
    assert_eq!(
        done,
        Task {
            adjust_min: 40.0,
            done: true,
            done_at: Some(now),
            ..t.clone()
        }
    );
    let sessions = vec![session(|s| {
        s.task_id = Some(t.id.clone());
        s.focus_min = 50.0;
    })];
    let took = set_took(&t, &sessions, 75.0);
    assert_eq!(took.adjust_min, 25.0);
    assert_eq!(actual_min(&took, &sessions), 75.0);
}

// --- a round's minutes, however it is split ----------------------------------------------------------------------------

// A seeded walk through random presses, target changes, pulls away, stops
// and ticks. Whenever a focus round ends, the minutes it logged add up to
// its focus time rounded to the minute, whatever parts it was split into.
#[test]
fn add_up_to_the_rounds_focus_time_rounded_across_2_000_random_rounds() {
    let mut rng = Lcg(7.0);
    let targets: Vec<FocusTarget> = ["a", "b", "c"]
        .iter()
        .map(|id| target(TargetKind::Task, id, id, None))
        .collect();
    let pick = |rng: &mut Lcg, n: usize| (rng.random() * n as f64).floor() as usize;
    let mut s = initial_focus();
    let mut now = t0();
    let (mut focus_ms, mut logged_min, mut rounds) = (0.0, 0.0, 0);
    let mut misses: Vec<String> = vec![];
    while rounds < 2_000 {
        let at = now + (rng.random() * 9.0 * MIN).floor();
        if s.phase == Phase::Focus && s.running {
            if let Some(ends_at) = s.ends_at {
                focus_ms += at.min(ends_at) - now;
            }
        }
        // The array is built before it is picked from, so its two target picks come first.
        let press_target = targets[pick(&mut rng, targets.len())].clone();
        let set_target = targets[pick(&mut rng, targets.len())].clone();
        let choices = [
            press(Some(press_target)),
            press(None),
            FocusEvent::SetTarget {
                target: Some(set_target),
            },
            FocusEvent::PulledAway,
            FocusEvent::Stop,
            FocusEvent::Tick,
            FocusEvent::Tick,
        ];
        let event = &choices[pick(&mut rng, choices.len())];
        let was_focus = s.phase == Phase::Focus;
        let r = step(&s, event, at);
        for e in &r.effects {
            if let FocusEffect::Log { session } = e {
                logged_min += session.focus_min;
            }
        }
        if was_focus && r.state.phase != Phase::Focus {
            if logged_min != wi_heat::model::js::round(focus_ms / MIN) {
                misses.push(format!("{logged_min} logged for {focus_ms} ms"));
            }
            rounds += 1;
            focus_ms = 0.0;
            logged_min = 0.0;
        }
        s = r.state;
        now = at;
    }
    assert_eq!(misses, Vec::<String>::new());
}
