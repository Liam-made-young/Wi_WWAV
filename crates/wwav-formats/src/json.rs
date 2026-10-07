//! JSON as python's `json` module reads and writes it, and `str()` of what
//! it reads. The reference tools print, compare and write JSON with these,
//! and their output has to come out of Rust character for character, so
//! this follows CPython 3.13 rather than RFC 8259 alone:
//!
//! - [`loads`] takes NaN, Infinity and -Infinity (unless `constants` is
//!   false, as swav_pack.py's `parse_constant=not_json`). An object's
//!   repeated key keeps its first place and its last value. A number with a
//!   fraction or an exponent is a float (1e400 is infinity); one without is
//!   an integer of any length up to python's 4,300 digits. A text that
//!   starts with a BOM isn't JSON, and neither is a control character
//!   inside a string.
//! - [`dumps`] is `json.dumps(v, ensure_ascii=False)`: ", " and ": ",
//!   non-ASCII as itself, floats by `repr`.
//! - [`py_str`] is `str(v)`, as an f-string prints a value: None, True,
//!   1.0, ['a'] and {'k': 1}.
//!
//! Where this parts from python, on texts no writer makes: a lone
//! surrogate from "\ud800" becomes U+FFFD, since a Rust `String` can't
//! hold one (python can't print one either: it raises UnicodeEncodeError);
//! and nesting deeper than 500 isn't JSON here, so reading, printing and
//! comparing a value never runs a thread out of stack, where python 3.13
//! reads 9,998 levels and raises RecursionError past them. `str()` of a
//! string inside a list escapes the characters python counts unprintable;
//! for a character Unicode assigned after 15.1 (python 3.13's tables) this
//! goes by Rust's tables.

use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    /// An integer as python prints it: its decimal digits, with "-" when
    /// negative.
    Int(String),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    /// Keys in the order they first appeared.
    Dict(Vec<(String, Value)>),
}

/// `dict.get(key)`.
pub fn get<'a>(dict: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    dict.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// python's truth value: what `x or {}` keeps.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Int(t) => t != "0",
        Value::Float(f) => *f != 0.0,
        Value::Str(s) => !s.is_empty(),
        Value::List(l) => !l.is_empty(),
        Value::Dict(d) => !d.is_empty(),
    }
}

const MAX_DEPTH: usize = 500;
const MAX_INT_DIGITS: usize = 4300; // sys.int_info.default_max_str_digits

