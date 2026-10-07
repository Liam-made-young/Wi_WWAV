//! Tags, pins, smart folders and search, from v3's library (docs/SPEC.md 2.5).

use rusqlite::types::Value as Sql;
use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::clips::{clip_from, require_clip, CLIP_COLS};
use crate::journal::{object, Row};
use crate::{refused, Clip, Colour, Error, Kind, Result, Store, Txn};

/// User tags are words you add, card tags name an NFC card, and system tags
/// are places: your solar systems and your shelves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TagKind {
    User,
    Card,
    System,
}

impl TagKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TagKind::User => "user",
            TagKind::Card => "card",
            TagKind::System => "system",
        }
    }

    pub fn parse(s: &str) -> Option<TagKind> {
        [TagKind::User, TagKind::Card, TagKind::System]
            .into_iter()
            .find(|k| k.as_str() == s)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub kind: TagKind,
}

/// At most 12 tags on a clip, of every kind (2.5).
const MAX_TAGS: i64 = 12;

fn tag_name(name: &str) -> Result<String> {
    match name.trim().to_lowercase() {
        n if n.is_empty() => refused("A tag needs a name."),
        n => Ok(n),
    }
}

/// What a pin holds: a clip or a smart folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pin {
    Clip(String),
    Folder(String),
}

impl Pin {
    fn encode(&self) -> String {
        match self {
            Pin::Clip(id) => format!("clip:{id}"),
            Pin::Folder(id) => format!("folder:{id}"),
        }
    }

    fn decode(s: &str) -> Option<Pin> {
        match s.split_once(':')? {
            ("clip", id) => Some(Pin::Clip(id.to_string())),
            ("folder", id) => Some(Pin::Folder(id.to_string())),
            _ => None,
        }
    }

    fn row(&self) -> (&'static str, &str) {
        match self {
            Pin::Clip(id) => ("clips", id),
            Pin::Folder(id) => ("smart_folders", id),
        }
    }
}

const PINS: usize = 4;

fn slots(pins: &Row) -> Result<Vec<Option<String>>> {
    let slots: Vec<Option<String>> = serde_json::from_str(pins["slots"].as_str().unwrap_or("[]"))?;
    if slots.len() != PINS {
        return Err(Error::Corrupt(format!(
            "pins hold {} slots, not {PINS}",
            slots.len()
        )));
    }
    Ok(slots)
}

/// A rule over the library, every part ANDed: "128–132 BPM · minor ·
/// tag:live-drums". A key of "major" or "minor" matches any key in that mode.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SmartRule {
    #[serde(default)]
    pub tags: Vec<String>,
    pub colour: Option<Colour>,
    pub bpm_min: Option<f64>,
    pub bpm_max: Option<f64>,
    pub key: Option<String>,
    pub kind: Option<Kind>,
}

