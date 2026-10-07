//! The bits of JavaScript the TypeScript model leans on, as Rust functions.
//!
//! The model is a port, so it has to give the TypeScript's answer for every
//! input. Most of the places where Rust and JavaScript disagree are small:
//!
//! - `Math.round` rounds ties toward +Infinity, where `f64::round` rounds them
//!   away from zero ([`round`]).
//! - `Math.min` and `Math.max` return NaN if either side is NaN, and know that
//!   -0 is below +0; `f64::min` and `f64::max` skip a NaN ([`min2`], [`max2`]).
//! - `String(n)` prints a number its own way: no `.0`, exponents past 1e21
//!   ([`num_to_string`]); `Number(text)` reads text its own way ([`to_number`]).
//! - `trim` and `\s` use the ECMAScript space set, which has U+FEFF and not
//!   U+0085 ([`is_space`], [`trim`]).
//! - `<`, `>` and `sort()` compare strings by UTF-16 code unit ([`cmp`]), and
//!   `.length` counts them ([`utf16_len`]).
//! - `Date.UTC` rolls days and months over and reads years 0 to 99 as 19xx
//!   ([`date_utc`]); `Date.parse` is V8's ([`parse_iso_instant`]).
//! - `Array.prototype.sort` is stable ([`sort_by`], which also never panics
//!   on a comparator that isn't a total order, as `slice::sort_by` may).
//!
//! Numbers are `f64` throughout the model for the same reason: the TypeScript
//! has one number type, and a record may hold a fraction or a NaN.

use std::cmp::Ordering;

/// `Math.round`: the nearest integer, a tie going toward +Infinity.
pub fn round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    if r == 0.0 && x.is_sign_negative() {
        -0.0
    } else {
        r
    }
}

/// `Math.max(a, b)`.
pub fn max2(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.is_sign_negative() { b } else { a };
    }
    if a > b {
        a
    } else {
        b
    }
}

/// `Math.min(a, b)`.
pub fn min2(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == 0.0 && b == 0.0 {
        return if a.is_sign_negative() { a } else { b };
    }
    if a < b {
        a
    } else {
        b
    }
}

/// `Math.max(...xs)`: -Infinity for none.
pub fn max_of(xs: impl IntoIterator<Item = f64>) -> f64 {
    xs.into_iter().fold(f64::NEG_INFINITY, max2)
}

/// `Number.isInteger`.
pub fn is_integer(x: f64) -> bool {
    x.is_finite() && x.trunc() == x
}

/// `String(x)` for a number.
pub fn num_to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let sign = if x < 0.0 { "-" } else { "" };
    // Rust's `{:e}` prints the shortest digits that read back as the same
    // number, which is the digit string JavaScript's rule starts from.
    let sci = format!("{:e}", x.abs());
    let (mantissa, exp) = sci
        .split_once('e')
        .expect("LowerExp always has an exponent");
    let exp: i32 = exp.parse().expect("the exponent is an integer");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let e = format!("{}{}", if e < 0 { '-' } else { '+' }, e.abs());
        if k == 1 {
            format!("{digits}e{e}")
        } else {
            format!("{}.{}e{e}", &digits[..1], &digits[1..])
        }
    };
    format!("{sign}{body}")
}

/// Whether a character is ECMAScript white space or a line terminator: what
/// `String.prototype.trim` strips and `\s` matches.
pub fn is_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}'
            | '\u{A}'
            | '\u{B}'
            | '\u{C}'
            | '\u{D}'
            | '\u{20}'
            | '\u{A0}'
            | '\u{1680}'
            | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// `text.trim()`.
pub fn trim(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// `text.trim().replace(/\s+/g, ' ').toLowerCase()`: names compared whatever
/// their case and spacing.
pub fn normalise_name(text: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in trim(text).chars() {
        if is_space(c) {
            gap = true;
        } else {
            if gap {
                out.push(' ');
                gap = false;
            }
            out.push(c);
        }
    }
    out.to_lowercase()
}

fn is_decimal_literal(t: &str) -> bool {
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - int_start;
    let mut frac_digits = 0;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac_digits = i - frac_start;
    }
    if int_digits == 0 && frac_digits == 0 {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return false;
        }
    }
    i == b.len()
}

