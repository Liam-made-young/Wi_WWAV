//! Gate 2.4's eyes, checked from design/tokens.json alone (docs/SPEC.md 8.9,
//! 8.11): every pair that carries text meets its bar in light and dark, and
//! every ratio the spec quotes comes out of the file's colours.
//!
//! Fails if: a body pair is under 7:1 or a secondary pair under 4.5:1 on any
//! colour of its ground in either appearance; a quoted ratio is more than 0.1
//! from the measured one; the night's text steps and its pairs disagree.

mod common;

use common::{colors, json, tokens, Token, APPEARANCES};
use serde_json::Value;
use wwav_tokens::contrast;

struct Pair {
    ink: String,
    alpha: f32,
    on: Vec<String>,
    over: Option<Vec<usize>>,
    role: String,
    quoted: Value,
}

fn pairs() -> Vec<Pair> {
    let known = ["ink", "alpha", "on", "over", "role", "quoted", "why"];
    json()["$contrast"]["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let o = p.as_object().unwrap();
            for key in o.keys() {
                assert!(known.contains(&key.as_str()), "unknown key {key} in {p}");
            }
            let on = match &p["on"] {
                Value::String(s) => vec![s.clone()],
                Value::Array(a) => a.iter().map(|s| s.as_str().unwrap().into()).collect(),
                other => panic!("on: {other}"),
            };
            let role = p["role"].as_str().unwrap().to_string();
            assert!(
                ["body", "secondary", "mark"].contains(&role.as_str()),
                "role {role}"
            );
            Pair {
                ink: p["ink"].as_str().unwrap().into(),
                alpha: p["alpha"].as_f64().unwrap_or(1.0) as f32,
                on,
                over: p["over"]
                    .as_array()
                    .map(|a| a.iter().map(|i| i.as_u64().unwrap() as usize).collect()),
                role,
                quoted: p["quoted"].clone(),
            }
        })
        .collect()
}

fn name(p: &Pair) -> String {
    let alpha = if p.alpha < 1.0 {
        format!(" at {}%", (p.alpha * 100.0).round())
    } else {
        String::new()
    };
    format!("{}{alpha} on {}", p.ink, p.on.join(" / "))
}

/// The lowest ratio of the ink against every colour of its ground.
fn measure(tokens: &[Token], p: &Pair, appearance: &str) -> f64 {
    let inks = colors(tokens, &p.ink, appearance);
    assert_eq!(inks.len(), 1, "{} is one colour", p.ink);
    let mut ink = inks[0];
    ink.a *= p.alpha;
    let mut grounds = Vec::new();
    for g in &p.on {
        let stops = colors(tokens, g, appearance);
        match &p.over {
            Some(over) => grounds.extend(over.iter().map(|&i| {
                *stops
                    .get(i)
                    .unwrap_or_else(|| panic!("{g} has no stop {i} in {appearance}"))
            })),
            None => grounds.extend(stops),
        }
    }
    grounds
        .iter()
        .map(|&g| contrast(ink, g))
        .fold(f64::INFINITY, f64::min)
}

fn bar(role: &str) -> Option<f64> {
    match role {
        "body" => Some(7.0),
        "secondary" => Some(4.5),
        _ => None,
    }
}

#[test]
fn every_text_pair_meets_its_bar_in_light_and_dark() {
    let tokens = tokens();
    let mut misses = Vec::new();
    let mut checked = 0;
    for p in pairs() {
        let Some(bar) = bar(&p.role) else { continue };
        for appearance in APPEARANCES {
            let ratio = measure(&tokens, &p, appearance);
            checked += 1;
            if ratio < bar {
                misses.push(format!(
                    "{} ({appearance}, {}): {ratio:.2}:1, under {bar}:1",
                    name(&p),
                    p.role
                ));
            }
        }
    }
    // 25 text pairs, each in two appearances; a pair the parse skipped shows here.
    assert_eq!(checked, 50, "text pairs checked");
    assert!(misses.is_empty(), "under the bar:\n{}", misses.join("\n"));
}

#[test]
fn every_ratio_the_spec_quotes_is_reproduced_within_a_tenth() {
    let tokens = tokens();
    let mut wrong = Vec::new();
    let mut checked = 0;
    for p in pairs() {
        for appearance in APPEARANCES {
            let Some(quoted) = p.quoted.get(appearance).and_then(Value::as_f64) else {
                continue;
            };
            let ratio = measure(&tokens, &p, appearance);
            checked += 1;
            if (ratio - quoted).abs() > 0.1 {
                wrong.push(format!(
                    "{} ({appearance}): the spec says {quoted}, the colours give {ratio:.2}",
                    name(&p)
                ));
            }
        }
    }
    // Every quote in the file, once for each appearance it's quoted for.
    assert_eq!(checked, 51, "quoted ratios checked");
    assert!(
        wrong.is_empty(),
        "quoted ratios that are off:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn the_nights_text_steps_are_its_text_pairs() {
    // 8.11: textSteps names which ink opacities carry text, and how.
    let json = json();
    let pairs = pairs();
    for (role, steps) in json["night"]["textSteps"].as_object().unwrap() {
        for step in steps.as_array().unwrap() {
            let step = step.as_f64().unwrap() as f32;
            assert!(
                pairs.iter().any(|p| p.ink == "night.ink"
                    && (p.alpha - step).abs() < 1e-6
                    && &p.role == role),
                "no {role} pair for night ink at {step}"
            );
        }
    }
    for p in pairs.iter().filter(|p| p.ink == "night.ink") {
        let steps = json["night"]["textSteps"][&p.role].as_array().unwrap();
        assert!(
            steps
                .iter()
                .any(|s| (s.as_f64().unwrap() as f32 - p.alpha).abs() < 1e-6),
            "night ink at {} is a {} pair but not a text step",
            p.alpha,
            p.role
        );
    }
}

#[test]
fn the_check_catches_the_colours_the_spec_fixed() {
    // 8.2's old values must fail where the new ones pass, or the check is blind.
    use wwav_tokens::Color;
    let lcd_bottom = Color::rgb(0xdf, 0xe3, 0xc6);
    assert!(contrast(Color::rgb(0x6b, 0x70, 0x50), lcd_bottom) < 4.5);
    assert!(contrast(Color::rgb(0x4b, 0x50, 0x34), lcd_bottom) >= 4.5);
    let white = Color::rgb(0xff, 0xff, 0xff);
    assert!(contrast(Color::rgb(0x7a, 0x7a, 0x7a), white) < 4.5);
    assert!(contrast(white, Color::rgb(0x38, 0x75, 0xd7)) < 4.5);
    // 35% night ink and below are fills, never text (8.2).
    let mut ink = Color::rgb(0xF4, 0xEF, 0xE6);
    ink.a = 0.35;
    assert!(contrast(ink, Color::rgb(0x07, 0x0A, 0x18)) < 4.5);
}

#[test]
fn no_key_colour_is_a_ground_for_text() {
    // 8.3: "in E minor and A major neither ink reaches 4.5:1".
    use wwav_tokens::{key_color, parse_key};
    let tokens = tokens();
    let inks = [
        colors(&tokens, "night.ink", "light")[0],
        colors(&tokens, "desk.ink", "light")[0],
    ];
    for key in ["E minor", "A major"] {
        let fill = key_color(parse_key(key));
        for ink in inks {
            let ratio = contrast(ink, fill);
            assert!(ratio < 4.5, "{key}: {ratio:.2}:1");
        }
    }
}
