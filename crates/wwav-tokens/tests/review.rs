//! An adversarial review of design/tokens.json against docs/SPEC.md 8 (F7).
//!
//! Each test here failed when it was written, because of a finding in the
//! review of build/tokens. Those whose finding is fixed run with the suite.
//! The rest are #[ignore]d with their finding or question until the founder
//! settles it; run them with `cargo test -p wwav-tokens --test review --
//! --ignored`. Each says which finding it shows.

mod common;

use common::{colors, json, tokens, APPEARANCES};
use wwav_tokens::{contrast, glow_tone, parse_key, Color};

const NIGHT_INK_50: f32 = 0.5;

/// Finding: the `over` rule hides two text pairs that miss their bar.
///
/// tokens.json's own $contrast doc says a pair is judged "on every colour of
/// its ground", and then narrows two pairs with `over`, a placement the spec
/// never states. MI-WWAV-OS draws deck metal as one vertical gradient with
/// stops at 0, 0.55 and 1 (theme.dart BrushedPanel), and a mixer strip
/// (5.8) carries M, S, arm, the fader scale and its name plate in its lower
/// half, where the soft ink #5D6975 falls to 3.6:1. The dark LCD's dim ink
/// is 4.3:1 on its top stop.
#[test]
fn every_text_pair_holds_on_every_stop_of_its_ground() {
    let tokens = tokens();
    let mut misses = Vec::new();
    for p in json()["$contrast"]["pairs"].as_array().unwrap() {
        let bar = match p["role"].as_str().unwrap() {
            "body" => 7.0,
            "secondary" => 4.5,
            _ => continue,
        };
        let on: Vec<String> = match &p["on"] {
            serde_json::Value::String(s) => vec![s.clone()],
            serde_json::Value::Array(a) => {
                a.iter().map(|s| s.as_str().unwrap().to_string()).collect()
            }
            other => panic!("on: {other}"),
        };
        let alpha = p["alpha"].as_f64().unwrap_or(1.0) as f32;
        let ink_name = p["ink"].as_str().unwrap();
        for appearance in APPEARANCES {
            let mut ink = colors(&tokens, ink_name, appearance)[0];
            ink.a *= alpha;
            for g in &on {
                for (i, stop) in colors(&tokens, g, appearance).into_iter().enumerate() {
                    let r = contrast(ink, stop);
                    if r < bar {
                        misses.push(format!("{ink_name} on {g}[{i}] ({appearance}): {r:.2}:1"));
                    }
                }
            }
        }
    }
    assert!(misses.is_empty(), "under the bar:\n{}", misses.join("\n"));
}

/// Finding: desk ink on case metal is marked secondary, but 8.2 puts sheets
/// and the inspector on case metal, and 8.9 makes running text and list rows
/// body text. The inspector's version list ("v3 · Oct 6 · new bridge", 5.17)
/// and a sheet's hint ("This trains your time averages…", 3.1) are body text,
/// and in dark they sit at 5.6:1 on the top stop.
#[test]
#[ignore = "finding: desk ink on dark case metal is 5.6:1, under 7:1 for body text"]
fn desk_ink_on_case_metal_holds_body_text_in_dark() {
    let tokens = tokens();
    let ink = colors(&tokens, "desk.ink", "dark")[0];
    for (i, stop) in colors(&tokens, "desk.caseMetal", "dark")
        .into_iter()
        .enumerate()
    {
        let r = contrast(ink, stop);
        assert!(r >= 7.0, "desk.ink on caseMetal[{i}] (dark): {r:.2}:1");
    }
}

/// Finding: clay is a night ground (8.11 lists night.ground · deep · clay;
/// v3's NightPalette calls it "raised indigo", the card and button surface),
/// but no pair puts the night's text on it. 50% ink on clay is 4.46:1, under
/// the secondary bar.
#[test]
#[ignore = "finding: night ink at 50% on night.clay is 4.46:1, and no pair checks it"]
fn night_secondary_ink_holds_on_clay() {
    let tokens = tokens();
    let mut ink = colors(&tokens, "night.ink", "light")[0];
    ink.a = NIGHT_INK_50;
    let clay = colors(&tokens, "night.clay", "light")[0];
    let r = contrast(ink, clay);
    assert!(r >= 4.5, "night ink 50% on clay: {r:.2}:1");
}

/// Finding: two of 8.2's five fixes aren't in the file. "The dark gel's top
/// stop goes from #6a6d71 to #5e6165" has no token, and "Blue gel buttons take
/// an ink label … the darkest stop under it is 4.6:1" has no pair, so neither
/// can be checked from the file alone.
#[test]
fn every_fix_in_8_2_is_in_the_file() {
    let text = std::fs::read_to_string(common::repo().join("design/tokens.json"))
        .unwrap()
        .to_lowercase();
    assert!(
        text.contains("#5e6165"),
        "no token holds the dark gel's top stop #5e6165"
    );
    let quotes_4_6 = json()["$contrast"]["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["quoted"]["light"].as_f64() == Some(4.6))
        .map(|p| p["ink"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(
        quotes_4_6.iter().any(|ink| ink != "desk.deckInkSoft"),
        "no pair quotes the blue gel's ink label at 4.6:1 (only {quotes_4_6:?})"
    );
}

/// Question for the spec (8.2): "That tone holds at least 4:1 against the
/// night … so no planet disappears in F minor." The builder's test holds it
/// against the flat `#070A18`, where it does (F major 4.28:1). But 8.2's night
/// is "`#070A18` under a key-tinted starfield", and "the starfield is v4's":
/// StarfieldCanvas.jsx `paletteFrom(glowRgb)` bands the sky with the glow
/// itself, its brightest (mid) band at 0.2 x glow (the luminance-52 cap never
/// binds, since 0.2 x 255 = 51), drawn truncated to whole channels. Against
/// that band F major's tone is 3.90:1. Behind the planet itself v4 also lays
/// the breathing core (glow lifted 16%, at alpha 0.32), which takes it lower.
#[test]
#[ignore = "question: against v4's key-tinted sky (its mid band) F major's glow tone is 3.90:1"]
fn the_glow_tone_holds_four_to_one_on_the_key_tinted_sky() {
    let mut names = vec!["unknown".to_string()];
    for pc in [
        "A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#",
    ] {
        names.extend(["major", "minor"].map(|mode| format!("{pc} {mode}")));
    }
    let mut under = Vec::new();
    for name in names {
        let glow = glow_tone(parse_key(&name));
        let band = |c: u8| (f64::from(c) * 0.2) as u8;
        let sky = Color::rgb(band(glow.r), band(glow.g), band(glow.b));
        let r = contrast(glow, sky);
        if r < 4.0 {
            under.push(format!("{name}: {r:.2}:1 on {sky:?}"));
        }
    }
    assert!(
        under.is_empty(),
        "under 4:1 on the sky's mid band:\n{}",
        under.join("\n")
    );
}

/// Finding: `Key`'s fields are public, so any u8 is a pitch class, and
/// `key_hue` multiplies it by 7 in u8. From pc 37 up that overflows: a panic
/// in a debug build and a wrong hue in release, where keyColor.ts gives the
/// pitch class mod 12 (pc 40 is 120°, as pc 4 is).
#[test]
fn key_hue_takes_any_pitch_class_mod_twelve() {
    use wwav_tokens::{key_hue, Key};
    for pc in [37u8, 40, 255] {
        let wrapped = Key {
            pc: pc % 12,
            minor: false,
        };
        assert_eq!(
            key_hue(Some(Key { pc, minor: false })),
            key_hue(Some(wrapped)),
            "pc {pc}"
        );
    }
}
