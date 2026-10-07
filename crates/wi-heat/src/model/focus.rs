//! The Pomodoro timer (`docs/SPEC.md` 3.5), as a state machine. It moves only
//! on explicit events, each stamped by the caller's clock, and returns the
//! effects for the caller to carry out: a focus session to save, a habit to
//! tick, the chime. Nothing starts without a press: only F sets it running.
//!
//! F starts or pauses. ⇧F stops and logs. I marks "Pulled away": it pauses
//! and records an interruption. Focus is 25 minutes, 50, or a custom 10–90;
//! breaks are 5 minutes and every fourth is 15. The state lives outside any
//! view, so switching views doesn't touch it (2.3). A port of `focus.ts`.

use super::estimate::{actual_min, format_minutes};
use super::records::{ser, FocusSession, FocusSource, Id, Room, Task, TaskOccurrence};
use super::recurrence::{check_occurrence, recurs};
use super::{copy, format, js};
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};

const MIN: f64 = 60_000.0;
const ROUNDS: f64 = 4.0;
pub const FOCUS_LENGTHS: [f64; 2] = [25.0, 50.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Task,
    Habit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FocusTarget {
    pub kind: TargetKind,
    pub id: Id,
    pub title: String,
    /// A habit's length: it ticks itself once a session on it reaches this.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser::opt_num"
    )]
    pub minutes: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Idle,
    Focus,
    Break,
}

/// The session in progress: the part of the round spent on one target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInProgress {
    #[serde(serialize_with = "ser::num")]
    pub started_at: f64,
    #[serde(serialize_with = "ser::num")]
    pub from_left_ms: f64,
    #[serde(serialize_with = "ser::num")]
    pub interruptions: f64,
    pub habit_ticked: bool,
}

/// The round's parts closed so far: their focus time and the minutes logged
/// for it. Each part logs the round's rounded total so far less what is
/// already logged, so a split round's parts add up to the round.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Closed {
    #[serde(serialize_with = "ser::num")]
    pub ms: f64,
    #[serde(serialize_with = "ser::num")]
    pub minutes: f64,
}

