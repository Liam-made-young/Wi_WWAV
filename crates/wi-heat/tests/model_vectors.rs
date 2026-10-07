//! Cross-check: the Rust model gives the TypeScript's answer for every input
//! the TypeScript was run on.
//!
//! `app/ui/src/heat/model/vectors.gen.test.ts` writes {input, output} pairs to
//! `tests/vectors/<module>.json` (see its header for how to regenerate them).
//! Each test here reads one module's file, runs the Rust function of the same
//! name on every input, and compares the result with the TypeScript's output
//! as `serde_json::Value`s: strings, booleans, nulls and integers exactly,
//! and every float bit for bit. (JSON can't write a -0, so a -0 and a 0 count
//! as one number.)

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use wi_heat::model::records::{FocusSession, Space, Task};
use wi_heat::model::{estimate, format, heat, js, zone};

mod vectors {
    use super::*;

    pub fn load(module: &str) -> Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/vectors")
            .join(format!("{module}.json"));
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }
}

fn norm(x: f64) -> f64 {
    if x == 0.0 {
        0.0
    } else {
        x
    }
}

fn same(got: &Value, want: &Value) -> bool {
    match (got, want) {
        (Value::Number(a), Value::Number(b)) => {
            if let (Some(x), Some(y)) = (a.as_i64(), b.as_i64()) {
                return x == y;
            }
            if let (Some(x), Some(y)) = (a.as_u64(), b.as_u64()) {
                return x == y;
            }
            match (a.as_f64(), b.as_f64()) {
                (Some(x), Some(y)) => norm(x).to_bits() == norm(y).to_bits(),
                _ => false,
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, x)| b.get(k).is_some_and(|y| same(x, y)))
        }
        _ => got == want,
    }
}