/// `Number(text)`: NaN for what isn't a number, 0 for blank text.
pub fn to_number(text: &str) -> f64 {
    let t = trim(text);
    if t.is_empty() {
        return 0.0;
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let b = t.as_bytes();
    if b.len() > 2 && b[0] == b'0' {
        let radix = match b[1] {
            b'x' | b'X' => 16,
            b'o' | b'O' => 8,
            b'b' | b'B' => 2,
            _ => 0,
        };
        if radix != 0 {
            let digits = &t[2..];
            if digits.chars().all(|c| c.is_digit(radix)) {
                return digits.chars().fold(0.0, |acc, c| {
                    acc * f64::from(radix) + f64::from(c.to_digit(radix).unwrap_or(0))
                });
            }
            return f64::NAN;
        }
    }
    if is_decimal_literal(t) {
        t.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

/// `a < b`, `a > b` and the default `sort()` on strings: by UTF-16 code unit.
pub fn cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `text.length`: UTF-16 code units, so an emoji counts 2.
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// A stable sort that, like `Array.prototype.sort`, takes whatever
/// comparator it is given: one that isn't a total order gives some order
/// rather than a panic. (With a comparator that is a total order, which every
/// one in the model is for the values it meets, the result is the same as any
/// other stable sort.)
pub fn sort_by<T>(items: Vec<T>, mut compare: impl FnMut(&T, &T) -> Ordering) -> Vec<T> {
    fn merge<T>(mut v: Vec<T>, compare: &mut impl FnMut(&T, &T) -> Ordering) -> Vec<T> {
        if v.len() <= 1 {
            return v;
        }
        let right = v.split_off(v.len() / 2);
        let left = merge(v, compare);
        let right = merge(right, compare);
        let mut out = Vec::with_capacity(left.len() + right.len());
        let mut l = left.into_iter().peekable();
        let mut r = right.into_iter().peekable();
        loop {
            let take_right = match (l.peek(), r.peek()) {
                (Some(a), Some(b)) => compare(b, a) == Ordering::Less,
                (Some(_), None) => false,
                (None, Some(_)) => true,
                (None, None) => break,
            };
            let next = if take_right { r.next() } else { l.next() };
            out.extend(next);
        }
        out
    }
    merge(items, &mut compare)
}

// --- localeCompare ----------------------------------------------------------

// ICU's root collation puts the ASCII punctuation and symbols, then the digits,
// then the letters, in this order (read off `Intl.Collator`, which Node and the
// browsers build on ICU). Letters ignore case at the first level; at the third,
// a lower-case letter sorts before its capital.
const ASCII_SYMBOLS: &str = "\t\n\u{b}\u{c}\r _-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

// Accented Latin letters sort with their base letter, then by accent, in this
// order of accents (ICU's secondary weights): acute, grave, breve, circumflex,
// caron, ring, diaeresis, double acute, tilde, dot above, stroke, cedilla,
// ogonek, macron.
const ACCENTED: &[(&str, char, [u8; 12])] = &[
    ("áàăâǎåäãąā", 'a', [1, 2, 3, 4, 5, 6, 7, 9, 13, 14, 0, 0]),
    ("ćĉčċç", 'c', [1, 4, 5, 10, 12, 0, 0, 0, 0, 0, 0, 0]),
    ("ďđ", 'd', [5, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("éèĕêěëėęē", 'e', [1, 2, 3, 4, 5, 7, 10, 13, 14, 0, 0, 0]),
    ("ğĝġģ", 'g', [3, 4, 10, 12, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("ĥħ", 'h', [4, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("íìĭîïĩįīı", 'i', [1, 2, 3, 4, 7, 9, 13, 14, 15, 0, 0, 0]),
    ("ĵ", 'j', [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("ķ", 'k', [12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("ĺľłļ", 'l', [1, 5, 11, 12, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("ńňñņ", 'n', [1, 5, 9, 12, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("óòŏôöőõøō", 'o', [1, 2, 3, 4, 7, 8, 9, 11, 14, 0, 0, 0]),
    ("ŕřŗ", 'r', [1, 5, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("śŝšş", 's', [1, 4, 5, 12, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("ťţŧ", 't', [5, 12, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("úùŭûůüűũųū", 'u', [1, 2, 3, 4, 6, 7, 8, 9, 13, 14, 0, 0]),
    ("ŵ", 'w', [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("ýŷÿ", 'y', [1, 4, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("źžż", 'z', [1, 5, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
];

fn accented(lower: char) -> Option<(char, u8)> {
    ACCENTED.iter().find_map(|(set, base, accents)| {
        set.chars()
            .position(|s| s == lower)
            .map(|i| (*base, accents[i]))
    })
}

/// One character's weights: (primary class, primary), the accent, and whether it is a capital.
type Weight = ((u8, u32), u8, bool);

fn weights(text: &str) -> Vec<Weight> {
    let mut out = Vec::new();
    for c in text.chars() {
        let lower: char = c.to_lowercase().next().unwrap_or(c);
        let upper = lower != c;
        if let Some(i) = ASCII_SYMBOLS.chars().position(|s| s == c) {
            out.push(((0, i as u32), 0, false));
        } else if c.is_ascii_digit() {
            out.push(((1, u32::from(c as u8 - b'0')), 0, false));
        } else if lower.is_ascii_lowercase() {
            out.push(((2, u32::from(lower as u8 - b'a')), 0, upper));
        } else if let Some((base, accent)) = accented(lower) {
            out.push(((2, u32::from(base as u8 - b'a')), accent, upper));
        } else if lower == 'ß' {
            out.push(((2, 18), 0, false));
            out.push(((2, 18), 16, false));
        } else if lower == 'æ' || lower == 'œ' {
            out.push(((2, if lower == 'æ' { 0 } else { 14 }), 0, upper));
            out.push(((2, 4), 16, upper));
        } else if (c as u32) < 9
            || matches!(c, '\u{200B}'..='\u{200F}' | '\u{300}'..='\u{36F}' | '\u{FEFF}')
        {
            // Ignorable: a control character, a zero-width mark, a combining accent.
        } else if c.is_alphabetic() {
            out.push(((3, c as u32), 0, upper));
        } else {
            out.push(((0, 1000 + c as u32), 0, false));
        }
    }
    out
}

/// `a.localeCompare(b)`, as ICU's root collation (what Node and the browsers
/// use for the default locale) orders it, as an [`Ordering`]. Exact for ASCII,
/// which is every course code and nearly every group name; Latin letters with
/// accents sort with their base letter as ICU sorts them; any other script
/// falls back to code point order within the letters, so Greek comes before
/// Cyrillic before Han, as it does there.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    let (wa, wb) = (weights(a), weights(b));
    let primary = wa.iter().map(|w| w.0).cmp(wb.iter().map(|w| w.0));
    primary
        .then_with(|| wa.iter().map(|w| w.1).cmp(wb.iter().map(|w| w.1)))
        .then_with(|| wa.iter().map(|w| w.2).cmp(wb.iter().map(|w| w.2)))
}

/// Days from 1970-01-01 to a proleptic Gregorian date (month 1 to 12).
pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The proleptic Gregorian date `days` days after 1970-01-01.
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The largest time value a `Date` holds, in ms either side of 1970.
pub const TIME_CLIP: f64 = 8.64e15;

/// `Date.UTC(year, month, day, hour, minute, second)`, with a 0-based month.
/// Days and months roll over (day 31 of a 30-day month is the 1st), each
/// argument loses its fraction, and a year from 0 to 99 means 1900 to 1999.
pub fn date_utc(year: f64, month0: f64, day: f64, hour: f64, minute: f64, second: f64) -> f64 {
    let mut y = year.trunc();
    if (0.0..=99.0).contains(&y) {
        y += 1900.0;
    }
    utc_ms(y, month0, day, hour, minute, second)
}

// The time value of a date, days and months rolling over, in the proleptic
// Gregorian calendar; NaN past the years a `Date` holds.
fn utc_ms(year: f64, month0: f64, day: f64, hour: f64, minute: f64, second: f64) -> f64 {
    if [year, month0, day, hour, minute, second]
        .iter()
        .any(|v| !v.is_finite())
    {
        return f64::NAN;
    }
    let m = month0.trunc();
    let ym = year.trunc() + (m / 12.0).floor();
    if ym.abs() > 400_000.0 {
        return f64::NAN;
    }
    let mn = m.rem_euclid(12.0);
    let days = days_from_civil(ym as i64, mn as i64 + 1, 1) as f64 + day.trunc() - 1.0;
    let t = days * 86_400_000.0
        + hour.trunc() * 3_600_000.0
        + minute.trunc() * 60_000.0
        + second.trunc() * 1_000.0;
    if t.abs() > TIME_CLIP {
        f64::NAN
    } else {
        t
    }
}

fn digits(b: &[u8], i: &mut usize, n: usize) -> Option<f64> {
    let part = b.get(*i..*i + n)?;
    if !part.iter().all(u8::is_ascii_digit) {
        return None;
    }
    *i += n;
    Some(
        part.iter()
            .fold(0.0, |acc, d| acc * 10.0 + f64::from(d - b'0')),
    )
}

fn expect(b: &[u8], i: &mut usize, c: u8) -> Option<()> {
    if b.get(*i) == Some(&c) {
        *i += 1;
        Some(())
    } else {
        None
    }
}

/// An ISO 8601 instant as the export writes it, `YYYY-MM-DDTHH:MM`, then
/// optionally `:SS` and `.fraction`, then `Z` or a `+hh:mm`/`-hh:mm` offset:
/// the epoch ms `Date.parse` gives it in V8, or None where `Date.parse` gives
/// NaN or the text isn't in that shape at all.
///
/// V8 takes a month from 1 to 12 and a day from 1 to 31 whatever the month
/// (February 30 is March 1 or 2), an hour from 0 to 23 (24 only for exactly
/// 24:00:00, with every fraction digit 0), minutes and seconds to 59, an
/// offset up to 23:59, and the first three fraction digits as milliseconds.
pub fn parse_iso_instant(text: &str) -> Option<f64> {
    let b = text.as_bytes();
    let mut i = 0;
    let year = digits(b, &mut i, 4)?;
    expect(b, &mut i, b'-')?;
    let month = digits(b, &mut i, 2)?;
    expect(b, &mut i, b'-')?;
    let day = digits(b, &mut i, 2)?;
    expect(b, &mut i, b'T')?;
    let hour = digits(b, &mut i, 2)?;
    expect(b, &mut i, b':')?;
    let minute = digits(b, &mut i, 2)?;
    let mut second = 0.0;
    let mut millis = 0.0;
    let mut fraction_is_zero = true;
    if b.get(i) == Some(&b':') {
        i += 1;
        second = digits(b, &mut i, 2)?;
        if b.get(i) == Some(&b'.') {
            i += 1;
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let fraction = &b[start..i];
            if fraction.is_empty() {
                return None;
            }
            fraction_is_zero = fraction.iter().all(|d| *d == b'0');
            let mut ms = 0.0;
            for k in 0..3 {
                ms = ms * 10.0 + fraction.get(k).map_or(0.0, |d| f64::from(d - b'0'));
            }
            millis = ms;
        }
    }
    let offset_minutes = match b.get(i) {
        Some(b'Z') => {
            i += 1;
            0.0
        }
        Some(sign @ (b'+' | b'-')) => {
            let sign = if *sign == b'-' { -1.0 } else { 1.0 };
            i += 1;
            let oh = digits(b, &mut i, 2)?;
            expect(b, &mut i, b':')?;
            let om = digits(b, &mut i, 2)?;
            if oh > 23.0 || om > 59.0 {
                return None;
            }
            sign * (oh * 60.0 + om)
        }
        _ => return None,
    };
    if i != b.len() {
        return None;
    }
    if !(1.0..=12.0).contains(&month) || !(1.0..=31.0).contains(&day) {
        return None;
    }
    let clock_ok = hour <= 23.0 && minute <= 59.0 && second <= 59.0;
    let midnight_24 = hour == 24.0 && minute == 0.0 && second == 0.0 && fraction_is_zero;
    if !clock_ok && !midnight_24 {
        return None;
    }
    // The ISO reader takes a year from 0000 to 0099 as itself, unlike `Date.UTC`.
    let t =
        utc_ms(year, month - 1.0, day, hour, minute, second) + millis - offset_minutes * 60_000.0;
    (t.abs() <= TIME_CLIP).then_some(t)
}