/// `json.loads(bytes.decode("utf-8"))`, or None where python raises
/// UnicodeDecodeError or ValueError. `constants`: whether NaN, Infinity
/// and -Infinity are read (wwav_pack.py) or refused (swav_pack.py).
pub fn loads(bytes: &[u8], constants: bool) -> Option<Value> {
    let text = std::str::from_utf8(bytes).ok()?;
    if text.starts_with('\u{feff}') {
        return None; // "Unexpected UTF-8 BOM"
    }
    let mut p = Parser {
        s: text.as_bytes(),
        at: 0,
        constants,
    };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    (p.at == p.s.len()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
    constants: bool,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn eat(&mut self, literal: &str) -> bool {
        let found = self.s[self.at..].starts_with(literal.as_bytes());
        if found {
            self.at += literal.len();
        }
        found
    }

    fn value(&mut self, depth: usize) -> Option<Value> {
        if depth > MAX_DEPTH {
            return None;
        }
        match self.peek()? {
            b'"' => self.string().map(Value::Str),
            b'{' => self.object(depth),
            b'[' => self.list(depth),
            b'n' if self.eat("null") => Some(Value::Null),
            b't' if self.eat("true") => Some(Value::Bool(true)),
            b'f' if self.eat("false") => Some(Value::Bool(false)),
            b'N' if self.constants && self.eat("NaN") => Some(Value::Float(f64::NAN)),
            b'I' if self.constants && self.eat("Infinity") => Some(Value::Float(f64::INFINITY)),
            b'-' if self.constants && self.eat("-Infinity") => {
                Some(Value::Float(f64::NEG_INFINITY))
            }
            _ => self.number(),
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        self.at - start
    }

    /// `-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][-+]?[0-9]+)?`, as the scanner
    /// matches it: a "." or an "e" that no digit follows isn't part of it.
    fn number(&mut self) -> Option<Value> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        match self.peek()? {
            b'0' => self.at += 1,
            b'1'..=b'9' => {
                self.digits();
            }
            _ => return None,
        }
        let mut float = false;
        if self.peek() == Some(b'.') && matches!(self.s.get(self.at + 1), Some(b'0'..=b'9')) {
            self.at += 1;
            self.digits();
            float = true;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            let e = self.at;
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if self.digits() > 0 {
                float = true;
            } else {
                self.at = e;
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.at]).ok()?;
        if float {
            return text.parse().ok().map(Value::Float);
        }
        let digits = text.trim_start_matches('-');
        if digits.len() > MAX_INT_DIGITS {
            return None; // python: "Exceeds the limit (4300 digits)"
        }
        Some(Value::Int(if digits == "0" {
            "0".into()
        } else {
            text.into()
        }))
    }

    fn hex4(&mut self) -> Option<u32> {
        let h = self.s.get(self.at..self.at + 4)?;
        let v = u32::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()?;
        if !h.iter().all(u8::is_ascii_hexdigit) {
            return None; // from_str_radix takes a leading "+"
        }
        self.at += 4;
        Some(v)
    }

    fn string(&mut self) -> Option<String> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let run = self.at;
            while matches!(self.peek(), Some(b) if b != b'"' && b != b'\\' && b >= 0x20) {
                self.at += 1;
            }
            // the text is UTF-8 and the run stops only at ASCII bytes
            out.push_str(std::str::from_utf8(&self.s[run..self.at]).ok()?);
            match self.peek()? {
                b'"' => {
                    self.at += 1;
                    return Some(out);
                }
                b'\\' => {
                    self.at += 1;
                    let e = self.peek()?;
                    self.at += 1;
                    out.push(match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.escaped_char()?,
                        _ => return None,
                    });
                }
                _ => return None, // a control character: "Invalid control character"
            }
        }
    }

    /// After "\u": a code point, joining a surrogate pair as python does.
    fn escaped_char(&mut self) -> Option<char> {
        let c = self.hex4()?;
        if (0xd800..0xdc00).contains(&c) && self.s[self.at..].starts_with(b"\\u") {
            let back = self.at;
            self.at += 2;
            let low = self.hex4()?;
            if (0xdc00..0xe000).contains(&low) {
                return char::from_u32(0x10000 + ((c - 0xd800) << 10) + (low - 0xdc00));
            }
            self.at = back;
        }
        Some(char::from_u32(c).unwrap_or('\u{fffd}'))
    }

    fn object(&mut self, depth: usize) -> Option<Value> {
        self.at += 1;
        let mut items: Vec<(String, Value)> = Vec::new();
        self.ws();
        if self.eat("}") {
            return Some(Value::Dict(items));
        }
        loop {
            if self.peek()? != b'"' {
                return None;
            }
            let key = self.string()?;
            self.ws();
            if !self.eat(":") {
                return None;
            }
            self.ws();
            let v = self.value(depth + 1)?;
            match items.iter_mut().find(|(k, _)| *k == key) {
                Some(item) => item.1 = v,
                None => items.push((key, v)),
            }
            self.ws();
            if self.eat("}") {
                return Some(Value::Dict(items));
            }
            if !self.eat(",") {
                return None;
            }
            self.ws();
        }
    }

    fn list(&mut self, depth: usize) -> Option<Value> {
        self.at += 1;
        let mut items = Vec::new();
        self.ws();
        if self.eat("]") {
            return Some(Value::List(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.ws();
            if self.eat("]") {
                return Some(Value::List(items));
            }
            if !self.eat(",") {
                return None;
            }
            self.ws();
        }
    }
}

/// `json.dumps(v, ensure_ascii=False)`.
pub fn dumps(v: &Value) -> String {
    let mut out = String::new();
    dump(v, &mut out);
    out
}

fn dump(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(t) => out.push_str(t),
        Value::Float(f) if f.is_nan() => out.push_str("NaN"),
        Value::Float(f) if f.is_infinite() => {
            out.push_str(if *f > 0.0 { "Infinity" } else { "-Infinity" })
        }
        Value::Float(f) => out.push_str(&float_repr(*f)),
        Value::Str(s) => quote_into(s, out),
        Value::List(items) => {
            out.push('[');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                dump(x, out);
            }
            out.push(']');
        }
        Value::Dict(items) => {
            out.push('{');
            for (i, (k, x)) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                quote_into(k, out);
                out.push_str(": ");
                dump(x, out);
            }
            out.push('}');
        }
    }
}

