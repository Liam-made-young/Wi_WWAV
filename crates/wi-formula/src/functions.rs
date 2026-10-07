//! The functions a formula may call. Each takes its arguments unevaluated, so
//! IF and IFERROR work out only the branch they need.

use std::cmp::Ordering;

use crate::dates::{
    civil_from_days, days_from_civil, days_in_month, parse_date, time_of, weekday_sunday_first,
};
use crate::eval::{compare, equal, eval, Context};
use crate::parse::Expr;
use crate::value::{format_number, parse_number, Error, Value};

pub(crate) const NAMES: &[&str] = &[
    "ABS",
    "AND",
    "AVERAGE",
    "AVERAGEIF",
    "AVERAGEIFS",
    "AVG",
    "CEILING",
    "CONCAT",
    "CONCATENATE",
    "CONTAINS",
    "COUNT",
    "COUNTA",
    "COUNTBLANK",
    "COUNTIF",
    "COUNTIFS",
    "COUNTUNIQUE",
    "DATE",
    "DATEDIF",
    "DATEVALUE",
    "DAY",
    "DAYS",
    "EDATE",
    "EOMONTH",
    "EXACT",
    "EXP",
    "FILTER",
    "FIND",
    "FLOOR",
    "HOUR",
    "IF",
    "IFBLANK",
    "IFERROR",
    "IFS",
    "INDEX",
    "INT",
    "ISBLANK",
    "ISERROR",
    "ISNUMBER",
    "ISTEXT",
    "LEFT",
    "LEN",
    "LN",
    "LOG",
    "LOG10",
    "LOOKUP",
    "LOWER",
    "MATCH",
    "MAX",
    "MAXIFS",
    "MEDIAN",
    "MID",
    "MIN",
    "MINIFS",
    "MINUTE",
    "MOD",
    "MONTH",
    "NOT",
    "NOW",
    "OR",
    "POWER",
    "PROPER",
    "REPLACE",
    "REPT",
    "RIGHT",
    "ROUND",
    "ROUNDDOWN",
    "ROUNDUP",
    "ROW",
    "SEARCH",
    "SIGN",
    "SQRT",
    "STARTOFWEEK",
    "STDEV",
    "SUBSTITUTE",
    "SUM",
    "SUMIF",
    "SUMIFS",
    "SWITCH",
    "TEXT",
    "TEXTJOIN",
    "TODAY",
    "TRIM",
    "UNIQUE",
    "UPPER",
    "VALUE",
    "WEEKDAY",
    "WEEKNUM",
    "XLOOKUP",
    "XOR",
    "YEAR",
];

pub(crate) fn exists(upper: &str) -> bool {
    NAMES.binary_search(&upper).is_ok()
}

type R = Result<Value, Error>;

fn wrong_count(name: &str, want: &str) -> Error {
    Error::value(format!("{name} takes {want}."))
}

struct Args<'a> {
    name: &'a str,
    exprs: &'a [Expr],
    ctx: &'a dyn Context,
}

