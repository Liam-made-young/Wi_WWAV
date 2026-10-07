//! Reading a command's args. A command missing what it needs answers
//! `bad_args` with a sentence naming it, never a panic.

use serde_json::{Map, Value};
use wi_store::Room;

use crate::CoreError;

pub(crate) struct Args<'a> {
    cmd: &'a str,
    v: &'a Value,
}

impl<'a> Args<'a> {
    pub fn new(cmd: &'a str, v: &'a Value) -> Args<'a> {
        Args { cmd, v }
    }

    fn missing(&self, key: &str, what: &str) -> CoreError {
        CoreError::new("bad_args", format!("{} needs {key}, {what}.", self.cmd))
    }

    pub fn get(&self, key: &str) -> Option<&'a Value> {
        self.v.get(key).filter(|v| !v.is_null())
    }

    /// The key as sent: a `null` is told from a key left out.
    pub fn raw(&self, key: &str) -> Option<&'a Value> {
        self.v.get(key)
    }

    /// All the args, as an object (empty if they weren't one).
    pub fn object(&self) -> Map<String, Value> {
        self.v.as_object().cloned().unwrap_or_default()
    }

    pub fn str(&self, key: &str) -> Result<&'a str, CoreError> {
        self.get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| self.missing(key, "a string"))
    }

    pub fn opt_str(&self, key: &str) -> Option<&'a str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn f64(&self, key: &str) -> Result<f64, CoreError> {
        self.get(key)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
            .ok_or_else(|| self.missing(key, "a number"))
    }

    pub fn opt_f64(&self, key: &str) -> Result<Option<f64>, CoreError> {
        match self.get(key) {
            None => Ok(None),
            Some(_) => self.f64(key).map(Some),
        }
    }

    pub fn opt_bool(&self, key: &str) -> Result<Option<bool>, CoreError> {
        match self.get(key) {
            None => Ok(None),
            Some(v) => v
                .as_bool()
                .map(Some)
                .ok_or_else(|| self.missing(key, "true or false")),
        }
    }

    pub fn opt_usize(&self, key: &str) -> Result<Option<usize>, CoreError> {
        match self.get(key) {
            None => Ok(None),
            Some(v) => v
                .as_u64()
                .map(|n| Some(n as usize))
                .ok_or_else(|| self.missing(key, "a whole number")),
        }
    }

    /// A list of strings, such as `ids` or `paths`.
    pub fn strings(&self, key: &str) -> Result<Vec<String>, CoreError> {
        let list = self
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| self.missing(key, "a list"))?;
        list.iter()
            .map(|v| v.as_str().map(String::from))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| self.missing(key, "a list of strings"))
    }

    pub fn opt_strings(&self, key: &str) -> Result<Vec<String>, CoreError> {
        match self.get(key) {
            None => Ok(Vec::new()),
            Some(_) => self.strings(key),
        }
    }

    /// Every mutation names itself, so ⌘Z can say "Undo move clip".
    pub fn label(&self) -> Result<&'a str, CoreError> {
        match self.str("label") {
            Ok(l) if !l.trim().is_empty() => Ok(l),
            _ => Err(self.missing("label", "the words ⌘Z will show")),
        }
    }

    /// The view a change was made in: heat, space or console, or library for
    /// the drawer over them. (`sync`, where changes from other devices are
    /// kept, is the core's own; no command names it.)
    pub fn room(&self) -> Result<Room, CoreError> {
        let name = self.str("room")?;
        Room::parse(name).filter(|r| *r != Room::Sync).ok_or_else(|| {
            CoreError::new(
                "bad_args",
                format!("There is no view called '{name}'. The views are heat, space and console, and library is the drawer over them."),
            )
        })
    }
}
