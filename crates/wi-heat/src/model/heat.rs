//! The heat algorithm, exactly as Heat runs it today (`docs/SPEC.md` 3.1):
//!
//! ```text
//!   done         -> no heat ("Done")
//!   no due date  -> v = 0.05, "Cool"
//!   days   = (due - now) / 86_400_000
//!   days < 0     -> v = 1.1, "Overdue"
//!   runway = difficulty * 2 + 1
//!   v      = clamp(1 - days / runway, 0, 1)
//!   v >= 0.70 -> "Hot";  v >= 0.34 -> "Warm";  else "Cool" (v floored at 0.05)
//! ```
//!
//! A recurring task's heat comes from its next occurrence: pass it through
//! [`super::recurrence::with_effective_due`] first. A port of `heat.ts`; the
//! level colours and the tube's size are display and stay in TypeScript.

use super::records::{ser, Task};
use super::{copy, format, js, zone};
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub use super::zone::DAY_MS;

const HOT: f64 = 0.7;
const WARM: f64 = 0.34;
const FLOOR: f64 = 0.05;
const OVERDUE: f64 = 1.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeatLevel {
    Overdue,
    Hot,
    Warm,
    Cool,
    Done,
}

impl HeatLevel {
    /// The level's word, as the Now strip and a draft's reason print it.
    pub fn as_str(self) -> &'static str {
        match self {
            HeatLevel::Overdue => "Overdue",
            HeatLevel::Hot => "Hot",
            HeatLevel::Warm => "Warm",
            HeatLevel::Cool => "Cool",
            HeatLevel::Done => "Done",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Heat {
    pub level: HeatLevel,
    #[serde(default, serialize_with = "ser::opt_num")]
    pub v: Option<f64>,
}

/// What heat reads of a task: whether it is done, when it is due, how hard it is.
pub trait HeatInput {
    fn is_done(&self) -> bool;
    fn due(&self) -> Option<f64>;
    fn difficulty(&self) -> f64;
}

impl HeatInput for Task {
    fn is_done(&self) -> bool {
        self.done
    }
    fn due(&self) -> Option<f64> {
        self.due
    }
    fn difficulty(&self) -> f64 {
        self.difficulty
    }
}

impl<T: HeatInput + ?Sized> HeatInput for &T {
    fn is_done(&self) -> bool {
        (**self).is_done()
    }
    fn due(&self) -> Option<f64> {
        (**self).due()
    }
    fn difficulty(&self) -> f64 {
        (**self).difficulty()
    }
}

/// Just the three fields heat reads, for a caller that has no whole task.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeatFields {
    pub done: bool,
    #[serde(default, serialize_with = "ser::opt_num")]
    pub due: Option<f64>,
    #[serde(serialize_with = "ser::num")]
    pub difficulty: f64,
}

impl HeatInput for HeatFields {
    fn is_done(&self) -> bool {
        self.done
    }
    fn due(&self) -> Option<f64> {
        self.due
    }
    fn difficulty(&self) -> f64 {
        self.difficulty
    }
}

/// A due date heat can read. One that isn't a finite number (a corrupt row)
/// counts as none, so it can't upset the sort.
pub(crate) fn dated(due: Option<f64>) -> Option<f64> {
    due.filter(|d| d.is_finite())
}

/// Days of runway: difficulty 1..5 gives 3, 5, 7, 9, 11.
pub fn runway(difficulty: f64) -> f64 {
    let d = js::min2(5.0, js::max2(1.0, js::round(difficulty)));
    d * 2.0 + 1.0
}

pub fn heat_of(t: &impl HeatInput, now: f64) -> Heat {
    if t.is_done() {
        return Heat {
            level: HeatLevel::Done,
            v: None,
        };
    }
    let Some(due) = dated(t.due()) else {
        return Heat {
            level: HeatLevel::Cool,
            v: Some(FLOOR),
        };
    };
    let days = (due - now) / DAY_MS;
    if days < 0.0 {
        return Heat {
            level: HeatLevel::Overdue,
            v: Some(OVERDUE),
        };
    }
    let v = js::min2(1.0, js::max2(0.0, 1.0 - days / runway(t.difficulty())));
    if v >= HOT {
        return Heat {
            level: HeatLevel::Hot,
            v: Some(v),
        };
    }
    if v >= WARM {
        return Heat {
            level: HeatLevel::Warm,
            v: Some(v),
        };
    }
    Heat {
        level: HeatLevel::Cool,
        v: Some(js::max2(v, FLOOR)),
    }
}

/// Days left when a task turns Warm and Hot, as 3.1's table prints them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thresholds {
    #[serde(serialize_with = "ser::num")]
    pub warm_at: f64,
    #[serde(serialize_with = "ser::num")]
    pub hot_at: f64,
}

