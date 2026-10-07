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
//! and nesting deeper than 9,998 levels is read here, as JSON.parse and
//! PRANA read it, where python 3.13 raises RecursionError and the tools
//! die with a traceback. Reading, printing, comparing, cloning and
//! dropping a value never recurse, so no nesting runs a thread out of
//! stack. `str()` of a string inside a list escapes the characters python
//! counts unprintable; for a character Unicode assigned after 15.1
//! (python 3.13's tables) this goes by Rust's tables.

use std::collections::HashMap;
use std::fmt::{self, Write};

/// A JSON value. Cloning, comparing (`==`, as Rust compares: NaN isn't
/// equal to itself, 1 isn't 1.0), printing with `{:?}` and dropping work
/// through the nesting with a stack of their own.
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

impl Drop for Value {
    /// Takes nested lists and objects apart onto a stack first, so dropping
    /// them doesn't recurse.
    fn drop(&mut self) {
        fn nested(v: &mut Value, into: &mut Vec<Value>) {
            let nests = |v: &Value| matches!(v, Value::List(_) | Value::Dict(_));
            match v {
                Value::List(items) => into.extend(items.drain(..).filter(nests)),
                Value::Dict(items) => into.extend(items.drain(..).map(|(_, v)| v).filter(nests)),
                _ => {}
            }
        }
        let mut stack = Vec::new();
        nested(self, &mut stack);
        while let Some(mut v) = stack.pop() {
            nested(&mut v, &mut stack);
        }
    }
}

impl Clone for Value {
    fn clone(&self) -> Value {
        enum Open<'a> {
            List(std::slice::Iter<'a, Value>, Vec<Value>),
            Dict(
                std::slice::Iter<'a, (String, Value)>,
                Vec<(String, Value)>,
                String,
            ),
        }
        let mut open: Vec<Open> = Vec::new();
        let mut next = self;
        loop {
            let mut done = match next {
                Value::List(items) => {
                    open.push(Open::List(items.iter(), Vec::with_capacity(items.len())));
                    None
                }
                Value::Dict(items) => {
                    let copy = Vec::with_capacity(items.len());
                    open.push(Open::Dict(items.iter(), copy, String::new()));
                    None
                }
                Value::Null => Some(Value::Null),
                Value::Bool(b) => Some(Value::Bool(*b)),
                Value::Int(t) => Some(Value::Int(t.clone())),
                Value::Float(f) => Some(Value::Float(*f)),
                Value::Str(s) => Some(Value::Str(s.clone())),
            };
            // a copy made goes in its list or object, which goes on to its
            // next item or, when it has none left, is made itself
            loop {
                let Some(top) = open.last_mut() else {
                    return done.unwrap_or(Value::Null); // always Some here
                };
                match top {
                    Open::List(items, copy) => {
                        copy.extend(done.take());
                        if let Some(x) = items.next() {
                            next = x;
                            break;
                        }
                    }
                    Open::Dict(items, copy, key) => {
                        if let Some(v) = done.take() {
                            copy.push((std::mem::take(key), v));
                        }
                        if let Some((k, x)) = items.next() {
                            key.clone_from(k);
                            next = x;
                            break;
                        }
                    }
                }
                done = match open.pop() {
                    Some(Open::List(_, copy)) => Some(Value::List(copy)),
                    Some(Open::Dict(_, copy, _)) => Some(Value::Dict(copy)),
                    None => None,
                };
            }
        }
    }
}

impl PartialEq for Value {
    /// Rust's `==`, item by item: NaN isn't equal to itself, and an Int is
    /// never a Float (python's `==` is [`py_eq`]).
    fn eq(&self, other: &Value) -> bool {
        use Value::*;
        let mut pairs = vec![(self, other)];
        while let Some(pair) = pairs.pop() {
            match pair {
                (Null, Null) => {}
                (Bool(a), Bool(b)) if a == b => {}
                (Int(a), Int(b)) | (Str(a), Str(b)) if a == b => {}
                (Float(a), Float(b)) if a == b => {}
                (List(a), List(b)) if a.len() == b.len() => pairs.extend(a.iter().zip(b)),
                (Dict(a), Dict(b)) if a.len() == b.len() => {
                    for ((j, v), (k, w)) in a.iter().zip(b) {
                        if j != k {
                            return false;
                        }
                        pairs.push((v, w));
                    }
                }
                _ => return false,
            }
        }
        true
    }
}

impl fmt::Debug for Value {
    /// The value as `json.dumps` writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&dumps(self))
    }
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
    let v = p.value()?;
    p.ws();
    (p.at == p.s.len()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
    constants: bool,
}

