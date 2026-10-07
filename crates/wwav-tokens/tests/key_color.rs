//! A work's colour is its key colour (docs/SPEC.md 8.3; v4's keyColor.js).
//!
//! Fails if: any of the 24 keys or "unknown" gives a colour other than v4's;
//! a sharp and its flat, or "#" and "♯", name different keys; anything that
//! isn't "<note> major|minor" parses; any glow tone is under 4:1 on the night
//! (8.2: "so no planet disappears in F minor").

mod common;

use common::{colors, tokens};
use wwav_tokens::{contrast, glow_tone, key_color, parse_key, Color, Key};

/// v4's colorsForKey() for every key: planet body, glow. Worked out with v4's
/// own hslToRgb and Math.round, so the app and the web sky agree.
const V4: [(&str, &str, &str); 25] = [
    ("A major", "#e14747", "#e66565"),
    ("A♯ major", "#4794e1", "#65a6e6"),
    ("B major", "#e1e147", "#e6e665"),
    ("C major", "#9447e1", "#a665e6"),
    ("C♯ major", "#47e147", "#65e665"),
    ("D major", "#e14794", "#e665a6"),
    ("D♯ major", "#47e1e1", "#65e6e6"),
    ("E major", "#e19447", "#e6a665"),
    ("F major", "#4747e1", "#6565e6"),
    ("F♯ major", "#94e147", "#a6e665"),
    ("G major", "#e147e1", "#e665e6"),
    ("G♯ major", "#47e194", "#65e6a6"),
    ("A minor", "#a92d2d", "#da7272"),
    ("A♯ minor", "#2d6ba9", "#72a6da"),
    ("B minor", "#a9a92d", "#dada72"),
    ("C minor", "#6b2da9", "#a672da"),
    ("C♯ minor", "#2da92d", "#72da72"),
    ("D minor", "#a92d6b", "#da72a6"),
    ("D♯ minor", "#2da9a9", "#72dada"),
    ("E minor", "#a96b2d", "#daa672"),
    ("F minor", "#2d2da9", "#7272da"),
    ("F♯ minor", "#6ba92d", "#a6da72"),
    ("G minor", "#a92da9", "#da72da"),
    ("G♯ minor", "#2da96b", "#72daa6"),
    ("unknown", "#2d3ea9", "#7280da"),
];

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

#[test]
fn every_key_and_unknown_match_v4() {
    for (name, body, glow) in V4 {
        let key = parse_key(name);
        assert_eq!(key.is_none(), name == "unknown", "{name}");
        assert_eq!(hex(key_color(key)), body, "{name} body");
        assert_eq!(hex(glow_tone(key)), glow, "{name} glow");
    }
}

#[test]
fn hue_walks_the_circle_of_fifths_from_a() {
    let hue = |name| wwav_tokens::key_hue(parse_key(name));
    assert_eq!(hue("A minor"), 0.0);
    assert_eq!(hue("E major"), 30.0); // a fifth up is the next hue
    assert_eq!(hue("B major"), 60.0);
    assert_eq!(hue("D major"), 330.0); // a fifth down is the previous one
    assert_eq!(hue("C major"), 270.0);
    assert_eq!(wwav_tokens::key_hue(None), 232.0); // night indigo
}

#[test]
fn key_names_parse_with_sharps_flats_and_either_case() {
    let k = |pc, minor| Some(Key { pc, minor });
    assert_eq!(parse_key("A minor"), k(0, true));
    assert_eq!(parse_key("F# major"), k(9, false));
    assert_eq!(parse_key("F♯ major"), k(9, false));
    assert_eq!(parse_key("Gb major"), k(9, false));
    assert_eq!(parse_key("G♭ major"), k(9, false));
    assert_eq!(parse_key("Bb minor"), k(1, true));
    assert_eq!(parse_key("A# minor"), k(1, true));
    assert_eq!(parse_key("Ab minor"), k(11, true)); // wraps below A
    assert_eq!(parse_key("Cb major"), k(2, false)); // is B
    assert_eq!(parse_key("E# minor"), k(8, true)); // is F
    assert_eq!(parse_key("c minor"), k(3, true));
    assert_eq!(parse_key("C MAJOR"), k(3, false));
    assert_eq!(parse_key("  D   minor "), k(5, true));
}

#[test]
fn anything_else_is_unknown() {
    for name in [
        "",
        "C",
        "minor",
        "H major",
        "C## major",
        "Cbb minor",
        "C dorian",
        "Am",
        "C major 7",
        "♯C major",
        "C♯♭ major",
    ] {
        assert_eq!(parse_key(name), None, "{name:?}");
    }
}

#[test]
fn every_glow_tone_holds_four_to_one_on_the_night() {
    // 8.2: an orb's rim "holds at least 4:1 against the night in all 24 keys
    // and in indigo". v4 derives the tone as hsl(hue, the mode's saturation,
    // 65%): keyColor.js glowRgb.
    let ground = colors(&tokens(), "night.ground", "dark")[0];
    let mut lowest = (f64::INFINITY, "");
    for (name, _, _) in V4 {
        let ratio = contrast(glow_tone(parse_key(name)), ground);
        if ratio < lowest.0 {
            lowest = (ratio, name);
        }
    }
    assert!(lowest.0 >= 4.0, "{} is {:.2}:1", lowest.1, lowest.0);
    // The closest is F major's violet-blue, at 4.28:1.
    assert_eq!(lowest.1, "F major");
    assert!((lowest.0 - 4.28).abs() < 0.01, "{:.3}", lowest.0);
}
