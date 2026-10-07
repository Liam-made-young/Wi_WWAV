//! wi-heat-store: Heat's reads and writes on `library.sqlite`
//! (docs/HEAT.md, docs/SPEC.md 3.16, 8.8).
//!
//! The core's `heat.*` commands and `wi-mcp`'s tools both call these
//! functions, so a rule such as "clamp 5–600" or "the same `source_id` twice
//! makes one task" exists once. Each write is one transaction through
//! `wi-store`'s journal, labelled, with who made it. The maths is
//! `wi_heat::model`'s; this crate never reads the clock (a [`Clock`] comes
//! in) and never touches the network or the Keychain.
//!
//! - [`mcp`]: the eight tools' store functions, by Claude.
//! - [`ops`]: what the views write (put, patch, done, blocks, capture, scores,
//!   the Public switch, shares, the weekly review, moving in), by the person.
//! - [`timer`]: Plan my day's drafts, the current task and the focus timer.
//! - [`snapshot`]: what the views read, and the public view.
//! - [`feed`]: calendars and Brightspace, from a feed the core fetched.

mod derive;
pub mod feed;
pub mod mcp;
pub mod ops;
mod schema;
pub mod snapshot;
pub mod timer;

use std::collections::BTreeMap;

use jiff::tz::TimeZone;
use jiff::Timestamp;
use serde_json::{json, Map, Value};
use wi_store::{Actor, Room, Store, Txn};

/// Every Heat change undoes in Heat.
pub const ROOM: Room = Room::Heat;

/// The kinds Heat keeps in the `docs` table (docs/HEAT.md).
pub mod kind {
    pub const SPACE: &str = "space";
    pub const TASK: &str = "task";
    pub const OCCURRENCE: &str = "taskOccurrence";
    pub const BLOCK: &str = "timeBlock";
    pub const FOCUS: &str = "focusSession";
    pub const PROJECT: &str = "project";
    pub const MILESTONE: &str = "milestone";
    pub const HABIT: &str = "habit";
    pub const TERM: &str = "term";
    pub const COURSE: &str = "course";
    pub const GRADE: &str = "grade";
    pub const MAIL: &str = "mailThread";
    pub const CALENDAR: &str = "calendar";
    pub const CAPTURE: &str = "capture";
    pub const DAILY_NOTE: &str = "dailyNote";
    pub const NOTE: &str = "note";
    pub const SHARE: &str = "profileShare";
    /// Outside the journal: the timer, the current task and plan drafts.
    pub const STATE: &str = "heatState";
    /// Outside the journal: Settings → Claude's switches, the School sheet.
    pub const SETTING: &str = "heatSetting";
    /// Outside the journal: what other calendars' feeds hold.
    pub const EVENT: &str = "calendarEvent";
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// One plain sentence for the person, or for Claude.
    #[error("{0}")]
    Refused(String),
    #[error(transparent)]
    Store(#[from] wi_store::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn refused<T>(sentence: impl Into<String>) -> Result<T> {
    Err(Error::Refused(sentence.into()))
}

/// Now, and the person's time zone: the only clock this crate reads.
#[derive(Clone, Debug)]
pub struct Clock {
    pub now_ms: f64,
    pub zone: TimeZone,
}

impl Clock {
    /// The system's clock and time zone.
    pub fn system() -> Clock {
        Clock {
            now_ms: Timestamp::now().as_millisecond() as f64,
            zone: TimeZone::system(),
        }
    }

    pub fn at(now_ms: f64, zone: TimeZone) -> Clock {
        Clock { now_ms, zone }
    }

    /// An instant as ISO 8601 with the local offset, as the tools speak:
    /// `2026-10-07T23:59:00-04:00`.
    pub fn iso(&self, ms: f64) -> String {
        match Timestamp::from_millisecond(ms as i64) {
            Ok(t) => t
                .to_zoned(self.zone.clone())
                .strftime("%Y-%m-%dT%H:%M:%S%:z")
                .to_string(),
            Err(_) => String::new(),
        }
    }