impl SmartRule {
    /// The rule as SQL over `clips c`, with its arguments appended to `args`.
    fn sql(&self, args: &mut Vec<Sql>) -> String {
        let mut parts = Vec::new();
        if let Some(kind) = self.kind {
            parts.push("c.kind = ?");
            args.push(Sql::Text(kind.as_str().into()));
        }
        if let Some(colour) = self.colour {
            parts.push("c.colour = ?");
            args.push(Sql::Text(colour.as_str().into()));
        }
        if let Some(min) = self.bpm_min {
            parts.push("c.bpm >= ?");
            args.push(Sql::Real(min));
        }
        if let Some(max) = self.bpm_max {
            parts.push("c.bpm <= ?");
            args.push(Sql::Real(max));
        }
        if let Some(key) = &self.key {
            let key = key.trim().to_lowercase();
            if key == "major" || key == "minor" {
                parts.push("lower(c.key) LIKE ?");
                args.push(Sql::Text(format!("% {key}")));
            } else {
                parts.push("lower(c.key) = ?");
                args.push(Sql::Text(key));
            }
        }
        for tag in &self.tags {
            parts.push("c.id IN (SELECT ct.clip_id FROM clip_tags ct JOIN tags t ON t.id = ct.tag_id WHERE t.name = ?)");
            args.push(Sql::Text(tag.trim().to_lowercase()));
        }
        parts.join(" AND ")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SmartFolder {
    pub id: String,
    pub name: String,
    pub rule: SmartRule,
}

/// Turns what was typed into an FTS5 query: every word must match from its
/// start, and nothing typed is ever query syntax. A NUL (from a paste) would
/// end the query string early, so it separates words like a space.
pub(crate) fn fts_query(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split(|c: char| c.is_whitespace() || c == '\0')
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

pub(crate) fn search_clips(
    conn: &Connection,
    text: &str,
    rule: &SmartRule,
    limit: Option<usize>,
) -> Result<Vec<Clip>> {
    let mut args = Vec::new();
    // Newest first, in the search index's own order, so a common word stops
    // at the limit instead of sorting every match.
    let (mut sql, order) = match fts_query(text) {
        Some(q) => {
            args.push(Sql::Text(q));
            let sql = format!("SELECT {CLIP_COLS} FROM clips_fts f JOIN clips c ON c.n = f.rowid WHERE clips_fts MATCH ?");
            (sql, "f.rowid")
        }
        None => (format!("SELECT {CLIP_COLS} FROM clips c WHERE 1"), "c.n"),
    };
    let rule = rule.sql(&mut args);
    if !rule.is_empty() {
        sql += &format!(" AND {rule}");
    }
    sql += &format!(" ORDER BY {order} DESC LIMIT ?");
    args.push(Sql::Integer(limit.map_or(-1, |l| l as i64)));
    let mut stmt = conn.prepare_cached(&sql)?;
    let clips = stmt
        .query_map(params_from_iter(args), clip_from)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(clips)
}

impl Store {
    pub fn tags_of(&self, clip_id: &str) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT t.name, t.kind FROM clip_tags ct JOIN tags t ON t.id = ct.tag_id
             WHERE ct.clip_id = ?1 ORDER BY t.kind, t.name",
        )?;
        let tags = stmt.query_map([clip_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        tags.map(|t| {
            let (name, kind) = t?;
            let kind =
                TagKind::parse(&kind).ok_or_else(|| Error::Corrupt(format!("tag kind {kind}")))?;
            Ok(Tag { name, kind })
        })
        .collect()
    }

    /// The four pin slots, left to right.
    pub fn pins(&self) -> Result<[Option<Pin>; PINS]> {
        let row = crate::journal::read_row(&self.conn, "pins", "pins")?
            .ok_or_else(|| Error::Corrupt("the pins row is missing".into()))?;
        let mut out: [Option<Pin>; PINS] = Default::default();
        for (slot, held) in out.iter_mut().zip(slots(&row)?) {
            *slot = held.as_deref().and_then(Pin::decode);
        }
        Ok(out)
    }

    pub fn smart_folders(&self) -> Result<Vec<SmartFolder>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT id, name, rule FROM smart_folders ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?)))?;
        rows.map(|row| {
            let (id, name, rule) = row?;
            Ok(SmartFolder {
                id,
                name,
                rule: serde_json::from_str(&rule)?,
            })
        })
        .collect()
    }

    /// Every clip a rule selects, newest first, evaluated in SQL.
    pub fn matching(&self, rule: &SmartRule) -> Result<Vec<Clip>> {
        search_clips(&self.conn, "", rule, None)
    }

    /// Library search: clips whose title, artist or words start with every
    /// word typed, narrowed by `rule`, newest first.
    pub fn search(&self, text: &str, rule: &SmartRule, limit: usize) -> Result<Vec<Clip>> {
        search_clips(&self.conn, text, rule, Some(limit))
    }
}

