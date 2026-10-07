//! Reads design/tokens.json on its own, without the compiler, so the tests
//! hold the compiled outputs to the file rather than to themselves.

#![allow(dead_code)] // each test binary uses its own part of this

use serde_json::Value;
use std::path::PathBuf;
use wwav_tokens::Color;

pub const APPEARANCES: [&str; 2] = ["light", "dark"];

pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn json() -> Value {
    let text = std::fs::read_to_string(repo().join("design/tokens.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// One token's value in one appearance.
#[derive(Clone, Debug)]
pub enum Val {
    Color(Color),
    Colors(Vec<Color>),
    Number(f64),
    Numbers(Vec<f64>),
    Names(Vec<String>),
    Curve([f64; 4]),
}

pub struct Token {
    pub path: Vec<String>,
    pub light: Val,
    pub dark: Val,
    pub themed: bool,
    /// The unit a dimension like "140ms" was written with.
    pub unit: Option<String>,
}

impl Token {
    pub fn dotted(&self) -> String {
        self.path.join(".")
    }

    pub fn get(&self, appearance: &str) -> &Val {
        if appearance == "dark" {
            &self.dark
        } else {
            &self.light
        }
    }
}

/// One scalar of a token, by its path, an index for list items, and the
/// appearance: ("desk.caseMetal[1]", "dark") -> the dark bottom stop.
#[derive(Clone, Debug)]
pub enum Leaf {
    Color(Color),
    Number(f64),
    Name(String),
}

impl Leaf {
    pub fn same(&self, other: &Leaf) -> bool {
        match (self, other) {
            (Leaf::Color(a), Leaf::Color(b)) => {
                (a.r, a.g, a.b) == (b.r, b.g, b.b) && (a.a - b.a).abs() < 1e-6
            }
            (Leaf::Number(a), Leaf::Number(b)) => (a - b).abs() < 1e-5,
            (Leaf::Name(a), Leaf::Name(b)) => a == b,
            _ => false,
        }
    }
}

pub fn leaves(val: &Val) -> Vec<(String, Leaf)> {
    let one = |l: Leaf| vec![(String::new(), l)];
    let many = |ls: Vec<Leaf>| {
        ls.into_iter()
            .enumerate()
            .map(|(i, l)| (format!("[{i}]"), l))
            .collect()
    };
    match val {
        Val::Color(c) => one(Leaf::Color(*c)),
        Val::Number(n) => one(Leaf::Number(*n)),
        Val::Colors(cs) => many(cs.iter().map(|c| Leaf::Color(*c)).collect()),
        Val::Numbers(ns) => many(ns.iter().map(|n| Leaf::Number(*n)).collect()),
        Val::Names(ns) => many(ns.iter().map(|n| Leaf::Name(n.clone())).collect()),
        Val::Curve(c) => many(c.iter().map(|n| Leaf::Number(*n)).collect()),
    }
}

pub fn parse_color(s: &str) -> Option<Color> {
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        return Some(Color::rgb(byte(0)?, byte(2)?, byte(4)?));
    }
    let inner = s.strip_prefix("rgba(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return None;
    }
    Some(Color::rgba(
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
        parts[3].parse().ok()?,
    ))
}

fn parse_dimension(s: &str) -> Option<(f64, String)> {
    for unit in ["px", "ms", "em"] {
        if let Some(n) = s.strip_suffix(unit) {
            return n.parse().ok().map(|n| (n, unit.to_string()));
        }
    }
    None
}

fn parse_curve(s: &str) -> Option<[f64; 4]> {
    let inner = s.strip_prefix("cubic-bezier(")?.strip_suffix(')')?;
    let ns: Vec<f64> = inner
        .split(',')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    ns.try_into().ok()
}

fn parse_value(v: &Value, path: &str) -> (Val, Option<String>) {
    match v {
        Value::Number(n) => (Val::Number(n.as_f64().unwrap()), None),
        Value::String(s) => {
            if let Some(c) = parse_color(s) {
                (Val::Color(c), None)
            } else if let Some((n, unit)) = parse_dimension(s) {
                (Val::Number(n), Some(unit))
            } else if let Some(c) = parse_curve(s) {
                (Val::Curve(c), None)
            } else {
                panic!("{path}: can't read {s:?}")
            }
        }
        Value::Array(items) => {
            if let Some(cs) = items
                .iter()
                .map(|i| i.as_str().and_then(parse_color))
                .collect::<Option<Vec<_>>>()
            {
                (Val::Colors(cs), None)
            } else if let Some(ns) = items.iter().map(Value::as_f64).collect() {
                (Val::Numbers(ns), None)
            } else if let Some(ns) = items.iter().map(|i| i.as_str().map(String::from)).collect() {
                (Val::Names(ns), None)
            } else {
                panic!("{path}: a list of mixed kinds")
            }
        }
        _ => panic!("{path}: can't read {v}"),
    }
}

fn is_themed(v: &Value) -> bool {
    v.as_object()
        .is_some_and(|o| o.len() == 2 && o.contains_key("light") && o.contains_key("dark"))
}

fn walk(v: &Value, path: &mut Vec<String>, out: &mut Vec<Token>) {
    let dotted = path.join(".");
    if is_themed(v) {
        let (light, unit) = parse_value(&v["light"], &dotted);
        let (dark, _) = parse_value(&v["dark"], &dotted);
        out.push(Token {
            path: path.clone(),
            light,
            dark,
            themed: true,
            unit,
        });
    } else if let Some(group) = v.as_object() {
        for (key, child) in group {
            if !key.starts_with('$') {
                path.push(key.clone());
                walk(child, path, out);
                path.pop();
            }
        }
    } else {
        let (val, unit) = parse_value(v, &dotted);
        out.push(Token {
            path: path.clone(),
            light: val.clone(),
            dark: val,
            themed: false,
            unit,
        });
    }
}

/// Every token in the file, in file order.
pub fn tokens() -> Vec<Token> {
    let mut out = Vec::new();
    walk(&json(), &mut Vec::new(), &mut out);
    out
}

pub fn find<'a>(tokens: &'a [Token], dotted: &str) -> &'a Token {
    tokens
        .iter()
        .find(|t| t.dotted() == dotted)
        .unwrap_or_else(|| panic!("no token {dotted}"))
}

/// A colour token, or a list of them, as the colours it puts on screen.
pub fn colors(tokens: &[Token], dotted: &str, appearance: &str) -> Vec<Color> {
    match find(tokens, dotted).get(appearance) {
        Val::Color(c) => vec![*c],
        Val::Colors(cs) => cs.clone(),
        other => panic!("{dotted} is not a colour: {other:?}"),
    }
}