    /// Today's date in the person's zone, `YYYY-MM-DD`.
    pub fn today(&self) -> String {
        self.date_of(self.now_ms)
    }

    pub fn date_of(&self, ms: f64) -> String {
        match Timestamp::from_millisecond(ms as i64) {
            Ok(t) => t
                .to_zoned(self.zone.clone())
                .strftime("%Y-%m-%d")
                .to_string(),
            Err(_) => String::new(),
        }
    }
}

/// Reads an ISO 8601 instant with an offset into epoch milliseconds.
pub fn parse_instant(s: &str) -> Result<f64> {
    match s.parse::<Timestamp>() {
        Ok(t) => Ok(t.as_millisecond() as f64),
        Err(_) => refused(format!(
            "'{s}' isn't a time Heat can read. Use ISO 8601 with an offset, such as 2026-10-07T23:59:00-04:00."
        )),
    }
}

/// A number as JSON the way the TypeScript writes it: whole numbers without
/// a fraction.
pub(crate) fn num(x: f64) -> Value {
    if x.is_finite() && x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
        json!(x as i64)
    } else if x.is_finite() {
        json!(x)
    } else {
        Value::Null
    }
}

/// Rounds to `places` decimals for what Claude reads.
pub(crate) fn round(x: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    (x * f).round() / f
}

/// Every record of `kind`, as stored, in the order they were made.
pub fn all(store: &Store, kind: &str) -> Result<Vec<Value>> {
    Ok(store
        .docs_oldest_first(kind)?
        .into_iter()
        .map(|d| d.json)
        .collect())
}

/// One record of `kind`, as stored.
pub fn one(store: &Store, kind: &str, id: &str) -> Result<Option<Value>> {
    Ok(store.doc(kind, id)?.map(|d| d.json))
}

/// What ⌘K's search reads for a record.
pub(crate) fn search_text(kind: &str, r: &Value) -> String {
    let field = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or("");
    let parts: Vec<&str> = match kind {
        kind::TASK => vec![field("title"), field("notes")],
        kind::MAIL => vec![field("subject"), field("from"), field("course")],
        kind::GRADE | kind::PROJECT | kind::MILESTONE | kind::HABIT => vec![field("title")],
        kind::COURSE => vec![field("code"), field("name")],
        kind::NOTE => vec![field("title"), field("markdown")],
        kind::DAILY_NOTE => vec![field("markdown")],
        kind::CAPTURE => vec![field("text")],
        kind::SPACE => vec![field("name")],
        _ => vec![],
    };
    parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A new record's id.
pub(crate) fn ulid() -> String {
    wwav_ids::ulid()
}

/// Puts one record inside an open change.
pub(crate) fn put(txn: &mut Txn<'_>, kind: &str, record: &Value) -> Result<()> {
    let id = record
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Refused(format!("A {kind} record needs an id.")))?;
    txn.put_doc(kind, id, record, &search_text(kind, record))?;
    Ok(())
}

/// One journal entry just made: its id, and the Edit menu's text for it.
pub(crate) struct Committed {
    pub undo: Option<String>,
    pub txn: Option<String>,
}

