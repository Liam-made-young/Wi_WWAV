//! wi-formula: the formulas of Learn's Database tab (docs/ASK.md).
//!
//! A formula is a spreadsheet formula that names columns instead of cells:
//!
//! ```text
//! IF([Due] < TODAY(), "Late", "On time")
//! SUMIFS(Focus sessions[Minutes], Focus sessions[Course], [Code]) / 60
//! ROUND(AVERAGE(Grades[Percent]), 1)
//! ```
//!
//! - `[Name]`, or a bare `Name` when the name is one word, is this row's
//!   value in that column.
//! - `Table[Name]` is the whole column of a table, this one or another: what
//!   SUM, COUNTIF, SUMIFS and LOOKUP read.
//! - Dates are days since 1 January 1970 in the person's own time zone, with
//!   the time of day as the fraction, so `[Due] - TODAY()` is a number of
//!   days and `TODAY() + 7` is a date.
//!
//! The crate was written for this app rather than taken from a library: the
//! spreadsheet engines with this coverage are GPL or carry a commercial
//! licence, and the language here is small. It reads no clock and no data of
//! its own: a [`Context`] hands it this row, other columns, and today.
//!
//! [`Formula::parse`] reads a formula once; [`Formula::eval`] works it out
//! for one row; [`Formula::refs`] lists the columns it reads, so the caller
//! can order computed columns and refuse a circle.

mod dates;
mod eval;
mod functions;
mod parse;
mod value;

pub use dates::{civil_from_days, days_from_civil, parse_date};
pub use eval::Context;
pub use parse::{Formula, ParseError, Ref};
pub use value::{Error, Value};

/// Every function a formula may call, for the editor's list and for Claude.
pub fn function_names() -> Vec<&'static str> {
    functions::NAMES.to_vec()
}