/// `json.dumps(s, ensure_ascii=False)` of a string: the tools' `js()`.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    quote_into(s, &mut out);
    out
}

fn quote_into(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `repr(f)`: the shortest text that reads back as `f`, in fixed notation
/// from 1e-4 up to 1e16 and with an exponent of at least two digits
/// outside it: 0.1, 1.0, 1e+16, 1e-05.
pub fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf" } else { "-inf" }.into();
    }
    let sign = if f.is_sign_negative() { "-" } else { "" };
    if f == 0.0 {
        return format!("{sign}0.0");
    }
    let (digits, exp) = shortest(f.abs());
    let point = exp + 1; // the value is 0.<digits> times 10^point
    if !(-4 < point && point <= 16) {
        let (first, rest) = digits.split_at(1);
        let dot = if rest.is_empty() { "" } else { "." };
        let esign = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{first}{dot}{rest}e{esign}{:02}", exp.abs());
    }
    if point <= 0 {
        return format!("{sign}0.{}{digits}", "0".repeat(-point as usize));
    }
    let point = point as usize;
    if point >= digits.len() {
        format!("{sign}{digits}{}.0", "0".repeat(point - digits.len()))
    } else {
        format!("{sign}{}.{}", &digits[..point], &digits[point..])
    }
}

/// The shortest digits that read back as `f` (positive), and the power of
/// ten of the first: Rust's `{:e}`, except where the value lies exactly
/// halfway between two such strings. Rust then rounds the last digit up;
/// python's dtoa rounds it to even (1394865425023536.25 is ...536.2).
fn shortest(f: f64) -> (String, i32) {
    let split = |s: &str| -> (Vec<u8>, i32) {
        let (m, e) = s.split_once('e').unwrap_or((s, "0"));
        (
            m.bytes().filter(u8::is_ascii_digit).collect(),
            e.parse().unwrap_or(0),
        )
    };
    let (mut digits, exp) = split(&format!("{f:e}"));
    // every double's exact decimal expansion fits in 800 digits
    let (exact, exact_exp) = split(&format!("{f:.800e}"));
    let n = digits.len();
    let halfway = exact_exp == exp && exact[n] == b'5' && exact[n + 1..].iter().all(|&d| d == b'0');
    if halfway {
        let mut even = exact[..n].to_vec();
        if even[n - 1] % 2 == 1 {
            even[n - 1] += 1; // odd, so never a 9 carrying over
        }
        // below a power of two the doubles are closer together, and the
        // even one may not read back: then it isn't a choice
        let text = format!("0.{}e{}", String::from_utf8_lossy(&even), exp + 1);
        if text.parse::<f64>() == Ok(f) {
            digits = even;
        }
    }
    (String::from_utf8(digits).unwrap_or_default(), exp)
}

/// The tools' `num(v)` for a float, "as the device writes them: whole, or
/// 2 decimals": `str(int(v)) if v == int(v) else f"{v:.2f}"`. NaN and the
/// infinities have no int(), which python raises (unpack's only way to
/// meet one).
pub fn num(v: f64) -> Result<String, String> {
    if v.is_nan() {
        return Err("cannot convert float NaN to integer".into());
    }
    if v.is_infinite() {
        return Err("cannot convert float infinity to integer".into());
    }
    if v == 0.0 {
        return Ok("0".into());
    }
    Ok(if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.2}")
    })
}

/// `str(v)`.
pub fn py_str(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        _ => {
            let mut out = String::new();
            repr(v, &mut out);
            out
        }
    }
}

fn repr(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("None"),
        Value::Bool(b) => out.push_str(if *b { "True" } else { "False" }),
        Value::Int(t) => out.push_str(t),
        Value::Float(f) => out.push_str(&float_repr(*f)),
        Value::Str(s) => repr_str(s, out),
        Value::List(items) => {
            out.push('[');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                repr(x, out);
            }
            out.push(']');
        }
        Value::Dict(items) => {
            out.push('{');
            for (i, (k, x)) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                repr_str(k, out);
                out.push_str(": ");
                repr(x, out);
            }
            out.push('}');
        }
    }
}