/// A list or object being read, its items so far.
enum Open {
    List(Vec<Value>),
    /// The items, where each key is in them (a repeated key keeps its
    /// first place: python's dict), and the key whose value comes next.
    Dict(Vec<(String, Value)>, HashMap<String, usize>, String),
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

    /// A value and everything in it, read with a stack of the lists and
    /// objects still open rather than by recursion, so any nesting reads.
    /// The grammar is python's scanner's: after "[" or "{" whitespace, then
    /// "]" or "}" at once or an item; after each item whitespace, then ","
    /// (and whitespace) or the close.
    fn value(&mut self) -> Option<Value> {
        let mut open: Vec<Open> = Vec::new();
        'item: loop {
            let mut done = match self.peek()? {
                b'[' => {
                    self.at += 1;
                    self.ws();
                    if !self.eat("]") {
                        open.push(Open::List(Vec::new()));
                        continue 'item;
                    }
                    Value::List(Vec::new())
                }
                b'{' => {
                    self.at += 1;
                    self.ws();
                    if !self.eat("}") {
                        let key = self.key()?;
                        open.push(Open::Dict(Vec::new(), HashMap::new(), key));
                        continue 'item;
                    }
                    Value::Dict(Vec::new())
                }
                _ => self.scalar()?,
            };
            // a whole value: it goes in the innermost open list or object,
            // and closes it if that's the end of it
            loop {
                let Some(top) = open.last_mut() else {
                    return Some(done);
                };
                self.ws();
                match top {
                    Open::List(items) => {
                        items.push(done);
                        if self.eat(",") {
                            self.ws();
                            continue 'item;
                        }
                        if !self.eat("]") {
                            return None;
                        }
                    }
                    Open::Dict(items, index, next) => {
                        let key = std::mem::take(next);
                        match index.get(&key) {
                            Some(&i) => items[i].1 = done,
                            None => {
                                index.insert(key.clone(), items.len());
                                items.push((key, done));
                            }
                        }
                        if self.eat(",") {
                            self.ws();
                            *next = self.key()?;
                            continue 'item;
                        }
                        if !self.eat("}") {
                            return None;
                        }
                    }
                }
                done = match open.pop() {
                    Some(Open::List(items)) => Value::List(items),
                    Some(Open::Dict(items, ..)) => Value::Dict(items),
                    None => return None,
                };
            }
        }
    }

    /// An object's key, its ":" and the whitespace around it.
    fn key(&mut self) -> Option<String> {
        if self.peek()? != b'"' {
            return None;
        }
        let key = self.string()?;
        self.ws();
        if !self.eat(":") {
            return None;
        }
        self.ws();
        Some(key)
    }

    fn scalar(&mut self) -> Option<Value> {
        match self.peek()? {
            b'"' => self.string().map(Value::Str),
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
}

/// `json.dumps(v, ensure_ascii=False)`.
pub fn dumps(v: &Value) -> String {
    let mut out = String::new();
    write_nested(v, &mut out, dump_scalar, quote_into);
    out
}

fn dump_scalar(v: &Value, out: &mut String) {
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
        Value::List(_) | Value::Dict(_) => {}
    }
}

