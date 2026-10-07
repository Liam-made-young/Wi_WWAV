//! Today's moving parts (docs/SPEC.md 3.5): Plan my day's drafts, the current
//! task and the focus timer. None of it is an edit until it ends in a record:
//! the drafts and the timer live in `heatState`, outside the journal, and
//! accepting a draft or ending a round writes the blocks and the focus record
//! as ordinary changes. The timer is `wi_heat::model::focus`'s state machine;
//! this module keeps its state between calls and carries out what it asks for.

use serde_json::{json, Value};
use wi_heat::model::focus::{
    focus_step, initial_focus, is_focus_length, FocusEffect, FocusEvent, FocusSettings, FocusState, FocusTarget, Phase, TargetKind,
};
use wi_heat::model::plan::{accept_draft, Draft};
use wi_heat::model::records::Room;
use wi_store::{Actor, Store};

use crate::derive::{self, World};
use crate::{commit, kind, num, one, put, refused, set_setting, set_state, setting, state, ulid, Clock, Outcome, Result};

/// The timer's state as the views read it: the summary 3.16 gives
/// (`{phase, round, endsAt}`), and what a paused or waiting timer needs too.
/// `taskId` is the task the round is on (a habit's round has none).
fn timer_summary(f: &FocusState) -> Value {
    let task = f.target.as_ref().filter(|t| t.kind == TargetKind::Task).map_or(Value::Null, |t| json!(t.id));
    json!({
        "phase": f.phase,
        "round": num(f.round),
        "endsAt": f.ends_at.filter(|_| f.running).map_or(Value::Null, num),
        "running": f.running,
        "leftMs": num(f.left_ms),
        "lengthMs": num(f.length_ms),
        "focusMin": num(f.focus_min),
        "taskId": task,
        "note": f.note,
    })
}

/// The whole timer machine kept in `heatState.focus`, or a fresh one.
pub(crate) fn load_focus(state: &Value) -> FocusState {
    state
        .get("focus")
        .and_then(|f| serde_json::from_value(f.clone()).ok())
        .unwrap_or_else(initial_focus)
}

fn keep_focus(state: &mut Value, f: &FocusState) {
    state["focus"] = serde_json::to_value(f).unwrap_or(Value::Null);
    state["timer"] = timer_summary(f);
}

/// `heatState` as the views see it: the machine behind the timer stays inside.
pub fn public_state(state: &Value) -> Value {
    let mut s = state.clone();
    if let Some(m) = s.as_object_mut() {
        m.remove("focus");
        m.entry("currentTaskId").or_insert(Value::Null);
        m.entry("planDrafts").or_insert_with(|| json!([]));
    }
    if s.get("timer").is_none() {
        s["timer"] = timer_summary(&initial_focus());
    }
    s
}

/// The time the day ends at, in minutes after midnight: the last one set,
/// else 11 PM.
pub fn day_ends(store: &Store) -> Result<f64> {
    Ok(setting(store, "dayEnds")?.and_then(|v| v.as_f64()).unwrap_or(derive::DAY_ENDS_MIN))
}

/// `heat.plan.make`: Plan my day's rule, as drafts waiting to be accepted.
/// Writes only `heatState.planDrafts` (8.8).
pub fn plan_make(store: &mut Store, clock: &Clock, date: Option<&str>, ends: Option<f64>) -> Result<Outcome> {
    let today = clock.today();
    let date = date.unwrap_or(&today).to_string();
    if !crate::schema::is_day(&date) {
        return refused("A day is written YYYY-MM-DD.");
    }
    if date < today {
        return refused("That day is over. Plan today or a day ahead.");
    }
    let ends = match ends {
        Some(m) if m.is_finite() && (0.0..=1440.0).contains(&m) => {
            set_setting(store, "dayEnds", &num(m))?;
            m
        }
        Some(_) => return refused("The day ends between midnight and midnight."),
        None => day_ends(store)?,
    };
    let world = World::load(store)?;
    let plan = derive::plan(&world, clock, &date, ends)?;
    let mut st = state(store)?;
    st["planDrafts"] = Value::Array(plan.drafts_stored.clone());
    set_state(store, &st)?;
    let unplanned: Vec<Value> = plan.unplanned.iter().filter_map(|u| u.get("task_id").cloned()).collect();
    Ok(Outcome::outside(
        json!({"drafts": plan.drafts_stored, "unplanned": unplanned, "minutesLeft": num(plan.minutes_left)}),
        &[kind::STATE],
    ))
}

