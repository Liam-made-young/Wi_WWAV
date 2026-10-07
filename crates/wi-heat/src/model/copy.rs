//! The sentences the model builds, in the spec's words: the part of
//! `app/ui/src/heat/model/copy.ts` that the ported functions fill in and
//! return (Plan my day's reasons, the focus timer's notes, what it would take
//! to reach a grade, the review's facts, and so on). The rest of `copy.ts` is
//! display text and stays in TypeScript. Plain, second person, present tense;
//! no exclamation marks; real numbers instead of adjectives.

use super::js::num_to_string;

/// `${n} ${n === 1 ? one : many}`, `many` being `${one}s` unless given.
pub fn plural(n: f64, one: &str, many: Option<&str>) -> String {
    let word = if n == 1.0 {
        one.to_string()
    } else {
        many.map_or_else(|| format!("{one}s"), str::to_string)
    };
    format!("{} {}", num_to_string(n), word)
}

// --- 3.1: heat, the LCD and the sidebar -------------------------------------

/// Due phrases: "Today 4:00 PM", "Tomorrow 11:59 PM", "2d overdue".
pub mod due {
    pub fn today(time: &str) -> String {
        format!("Today {time}")
    }
    pub fn tomorrow(time: &str) -> String {
        format!("Tomorrow {time}")
    }
    pub fn on_day(day: &str, time: &str) -> String {
        format!("{day} {time}")
    }
    pub fn overdue(amount: &str) -> String {
        format!("{amount} overdue")
    }
}

/// "Homework 1h 15m (6)"
pub fn average_time(kind: &str, time: &str, count: f64) -> String {
    format!("{kind} {time} ({})", num_to_string(count))
}

/// "This week: 3h 20m across 5 tasks"
pub fn weekly_load(time: &str, count: f64) -> String {
    format!("This week: {time} across {}", plural(count, "task", None))
}

/// The Now strip's task half (2.2): "Hot: Grammar quiz 4".
pub mod strip {
    pub fn task(level: &str, title: &str) -> String {
        format!("{level}: {title}")
    }
    pub const ALL_CLEAR: &str = "All clear";
    pub const NOTHING_OPEN: &str = "Nothing open right now.";
}

// --- 3.5: Today -------------------------------------------------------------

pub mod today {
    use super::plural;
    use crate::model::js::num_to_string;

    /// "4 blocks · 3h 10m planned · 2 due today"
    pub fn subtitle(blocks: f64, planned: &str, due_today: f64) -> String {
        format!(
            "{} · {planned} planned · {} due today",
            plural(blocks, "block", None),
            num_to_string(due_today)
        )
    }
    pub const PLANNED: &str = "Planned";
    pub const DUE_TODAY: &str = "Due today, not planned";
    pub const RECURRING: &str = "Recurring today ↻";
    pub const HOT: &str = "Hot, not planned";
}

/// Plan my day's drafts: "Due tomorrow 11:59 PM, Hot."
pub mod draft {
    /// The phrase with a leading "Today " or "Tomorrow " lower-cased, as
    /// `phrase.replace(/^(Today|Tomorrow) /, w => w.toLowerCase())` does.
    pub fn reason(phrase: &str, level: &str) -> String {
        let lowered = if let Some(rest) = phrase.strip_prefix("Today ") {
            format!("today {rest}")
        } else if let Some(rest) = phrase.strip_prefix("Tomorrow ") {
            format!("tomorrow {rest}")
        } else {
            phrase.to_string()
        };
        format!("Due {lowered}, {level}.")
    }
    pub fn overdue(phrase: &str) -> String {
        format!("{phrase}.")
    }
    pub const NO_DUE: &str = "No due date.";
    pub fn left_to_plan(time: &str) -> String {
        format!("{time} left to plan")
    }
}

/// The Pomodoro LCD, and the strip's "focus 18:40 left" (2.2).
pub mod focus {
    use crate::model::js::num_to_string;

    pub fn line(round: f64, of: f64, title: &str) -> String {
        format!(
            "Focus {} of {} · {title}",
            num_to_string(round),
            num_to_string(of)
        )
    }
    pub fn done(time: &str, title: &str) -> String {
        format!("Focus done. {time} logged to {title}.")
    }
    pub const DONE_UNLOGGED: &str = "Focus done.";
    pub fn break_waits(length: &str) -> String {
        format!("Break {length}. Press F to start it.")
    }
    pub const BREAK_LINE: &str = "Break";
    pub fn stopped(time: &str, title: &str) -> String {
        format!("Focus stopped. {time} logged to {title}.")
    }
    pub fn break_done(round: f64, of: f64) -> String {
        format!(
            "Break done. Press F to start focus {} of {}.",
            num_to_string(round),
            num_to_string(of)
        )
    }
    pub fn strip(left: &str) -> String {
        format!("focus {left} left")
    }
    pub fn break_strip(left: &str) -> String {
        format!("break {left} left")
    }
}

