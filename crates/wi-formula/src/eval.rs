//! Working a formula out for one row.

use std::cmp::Ordering;
use std::rc::Rc;

use crate::functions;
use crate::parse::{Expr, Op};
use crate::value::{Error, Value};

/// What a formula can see: the row it is on, whole columns, and the day.
pub trait Context {
    /// This row's value in the column called `name` (any case). None when
    /// the table has no such column.
    fn field(&self, name: &str) -> Option<Value>;

    /// A whole column of a table, by their names (any case).
    fn column(&self, table: &str, name: &str) -> Result<Rc<Vec<Value>>, Error>;

    /// Today as a day count in the person's zone.
    fn today(&self) -> f64;

    /// Now as a day count with the time of day.
    fn now(&self) -> f64;

    /// This row's place in its table, from 1.
    fn row_number(&self) -> usize {
        1
    }
}

pub(crate) fn eval(e: &Expr, ctx: &dyn Context) -> Value {
    match e {
        Expr::Literal(v) => v.clone(),
        Expr::Field(name) => ctx.field(name).unwrap_or_else(|| {
            Value::Error(Error::reference(format!(
                "This table has no column called '{name}'."
            )))
        }),
        Expr::Column(table, name) => match ctx.column(table, name) {
            Ok(items) => Value::List(items),
            Err(e) => Value::Error(e),
        },
        Expr::Neg(a) => match eval(a, ctx).number() {
            Ok(n) => Value::Number(-n),
            Err(e) => Value::Error(e),
        },
        Expr::Percent(a) => match eval(a, ctx).number() {
            Ok(n) => Value::Number(n / 100.0),
            Err(e) => Value::Error(e),
        },
        Expr::Binary(op, a, b) => binary(op, eval(a, ctx), eval(b, ctx)),
        Expr::Call(name, args) => functions::call(name, args, ctx),
    }
}

fn binary(op: &Op, a: Value, b: Value) -> Value {
    if let Value::Error(e) = &a {
        return Value::Error(e.clone());
    }
    if let Value::Error(e) = &b {
        return Value::Error(e.clone());
    }
    match op {
        Op::Concat => Value::Text(format!("{}{}", a.to_text(), b.to_text())),
        Op::Eq => Value::Bool(equal(&a, &b)),
        Op::Ne => Value::Bool(!equal(&a, &b)),
        Op::Lt => Value::Bool(compare(&a, &b) == Ordering::Less),
        Op::Le => Value::Bool(compare(&a, &b) != Ordering::Greater),
        Op::Gt => Value::Bool(compare(&a, &b) == Ordering::Greater),
        Op::Ge => Value::Bool(compare(&a, &b) != Ordering::Less),
        Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Pow => {
            let (x, y) = match (a.number(), b.number()) {
                (Ok(x), Ok(y)) => (x, y),
                (Err(e), _) | (_, Err(e)) => return Value::Error(e),
            };
            let date_a = matches!(a, Value::Date(_));
            let date_b = matches!(b, Value::Date(_));
            match op {
                // A date plus days is a date; the gap between two dates is days.
                Op::Add if date_a != date_b => Value::Date(x + y),
                Op::Add => Value::Number(x + y),
                Op::Sub if date_a && !date_b => Value::Date(x - y),
                Op::Sub => Value::Number(x - y),
                Op::Mul => Value::Number(x * y),
                Op::Div if y == 0.0 => Value::Error(Error::div0()),
                Op::Div => Value::Number(x / y),
                _ => {
                    let p = x.powf(y);
                    if p.is_finite() {
                        Value::Number(p)
                    } else {
                        Value::Error(Error::new(
                            "#NUM!",
                            "That power is too large or isn't a real number.",
                        ))
                    }
                }
            }
        }
    }
}

/// Text compares without regard to case, as a spreadsheet's `=` does; a
/// blank equals an empty text and zero.
pub(crate) fn equal(a: &Value, b: &Value) -> bool {
    compare(a, b) == Ordering::Equal
}

fn rank(v: &Value) -> u8 {
    match v {
        Value::Blank => 0,
        Value::Number(_) | Value::Date(_) => 1,
        Value::Text(_) => 2,
        Value::Bool(_) => 3,
        Value::List(_) => 4,
        Value::Error(_) => 5,
    }
}

/// The order a sort, MIN, MAX and the comparisons share: numbers and dates
/// first, then text without regard to case, then true and false.
pub fn compare(a: &Value, b: &Value) -> Ordering {
    use Value::*;
    match (a, b) {
        (Blank, Blank) => Ordering::Equal,
        (Blank, Number(n)) | (Blank, Date(n)) => 0f64.partial_cmp(n).unwrap_or(Ordering::Equal),
        (Number(n), Blank) | (Date(n), Blank) => n.partial_cmp(&0.0).unwrap_or(Ordering::Equal),
        (Blank, Text(s)) => "".cmp(s.as_str()),
        (Text(s), Blank) => s.as_str().cmp(""),
        (Blank, Bool(b)) => false.cmp(b),
        (Bool(b), Blank) => b.cmp(&false),
        (Number(x) | Date(x), Number(y) | Date(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Text(x), Text(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
        (Bool(x), Bool(y)) => x.cmp(y),
        // Text that reads as a number or a date compares as one against a number.
        (Text(s), Number(n) | Date(n)) => match as_number(s) {
            Some(x) => x.partial_cmp(n).unwrap_or(Ordering::Equal),
            None => Ordering::Greater,
        },
        (Number(n) | Date(n), Text(s)) => match as_number(s) {
            Some(y) => n.partial_cmp(&y).unwrap_or(Ordering::Equal),
            None => Ordering::Less,
        },
        _ => rank(a).cmp(&rank(b)),
    }
}

fn as_number(s: &str) -> Option<f64> {
    crate::value::parse_number(s).or_else(|| crate::dates::parse_date(s))
}