const NOTHING_CLOSED: Closed = Closed {
    ms: 0.0,
    minutes: 0.0,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusState {
    pub phase: Phase,
    pub running: bool,
    /// The focus round, 1 to 4. During a break, the round just finished.
    #[serde(serialize_with = "ser::num")]
    pub round: f64,
    #[serde(serialize_with = "ser::num")]
    pub focus_min: f64,
    /// The whole length of the current focus or break.
    #[serde(serialize_with = "ser::num")]
    pub length_ms: f64,
    /// While running.
    #[serde(default, serialize_with = "ser::opt_num")]
    pub ends_at: Option<f64>,
    /// While paused or waiting for a press.
    #[serde(serialize_with = "ser::num")]
    pub left_ms: f64,
    #[serde(default)]
    pub target: Option<FocusTarget>,
    #[serde(default)]
    pub session: Option<SessionInProgress>,
    pub closed: Closed,
    pub room: Room,
    /// What just ended ("Focus done. 25m logged to …"), until the next press.
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FocusEvent {
    Press {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<FocusTarget>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        room: Option<Room>,
    },
    Stop,
    PulledAway,
    Tick,
    SetTarget {
        #[serde(default)]
        target: Option<FocusTarget>,
    },
    SetLength {
        #[serde(serialize_with = "ser::num")]
        minutes: f64,
    },
}

/// A focus session to save: a [`FocusSession`] that has no id yet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewFocusSession {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub habit_id: Option<Id>,
    #[serde(serialize_with = "ser::num")]
    pub started_at: f64,
    #[serde(serialize_with = "ser::num")]
    pub ended_at: f64,
    #[serde(serialize_with = "ser::num")]
    pub focus_min: f64,
    #[serde(serialize_with = "ser::num")]
    pub interruptions: f64,
    pub room: Room,
}

impl NewFocusSession {
    /// The session as it is saved, with the id the store gives it.
    pub fn with_id(self, id: Id) -> FocusSession {
        FocusSession {
            id,
            task_id: self.task_id,
            habit_id: self.habit_id,
            started_at: self.started_at,
            ended_at: self.ended_at,
            focus_min: self.focus_min,
            interruptions: self.interruptions,
            view: self.room,
            source: FocusSource::Timer,
            public: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FocusEffect {
    Log {
        session: NewFocusSession,
    },
    /// Apply with [`super::habits::mark_habit_done`], never a toggle: the habit may already be done today.
    TickHabit {
        #[serde(rename = "habitId")]
        habit_id: Id,
    },
    Chime,
}

/// Heat's chime is off by default (8.7, Open #10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusSettings {
    pub chime: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub state: FocusState,
    pub effects: Vec<FocusEffect>,
}

pub fn is_focus_length(minutes: f64) -> bool {
    js::is_integer(minutes) && (10.0..=90.0).contains(&minutes)
}

pub fn initial_focus() -> FocusState {
    idle(1.0, 25.0, None, Room::Heat, None)
}

fn idle(
    round: f64,
    focus_min: f64,
    target: Option<FocusTarget>,
    room: Room,
    note: Option<String>,
) -> FocusState {
    let length_ms = focus_min * MIN;
    FocusState {
        phase: Phase::Idle,
        running: false,
        round,
        focus_min,
        length_ms,
        ends_at: None,
        left_ms: length_ms,
        target,
        session: None,
        closed: NOTHING_CLOSED,
        room,
        note,
    }
}

/// The time left in the focus or break, in ms: counting down while it runs.
pub fn left_at(s: &FocusState, now: f64) -> f64 {
    match (s.running, s.ends_at) {
        (true, Some(ends_at)) => js::max2(0.0, ends_at - now),
        _ => s.left_ms,
    }
}

fn break_ms(round: f64) -> f64 {
    (if round % ROUNDS == 0.0 { 15.0 } else { 5.0 }) * MIN
}

fn next_round(round: f64) -> f64 {
    (round % ROUNDS) + 1.0
}

// A habit with a length ticks once the session on it reaches that length.
fn habit_tick(s: &FocusState, left_now: f64) -> (FocusState, Vec<FocusEffect>) {
    let unchanged = || (s.clone(), vec![]);
    let Some(session) = &s.session else {
        return unchanged();
    };
    let Some(target) = &s.target else {
        return unchanged();
    };
    let Some(minutes) = target.minutes else {
        return unchanged();
    };
    if session.habit_ticked || target.kind != TargetKind::Habit {
        return unchanged();
    }
    if session.from_left_ms - left_now < minutes * MIN {
        return unchanged();
    }
    let mut state = s.clone();
    if let Some(session) = &mut state.session {
        session.habit_ticked = true;
    }
    (
        state,
        vec![FocusEffect::TickHabit {
            habit_id: target.id.clone(),
        }],
    )
}

struct Closing {
    minutes: f64,
    effects: Vec<FocusEffect>,
    closed: Closed,
}

// Closes the session in progress; it is logged if it comes to a minute or more.
fn close_session(s: &FocusState, ended_at: f64, left_now: f64) -> Closing {
    let (_, mut effects) = habit_tick(s, left_now);
    let (Some(session), Some(target)) = (&s.session, &s.target) else {
        return Closing {
            minutes: 0.0,
            effects,
            closed: s.closed,
        };
    };
    let ms = s.closed.ms + (session.from_left_ms - left_now);
    let minutes = js::round(ms / MIN) - s.closed.minutes;
    let closed = Closed {
        ms,
        minutes: s.closed.minutes + minutes,
    };
    if minutes < 1.0 {
        return Closing {
            minutes,
            effects,
            closed,
        };
    }
    let (task_id, habit_id) = match target.kind {
        TargetKind::Task => (Some(target.id.clone()), None),
        TargetKind::Habit => (None, Some(target.id.clone())),
    };
    effects.push(FocusEffect::Log {
        session: NewFocusSession {
            task_id,
            habit_id,
            started_at: session.started_at,
            ended_at,
            focus_min: minutes,
            interruptions: session.interruptions,
            room: s.room,
        },
    });
    Closing {
        minutes,
        effects,
        closed,
    }
}

// A running focus or break whose time is up ends, whatever event arrives.
fn settle(s: &FocusState, now: f64, settings: FocusSettings) -> (FocusState, Vec<FocusEffect>) {
    let Some(ends_at) = s.ends_at.filter(|_| s.running) else {
        return (s.clone(), vec![]);
    };
    if now < ends_at {
        return if s.phase == Phase::Focus {
            habit_tick(s, ends_at - now)
        } else {
            (s.clone(), vec![])
        };
    }
    if s.phase == Phase::Break {
        let round = next_round(s.round);
        let note = Some(copy::focus::break_done(round, ROUNDS));
        return (
            idle(round, s.focus_min, s.target.clone(), s.room, note),
            vec![],
        );
    }
    let Closing {
        minutes,
        mut effects,
        ..
    } = close_session(s, ends_at, 0.0);
    let length_ms = break_ms(s.round);
    let title = s.target.as_ref().map_or("", |t| t.title.as_str());
    let mut state = s.clone();
    state.phase = Phase::Break;
    state.running = false;
    state.length_ms = length_ms;
    state.ends_at = None;
    state.left_ms = length_ms;
    state.session = None;
    state.closed = NOTHING_CLOSED;
    state.note = Some(if minutes >= 1.0 {
        copy::focus::done(&format_minutes(minutes), title)
    } else {
        copy::focus::DONE_UNLOGGED.to_string()
    });
    if settings.chime {
        effects.push(FocusEffect::Chime);
    }
    (state, effects)
}

fn pause(s: &FocusState, now: f64) -> FocusState {
    FocusState {
        running: false,
        left_ms: left_at(s, now),
        ends_at: None,
        ..s.clone()
    }
}

fn start(s: &FocusState, now: f64) -> FocusState {
    FocusState {
        running: true,
        ends_at: Some(now + s.left_ms),
        note: None,
        ..s.clone()
    }
}

fn restart_idle(s: &FocusState, round: f64, focus_min: f64, note: Option<String>) -> FocusState {
    idle(round, focus_min, s.target.clone(), s.room, note)
}

pub fn focus_step(
    state: &FocusState,
    event: &FocusEvent,
    now: f64,
    settings: FocusSettings,
) -> Step {
    let (s, settled_effects) = settle(state, now, settings);
    let done = |next: FocusState, effects: Vec<FocusEffect>| {
        let mut all = settled_effects.clone();
        all.extend(effects);
        Step {
            state: next,
            effects: all,
        }
    };
    match event {
        FocusEvent::Tick => done(s, vec![]),

        FocusEvent::Press { target, room } => {
            if s.phase == Phase::Idle {
                let Some(target) = target.clone().or_else(|| s.target.clone()) else {
                    return done(s, vec![]);
                };
                let begun = FocusState {
                    target: Some(target),
                    room: room.unwrap_or(s.room),
                    phase: Phase::Focus,
                    ..s.clone()
                };
                let next = FocusState {
                    session: Some(SessionInProgress {
                        started_at: now,
                        from_left_ms: s.left_ms,
                        interruptions: 0.0,
                        habit_ticked: false,
                    }),
                    ..start(&begun, now)
                };
                return done(next, vec![]);
            }
            let next = if s.running {
                pause(&s, now)
            } else {
                start(&s, now)
            };
            done(next, vec![])
        }

        FocusEvent::PulledAway => {
            let Some(session) = s
                .session
                .clone()
                .filter(|_| s.phase == Phase::Focus && s.running)
            else {
                return done(s, vec![]);
            };
            let next = FocusState {
                session: Some(SessionInProgress {
                    interruptions: session.interruptions + 1.0,
                    ..session
                }),
                ..pause(&s, now)
            };
            done(next, vec![])
        }

        FocusEvent::Stop => {
            if s.phase == Phase::Break {
                return done(
                    restart_idle(&s, next_round(s.round), s.focus_min, None),
                    vec![],
                );
            }
            if s.phase != Phase::Focus {
                return done(s, vec![]);
            }
            let Closing {
                minutes, effects, ..
            } = close_session(&s, now, left_at(&s, now));
            let title = s.target.as_ref().map_or("", |t| t.title.as_str());
            let note =
                (minutes >= 1.0).then(|| copy::focus::stopped(&format_minutes(minutes), title));
            done(restart_idle(&s, s.round, s.focus_min, note), effects)
        }

        FocusEvent::SetTarget { target } => {
            if s.phase != Phase::Focus {
                return done(
                    FocusState {
                        target: target.clone(),
                        ..s
                    },
                    vec![],
                );
            }
            // Every session belongs to one task: a new current task closes this
            // one and opens the next, and the round runs on.
            let Some(target) = target else {
                return done(s, vec![]);
            };
            if Some(&target.id) == s.target.as_ref().map(|t| &t.id) {
                return done(s, vec![]);
            }
            let left_now = left_at(&s, now);
            let Closing {
                effects, closed, ..
            } = close_session(&s, now, left_now);
            let next = FocusState {
                target: Some(target.clone()),
                session: Some(SessionInProgress {
                    started_at: now,
                    from_left_ms: left_now,
                    interruptions: 0.0,
                    habit_ticked: false,
                }),
                closed,
                ..s
            };
            done(next, effects)
        }

        FocusEvent::SetLength { minutes } => {
            if s.phase == Phase::Focus || !is_focus_length(*minutes) {
                return done(s, vec![]);
            }
            let next = FocusState {
                focus_min: *minutes,
                ..s.clone()
            };
            if s.phase == Phase::Idle {
                done(
                    restart_idle(&next, next.round, next.focus_min, next.note.clone()),
                    vec![],
                )
            } else {
                done(next, vec![])
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FocusLcd {
    pub digits: String,
    pub line: String,
    pub note: Option<String>,
    /// The meter drains from 1 to 0.
    #[serde(serialize_with = "ser::num")]
    pub meter: f64,
    pub paused: bool,
}

/// The olive LCD panel: 32 px digits, one line, a meter.
pub fn focus_lcd(s: &FocusState, now: f64) -> FocusLcd {
    let left = left_at(s, now);
    let digits = format::countdown(left);
    let meter = if s.length_ms > 0.0 {
        left / s.length_ms
    } else {
        0.0
    };
    let focus_line = match &s.target {
        Some(t) => copy::focus::line(s.round, ROUNDS, &t.title),
        None => copy::widgets::NOW_EMPTY.to_string(),
    };
    match s.phase {
        Phase::Idle => FocusLcd {
            digits,
            line: focus_line,
            note: s.note.clone(),
            meter: 1.0,
            paused: false,
        },
        Phase::Focus => FocusLcd {
            digits,
            line: focus_line,
            note: s.note.clone(),
            meter,
            paused: !s.running,
        },
        Phase::Break => {
            let waiting = !s.running && left == s.length_ms;
            FocusLcd {
                digits,
                line: if waiting {
                    copy::focus::break_waits(&format::countdown(s.length_ms))
                } else {
                    copy::focus::BREAK_LINE.to_string()
                },
                note: s.note.clone(),
                meter,
                paused: !s.running && !waiting,
            }
        }
    }
}

/// The Now strip's focus half: "focus 18:40 left".
pub fn focus_strip(s: &FocusState, now: f64) -> Option<String> {
    match s.phase {
        Phase::Focus => Some(copy::focus::strip(&format::countdown(left_at(s, now)))),
        Phase::Break if s.running || left_at(s, now) < s.length_ms => Some(
            copy::focus::break_strip(&format::countdown(left_at(s, now))),
        ),
        _ => None,
    }
}

// One answer handed straight back, so the size of its largest variant doesn't matter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CheckOff {
    Done {
        task: Task,
        message: String,
    },
    Ask {
        title: String,
        hint: String,
    },
    /// A recurring task: its next open occurrence is ticked as a row of its own, and the series stays open.
    Occurrence {
        occurrence: TaskOccurrence,
        message: Option<String>,
    },
    /// A recurring task whose series has ended: nothing is left to check.
    #[serde(rename = "none")]
    Nothing,
}

/// Checking a task off. With logged time it is done at once and the status
/// bar says what it took; only a task with no logged time still asks.
///
/// A recurring task never flips to done (3.6): checking it ticks its next
/// open occurrence. The status bar then names the focus logged on it since
/// the previous tick; with none, it doesn't ask, because an occurrence keeps
/// no time of its own.
pub fn check_off(
    task: &Task,
    sessions: &[FocusSession],
    occurrences: &[TaskOccurrence],
    now: f64,
    tz: &TimeZone,
    new_id: &mut dyn FnMut() -> Id,
) -> CheckOff {
    let own: Vec<FocusSession> = sessions
        .iter()
        .filter(|s| s.task_id.as_deref() == Some(task.id.as_str()))
        .cloned()
        .collect();
    if recurs(task) {
        let Some(occurrence) = check_occurrence(task, occurrences, now, tz, new_id) else {
            return CheckOff::Nothing;
        };
        let since = js::max_of(
            occurrences
                .iter()
                .filter(|o| o.task_id == task.id)
                .map(|o| o.done_at),
        );
        let recent: Vec<&FocusSession> = own.iter().filter(|s| s.ended_at > since).collect();
        let took = recent.iter().fold(0.0, |sum, s| sum + s.focus_min);
        return CheckOff::Occurrence {
            occurrence,
            message: (!recent.is_empty())
                .then(|| copy::check::done(&format_minutes(took), recent.len() as f64)),
        };
    }
    let took = actual_min(task, &own);
    if own.is_empty() && took <= 0.0 {
        return CheckOff::Ask {
            title: copy::check::TITLE.to_string(),
            hint: copy::check::HINT.to_string(),
        };
    }
    CheckOff::Done {
        task: Task {
            done: true,
            done_at: Some(now),
            ..task.clone()
        },
        message: copy::check::done(&format_minutes(took), own.len() as f64),
    }
}

/// The answer to "Time it took", for a task with no logged time.
pub fn finish_with_time(task: &Task, minutes: f64, now: f64) -> Task {
    Task {
        adjust_min: minutes,
        done: true,
        done_at: Some(now),
        ..task.clone()
    }
}

/// Get Info's "Took": set the total by hand; focus minutes stay as logged.
pub fn set_took(task: &Task, sessions: &[FocusSession], minutes: f64) -> Task {
    Task {
        adjust_min: minutes - (actual_min(task, sessions) - task.adjust_min),
        ..task.clone()
    }
}
