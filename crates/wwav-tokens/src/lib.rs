//! The design tokens on the Rust side, for the wgpu compositor's title cards
//! and scopes (`docs/SPEC.md` 8.11).
//!
//! Every value lives in `design/tokens.json`. `tools/tokens/compile.mjs`
//! writes it out as `generated.rs` (re-exported here, one module per group:
//! `desk::INK`, `motion::BEAT_MS`), as the web UI's CSS and as the engine's
//! C++ header, so no token can drift between processes. This file holds what
//! isn't a value: the types, a work's key colour (8.3) and WCAG 2's contrast
//! ratio, which the tests use to hold every text pair to gate 2.4's bars.

#[rustfmt::skip]
mod generated;
pub use generated::*;

/// An sRGB colour. `a` is 1 except for the few translucent tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

/// A token with a value for each appearance: Heat's light and dark tokens,
/// or the shop at dusk and after hours (8.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Themed<T> {
    pub light: T,
    pub dark: T,
}

/// A CSS `cubic-bezier()` timing curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

/// Any one token, so [`ALL`] can list them by their path in the file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Token {
    Color(Color),
    ThemedColor(Themed<Color>),
    /// A gradient's stops, top (or centre) first, or a set like the lamps.
    Colors(&'static [Color]),
    ThemedColors(Themed<&'static [Color]>),
    Number(f32),
    Numbers(&'static [f32]),
    Names(&'static [&'static str]),
    Curve(Curve),
}

// ---- Contrast -----------------------------------------------------------------

fn linear(channel: f64) -> f64 {
    let c = channel / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance([r, g, b]: [f64; 3]) -> f64 {
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/// WCAG 2's contrast ratio of an ink on a ground, from 1 to 21. A translucent
/// ink (the night's 70% and 50%) is laid over the ground first, as the screen
/// shows it; the ground is taken as opaque.
pub fn contrast(ink: Color, ground: Color) -> f64 {
    let a = f64::from(ink.a);
    let g = [ground.r, ground.g, ground.b].map(f64::from);
    let i = [ink.r, ink.g, ink.b].map(f64::from);
    let shown = [0, 1, 2].map(|k| i[k] * a + g[k] * (1.0 - a));
    let (x, y) = (luminance(shown), luminance(g));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

// ---- Key colour ---------------------------------------------------------------

/// A musical key: its tonic as a pitch class with A = 0, as v4's analysis
/// counts them, and its mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub pc: u8,
    pub minor: bool,
}

/// Reads a key the way `.wwav` writes it (6.1's `wmet.key`): a note A to G
/// with at most one sharp (`#`, `♯`) or flat (`b`, `♭`), then `major` or
/// `minor`, as in "A minor", "F# major", "Bb minor". Anything else is `None`,
/// a key not known yet.
pub fn parse_key(name: &str) -> Option<Key> {
    let mut words = name.split_whitespace();
    let (note, mode) = (words.next()?, words.next()?);
    if words.next().is_some() {
        return None;
    }
    let mut chars = note.chars();
    let natural: i32 = match chars.next()?.to_ascii_uppercase() {
        'A' => 0,
        'B' => 2,
        'C' => 3,
        'D' => 5,
        'E' => 7,
        'F' => 8,
        'G' => 10,
        _ => return None,
    };
    let shift = match chars.as_str() {
        "" => 0,
        "#" | "♯" => 1,
        "b" | "♭" => -1,
        _ => return None,
    };
    let minor = if mode.eq_ignore_ascii_case("minor") {
        true
    } else if mode.eq_ignore_ascii_case("major") {
        false
    } else {
        return None;
    };
    Some(Key {
        pc: (natural + shift).rem_euclid(12) as u8,
        minor,
    })
}

/// The hue in degrees: ((pc · 7) mod 12) · 30, which walks the circle of
/// fifths, so a key a fifth away is the neighbouring shade. An unknown key
/// takes night indigo, the sky's own hue: "still condensing".
pub fn key_hue(key: Option<Key>) -> f32 {
    match key {
        Some(k) => f32::from((k.pc * 7) % 12) * 30.0,
        None => key::UNKNOWN_HUE,
    }
}

// An unknown key wears the minor look, as in v4.
fn mode(key: Option<Key>) -> (f32, f32) {
    if key.map_or(true, |k| k.minor) {
        (key::minor::SATURATION, key::minor::LIGHTNESS)
    } else {
        (key::major::SATURATION, key::major::LIGHTNESS)
    }
}

/// A work's colour (8.3): major hsl(h, 72%, 58%), minor hsl(h, 58%, 42%). It
/// is a fill, never a ground for text.
pub fn key_color(key: Option<Key>) -> Color {
    let (s, l) = mode(key);
    hsl(key_hue(key), s, l)
}

/// The tone an orb's rim and the sky's glow take (8.2): the key's hue and
/// saturation at 65% lightness, as v4's `glowRgb`. It holds 4:1 on the night
/// in every key.
pub fn glow_tone(key: Option<Key>) -> Color {
    let (s, _) = mode(key);
    hsl(key_hue(key), s, key::GLOW_LIGHTNESS)
}

/// hsl(h, s%, l%) worked out as v4's `hslToRgb` does, rounded to 8 bits, so a
/// planet is the same colour in the app and on the web.
fn hsl(h: f32, s: f32, l: f32) -> Color {
    let (h, s, l) = (f64::from(h), f64::from(s) / 100.0, f64::from(l) / 100.0);
    let a = s * l.min(1.0 - l);
    let channel = |n: f64| {
        let k = (n + h / 30.0) % 12.0;
        let v = l - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0);
        (v * 255.0).round() as u8
    };
    Color::rgb(channel(0.0), channel(8.0), channel(4.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Color = Color::rgb(255, 255, 255);
    const BLACK: Color = Color::rgb(0, 0, 0);

    #[test]
    fn contrast_runs_from_one_to_twenty_one_either_way_round() {
        assert_eq!(contrast(BLACK, WHITE), 21.0);
        assert_eq!(contrast(WHITE, BLACK), 21.0);
        assert_eq!(contrast(WHITE, WHITE), 1.0);
        let navy = Color::rgb(0x1B, 0x4C, 0x8C);
        assert_eq!(contrast(WHITE, navy), contrast(navy, WHITE));
    }

    #[test]
    fn contrast_reproduces_known_ratios() {
        // WCAG's own example of the AA line: #767676 on white is 4.54:1.
        let grey = Color::rgb(0x76, 0x76, 0x76);
        assert!((contrast(grey, WHITE) - 4.54).abs() < 0.005);
        // 8.2: white on the deep Aqua selection is 8.5:1.
        let r = contrast(WHITE, Color::rgb(0x1B, 0x4C, 0x8C));
        assert!((r - 8.54).abs() < 0.005, "{r}");
    }

    #[test]
    fn a_translucent_ink_is_laid_over_its_ground() {
        let half = Color::rgba(255, 255, 255, 0.5);
        let mid = contrast(half, BLACK);
        assert!(mid > 1.0 && mid < 21.0);
        // Fully transparent ink is the ground itself.
        assert_eq!(contrast(Color::rgba(255, 255, 255, 0.0), BLACK), 1.0);
    }
}