/// Runs every function in a module's file; `run` gives the Rust answer for a
/// function name and an input, or None for a name it doesn't know.
fn check_module(module: &str, run: impl Fn(&str, &Value) -> Option<Value>) {
    let file = vectors::load(module);
    let functions = file["functions"].as_object().expect("a functions object");
    assert!(!functions.is_empty(), "{module}: no functions");
    let mut failures = vec![];
    let mut total = 0;
    for (name, cases) in functions {
        let cases = cases.as_array().expect("an array of cases");
        assert!(!cases.is_empty(), "{module}.{name}: no cases");
        for (i, case) in cases.iter().enumerate() {
            total += 1;
            let input = &case["input"];
            let want = &case["output"];
            let got = run(name, input)
                .unwrap_or_else(|| panic!("{module}.{name}: the Rust has no such function"));
            if !same(&got, want) && failures.len() < 12 {
                failures.push(format!(
                    "{module}.{name} #{i}\n  input: {input}\n  want:  {want}\n  got:   {got}"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {total} cases differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
    eprintln!("{module}: {total} cases agree");
}

// --- Reading an input -------------------------------------------------------------

fn arg<T: DeserializeOwned>(input: &Value, key: &str) -> T {
    serde_json::from_value(input[key].clone())
        .unwrap_or_else(|e| panic!("input {key}: {e}: {}", input[key]))
}

fn num(input: &Value, key: &str) -> f64 {
    input[key]
        .as_f64()
        .unwrap_or_else(|| panic!("input {key} is not a number: {}", input[key]))
}

fn text<'a>(input: &'a Value, key: &str) -> &'a str {
    input[key]
        .as_str()
        .unwrap_or_else(|| panic!("input {key} is not a string: {}", input[key]))
}

fn tz(input: &Value) -> jiff::tz::TimeZone {
    zone::zone(text(input, "tz")).unwrap_or_else(|| panic!("jiff doesn't know {}", input["tz"]))
}

fn out<T: Serialize>(v: T) -> Value {
    serde_json::to_value(v).expect("serializable")
}

// --- js -------------------------------------------------------------------------------

#[test]
fn js_vectors() {
    check_module("js", |name, i| {
        Some(match name {
            "round" => out(js::round(num(i, "x"))),
            "max2" => out(js::max2(num(i, "a"), num(i, "b"))),
            "min2" => out(js::min2(num(i, "a"), num(i, "b"))),
            "numToString" => out(js::num_to_string(num(i, "x"))),
            "toNumber" => out(js::to_number(text(i, "s"))),
            "trim" => out(js::trim(text(i, "s"))),
            "normaliseName" => out(js::normalise_name(text(i, "s"))),
            "cmp" => out(js::cmp(text(i, "a"), text(i, "b")) as i32),
            "utf16Len" => out(js::utf16_len(text(i, "s"))),
            "localeCompare" => out(js::locale_compare(text(i, "a"), text(i, "b")) as i32),
            "dateUtc" => {
                let a: Vec<f64> = arg(i, "args");
                out(js::date_utc(a[0], a[1], a[2], a[3], a[4], a[5]))
            }
            "parseIsoInstant" => out(js::parse_iso_instant(text(i, "s"))),
            _ => return None,
        })
    });
}

// --- zone and format ---------------------------------------------------------------------

#[test]
fn zone_vectors() {
    check_module("zone", |name, i| {
        Some(match name {
            "wallTime" => out(zone::wall_time(num(i, "ms"), &tz(i))),
            "epochOf" => out(zone::epoch_of(&arg(i, "w"), &tz(i))),
            "keyOf" => out(zone::key_of(num(i, "year"), num(i, "month"), num(i, "day"))),
            "keyParts" => {
                let (year, month, day) = zone::key_parts(text(i, "key"));
                json!({"year": year, "month": month, "day": day})
            }
            "dayKey" => out(zone::day_key(num(i, "ms"), &tz(i))),
            "minuteOfDay" => out(zone::minute_of_day(num(i, "ms"), &tz(i))),
            "addDays" => out(zone::add_days(text(i, "key"), num(i, "n"))),
            "daysBetween" => out(zone::days_between(text(i, "a"), text(i, "b"))),
            "weekdayOf" => out(zone::weekday_of(text(i, "key"))),
            "daysInMonth" => out(zone::days_in_month(num(i, "year"), num(i, "month"))),
            "atMinute" => out(zone::at_minute(text(i, "key"), num(i, "minutes"), &tz(i))),
            "startOfDay" => out(zone::start_of_day(text(i, "key"), &tz(i))),
            "zoneStarts" => {
                let z = tz(i);
                let key = text(i, "key");
                json!({
                    "start": zone::start_of_day(key, &z),
                    "next": zone::start_of_day(&zone::add_days(key, 1.0), &z),
                })
            }
            _ => return None,
        })
    });
}

#[test]
fn format_vectors() {
    check_module("format", |name, i| {
        Some(match name {
            "clock" => out(format::clock(num(i, "minutes"))),
            "clockAt" => out(format::clock_at(num(i, "ms"), &tz(i))),
            "monthDay" => out(format::month_day(text(i, "key"))),
            "shortMonthDay" => out(format::short_month_day(text(i, "key"))),
            "countdown" => out(format::countdown(num(i, "ms"))),
            _ => return None,
        })
    });
}

// --- heat ------------------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct Item {
    id: String,
    #[serde(flatten)]
    fields: heat::HeatFields,
}

impl heat::HeatInput for Item {
    fn is_done(&self) -> bool {
        self.fields.done
    }
    fn due(&self) -> Option<f64> {
        self.fields.due
    }
    fn difficulty(&self) -> f64 {
        self.fields.difficulty
    }
}

#[test]
fn heat_vectors() {
    check_module("heat", |name, i| {
        Some(match name {
            "runway" => out(heat::runway(num(i, "difficulty"))),
            "heatThresholds" => out(heat::heat_thresholds(num(i, "difficulty"))),
            "tubeFill" => out(heat::tube_fill(&arg(i, "h"))),
            "heatOf" => out(heat::heat_of(
                &arg::<heat::HeatFields>(i, "t"),
                num(i, "now"),
            )),
            "byHeat" => {
                let items: Vec<Item> = arg(i, "items");
                out(heat::by_heat(&items, num(i, "now"))
                    .iter()
                    .map(|t| t.id.clone())
                    .collect::<Vec<_>>())
            }
            "duePhrase" => out(heat::due_phrase(num(i, "due"), num(i, "now"), &tz(i))),
            "nextHeatChange" => {
                let items: Vec<heat::HeatFields> = arg(i, "items");
                out(heat::next_heat_change(&items, num(i, "now")))
            }
            _ => return None,
        })
    });
}

// --- estimate ----------------------------------------------------------------------------------

#[test]
fn estimate_vectors() {
    check_module("estimate", |name, i| {
        Some(match name {
            "formatMinutes" => out(estimate::format_minutes(num(i, "minutes"))),
            "actualMin" => {
                let (task, sessions): (Task, Vec<FocusSession>) =
                    (arg(i, "task"), arg(i, "sessions"));
                out(estimate::actual_min(&task, &sessions))
            }
            "estimateContext" => {
                let (tasks, sessions): (Vec<Task>, Vec<FocusSession>) =
                    (arg(i, "tasks"), arg(i, "sessions"));
                let ctx = estimate::estimate_context(&tasks, &sessions);
                let averages: serde_json::Map<String, Value> = ctx
                    .averages
                    .iter()
                    .map(|(k, v)| (k.clone(), out(v)))
                    .collect();
                let children: serde_json::Map<String, Value> = ctx
                    .children
                    .iter()
                    .map(|(k, v)| {
                        (
                            (*k).to_string(),
                            out(v.iter().map(|t| t.id.clone()).collect::<Vec<_>>()),
                        )
                    })
                    .collect();
                json!({"averages": averages, "children": children})
            }
            "estimateMin" => {
                let (tasks, sessions): (Vec<Task>, Vec<FocusSession>) =
                    (arg(i, "tasks"), arg(i, "sessions"));
                let ctx = estimate::estimate_context(&tasks, &sessions);
                let id = text(i, "taskId");
                out(estimate::estimate_min(
                    tasks.iter().find(|t| t.id == id).expect("the task"),
                    &ctx,
                ))
            }
            "averageLines" => {
                let (tasks, sessions): (Vec<Task>, Vec<FocusSession>) =
                    (arg(i, "tasks"), arg(i, "sessions"));
                let ctx = estimate::estimate_context(&tasks, &sessions);
                out(estimate::average_lines(&arg::<Space>(i, "space"), &ctx))
            }
            "weeklyLoad" => {
                let (tasks, sessions): (Vec<Task>, Vec<FocusSession>) =
                    (arg(i, "tasks"), arg(i, "sessions"));
                let ctx = estimate::estimate_context(&tasks, &sessions);
                out(estimate::weekly_load(&tasks, &ctx, num(i, "now")))
            }
            "weeklyLoadLine" => out(estimate::weekly_load_line(&arg(i, "load"))),
            _ => return None,
        })
    });
}