/// `heat.plan.accept`: every draft of `date` becomes a block (Return), or just
/// the ones named (a click).
pub fn plan_accept(store: &mut Store, _clock: &Clock, date: &str, task_ids: Option<&[String]>) -> Result<Outcome> {
    let mut st = state(store)?;
    let drafts: Vec<Draft> = st
        .get("planDrafts")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|d| serde_json::from_value(d.clone()).ok()).collect())
        .unwrap_or_default();
    let world = World::load(store)?;
    let wanted = |d: &Draft| {
        d.date == date
            && task_ids.map_or(true, |ids| ids.contains(&d.task_id))
            && world.tasks.iter().any(|t| t.id == d.task_id && !t.done)
    };
    let (taken, left): (Vec<Draft>, Vec<Draft>) = drafts.into_iter().partition(wanted);
    if taken.is_empty() {
        return Ok(Outcome::unchanged(json!({"blocks": []})));
    }
    let blocks: Vec<Value> = taken
        .iter()
        .map(|d| serde_json::to_value(accept_draft(d, &mut ulid)).unwrap_or(Value::Null))
        .collect();
    let label = if task_ids.is_some() { "accept draft" } else { "plan my day" };
    let c = commit(store, label, Actor::You, |txn| {
        for b in &blocks {
            put(txn, kind::BLOCK, b)?;
        }
        Ok(())
    })?;
    st["planDrafts"] = serde_json::to_value(&left).unwrap_or(json!([]));
    set_state(store, &st)?;
    Ok(Outcome::new(json!({ "blocks": blocks }), c).also(&[kind::STATE]))
}

/// `heat.plan.clear`: Esc, the drafts gone.
pub fn plan_clear(store: &mut Store) -> Result<Outcome> {
    let mut st = state(store)?;
    st["planDrafts"] = json!([]);
    set_state(store, &st)?;
    Ok(Outcome::outside(json!({}), &[kind::STATE]))
}

/// What a step of the machine asked for, carried out: the focus record to
/// keep, a habit to tick.
struct Carried {
    logged: Option<Value>,
    habit: Option<Value>,
}

fn carry_out(store: &Store, clock: &Clock, effects: &[FocusEffect]) -> Result<Carried> {
    let mut out = Carried { logged: None, habit: None };
    for effect in effects {
        match effect {
            FocusEffect::Log { session } => {
                let record = session.clone().with_id(ulid());
                let mut v = serde_json::to_value(&record).unwrap_or(Value::Null);
                v["public"] = json!(false);
                out.logged = Some(v);
            }
            FocusEffect::TickHabit { habit_id } => {
                if let Some(mut h) = one(store, kind::HABIT, habit_id)? {
                    let today = clock.today();
                    if h["log"].get(&today) != Some(&json!(true)) {
                        h["log"][&today] = json!(true);
                        out.habit = Some(h);
                    }
                }
            }
            // The chime is the window's, and it is off unless the person turned it on.
            FocusEffect::Chime => {}
        }
    }
    Ok(out)
}

/// Writes what the machine produced as one entry, "focus session".
fn log_round(store: &mut Store, carried: &Carried) -> Result<crate::Committed> {
    commit(store, "focus session", Actor::You, |txn| {
        if let Some(s) = &carried.logged {
            put(txn, kind::FOCUS, s)?;
        }
        if let Some(h) = &carried.habit {
            put(txn, kind::HABIT, h)?;
        }
        Ok(())
    })
}

fn target_of(world: &World, id: &str) -> Result<FocusTarget> {
    if let Some(t) = world.tasks.iter().find(|t| t.id == id) {
        return Ok(FocusTarget { kind: TargetKind::Task, id: t.id.clone(), title: t.title.clone(), minutes: None });
    }
    if let Some(h) = world.habits.iter().find(|h| h.id == id) {
        return Ok(FocusTarget { kind: TargetKind::Habit, id: h.id.clone(), title: h.title.clone(), minutes: h.minutes });
    }
    refused("No task has that id.")
}