/// Runs `f` as one labelled change by `actor`. Nothing changed (nothing to
/// undo) comes back as None, and no entry is made.
pub(crate) fn commit(
    store: &mut Store,
    label: &str,
    actor: Actor,
    f: impl FnOnce(&mut Txn<'_>) -> Result<()>,
) -> Result<Committed> {
    let mut txn = store.begin_by(ROOM, label, actor)?;
    f(&mut txn)?;
    let id = txn.commit()?;
    Ok(Committed {
        undo: id.as_ref().map(|_| format!("Undo {label}")),
        txn: id,
    })
}

/// Runs `f` as one labelled change by `actor`, and returns the Edit menu's
/// text for it ("Undo Claude's task"), or None when nothing changed.
pub(crate) fn change(
    store: &mut Store,
    label: &str,
    actor: Actor,
    f: impl FnOnce(&mut Txn<'_>) -> Result<()>,
) -> Result<Option<String>> {
    Ok(commit(store, label, actor, f)?.undo)
}

/// What a view's write gives back: the answer, the Edit menu's text for the
/// change, and where the change is, so the core can tell the views and sync.
pub struct Outcome {
    /// The command's answer, without `undo`.
    pub result: Value,
    /// "Undo add task"; None when nothing changed.
    pub undo: Option<String>,
    /// The journal entry the write made, if it made one.
    pub txn: Option<String>,
    /// Kinds written outside the journal (the timer, a setting), for the
    /// `heat` event.
    pub outside: Vec<&'static str>,
    /// Whether the answer carries `undo` (the commands that answer `{}` don't).
    pub with_undo: bool,
}

impl Outcome {
    pub(crate) fn new(result: Value, c: Committed) -> Outcome {
        Outcome {
            result,
            undo: c.undo,
            txn: c.txn,
            outside: Vec::new(),
            with_undo: true,
        }
    }

    /// A write that changed nothing the journal keeps.
    pub(crate) fn unchanged(result: Value) -> Outcome {
        Outcome {
            result,
            undo: None,
            txn: None,
            outside: Vec::new(),
            with_undo: true,
        }
    }

    /// A write outside the journal, answered with `result` alone.
    pub(crate) fn outside(result: Value, kinds: &[&'static str]) -> Outcome {
        Outcome {
            result,
            undo: None,
            txn: None,
            outside: kinds.to_vec(),
            with_undo: false,
        }
    }

    pub(crate) fn also(mut self, kinds: &[&'static str]) -> Outcome {
        self.outside.extend_from_slice(kinds);
        self
    }

    /// The answer as the command sends it: `{..., undo}` or the bare result.
    pub fn value(self) -> Value {
        let mut v = self.result;
        if self.with_undo {
            if let Some(m) = v.as_object_mut() {
                m.insert("undo".into(), self.undo.map_or(Value::Null, Value::String));
            }
        }
        v
    }
}

/// The eight tools, every one on until switched off in Settings → Claude.
pub fn tool_switches(store: &Store) -> Result<BTreeMap<String, bool>> {
    let saved = one(store, kind::SETTING, "claude.tools")?.unwrap_or(Value::Null);
    Ok(mcp::TOOLS
        .iter()
        .map(|t| {
            (
                t.to_string(),
                saved.get(*t).and_then(Value::as_bool).unwrap_or(true),
            )
        })
        .collect())
}

/// Switches one tool on or off. Not a journal entry: it's a setting.
pub fn set_tool(store: &mut Store, name: &str, on: bool) -> Result<()> {
    if !mcp::TOOLS.contains(&name) {
        return refused(format!("There's no tool called {name}."));
    }
    let mut saved: Map<String, Value> = one(store, kind::SETTING, "claude.tools")?
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    saved.insert(name.to_string(), Value::Bool(on));
    store.set_doc(kind::SETTING, "claude.tools", &Value::Object(saved), "")?;
    Ok(())
}

/// The one `heatState` record's key (docs/HEAT.md).
pub const STATE_KEY: &str = "state";

/// Heat's state outside the journal: `{currentTaskId?, timer, planDrafts}`,
/// and the focus timer's whole state under `focus` (docs/HEAT.md).
pub fn state(store: &Store) -> Result<Value> {
    Ok(one(store, kind::STATE, STATE_KEY)?.unwrap_or_else(
        || json!({"timer": {"phase": "idle", "round": 1, "endsAt": null}, "planDrafts": []}),
    ))
}

pub(crate) fn set_state(store: &mut Store, state: &Value) -> Result<()> {
    store.set_doc(kind::STATE, STATE_KEY, state, "")?;
    Ok(())
}

/// A setting outside the journal, or None. Settings are `heatSetting`
/// records: `claude.tools`, `school`, `dayEnds` and the feeds' bookkeeping.
pub fn setting(store: &Store, key: &str) -> Result<Option<Value>> {
    one(store, kind::SETTING, key)
}

pub fn set_setting(store: &mut Store, key: &str, value: &Value) -> Result<()> {
    store.set_doc(kind::SETTING, key, value, "")?;
    Ok(())
}