pub fn heat_thresholds(difficulty: f64) -> Thresholds {
    let r = runway(difficulty);
    let round2 = |x: f64| js::round(x * 100.0) / 100.0;
    Thresholds {
        warm_at: round2((1.0 - WARM) * r),
        hot_at: round2((1.0 - HOT) * r),
    }
}

/// How full the tube is: v, capped at full; a done task's tube is empty.
pub fn tube_fill(h: &Heat) -> f64 {
    match h.v {
        None => 0.0,
        Some(v) => js::min2(1.0, v),
    }
}

/// A JavaScript sort comparator's answer: below 0 first, above 0 last, 0 or NaN equal.
pub(crate) fn order_of(x: f64) -> Ordering {
    if x < 0.0 {
        Ordering::Less
    } else if x > 0.0 {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// Open tasks by heat descending, then by due date, undated last; done tasks
/// after them. Stable, so tasks that tie keep the order they came in.
pub fn by_heat<T: HeatInput>(items: &[T], now: f64) -> Vec<&T> {
    by_heat_order(items, now)
        .into_iter()
        .map(|i| &items[i])
        .collect()
}

/// [`by_heat`] as positions in `items`, for a caller that needs to know which is which.
pub fn by_heat_order<T: HeatInput>(items: &[T], now: f64) -> Vec<usize> {
    let keyed: Vec<(usize, Option<f64>, Option<f64>)> = items
        .iter()
        .enumerate()
        .map(|(i, t)| (i, heat_of(t, now).v, dated(t.due())))
        .collect();
    js::sort_by(keyed, |a, b| {
        let (Some(av), Some(bv)) = (a.1, b.1) else {
            return u8::from(a.1.is_none()).cmp(&u8::from(b.1.is_none()));
        };
        // `a.v !== b.v`: true for a NaN, whose difference then sorts as a tie.
        if av != bv {
            return order_of(bv - av);
        }
        let (Some(ad), Some(bd)) = (a.2, b.2) else {
            return u8::from(a.2.is_none()).cmp(&u8::from(b.2.is_none()));
        };
        order_of(ad - bd)
    })
    .into_iter()
    .map(|k| k.0)
    .collect()
}

/// "Today 4:00 PM", "Tomorrow 11:59 PM", "Wednesday 11:59 PM", "3h overdue", "2d overdue".
pub fn due_phrase(due: f64, now: f64, tz: &TimeZone) -> String {
    if due < now {
        let minutes = ((now - due) / 60_000.0).floor();
        if minutes < 60.0 {
            return copy::due::overdue(&format!("{}m", js::num_to_string(js::max2(1.0, minutes))));
        }
        if minutes < 24.0 * 60.0 {
            return copy::due::overdue(&format!(
                "{}h",
                js::num_to_string((minutes / 60.0).floor())
            ));
        }
        return copy::due::overdue(&format!(
            "{}d",
            js::num_to_string((minutes / (24.0 * 60.0)).floor())
        ));
    }
    let time = format::clock_at(due, tz);
    let day = zone::day_key(due, tz);
    let ahead = zone::days_between(&zone::day_key(now, tz), &day);
    if ahead == 0.0 {
        return copy::due::today(&time);
    }
    if ahead == 1.0 {
        return copy::due::tomorrow(&time);
    }
    if ahead <= 6.0 {
        return copy::due::on_day(format::weekday_name(zone::weekday_of(&day)), &time);
    }
    let year = zone::wall_time(due, tz).year;
    let date = if year == zone::wall_time(now, tz).year {
        format::short_month_day(&day)
    } else {
        format!(
            "{}, {}",
            format::short_month_day(&day),
            js::num_to_string(year)
        )
    };
    copy::due::on_day(&date, &time)
}

/// The first millisecond after `now` at which any task's level changes, or
/// None when none will. Levels only climb (Cool, Warm, Hot, Overdue), so a
/// binary search between now and just past the due date finds the exact
/// millisecond, with v computed exactly as [`heat_of`] computes it.
///
/// The TypeScript's search never returns for a due that isn't a whole
/// millisecond: its midpoint rounds down onto `lo` and the bracket stops
/// narrowing. Here the search stops when it can't narrow (docs/QUESTIONS.md,
/// "TS bug: nextHeatChange never returns"); for a whole-millisecond due, the
/// only kind a stored record holds, the answer is the TypeScript's.
pub fn next_heat_change<T: HeatInput>(items: &[T], now: f64) -> Option<f64> {
    let mut soonest: Option<f64> = None;
    for t in items {
        let Some(due) = dated(t.due()) else { continue };
        if t.is_done() || due < now {
            continue;
        }
        let level = heat_of(t, now).level;
        let mut lo = now;
        let mut hi = due + 1.0;
        while hi - lo > 1.0 {
            let mid = ((lo + hi) / 2.0).floor();
            if mid <= lo {
                break;
            }
            if heat_of(t, mid).level == level {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        if soonest.map_or(true, |s| hi < s) {
            soonest = Some(hi);
        }
    }
    soonest
}