impl Args<'_> {
    fn len(&self) -> usize {
        self.exprs.len()
    }

    /// The argument's value; an error value is the function's error.
    fn get(&self, i: usize) -> R {
        match self.raw(i) {
            Value::Error(e) => Err(e),
            v => Ok(v),
        }
    }

    /// The argument's value, errors and all. An argument that was left out
    /// is an error, never a panic.
    fn raw(&self, i: usize) -> Value {
        match self.exprs.get(i) {
            Some(e) => eval(e, self.ctx),
            None => Value::Error(Error::value(format!(
                "{} needs more than {} {}.",
                self.name,
                self.exprs.len(),
                if self.exprs.len() == 1 {
                    "value"
                } else {
                    "values"
                }
            ))),
        }
    }

    fn num(&self, i: usize) -> Result<f64, Error> {
        self.get(i)?.number()
    }

    fn opt_num(&self, i: usize, default: f64) -> Result<f64, Error> {
        if i < self.len() {
            self.num(i)
        } else {
            Ok(default)
        }
    }

    fn text(&self, i: usize) -> Result<String, Error> {
        Ok(self.get(i)?.to_text())
    }

    fn date(&self, i: usize) -> Result<f64, Error> {
        as_date(&self.get(i)?)
    }

    fn need(&self, min: usize, max: usize, want: &str) -> Result<(), Error> {
        if self.len() < min || self.len() > max {
            Err(wrong_count(self.name, want))
        } else {
            Ok(())
        }
    }

    /// Every value of every argument, lists flattened: what SUM reads.
    fn flat(&self) -> Result<Vec<Value>, Error> {
        let mut out = Vec::new();
        for i in 0..self.len() {
            match self.get(i)? {
                Value::List(items) => out.extend(items.iter().cloned()),
                v => out.push(v),
            }
        }
        Ok(out)
    }

    /// The numbers among those values. Text and blanks in a column are
    /// skipped, as a spreadsheet's SUM skips them; an error in it is the answer.
    fn numbers(&self) -> Result<Vec<f64>, Error> {
        let mut out = Vec::new();
        for i in 0..self.len() {
            match self.get(i)? {
                Value::List(items) => {
                    for v in items.iter() {
                        match v {
                            Value::Number(n) | Value::Date(n) => out.push(*n),
                            Value::Error(e) => return Err(e.clone()),
                            Value::Text(s) => {
                                if let Some(n) = parse_number(s) {
                                    out.push(n)
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Value::Blank => {}
                v => out.push(v.number()?),
            }
        }
        Ok(out)
    }
}

fn as_date(v: &Value) -> Result<f64, Error> {
    match v {
        Value::Date(d) | Value::Number(d) => Ok(*d),
        Value::Text(s) => parse_date(s).ok_or_else(|| Error::value(format!("'{s}' isn't a date."))),
        Value::Blank => Err(Error::value("A date is needed, and this is empty.")),
        Value::List(items) if items.len() == 1 => as_date(&items[0]),
        _ => Err(Error::value("A date is needed here.")),
    }
}

fn round_to(x: f64, places: f64, mode: i8) -> f64 {
    let f = 10f64.powf(places.trunc());
    let y = x * f;
    let r = match mode {
        0 => {
            // Half away from zero, as spreadsheets round; a hair of tolerance
            // for values such as 1.005 that binary can't hold exactly.
            let nudged = y + y.signum() * y.abs() * 4.0 * f64::EPSILON;
            nudged.round()
        }
        1 => {
            if y >= 0.0 {
                y.ceil()
            } else {
                y.floor()
            }
        }
        _ => y.trunc(),
    };
    r / f
}

/// One test of COUNTIF and its family: `">=5"`, `"<>done"`, `"jpn*"`, 12, a
/// date. Text matches without regard to case, with `*` and `?` wildcards.
struct Criterion {
    op: &'static str,
    operand: Value,
}

impl Criterion {
    fn new(v: &Value) -> Criterion {
        if let Value::Text(s) = v {
            for op in ["<>", ">=", "<=", "=", ">", "<"] {
                if let Some(rest) = s.strip_prefix(op) {
                    return Criterion {
                        op,
                        operand: read_operand(rest),
                    };
                }
            }
            return Criterion {
                op: "=",
                operand: read_operand(s),
            };
        }
        Criterion {
            op: "=",
            operand: v.clone(),
        }
    }

    fn matches(&self, v: &Value) -> bool {
        if v.is_error() {
            return false;
        }
        match self.op {
            "=" => self.same(v),
            "<>" => !self.same(v),
            op => {
                // An inequality never matches a blank or a value of another sort.
                let comparable = matches!(
                    (v, &self.operand),
                    (
                        Value::Number(_) | Value::Date(_),
                        Value::Number(_) | Value::Date(_)
                    ) | (Value::Text(_), Value::Text(_))
                ) || matches!((v, &self.operand), (Value::Text(s), Value::Number(_) | Value::Date(_)) if parse_number(s).or_else(|| parse_date(s)).is_some());
                if !comparable {
                    return false;
                }
                let o = compare(v, &self.operand);
                match op {
                    ">" => o == Ordering::Greater,
                    ">=" => o != Ordering::Less,
                    "<" => o == Ordering::Less,
                    _ => o != Ordering::Greater,
                }
            }
        }
    }

    fn same(&self, v: &Value) -> bool {
        match (&self.operand, v) {
            (Value::Text(pat), _) if pat.contains('*') || pat.contains('?') => {
                wildcard(&pat.to_lowercase(), &v.to_text().to_lowercase())
            }
            (Value::Text(pat), Value::Blank) => pat.is_empty(),
            (Value::Blank, other) => other.is_blank(),
            _ => equal(&self.operand, v),
        }
    }
}

fn read_operand(s: &str) -> Value {
    let t = s.trim();
    if let Some(n) = parse_number(t) {
        return Value::Number(n);
    }
    if let Some(d) = parse_date(t) {
        return Value::Date(d);
    }
    match t.to_ascii_lowercase().as_str() {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::Text(t.to_string()),
    }
}

fn wildcard(pat: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pat.chars().collect(), text.chars().collect());
    let (mut pi, mut ti, mut star, mut mark) = (0usize, 0usize, None, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn list_of(v: Value) -> Vec<Value> {
    v.items()
}

/// The rows every (range, criterion) pair from `first` on agrees to.
fn matching_rows(a: &Args, first: usize, len: Option<usize>) -> Result<Vec<bool>, Error> {
    let mut keep: Option<Vec<bool>> = len.map(|n| vec![true; n]);
    let mut i = first;
    while i + 1 < a.len() {
        let range = list_of(a.get(i)?);
        let crit = Criterion::new(&a.get(i + 1)?);
        let k = keep.get_or_insert_with(|| vec![true; range.len()]);
        if range.len() != k.len() {
            return Err(Error::value(format!(
                "{} needs columns of the same length. Take them from the same table.",
                a.name
            )));
        }
        for (slot, v) in k.iter_mut().zip(range.iter()) {
            *slot = *slot && crit.matches(v);
        }
        i += 2;
    }
    Ok(keep.unwrap_or_default())
}

fn numbers_where(values: &[Value], keep: &[bool]) -> Result<Vec<f64>, Error> {
    let mut out = Vec::new();
    for (v, k) in values.iter().zip(keep) {
        if !*k {
            continue;
        }
        match v {
            Value::Number(n) | Value::Date(n) => out.push(*n),
            Value::Error(e) => return Err(e.clone()),
            Value::Text(s) => {
                if let Some(n) = parse_number(s) {
                    out.push(n)
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

fn mean(xs: &[f64]) -> R {
    if xs.is_empty() {
        return Err(Error::div0());
    }
    Ok(Value::Number(xs.iter().sum::<f64>() / xs.len() as f64))
}

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn proper(s: &str) -> String {
    let mut out = String::new();
    let mut start = true;
    for c in s.chars() {
        if c.is_alphabetic() {
            if start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            start = false;
        } else {
            out.push(c);
            start = true;
        }
    }
    out
}

/// TEXT's formats: a count of decimals (`"0.00"`, `"0"`), a percentage
/// (`"0%"`, `"0.0%"`), thousands (`"#,##0"`), and date parts (`yyyy`, `mm`,
/// `dd`, `mmm`, `ddd`, `hh`, `nn`).
fn format_as(v: &Value, fmt: &str) -> Result<String, Error> {
    let lower = fmt.to_ascii_lowercase();
    let is_date_fmt =
        lower.contains('y') || lower.contains('d') || lower.contains("mm") || lower.contains('h');
    if is_date_fmt && !lower.contains('0') && !lower.contains('#') {
        let d = as_date(v)?;
        let (y, m, day) = civil_from_days(d);
        let (h, min, s) = time_of(d);
        const MON: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        const DOW: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let wd = (weekday_sunday_first(d) - 1) as usize;
        let mut out = String::new();
        let cs: Vec<char> = lower.chars().collect();
        let mut i = 0;
        while i < cs.len() {
            let c = cs[i];
            let mut n = 1;
            while i + n < cs.len() && cs[i + n] == c {
                n += 1;
            }
            match (c, n) {
                ('y', 2) => out.push_str(&format!("{:02}", y.rem_euclid(100))),
                ('y', _) => out.push_str(&format!("{y:04}")),
                ('m', 1) => out.push_str(&m.to_string()),
                ('m', 2) => out.push_str(&format!("{m:02}")),
                ('m', _) => out.push_str(MON[(m - 1) as usize]),
                ('d', 1) => out.push_str(&day.to_string()),
                ('d', 2) => out.push_str(&format!("{day:02}")),
                ('d', _) => out.push_str(DOW[wd]),
                ('h', 1) => out.push_str(&h.to_string()),
                ('h', _) => out.push_str(&format!("{h:02}")),
                ('n', _) => out.push_str(&format!("{min:02}")),
                ('s', _) => out.push_str(&format!("{s:02}")),
                (other, n) => out.extend(std::iter::repeat(other).take(n)),
            }
            i += n;
        }
        return Ok(out);
    }
    let x = v.number()?;
    let percent = fmt.contains('%');
    let x = if percent { x * 100.0 } else { x };
    let decimals = fmt
        .split('.')
        .nth(1)
        .map_or(0, |d| d.chars().filter(|c| *c == '0' || *c == '#').count());
    let mut s = format!("{:.*}", decimals, round_to(x, decimals as f64, 0));
    if fmt.contains(',') {
        let (whole, frac) = match s.split_once('.') {
            Some((w, f)) => (w.to_string(), Some(f.to_string())),
            None => (s.clone(), None),
        };
        let (sign, digits) = match whole.strip_prefix('-') {
            Some(d) => ("-", d.to_string()),
            None => ("", whole),
        };
        let mut grouped = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                grouped.push(',');
            }
            grouped.push(c);
        }
        s = format!("{sign}{grouped}");
        if let Some(f) = frac {
            s.push('.');
            s.push_str(&f);
        }
    }
    if percent {
        s.push('%');
    }
    Ok(s)
}

pub(crate) fn call(name: &str, exprs: &[Expr], ctx: &dyn Context) -> Value {
    let a = Args { name, exprs, ctx };
    match run(&a) {
        Ok(v) => v,
        Err(e) => Value::Error(e),
    }
}

fn run(a: &Args) -> R {
    let n = Value::Number;
    match a.name {
        // ----- totals -----
        "SUM" => Ok(n(a.numbers()?.iter().sum())),
        "AVERAGE" | "AVG" => mean(&a.numbers()?),
        "MIN" | "MAX" => {
            let vals = a.flat()?;
            let all_dates = vals
                .iter()
                .filter(|v| !v.is_blank())
                .all(|v| matches!(v, Value::Date(_)));
            let xs = a.numbers()?;
            if xs.is_empty() {
                return Ok(n(0.0));
            }
            let x = if a.name == "MIN" {
                xs.iter().cloned().fold(f64::INFINITY, f64::min)
            } else {
                xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            };
            Ok(if all_dates { Value::Date(x) } else { n(x) })
        }
        "COUNT" => Ok(n(a
            .flat()?
            .iter()
            .filter(|v| matches!(v, Value::Number(_) | Value::Date(_)))
            .count() as f64)),
        "COUNTA" => Ok(n(a.flat()?.iter().filter(|v| !v.is_blank()).count() as f64)),
        "COUNTBLANK" => Ok(n(a.flat()?.iter().filter(|v| v.is_blank()).count() as f64)),
        "COUNTUNIQUE" => Ok(n(unique(a.flat()?).len() as f64)),
        "MEDIAN" => {
            let mut xs = a.numbers()?;
            if xs.is_empty() {
                return Err(Error::value("MEDIAN has no numbers to read."));
            }
            xs.sort_by(|x, y| x.partial_cmp(y).unwrap_or(Ordering::Equal));
            let mid = xs.len() / 2;
            Ok(n(if xs.len() % 2 == 1 {
                xs[mid]
            } else {
                (xs[mid - 1] + xs[mid]) / 2.0
            }))
        }
        "STDEV" => {
            let xs = a.numbers()?;
            if xs.len() < 2 {
                return Err(Error::div0());
            }
            let m = xs.iter().sum::<f64>() / xs.len() as f64;
            let var = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
            Ok(n(var.sqrt()))
        }
        "COUNTIF" => {
            a.need(2, 2, "a column and a test")?;
            Ok(n(
                matching_rows(a, 0, None)?.iter().filter(|k| **k).count() as f64
            ))
        }
        "COUNTIFS" => {
            if a.len() < 2 || a.len() % 2 != 0 {
                return Err(wrong_count(a.name, "pairs of a column and a test"));
            }
            Ok(n(
                matching_rows(a, 0, None)?.iter().filter(|k| **k).count() as f64
            ))
        }
        "SUMIF" | "AVERAGEIF" => {
            a.need(2, 3, "a column, a test, and the column to total")?;
            let keep = matching_rows(a, 0, None)?;
            let values = list_of(a.get(if a.len() == 3 { 2 } else { 0 })?);
            if values.len() != keep.len() {
                return Err(Error::value(format!(
                    "{} needs columns of the same length. Take them from the same table.",
                    a.name
                )));
            }
            let xs = numbers_where(&values, &keep)?;
            if a.name == "SUMIF" {
                Ok(n(xs.iter().sum()))
            } else {
                mean(&xs)
            }
        }
        "SUMIFS" | "AVERAGEIFS" | "MINIFS" | "MAXIFS" => {
            if a.len() < 3 || a.len() % 2 != 1 {
                return Err(wrong_count(
                    a.name,
                    "the column to total, then pairs of a column and a test",
                ));
            }
            let values = list_of(a.get(0)?);
            let keep = matching_rows(a, 1, Some(values.len()))?;
            let xs = numbers_where(&values, &keep)?;
            match a.name {
                "SUMIFS" => Ok(n(xs.iter().sum())),
                "AVERAGEIFS" => mean(&xs),
                "MINIFS" => Ok(n(if xs.is_empty() {
                    0.0
                } else {
                    xs.iter().cloned().fold(f64::INFINITY, f64::min)
                })),
                _ => Ok(n(if xs.is_empty() {
                    0.0
                } else {
                    xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                })),
            }
        }

        // ----- logic -----
        "IF" => {
            a.need(2, 3, "a test, a value if true, and a value if false")?;
            if a.get(0)?.truthy()? {
                a.get(1)
            } else if a.len() == 3 {
                a.get(2)
            } else {
                Ok(Value::Bool(false))
            }
        }
        "IFS" => {
            if a.len() < 2 || a.len() % 2 != 0 {
                return Err(wrong_count(a.name, "pairs of a test and a value"));
            }
            let mut i = 0;
            while i < a.len() {
                if a.get(i)?.truthy()? {
                    return a.get(i + 1);
                }
                i += 2;
            }
            Err(Error::missing("No test in IFS was true."))
        }
        "SWITCH" => {
            if a.len() < 3 {
                return Err(wrong_count(
                    a.name,
                    "a value, then pairs of a match and a result",
                ));
            }
            let v = a.get(0)?;
            let mut i = 1;
            while i + 1 < a.len() {
                if equal(&v, &a.get(i)?) {
                    return a.get(i + 1);
                }
                i += 2;
            }
            if i < a.len() {
                return a.get(i);
            }
            Err(Error::missing("SWITCH found no match and has no default."))
        }
        "AND" | "OR" | "XOR" => {
            if a.len() == 0 {
                return Err(wrong_count(a.name, "at least one test"));
            }
            let mut trues = 0;
            let vals = a.flat()?;
            for v in &vals {
                if v.truthy()? {
                    trues += 1;
                }
            }
            Ok(Value::Bool(match a.name {
                "AND" => trues == vals.len(),
                "OR" => trues > 0,
                _ => trues % 2 == 1,
            }))
        }
        "NOT" => {
            a.need(1, 1, "one test")?;
            Ok(Value::Bool(!a.get(0)?.truthy()?))
        }
        "IFERROR" => {
            a.need(1, 2, "a value and what to show if it fails")?;
            match a.raw(0) {
                Value::Error(_) if a.len() == 2 => a.get(1),
                Value::Error(_) => Ok(Value::Blank),
                v => Ok(v),
            }
        }
        "IFBLANK" => {
            a.need(2, 2, "a value and what to show if it is empty")?;
            let v = a.get(0)?;
            if v.is_blank() {
                a.get(1)
            } else {
                Ok(v)
            }
        }
        "ISBLANK" => {
            a.need(1, 1, "one value")?;
            Ok(Value::Bool(a.raw(0).is_blank()))
        }
        "ISERROR" => {
            a.need(1, 1, "one value")?;
            Ok(Value::Bool(a.raw(0).is_error()))
        }
        "ISNUMBER" => {
            a.need(1, 1, "one value")?;
            Ok(Value::Bool(matches!(a.raw(0), Value::Number(_))))
        }
        "ISTEXT" => {
            a.need(1, 1, "one value")?;
            Ok(Value::Bool(
                matches!(a.raw(0), Value::Text(s) if !s.is_empty()),
            ))
        }

        // ----- numbers -----
        "ROUND" | "ROUNDUP" | "ROUNDDOWN" => {
            a.need(1, 2, "a number and how many decimals")?;
            let mode = match a.name {
                "ROUND" => 0,
                "ROUNDUP" => 1,
                _ => 2,
            };
            Ok(n(round_to(a.num(0)?, a.opt_num(1, 0.0)?, mode)))
        }
        "FLOOR" | "CEILING" => {
            a.need(1, 2, "a number and the step to round to")?;
            let (x, step) = (a.num(0)?, a.opt_num(1, 1.0)?);
            if step == 0.0 {
                return Err(Error::div0());
            }
            let q = x / step;
            Ok(n(if a.name == "FLOOR" {
                q.floor()
            } else {
                q.ceil()
            } * step))
        }
        "INT" => {
            a.need(1, 1, "one number")?;
            Ok(n(a.num(0)?.floor()))
        }
        "ABS" => {
            a.need(1, 1, "one number")?;
            Ok(n(a.num(0)?.abs()))
        }
        "SIGN" => {
            a.need(1, 1, "one number")?;
            let x = a.num(0)?;
            Ok(n(if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            }))
        }
        "MOD" => {
            a.need(2, 2, "a number and what to divide it by")?;
            let (x, y) = (a.num(0)?, a.num(1)?);
            if y == 0.0 {
                return Err(Error::div0());
            }
            Ok(n(x - y * (x / y).floor()))
        }
        "POWER" => {
            a.need(2, 2, "a number and a power")?;
            Ok(n(a.num(0)?.powf(a.num(1)?)))
        }
        "SQRT" => {
            a.need(1, 1, "one number")?;
            let x = a.num(0)?;
            if x < 0.0 {
                return Err(Error::new(
                    "#NUM!",
                    "SQRT needs a number that isn't negative.",
                ));
            }
            Ok(n(x.sqrt()))
        }
        "EXP" => {
            a.need(1, 1, "one number")?;
            Ok(n(a.num(0)?.exp()))
        }
        "LN" | "LOG10" | "LOG" => {
            a.need(
                1,
                if a.name == "LOG" { 2 } else { 1 },
                "a number above zero",
            )?;
            let x = a.num(0)?;
            if x <= 0.0 {
                return Err(Error::new(
                    "#NUM!",
                    format!("{} needs a number above zero.", a.name),
                ));
            }
            Ok(n(match a.name {
                "LN" => x.ln(),
                "LOG10" => x.log10(),
                _ => x.log(a.opt_num(1, 10.0)?),
            }))
        }

        // ----- dates -----
        "TODAY" => Ok(Value::Date(a.ctx.today())),
        "NOW" => Ok(Value::Date(a.ctx.now())),
        "DATE" => {
            a.need(3, 3, "a year, a month and a day")?;
            let (y, m, d) = (a.num(0)?, a.num(1)?, a.num(2)?);
            Ok(Value::Date(
                days_from_civil(y as i64, m as i64, 1) + d.trunc() - 1.0,
            ))
        }
        "DATEVALUE" => {
            a.need(1, 1, "a date as text")?;
            Ok(Value::Date(a.date(0)?))
        }
        "YEAR" | "MONTH" | "DAY" => {
            a.need(1, 1, "one date")?;
            let (y, m, d) = civil_from_days(a.date(0)?);
            Ok(n(match a.name {
                "YEAR" => y,
                "MONTH" => m,
                _ => d,
            } as f64))
        }
        "HOUR" | "MINUTE" => {
            a.need(1, 1, "one date with a time")?;
            let (h, m, _) = time_of(a.date(0)?);
            Ok(n(if a.name == "HOUR" { h } else { m } as f64))
        }
        "WEEKDAY" => {
            a.need(1, 2, "a date, and 2 to start the week on Monday")?;
            let sunday_first = weekday_sunday_first(a.date(0)?);
            Ok(n(match a.opt_num(1, 1.0)? as i64 {
                2 => (sunday_first + 5) % 7 + 1,
                3 => (sunday_first + 5) % 7,
                _ => sunday_first,
            } as f64))
        }
        "WEEKNUM" => {
            a.need(1, 1, "one date")?;
            let d = a.date(0)?.floor();
            let (y, _, _) = civil_from_days(d);
            let jan1 = days_from_civil(y, 1, 1);
            let offset = (weekday_sunday_first(jan1) - 1) as f64;
            Ok(n(((d - jan1 + offset) / 7.0).floor() + 1.0))
        }
        "STARTOFWEEK" => {
            a.need(1, 2, "a date, and 2 to start the week on Monday")?;
            let d = a.date(0)?.floor();
            let sunday_first = weekday_sunday_first(d);
            let back = if a.opt_num(1, 1.0)? as i64 == 2 {
                (sunday_first + 5) % 7
            } else {
                sunday_first - 1
            };
            Ok(Value::Date(d - back as f64))
        }
        "DAYS" => {
            a.need(2, 2, "an end date and a start date")?;
            Ok(n(a.date(0)?.floor() - a.date(1)?.floor()))
        }
        "DATEDIF" => {
            a.need(
                2,
                3,
                "a start date, an end date and a unit: \"d\", \"m\" or \"y\"",
            )?;
            let (s, e) = (a.date(0)?.floor(), a.date(1)?.floor());
            let unit = if a.len() == 3 {
                a.text(2)?.to_ascii_lowercase()
            } else {
                "d".to_string()
            };
            let ((y1, m1, d1), (y2, m2, d2)) = (civil_from_days(s), civil_from_days(e));
            let months = (y2 - y1) * 12 + (m2 - m1) - if d2 < d1 { 1 } else { 0 };
            match unit.as_str() {
                "d" => Ok(n(e - s)),
                "w" => Ok(n(((e - s) / 7.0).trunc())),
                "m" => Ok(n(months as f64)),
                "y" => Ok(n((months / 12) as f64)),
                other => Err(Error::value(format!(
                    "DATEDIF's unit is \"d\", \"w\", \"m\" or \"y\", not \"{other}\"."
                ))),
            }
        }
        "EDATE" | "EOMONTH" => {
            a.need(2, 2, "a date and a number of months")?;
            let (y, m, d) = civil_from_days(a.date(0)?);
            let months = y * 12 + (m - 1) + a.num(1)?.trunc() as i64;
            let (y2, m2) = (months.div_euclid(12), months.rem_euclid(12) + 1);
            let last = days_in_month(y2, m2);
            let day = if a.name == "EOMONTH" {
                last
            } else {
                d.min(last)
            };
            Ok(Value::Date(days_from_civil(y2, m2, day)))
        }

        // ----- text -----
        "CONCAT" | "CONCATENATE" => Ok(Value::Text(
            a.flat()?.iter().map(Value::to_text).collect::<String>(),
        )),
        "TEXTJOIN" => {
            if a.len() < 2 {
                return Err(wrong_count(a.name, "a separator, then the values to join"));
            }
            let sep = a.text(0)?;
            let mut parts = Vec::new();
            for i in 1..a.len() {
                for v in a.get(i)?.items() {
                    if !v.is_blank() {
                        parts.push(v.to_text());
                    }
                }
            }
            Ok(Value::Text(parts.join(&sep)))
        }
        "LEN" => {
            a.need(1, 1, "one text")?;
            Ok(n(a.text(0)?.chars().count() as f64))
        }
        "LEFT" | "RIGHT" => {
            a.need(1, 2, "a text and how many characters")?;
            let s = chars(&a.text(0)?);
            let k = (a.opt_num(1, 1.0)?.max(0.0) as usize).min(s.len());
            Ok(Value::Text(if a.name == "LEFT" {
                s[..k].iter().collect()
            } else {
                s[s.len() - k..].iter().collect()
            }))
        }
        "MID" => {
            a.need(
                3,
                3,
                "a text, where to start (from 1) and how many characters",
            )?;
            let s = chars(&a.text(0)?);
            let start = (a.num(1)?.max(1.0) as usize - 1).min(s.len());
            let k = (a.num(2)?.max(0.0) as usize).min(s.len() - start);
            Ok(Value::Text(s[start..start + k].iter().collect()))
        }
        "UPPER" => Ok(Value::Text(a.text(0)?.to_uppercase())),
        "LOWER" => Ok(Value::Text(a.text(0)?.to_lowercase())),
        "PROPER" => Ok(Value::Text(proper(&a.text(0)?))),
        "TRIM" => {
            a.need(1, 1, "one text")?;
            Ok(Value::Text(
                a.text(0)?.split_whitespace().collect::<Vec<_>>().join(" "),
            ))
        }
        "SUBSTITUTE" => {
            a.need(3, 3, "a text, what to find and what to put instead")?;
            let (s, old, new) = (a.text(0)?, a.text(1)?, a.text(2)?);
            Ok(Value::Text(if old.is_empty() {
                s
            } else {
                s.replace(&old, &new)
            }))
        }
        "REPLACE" => {
            a.need(
                4,
                4,
                "a text, where to start (from 1), how many characters and the new text",
            )?;
            let s = chars(&a.text(0)?);
            let start = (a.num(1)?.max(1.0) as usize - 1).min(s.len());
            let k = (a.num(2)?.max(0.0) as usize).min(s.len() - start);
            let mut out: String = s[..start].iter().collect();
            out.push_str(&a.text(3)?);
            out.extend(s[start + k..].iter());
            Ok(Value::Text(out))
        }
        "FIND" | "SEARCH" => {
            a.need(2, 2, "what to find and the text to look in")?;
            let (needle, hay) = (a.text(0)?, a.text(1)?);
            let (needle, hay) = if a.name == "SEARCH" {
                (needle.to_lowercase(), hay.to_lowercase())
            } else {
                (needle, hay)
            };
            match hay.find(&needle) {
                Some(byte) => Ok(n(hay[..byte].chars().count() as f64 + 1.0)),
                None => Err(Error::missing(format!("'{needle}' isn't in that text."))),
            }
        }
        "CONTAINS" => {
            a.need(2, 2, "a text and what to look for in it")?;
            Ok(Value::Bool(
                a.text(0)?
                    .to_lowercase()
                    .contains(&a.text(1)?.to_lowercase()),
            ))
        }
        "EXACT" => {
            a.need(2, 2, "two texts")?;
            Ok(Value::Bool(a.text(0)? == a.text(1)?))
        }
        "REPT" => {
            a.need(2, 2, "a text and how many times")?;
            let times = a.num(1)?.max(0.0) as usize;
            let s = a.text(0)?;
            if s.len().saturating_mul(times) > 10_000 {
                return Err(Error::value("REPT would make more than 10,000 characters."));
            }
            Ok(Value::Text(s.repeat(times)))
        }
        "VALUE" => {
            a.need(1, 1, "one text")?;
            let v = a.get(0)?;
            match &v {
                Value::Text(s) => match parse_number(s) {
                    Some(x) => Ok(n(x)),
                    None => parse_date(s)
                        .map(Value::Date)
                        .ok_or_else(|| Error::value(format!("'{s}' isn't a number or a date."))),
                },
                _ => Ok(n(v.number()?)),
            }
        }
        "TEXT" => {
            a.need(1, 2, "a value and a format such as \"0.0\" or \"mmm d\"")?;
            let v = a.get(0)?;
            if a.len() == 1 {
                return Ok(Value::Text(v.to_text()));
            }
            Ok(Value::Text(format_as(&v, &a.text(1)?)?))
        }

        // ----- lookups -----
        "LOOKUP" | "XLOOKUP" => {
            a.need(3, 4, "a value, the column to find it in, the column to answer from, and what to show if it isn't there")?;
            let (v, keys, answers) = (a.get(0)?, list_of(a.get(1)?), list_of(a.get(2)?));
            if keys.len() != answers.len() {
                return Err(Error::value(
                    "LOOKUP needs two columns of the same length. Take them from the same table.",
                ));
            }
            match keys.iter().position(|k| !k.is_blank() && equal(k, &v)) {
                Some(i) => Ok(answers[i].clone()),
                None if a.len() == 4 => a.get(3),
                None => Err(Error::missing(format!(
                    "LOOKUP didn't find '{}'.",
                    v.to_text()
                ))),
            }
        }
        "MATCH" => {
            a.need(2, 2, "a value and the column to find it in")?;
            let (v, keys) = (a.get(0)?, list_of(a.get(1)?));
            match keys.iter().position(|k| equal(k, &v)) {
                Some(i) => Ok(n(i as f64 + 1.0)),
                None => Err(Error::missing(format!(
                    "MATCH didn't find '{}'.",
                    v.to_text()
                ))),
            }
        }
        "INDEX" => {
            a.need(2, 2, "a column and a place in it, from 1")?;
            let (items, i) = (list_of(a.get(0)?), a.num(1)?);
            if i < 1.0 || i as usize > items.len() {
                return Err(Error::reference(format!(
                    "INDEX asked for place {} of {}.",
                    format_number(i),
                    items.len()
                )));
            }
            Ok(items[i as usize - 1].clone())
        }
        "FILTER" => {
            a.need(2, 2, "a column and a column of tests")?;
            let (items, tests) = (list_of(a.get(0)?), list_of(a.get(1)?));
            if items.len() != tests.len() {
                return Err(Error::value(
                    "FILTER needs two columns of the same length. Take them from the same table.",
                ));
            }
            let mut out = Vec::new();
            for (v, t) in items.into_iter().zip(tests.iter()) {
                if t.truthy()? {
                    out.push(v);
                }
            }
            Ok(Value::list(out))
        }
        "UNIQUE" => Ok(Value::list(unique(a.flat()?))),
        "ROW" => Ok(n(a.ctx.row_number() as f64)),

        other => Err(Error::name(format!("There is no function called {other}."))),
    }
}

fn unique(values: Vec<Value>) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for v in values {
        if v.is_blank() {
            continue;
        }
        if !out.iter().any(|seen| equal(seen, &v)) {
            out.push(v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_of_names_is_sorted_for_the_search() {
        let mut sorted = NAMES.to_vec();
        sorted.sort();
        assert_eq!(sorted, NAMES);
    }

    #[test]
    fn wildcards_match_as_a_spreadsheet_does() {
        assert!(wildcard("jpn*", "jpn 101"));
        assert!(wildcard("*101", "jpn 101"));
        assert!(wildcard("j?n*", "jpn 101"));
        assert!(!wildcard("jpn", "jpn 101"));
        assert!(wildcard("*", ""));
    }
}
