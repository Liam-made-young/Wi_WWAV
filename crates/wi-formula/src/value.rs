//! What a cell or a formula can hold.

use std::fmt;
use std::rc::Rc;

use crate::dates;

/// A formula's failure, as a spreadsheet shows it: a short code for the cell
/// and one sentence for the person.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    /// `#DIV/0!`, `#NAME?`, `#VALUE!`, `#REF!`, `#N/A` or `#CYCLE!`.
    pub code: &'static str,
    pub message: String,
}

impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Error {
        Error {
            code,
            message: message.into(),
        }
    }

    pub fn value(message: impl Into<String>) -> Error {
        Error::new("#VALUE!", message)
    }

    pub fn name(message: impl Into<String>) -> Error {
        Error::new("#NAME?", message)
    }

    pub fn reference(message: impl Into<String>) -> Error {
        Error::new("#REF!", message)
    }

    pub fn missing(message: impl Into<String>) -> Error {
        Error::new("#N/A", message)
    }

    pub fn div0() -> Error {
        Error::new("#DIV/0!", "Divided by zero.")
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.code, self.message)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Blank,
    Number(f64),
    Text(String),
    Bool(bool),
    /// Days since 1970-01-01 in the person's zone; the fraction is the time.
    Date(f64),
    /// A whole column, or what FILTER and UNIQUE give back.
    List(Rc<Vec<Value>>),
    Error(Error),
}

impl Value {
    pub fn text(s: impl Into<String>) -> Value {
        Value::Text(s.into())
    }

    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::new(items))
    }

    pub fn is_blank(&self) -> bool {
        match self {
            Value::Blank => true,
            Value::Text(s) => s.is_empty(),
            _ => false,
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Value::Error(_))
    }

    /// The value as a number, the way arithmetic reads it: blank is 0, TRUE
    /// is 1, a date is its day count, and text is read if it is a number.
    pub fn number(&self) -> Result<f64, Error> {
        match self {
            Value::Blank => Ok(0.0),
            Value::Number(n) | Value::Date(n) => Ok(*n),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::Text(s) => {
                let t = s.trim();
                if t.is_empty() {
                    return Ok(0.0);
                }
                parse_number(t)
                    .ok_or_else(|| Error::value(format!("'{s}' is text, not a number.")))
            }
            Value::List(items) if items.len() == 1 => items[0].number(),
            Value::List(_) => Err(Error::value(
                "A whole column was used where one value was needed. Wrap it in SUM, AVERAGE or another total.",
            )),
            Value::Error(e) => Err(e.clone()),
        }
    }

    /// The value as true or false, the way IF reads it.
    pub fn truthy(&self) -> Result<bool, Error> {
        match self {
            Value::Blank => Ok(false),
            Value::Bool(b) => Ok(*b),
            Value::Number(n) | Value::Date(n) => Ok(*n != 0.0),
            Value::Text(s) => match s.trim().to_ascii_lowercase().as_str() {
                "true" | "yes" => Ok(true),
                "false" | "no" | "" => Ok(false),
                _ => Err(Error::value(format!("'{s}' is text, not true or false."))),
            },
            Value::List(items) if items.len() == 1 => items[0].truthy(),
            Value::List(_) => Err(Error::value(
                "A whole column was used where true or false was needed.",
            )),
            Value::Error(e) => Err(e.clone()),
        }
    }

    /// The value as text, the way `&` and the text functions read it.
    pub fn to_text(&self) -> String {
        match self {
            Value::Blank => String::new(),
            Value::Number(n) => format_number(*n),
            Value::Text(s) => s.clone(),
            Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            Value::Date(d) => dates::format_date(*d),
            Value::List(items) => items
                .iter()
                .map(Value::to_text)
                .collect::<Vec<_>>()
                .join(", "),
            Value::Error(e) => e.code.to_string(),
        }
    }

    /// The values a total reads: a list's own, or the one value.
    pub fn items(&self) -> Vec<Value> {
        match self {
            Value::List(items) => items.as_ref().clone(),
            other => vec![other.clone()],
        }
    }
}

/// A number as a person writes it: `1,234.5`, `45%`, `$12`.
pub fn parse_number(s: &str) -> Option<f64> {
    let t = s.trim();
    let (t, percent) = match t.strip_suffix('%') {
        Some(rest) => (rest.trim(), true),
        None => (t, false),
    };
    let t = t.strip_prefix('$').unwrap_or(t);
    if t.is_empty() || !t.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+' | 'e' | 'E'))
    {
        return None;
    }
    let clean: String = t.chars().filter(|c| *c != ',').collect();
    let n: f64 = clean.parse().ok()?;
    if !n.is_finite() {
        return None;
    }
    Some(if percent { n / 100.0 } else { n })
}

/// A number as a cell shows it: whole numbers without a fraction, the rest
/// to at most ten significant decimals, with no trailing zeros.
pub fn format_number(n: f64) -> String {
    if !n.is_finite() {
        return "#NUM!".to_string();
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        return format!("{}", n as i64);
    }
    let s = format!("{n:.10}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}
