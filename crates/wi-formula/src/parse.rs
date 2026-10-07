//! Reading a formula: tokens, then a tree by precedence climbing.
//!
//! Lowest to highest: comparison (`= <> < <= > >=`), `&`, `+ -`, `* /`, `^`,
//! unary minus, `%`. A leading `=` is allowed and ignored, as a spreadsheet
//! writes it. Arguments are separated by `,` or `;`.

use std::fmt;

use crate::eval::{self, Context};
use crate::functions;
use crate::value::Value;

/// Why a formula couldn't be read: one sentence, and where.
#[derive(Clone, Debug, PartialEq)]
pub struct ParseError {
    pub message: String,
    /// The character the trouble starts at, counted from 0.
    pub at: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// A column a formula reads: this row's (`table` is None) or a whole column
/// of a named table.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ref {
    pub table: Option<String>,
    pub column: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Expr {
    Literal(Value),
    /// This row's value in a column.
    Field(String),
    /// A whole column of a table.
    Column(String, String),
    Neg(Box<Expr>),
    Percent(Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    /// `[Name]`
    Bracket(String),
    Op(&'static str),
    LParen,
    RParen,
    Comma,
}

fn lex(src: &str) -> Result<Vec<(Tok, usize)>, ParseError> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let err = |message: &str, at: usize| ParseError {
        message: message.to_string(),
        at,
    };
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            let mut s = String::new();
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                s.push(chars[i]);
                i += 1;
            }
            // An exponent: 1e3, 2.5E-2.
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < chars.len() && chars[j].is_ascii_digit() {
                    while i < j {
                        s.push(chars[i]);
                        i += 1;
                    }
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        s.push(chars[i]);
                        i += 1;
                    }
                }
            }
            let n: f64 = s
                .parse()
                .map_err(|_| err(&format!("'{s}' isn't a number."), start))?;
            out.push((Tok::Num(n), start));
            continue;
        }
        if c == '"' || c == '\u{201C}' || c == '\u{201D}' {
            // A straight or a curly quote opens; either kind closes.
            let mut s = String::new();
            i += 1;
            loop {
                match chars.get(i) {
                    None => return Err(err("This text has no closing quote.", start)),
                    Some('"') if chars.get(i + 1) == Some(&'"') => {
                        s.push('"');
                        i += 2;
                    }
                    Some('"') | Some('\u{201C}') | Some('\u{201D}') => {
                        i += 1;
                        break;
                    }
                    Some(ch) => {
                        s.push(*ch);
                        i += 1;
                    }
                }
            }
            out.push((Tok::Str(s), start));
            continue;
        }
        if c == '[' {
            let mut s = String::new();
            i += 1;
            loop {
                match chars.get(i) {
                    None => return Err(err("A column name in [ ] has no closing ].", start)),
                    Some(']') => {
                        i += 1;
                        break;
                    }
                    Some(ch) => {
                        s.push(*ch);
                        i += 1;
                    }
                }
            }
            let s = s.trim().trim_start_matches('@').trim().to_string();
            if s.is_empty() {
                return Err(err("[ ] needs a column name inside it.", start));
            }
            out.push((Tok::Bracket(s), start));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut s = String::new();
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
            {
                s.push(chars[i]);
                i += 1;
            }
            out.push((Tok::Ident(s), start));
            continue;
        }
        let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
        let op = match two.as_str() {
            "<>" | "!=" => Some("<>"),
            "<=" => Some("<="),
            ">=" => Some(">="),
            "==" => Some("="),
            _ => None,
        };
        if let Some(op) = op {
            out.push((Tok::Op(op), start));
            i += 2;
            continue;
        }
        let tok = match c {
            '+' => Tok::Op("+"),
            '-' | '\u{2212}' => Tok::Op("-"),
            '*' | '\u{00D7}' => Tok::Op("*"),
            '/' | '\u{00F7}' => Tok::Op("/"),
            '^' => Tok::Op("^"),
            '&' => Tok::Op("&"),
            '=' => Tok::Op("="),
            '<' => Tok::Op("<"),
            '>' => Tok::Op(">"),
            '%' => Tok::Op("%"),
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            ',' | ';' => Tok::Comma,
            other => {
                return Err(err(
                    &format!("'{other}' doesn't belong in a formula."),
                    start,
                ))
            }
        };
        out.push((tok, start));
        i += 1;
    }
    Ok(out)
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    at: usize,
    end: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at).map(|t| &t.0)
    }

    fn pos(&self) -> usize {
        self.toks.get(self.at).map_or(self.end, |t| t.1)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.at).map(|t| t.0.clone());
        self.at += 1;
        t
    }

    fn err<T>(&self, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError {
            message: message.into(),
            at: self.pos(),
        })
    }

    fn op(&self, ops: &[&'static str]) -> Option<&'static str> {
        match self.peek() {
            Some(Tok::Op(o)) => ops.iter().find(|x| *x == o).copied(),
            _ => None,
        }
    }

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.concat()?;
        while let Some(o) = self.op(&["=", "<>", "<", "<=", ">", ">="]) {
            self.at += 1;
            let right = self.concat()?;
            let op = match o {
                "=" => Op::Eq,
                "<>" => Op::Ne,
                "<" => Op::Lt,
                "<=" => Op::Le,
                ">" => Op::Gt,
                _ => Op::Ge,
            };
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn concat(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.sum()?;
        while self.op(&["&"]).is_some() {
            self.at += 1;
            let right = self.sum()?;
            left = Expr::Binary(Op::Concat, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn sum(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.product()?;
        while let Some(o) = self.op(&["+", "-"]) {
            self.at += 1;
            let right = self.product()?;
            let op = if o == "+" { Op::Add } else { Op::Sub };
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn product(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.power()?;
        while let Some(o) = self.op(&["*", "/"]) {
            self.at += 1;
            let right = self.power()?;
            let op = if o == "*" { Op::Mul } else { Op::Div };
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn power(&mut self) -> Result<Expr, ParseError> {
        let left = self.unary()?;
        if self.op(&["^"]).is_some() {
            self.at += 1;
            // Right to left: 2^3^2 is 2^9.
            let right = self.power()?;
            return Ok(Expr::Binary(Op::Pow, Box::new(left), Box::new(right)));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if self.op(&["-"]).is_some() {
            self.at += 1;
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.op(&["+"]).is_some() {
            self.at += 1;
            return self.unary();
        }
        let mut e = self.atom()?;
        while self.op(&["%"]).is_some() {
            self.at += 1;
            e = Expr::Percent(Box::new(e));
        }
        Ok(e)
    }

    fn atom(&mut self) -> Result<Expr, ParseError> {
        let at = self.pos();
        match self.next() {
            None => Err(ParseError {
                message: "The formula stops before it is finished.".to_string(),
                at,
            }),
            Some(Tok::Num(n)) => Ok(Expr::Literal(Value::Number(n))),
            Some(Tok::Str(s)) => Ok(Expr::Literal(Value::Text(s))),
            Some(Tok::Bracket(name)) => Ok(Expr::Field(name)),
            Some(Tok::LParen) => {
                let e = self.comparison()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(e),
                    _ => Err(ParseError {
                        message: "A ( here has no closing ).".to_string(),
                        at,
                    }),
                }
            }
            Some(Tok::Ident(name)) => {
                match self.peek() {
                    Some(Tok::LParen) => {
                        self.at += 1;
                        let mut args = Vec::new();
                        if matches!(self.peek(), Some(Tok::RParen)) {
                            self.at += 1;
                        } else {
                            loop {
                                args.push(self.comparison()?);
                                match self.next() {
                                    Some(Tok::Comma) => continue,
                                    Some(Tok::RParen) => break,
                                    _ => {
                                        return Err(ParseError {
                                            message: format!("{name}( has no closing )."),
                                            at,
                                        })
                                    }
                                }
                            }
                        }
                        let upper = name.to_ascii_uppercase();
                        if !functions::exists(&upper) {
                            return Err(ParseError {
                                message: format!("There is no function called {upper}."),
                                at,
                            });
                        }
                        Ok(Expr::Call(upper, args))
                    }
                    // `Table[Column]`: a whole column.
                    Some(Tok::Bracket(_)) => self.column_of(name),
                    _ => match name.to_ascii_uppercase().as_str() {
                        "TRUE" => Ok(Expr::Literal(Value::Bool(true))),
                        "FALSE" => Ok(Expr::Literal(Value::Bool(false))),
                        _ => {
                            // `tasks.minutes` is the same whole column, written with a dot.
                            match name.split_once('.') {
                                Some((t, c)) if !t.is_empty() && !c.is_empty() => {
                                    Ok(Expr::Column(t.to_string(), c.to_string()))
                                }
                                _ => {
                                    // A table name of several words reaches its bracket here.
                                    let mut words = vec![name];
                                    while let Some(Tok::Ident(more)) = self.peek().cloned() {
                                        self.at += 1;
                                        words.push(more);
                                    }
                                    if matches!(self.peek(), Some(Tok::Bracket(_))) {
                                        return self.column_of(words.join(" "));
                                    }
                                    if words.len() > 1 {
                                        return Err(ParseError {
                                            message: format!(
                                                "Put a column name of more than one word in brackets: [{}].",
                                                words.join(" ")
                                            ),
                                            at,
                                        });
                                    }
                                    Ok(Expr::Field(words.remove(0)))
                                }
                            }
                        }
                    },
                }
            }
            Some(Tok::RParen) => Err(ParseError {
                message: "A ) here has no opening (.".to_string(),
                at,
            }),
            Some(Tok::Comma) => Err(ParseError {
                message: "A comma here separates nothing.".to_string(),
                at,
            }),
            Some(Tok::Op(o)) => Err(ParseError {
                message: format!("'{o}' needs a value before it."),
                at,
            }),
        }
    }

    fn column_of(&mut self, table: String) -> Result<Expr, ParseError> {
        match self.next() {
            Some(Tok::Bracket(col)) => Ok(Expr::Column(table, col)),
            _ => self.err("A table name is followed by its column in [ ]."),
        }
    }
}

/// A formula, read once and worked out for as many rows as asked.
#[derive(Clone, Debug, PartialEq)]
pub struct Formula {
    pub(crate) expr: Expr,
    source: String,
}

impl Formula {
    pub fn parse(source: &str) -> Result<Formula, ParseError> {
        let body = source.trim();
        let body = body.strip_prefix('=').unwrap_or(body);
        if body.trim().is_empty() {
            return Err(ParseError {
                message: "Type a formula first.".to_string(),
                at: 0,
            });
        }
        let toks = lex(body)?;
        let mut p = Parser {
            toks,
            at: 0,
            end: body.chars().count(),
        };
        let expr = p.comparison()?;
        if p.at < p.toks.len() {
            return p.err("The formula goes on after it should have ended. An operator or a comma may be missing.");
        }
        Ok(Formula {
            expr,
            source: source.trim().to_string(),
        })
    }

    /// The formula as it was typed.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The value for one row.
    pub fn eval(&self, ctx: &dyn Context) -> Value {
        eval::eval(&self.expr, ctx)
    }

    /// Every column the formula reads, each once, in a fixed order.
    pub fn refs(&self) -> Vec<Ref> {
        fn walk(e: &Expr, out: &mut Vec<Ref>) {
            match e {
                Expr::Literal(_) => {}
                Expr::Field(c) => out.push(Ref {
                    table: None,
                    column: c.clone(),
                }),
                Expr::Column(t, c) => out.push(Ref {
                    table: Some(t.clone()),
                    column: c.clone(),
                }),
                Expr::Neg(a) | Expr::Percent(a) => walk(a, out),
                Expr::Binary(_, a, b) => {
                    walk(a, out);
                    walk(b, out);
                }
                Expr::Call(_, args) => args.iter().for_each(|a| walk(a, out)),
            }
        }
        let mut out = Vec::new();
        walk(&self.expr, &mut out);
        out.sort();
        out.dedup();
        out
    }
}