/// `heat.current.set`: the task focus is on. While a round runs, a new
/// current task closes that task's part of it and opens the next's.
pub fn set_current(store: &mut Store, clock: &Clock, task_id: Option<&str>) -> Result<Outcome> {
    let world = World::load(store)?;
    let target = match task_id {
        Some(id) => {
            if !world.tasks.iter().any(|t| t.id == id) {
                return refused("No task has that id.");
            }
            Some(target_of(&world, id)?)
        }
        None => None,
    };
    let mut st = state(store)?;
    st["currentTaskId"] = task_id.map_or(Value::Null, |i| json!(i));
    let mut fs = load_focus(&st);
    let step = focus_step(&fs, &FocusEvent::SetTarget { target }, clock.now_ms, FocusSettings::default());
    fs = step.state;
    keep_focus(&mut st, &fs);
    let carried = carry_out(store, clock, &step.effects)?;
    let committed = if carried.logged.is_some() || carried.habit.is_some() { Some(log_round(store, &carried)?) } else { None };
    set_state(store, &st)?;
    Ok(match committed {
        Some(c) => {
            let mut out = Outcome::new(json!({"logged": carried.logged}), c).also(&[kind::STATE]);
            out.with_undo = true;
            out
        }
        None => Outcome::outside(json!({}), &[kind::STATE]),
    })
}

/// `heat.focus.*`: the timer, one press at a time. Nothing starts without a
/// call (3.5): the machine only moves on the events below.
pub fn focus(store: &mut Store, clock: &Clock, step: &str, task_id: Option<&str>, length: Option<f64>) -> Result<Outcome> {
    let now = clock.now_ms;
    let world = World::load(store)?;
    let was = state(store)?;
    let mut st = was.clone();
    let mut fs = load_focus(&st);
    let mut events: Vec<FocusEvent> = Vec::new();
    match step {
        "start" | "resume" => {
            let target = match task_id {
                Some(id) => Some(target_of(&world, id)?),
                None => match st["currentTaskId"].as_str().filter(|id| world.tasks.iter().any(|t| t.id == *id && !t.done)) {
                    Some(id) => Some(target_of(&world, id)?),
                    None => fs.target.clone(),
                },
            };
            if target.is_none() && fs.phase == Phase::Idle {
                return refused("Pick a task and press C first.");
            }
            if let Some(minutes) = length {
                if fs.phase != Phase::Focus {
                    if !is_focus_length(minutes) {
                        return refused("A focus is 25 or 50 minutes, or any whole number from 10 to 90.");
                    }
                    events.push(FocusEvent::SetLength { minutes });
                }
            }
            // Starting a timer that runs already doesn't pause it.
            if fs.running && fs.phase != Phase::Idle {
                events.clear();
            } else {
                events.push(FocusEvent::Press { target: target.clone(), room: Some(Room::Heat) });
            }
            // Focus on a selection makes it the current task (3.5).
            if let Some(t) = target.filter(|t| t.kind == TargetKind::Task) {
                st["currentTaskId"] = json!(t.id);
            }
        }
        "pause" => {
            if fs.running {
                events.push(FocusEvent::Press { target: None, room: None });
            }
        }
        "interrupt" => events.push(FocusEvent::PulledAway),
        "stop" => events.push(FocusEvent::Stop),
        // Time is up: the round settles at its own end, logs its minutes once
        // and waits for a press. Called early, late or twice, it changes
        // nothing more: the window asks until it sees the round has ended.
        "finish" => events.push(FocusEvent::Tick),
        other => return refused(format!("There is no focus step called {other}.")),
    }
    let mut effects = Vec::new();
    for e in &events {
        let s = focus_step(&fs, e, now, FocusSettings::default());
        fs = s.state;
        effects.extend(s.effects);
    }
    keep_focus(&mut st, &fs);
    let carried = carry_out(store, clock, &effects)?;
    let committed = if carried.logged.is_some() || carried.habit.is_some() { Some(log_round(store, &carried)?) } else { None };
    // A press that moves nothing (a second finish) writes nothing and tells no one.
    let moved = st != was;
    if moved {
        set_state(store, &st)?;
    }
    let mut result = json!({ "heatState": public_state(&st) });
    if let Some(s) = &carried.logged {
        result["logged"] = s.clone();
    }
    let out = match committed {
        Some(c) => Outcome::new(result, c),
        None => Outcome::unchanged(result),
    };
    Ok(if moved { out.also(&[kind::STATE]) } else { out })
}