/// Checking a task off.
pub mod check {
    use super::plural;

    pub const TITLE: &str = "Time it took";
    pub const HINT: &str = "This trains your time averages for this type of task.";
    pub fn done(time: &str, sessions: f64) -> String {
        if sessions > 0.0 {
            format!(
                "Done. Took {time} across {}.",
                plural(sessions, "focus session", None)
            )
        } else {
            format!("Done. Took {time}.")
        }
    }
}

/// The right column's widgets.
pub mod widgets {
    pub const NOW_EMPTY: &str = "Nothing is current. Pick a task and press C, or drag one here.";
}

// --- 3.7: Calendar ----------------------------------------------------------

pub mod calendar {
    use crate::model::js::num_to_string;

    pub fn more(n: f64) -> String {
        format!("{} more", num_to_string(n))
    }
    pub fn due_flag(time: &str) -> String {
        format!("due {time}")
    }
}

// --- 3.8: Grades ------------------------------------------------------------

/// "An A", "a B": the article a letter takes when read aloud.
fn with_article(letter: &str, capital: bool) -> String {
    let a = if letter.starts_with(|c| "AEFHILMNORSX".contains(c)) {
        "an"
    } else {
        "a"
    };
    if capital {
        let mut chars = a.chars();
        let first = chars
            .next()
            .map(|c| c.to_ascii_uppercase())
            .unwrap_or_default();
        format!("{first}{} {letter}", chars.as_str())
    } else {
        format!("{a} {letter}")
    }
}

pub mod grades {
    use super::{plural, with_article};

    pub fn based_on(pct: &str) -> String {
        format!("Based on {pct}% of the course so far")
    }
    pub fn to_enter(n: f64) -> String {
        format!("{} to enter", plural(n, "new grade", None))
    }
    pub fn weights_short(total: &str, rest: &str) -> String {
        format!("Weights add to {total}%. The other {rest}% is unassigned.")
    }
    pub fn weights_over(total: &str, over: &str) -> String {
        format!("Weights add to {total}%. That is {over}% more than 100.")
    }
    pub fn need(letter: &str, min: &str, need: &str, left: &str) -> String {
        format!(
            "To finish with {} ({min}%), you need {need}% on the remaining {left}%.",
            with_article(letter, false)
        )
    }
    pub fn out_of_reach(letter: &str, best: &str, best_letter: &str) -> String {
        format!(
            "{} is out of reach; the highest possible is {best}% ({best_letter}).",
            with_article(letter, true)
        )
    }
    pub fn safe(letter: &str, left: &str) -> String {
        format!(
            "You keep {} even with 0% on the remaining {left}%.",
            with_article(letter, false)
        )
    }
    pub fn all_graded(pct: &str, letter: &str) -> String {
        format!("Every category is graded. The course stands at {pct}% ({letter}).")
    }
}

// --- 3.9: Habits ------------------------------------------------------------

pub mod habits {
    use super::plural;
    use crate::model::js::num_to_string;

    pub const LIMIT: &str = "Habit limit reached";
    pub const NAME_FIRST: &str = "Give the habit a name first.";
    pub fn of_done(done: f64, all: f64) -> String {
        format!("{} of {} done", num_to_string(done), num_to_string(all))
    }
    pub fn record(days: f64, since: &str) -> String {
        format!("Done {} since {since}", plural(days, "day", None))
    }
    pub const NOT_YET: &str = "Not done yet";
    pub fn streak(days: f64) -> String {
        format!("{}-day streak", num_to_string(days))
    }
}

// --- 3.13: the weekly review ------------------------------------------------

pub mod review {
    use super::plural;
    use crate::model::js::num_to_string;

    pub fn space(name: &str, done: f64, focus: &str) -> String {
        format!(
            "{name}: {} done, {focus} of focus",
            plural(done, "task", None)
        )
    }
    pub fn habits(focus: &str) -> String {
        format!("Habits: {focus} of focus")
    }
    pub fn milestone(title: &str) -> String {
        format!("Milestone reached: {title}")
    }
    pub fn accuracy(kind: &str, estimated: &str, took: &str, n: f64) -> String {
        format!(
            "{kind}: estimated {estimated}, took {took} across {}",
            num_to_string(n)
        )
    }
}

// --- 3.15: moving in --------------------------------------------------------

pub mod moving {
    pub const NOT_EXPORT: &str = "This file isn’t a Heat export.";
    pub const NEWER: &str = "This export is from a newer Heat. Update Wi_WWAV, then try again.";
}