/// `repr(s)`: single quotes unless the text has one and no double quote.
fn repr_str(s: &str, out: &mut String) {
    let q = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    out.push(q);
    for c in s.chars() {
        let n = c as u32;
        match c {
            c if c == q || c == '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ if n < 0x20 || n == 0x7f => {
                let _ = write!(out, "\\x{n:02x}");
            }
            _ if n < 0x7f || printable(c) => out.push(c),
            _ if n <= 0xff => {
                let _ = write!(out, "\\x{n:02x}");
            }
            _ if n <= 0xffff => {
                let _ = write!(out, "\\u{n:04x}");
            }
            _ => {
                let _ = write!(out, "\\U{n:08x}");
            }
        }
    }
    out.push(q);
}

/// python's `str.isprintable()` for one non-ASCII character: not a control,
/// format, private-use, unassigned or separator character other than the
/// space. Rust's `escape_debug` escapes exactly those (and, as the first
/// character of a text, a combining mark: hence the "a" before it).
fn printable(c: char) -> bool {
    let mut probe = String::from("a");
    probe.push(c);
    probe.escape_debug().to_string() == probe
}

/// python's `==` between two JSON values: 1 == 1.0 == True, a list or a
/// dict equal item by item, NaN equal to nothing.
pub fn py_eq(a: &Value, b: &Value) -> bool {
    use Value::*;
    match (a, b) {
        (Null, Null) => true,
        (Str(x), Str(y)) => x == y,
        (List(x), List(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| py_eq(p, q)),
        (Dict(x), Dict(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| get(y, k).is_some_and(|w| py_eq(v, w)))
        }
        _ => match (number(a), number(b)) {
            (Some(Num::Int(x)), Some(Num::Int(y))) => x == y,
            (Some(Num::Float(x)), Some(Num::Float(y))) => x == y,
            (Some(Num::Int(i)), Some(Num::Float(f))) | (Some(Num::Float(f)), Some(Num::Int(i))) => {
                // exactly, as python compares an int with a float
                f.is_finite()
                    && f.fract() == 0.0
                    && (if f == 0.0 {
                        "0".into()
                    } else {
                        format!("{f:.0}")
                    }) == i
            }
            _ => false,
        },
    }
}

enum Num<'a> {
    Int(std::borrow::Cow<'a, str>),
    Float(f64),
}

fn number(v: &Value) -> Option<Num<'_>> {
    match v {
        Value::Bool(b) => Some(Num::Int(if *b { "1" } else { "0" }.into())),
        Value::Int(t) => Some(Num::Int(t.as_str().into())),
        Value::Float(f) => Some(Num::Float(*f)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repr_lays_floats_out_as_python() {
        let cases = [
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (1e-4, "0.0001"),
            (1e-5, "1e-05"),
            (0.1, "0.1"),
            (-2.5, "-2.5"),
            (123456789012345678.0, "1.2345678901234568e+17"),
            (5e-324, "5e-324"),
            (1.5e300, "1.5e+300"),
            (-0.0, "-0.0"),
            (88200.0, "88200.0"),
            (1_394_865_425_023_536.0 + 0.25, "1394865425023536.2"),
            (2f64.powi(-24), "5.960464477539063e-08"),
            (f64::from_bits(3), "1.5e-323"),
        ];
        for (f, want) in cases {
            assert_eq!(float_repr(f), want);
        }
    }

    #[test]
    fn loads_keeps_a_repeated_keys_first_place_and_last_value() {
        let v = loads(br#"{"a": 1, "b": 2, "a": 3}"#, true).unwrap();
        assert_eq!(dumps(&v), r#"{"a": 3, "b": 2}"#);
    }

    #[test]
    fn numbers_compare_as_python_compares_them() {
        let int = |t: &str| Value::Int(t.into());
        assert!(py_eq(&int("600"), &Value::Float(600.0)));
        assert!(py_eq(&int("1"), &Value::Bool(true)));
        assert!(!py_eq(
            &int("9007199254740993"),
            &Value::Float(9007199254740992.0)
        ));
        assert!(!py_eq(&Value::Float(f64::NAN), &Value::Float(f64::NAN)));
        assert!(!py_eq(&int("600"), &Value::Str("600".into())));
    }
}