impl Txn<'_> {
    /// Tags a clip. Names are lowercased; a 13th tag is refused.
    pub fn add_tag(&mut self, clip_id: &str, name: &str, kind: TagKind) -> Result<()> {
        let name = tag_name(name)?;
        let clip = require_clip(self, clip_id)?;
        let tag = format!("{}:{name}", kind.as_str());
        let link = format!("{clip_id}/{tag}");
        if self.get("clip_tags", &link)?.is_some() {
            return Ok(());
        }
        let count = clip["tag_count"].as_i64().unwrap_or(0);
        if count >= MAX_TAGS {
            return refused("A clip holds 12 tags. Remove one first.");
        }
        if self.get("tags", &tag)?.is_none() {
            self.insert(
                "tags",
                object(json!({"id": tag, "name": name, "kind": kind.as_str()})),
            )?;
        }
        self.insert(
            "clip_tags",
            object(json!({"id": link, "clip_id": clip_id, "tag_id": tag})),
        )?;
        // The count lives on the clip, so two rooms tagging one clip touch the
        // same value and the limit holds through any undo and redo.
        self.update("clips", clip_id, &[("tag_count", (count + 1).into())])?;
        Ok(())
    }

    pub fn remove_tag(&mut self, clip_id: &str, name: &str, kind: TagKind) -> Result<()> {
        let link = format!("{clip_id}/{}:{}", kind.as_str(), tag_name(name)?);
        if self.delete("clip_tags", &link)? {
            let count = require_clip(self, clip_id)?["tag_count"]
                .as_i64()
                .unwrap_or(0);
            self.update("clips", clip_id, &[("tag_count", (count - 1).into())])?;
        }
        Ok(())
    }

    /// Pins a clip or smart folder in the first free slot and returns the
    /// slot; one already pinned keeps its place. A fifth is refused.
    pub fn pin(&mut self, pin: &Pin) -> Result<usize> {
        let (tbl, id) = pin.row();
        if self.get(tbl, id)?.is_none() {
            return match pin {
                Pin::Clip(_) => refused("That clip isn't in the library any more."),
                Pin::Folder(_) => refused("That smart folder isn't in the library any more."),
            };
        }
        let mut slots = slots(&self.pins_row()?)?;
        let held = pin.encode();
        if let Some(slot) = slots
            .iter()
            .position(|s| s.as_deref() == Some(held.as_str()))
        {
            return Ok(slot);
        }
        let Some(free) = slots.iter().position(Option::is_none) else {
            return refused("Pins hold 4. Unpin one first.");
        };
        slots[free] = Some(held);
        self.set_slots(slots)?;
        // The pin names the row inside JSON, so say so: undoing the row's
        // creation waits for this pin to be undone.
        self.lean_on(tbl, id)?;
        Ok(free)
    }

    /// Unpins, closing the gap so the pins stay in a row from the left.
    pub fn unpin(&mut self, pin: &Pin) -> Result<()> {
        let mut slots = slots(&self.pins_row()?)?;
        let held = pin.encode();
        if let Some(slot) = slots
            .iter()
            .position(|s| s.as_deref() == Some(held.as_str()))
        {
            slots.remove(slot);
            slots.push(None);
            self.set_slots(slots)?;
            // Undoing this puts the pin back, naming the row, so it leans on
            // the row as `pin` does: a later delete of it is undone first.
            let (tbl, id) = pin.row();
            self.lean_on(tbl, id)?;
        }
        Ok(())
    }

    fn pins_row(&self) -> Result<Row> {
        self.get("pins", "pins")?
            .ok_or_else(|| Error::Corrupt("the pins row is missing".into()))
    }

    fn set_slots(&mut self, slots: Vec<Option<String>>) -> Result<()> {
        self.update(
            "pins",
            "pins",
            &[("slots", serde_json::to_string(&slots)?.into())],
        )?;
        Ok(())
    }

    /// Saves a new smart folder and returns its id.
    pub fn save_smart_folder(&mut self, name: &str, rule: &SmartRule) -> Result<String> {
        let id = wwav_ids::ulid();
        let rule = serde_json::to_string(rule)?;
        self.insert(
            "smart_folders",
            object(json!({"id": id, "name": name, "rule": rule})),
        )?;
        Ok(id)
    }

    pub fn delete_smart_folder(&mut self, id: &str) -> Result<()> {
        if self.get("smart_folders", id)?.is_none() {
            return refused("That smart folder isn't in the library any more.");
        }
        self.unpin(&Pin::Folder(id.to_string()))?;
        self.delete("smart_folders", id)?;
        Ok(())
    }
}