/// Writes `v` as `[a, b]` and `{k: v}` with ", " and ": ", its scalars by
/// `scalar` and its keys by `key`, keeping the open lists and objects on a
/// stack of its own: `json.dumps` and `repr` both lay a value out this way.
fn write_nested(
    v: &Value,
    out: &mut String,
    scalar: fn(&Value, &mut String),
    key: fn(&str, &mut String),
) {
    enum Open<'a> {
        List(std::slice::Iter<'a, Value>, bool),
        Dict(std::slice::Iter<'a, (String, Value)>, bool),
    }
    let mut open: Vec<Open> = Vec::new();
    let mut next = v;
    loop {
        match next {
            Value::List(items) => {
                out.push('[');
                open.push(Open::List(items.iter(), true));
            }
            Value::Dict(items) => {
                out.push('{');
                open.push(Open::Dict(items.iter(), true));
            }
            _ => scalar(next, out),
        }
        // the next item of the innermost list or object still open
        loop {
            match open.last_mut() {
                None => return,
                Some(Open::List(items, first)) => match items.next() {
                    Some(x) => {
                        if !std::mem::take(first) {
                            out.push_str(", ");
                        }
                        next = x;
                        break;
                    }
                    None => {
                        out.push(']');
                        open.pop();
                    }
                },
                Some(Open::Dict(items, first)) => match items.next() {
                    Some((k, x)) => {
                        if !std::mem::take(first) {
                            out.push_str(", ");
                        }
                        key(k, out);
                        out.push_str(": ");
                        next = x;
                        break;
                    }
                    None => {
                        out.push('}');
                        open.pop();
                    }
                },
            }
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
    // Halfway, the value is exactly those digits with a 5 after them. One
    // more digit, correctly rounded, then ends in 5, and is the value itself.
    let n = digits.len();
    let (longer, longer_exp) = split(&format!("{f:.n$e}"));
    if longer_exp == exp && longer[n] == b'5' && exactly(f, &longer, exp - n as i32) {
        let mut even = longer[..n].to_vec();
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

/// Whether `f` (finite, positive) is exactly the decimal `digits` × 10^`p`
/// (at most 18 digits): m × 2^q = d × 2^p × 5^p, compared by their powers
/// of 2 and 5 and what's left of each.
fn exactly(f: f64, digits: &[u8], p: i32) -> bool {
    let d = digits
        .iter()
        .fold(0u64, |d, &c| d * 10 + u64::from(c - b'0'));
    let bits = f.to_bits();
    let e = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1 << 52) - 1);
    let (m, q) = if e == 0 {
        (fraction, -1074)
    } else {
        (fraction | 1 << 52, e - 1075)
    };
    let strip = |mut x: u64, by: u64| {
        let mut k = 0;
        while x != 0 && x % by == 0 {
            x /= by;
            k += 1;
        }
        (x, k)
    };
    let (m, m2) = strip(m, 2);
    let (m, m5) = strip(m, 5);
    let (d, d2) = strip(d, 2);
    let (d, d5) = strip(d, 5);
    m == d && m2 + q == d2 + p && m5 == d5 + p
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
            write_nested(v, &mut out, repr_scalar, repr_str);
            out
        }
    }
}

fn repr_scalar(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("None"),
        Value::Bool(b) => out.push_str(if *b { "True" } else { "False" }),
        Value::Int(t) => out.push_str(t),
        Value::Float(f) => out.push_str(&float_repr(*f)),
        Value::Str(s) => repr_str(s, out),
        Value::List(_) | Value::Dict(_) => {}
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
    let mut pairs = vec![(a, b)];
    while let Some(pair) = pairs.pop() {
        match pair {
            (Null, Null) => {}
            (Str(x), Str(y)) if x == y => {}
            (List(x), List(y)) if x.len() == y.len() => pairs.extend(x.iter().zip(y)),
            (Dict(x), Dict(y)) if x.len() == y.len() => {
                let mut index = HashMap::with_capacity(y.len());
                for (k, w) in y {
                    index.entry(k.as_str()).or_insert(w);
                }
                for (k, v) in x {
                    match index.get(k.as_str()) {
                        Some(w) => pairs.push((v, w)),
                        None => return false,
                    }
                }
            }
            (a, b) if numbers_equal(a, b) => {}
            _ => return false,
        }
    }
    true
}

fn numbers_equal(a: &Value, b: &Value) -> bool {
    match (number(a), number(b)) {
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

    /// 200,000 levels, on a test thread's 2 MB stack: reading, printing,
    /// comparing, cloning and dropping never recurse.
    #[test]
    fn any_nesting_reads_without_running_out_of_stack() {
        let n = 200_000;
        for (open, close) in [("[", "]"), (r#"{"k": "#, "}")] {
            let text = format!("{}1{}", open.repeat(n), close.repeat(n));
            let v = loads(text.as_bytes(), true).expect("deep JSON reads");
            assert_eq!(dumps(&v), text);
            assert_eq!(format!("{v:?}"), text);
            assert_eq!(py_str(&v).len(), py_str(&v).len());
            let copy = v.clone();
            assert!(copy == v && py_eq(&copy, &v));
            drop(copy);
            assert!(loads(&text.as_bytes()[..text.len() - 1], true).is_none());
        }
    }

    #[test]
    fn a_wide_object_reads_in_linear_time() {
        let keys: Vec<String> = (0..200_000).map(|i| format!(r#""k{i}": {i}"#)).collect();
        let text = format!("{{{}}}", keys.join(", "));
        let start = std::time::Instant::now();
        let v = loads(text.as_bytes(), true).unwrap();
        assert!(py_eq(&v, &v.clone()));
        let took = start.elapsed();
        assert_eq!(dumps(&v), text);
        assert!(took.as_secs() < 2, "200,000 keys took {took:?}");
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
