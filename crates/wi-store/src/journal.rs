//! The undo journal (docs/SPEC.md 2.7 and 9.6), after MI-WWAV-OS's
//! `engine/src/store.rs`: the idea, reimplemented.
//!
//! Every change runs in a [`Txn`] that snapshots each row it touches, before
//! and after, as JSON. Committing writes the rows and their snapshots in one
//! SQLite transaction. Undo writes `before` back in reverse order; redo writes
//! `after` forward. An unchanged row (a row the change only pointed at, such as
//! the clip a new tag link names) is kept with `before` equal to `after`, so
//! the journal knows the change leaned on it.
//!
//! There is one journal, but ⌘Z acts on the room you are in, so changes made
//! in different rooms can touch the same value. Three rules keep undo exact:
//!
//! 1. Undo only puts back values no later change still holds. If a later
//!    change, done in another room, changed the same value of the same row,
//!    made or removed a row this one touched, or leaned on a row this one made
//!    or removed, the undo waits: "Can't undo rename clip yet. Undo retitle in
//!    Space first."
//! 2. Redo waits in the same way for an earlier undone change in another room
//!    that touched the same value.
//! 3. History is linear per room. A new change discards its room's undone
//!    changes (the redo branch), and any undone change in another room that
//!    touched the same values, since it could never be redone onto them;
//!    then, in turn, any later undone change that depended on one discarded.
//!
//! Values are compared column by column, so a rename in the library and a
//! publish in Space of the same clip never hold each other back. Together the
//! rules mean the done changes touching any value are always the oldest ones,
//! so undoing everything returns the first state byte for byte and redoing
//! everything the last.
//!
//! Work that left the machine is never a journal entry. Publishing is
//! journaled (it only sets `published_at`), but once the server has the work
//! (`remote_id` is set), the undo that would clear `published_at` is refused:
//! "Can't undo a publish. Unpublish 'World Ending'…". `remote_id` is written
//! only by the server's acknowledgement, which also writes it into every
//! snapshot of the clip, so no undo or redo ever takes it back. A sent message
//! is recorded as an outward act, and the room's Edit menu reads "Can't undo a
//! message." until a change is made, or redone, on top of it.
//!
//! Changes that arrive from your other devices are journaled in
//! [`Room::Sync`], which no view's ⌘Z acts on. They never take the redo of
//! what the person undid, unless they touched the same values, in which case
//! rule 3 discards it; and an undo that would put back a value they changed
//! since waits (rule 1) and says it changed on another device.
//!
//! Snapshots are JSON text, and serde_json is built with `float_roundtrip`,
//! so a REAL (a measured BPM) comes back as the same f64, bit for bit.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use rusqlite::types::{Value as Sql, ValueRef};
use rusqlite::{
    params, params_from_iter, Connection, OptionalExtension, Transaction, TransactionBehavior,
};
use serde_json::{Map, Value};

use crate::{Error, Result, Room};

/// A row as the journal keeps it: column name to value, in table order.
pub(crate) type Row = Map<String, Value>;

/// The row a `json!({...})` object literal describes.
pub(crate) fn object(v: Value) -> Row {
    match v {
        Value::Object(row) => row,
        _ => unreachable!("a row is written as an object literal"),
    }
}

/// Tables whose rows change only through a [`Txn`].
const JOURNALED: [&str; 7] = [
    "sequences",
    "clips",
    "tags",
    "clip_tags",
    "pins",
    "smart_folders",
    "docs",
];

fn journaled(tbl: &str) -> Result<&'static str> {
    JOURNALED
        .into_iter()
        .find(|t| *t == tbl)
        .ok_or_else(|| Error::Corrupt(format!("the journal names a table it doesn't keep: {tbl}")))
}

/// Which columns of each journaled table name another table's row.
pub(crate) struct Tables {
    refs: HashMap<&'static str, Vec<(String, &'static str)>>,
}

impl Tables {
    pub(crate) fn read(conn: &Connection) -> Result<Tables> {
        let mut refs = HashMap::new();
        for tbl in JOURNALED {
            let mut stmt = conn.prepare(&format!(
                "SELECT \"from\", \"table\" FROM pragma_foreign_key_list('{tbl}')"
            ))?;
            let pairs = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let pairs = pairs
                .into_iter()
                .map(|(col, parent)| Ok((col, journaled(&parent)?)))
                .collect::<Result<Vec<_>>>()?;
            refs.insert(tbl, pairs);
        }
        Ok(Tables { refs })
    }
}

// --- rows <-> JSON ---------------------------------------------------------

fn to_json(v: ValueRef<'_>) -> Result<Value> {
    Ok(match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(n) => Value::from(n),
        ValueRef::Real(f) => Value::from(f),
        ValueRef::Text(t) => Value::from(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(_) => return Err(Error::Corrupt("a journaled table holds a blob".into())),
    })
}

pub(crate) fn to_sql(v: &Value) -> Sql {
    match v {
        Value::Null => Sql::Null,
        Value::Bool(b) => Sql::Integer(*b as i64),
        Value::Number(n) => match n.as_i64() {
            Some(i) => Sql::Integer(i),
            None => Sql::Real(n.as_f64().unwrap_or(0.0)),
        },
        Value::String(s) => Sql::Text(s.clone()),
        other => Sql::Text(other.to_string()),
    }
}

pub(crate) fn read_row(conn: &Connection, tbl: &str, id: &str) -> Result<Option<Row>> {
    let mut stmt = conn.prepare_cached(&format!("SELECT * FROM {tbl} WHERE id = ?1"))?;
    let names: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
    let mut rows = stmt.query([id])?;
    let Some(r) = rows.next()? else {
        return Ok(None);
    };
    let mut row = Row::new();
    for (i, name) in names.into_iter().enumerate() {
        row.insert(name, to_json(r.get_ref(i)?)?);
    }
    Ok(Some(row))
}

pub(crate) fn insert_row(conn: &Connection, tbl: &str, row: &Row) -> Result<()> {
    let cols: Vec<&str> = row.keys().map(String::as_str).collect();
    let marks: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "INSERT INTO {tbl} ({}) VALUES ({})",
        cols.join(", "),
        marks.join(", ")
    );
    conn.prepare_cached(&sql)?
        .execute(params_from_iter(row.values().map(to_sql)))?;
    Ok(())
}

fn update_cols(conn: &Connection, tbl: &str, id: &str, set: &[(&str, &Value)]) -> Result<()> {
    let sets: Vec<String> = set
        .iter()
        .enumerate()
        .map(|(i, (c, _))| format!("{c} = ?{}", i + 1))
        .collect();
    let sql = format!(
        "UPDATE {tbl} SET {} WHERE id = ?{}",
        sets.join(", "),
        set.len() + 1
    );
    let mut args: Vec<Sql> = set.iter().map(|(_, v)| to_sql(v)).collect();
    args.push(Sql::Text(id.to_string()));
    conn.prepare_cached(&sql)?.execute(params_from_iter(args))?;
    Ok(())
}

fn delete_row(conn: &Connection, tbl: &str, id: &str) -> Result<()> {
    conn.prepare_cached(&format!("DELETE FROM {tbl} WHERE id = ?1"))?
        .execute([id])?;
    Ok(())
}

/// The columns whose values differ between two snapshots of one row.
fn changed<'a>(from: &'a Row, to: &'a Row) -> impl Iterator<Item = (&'a String, &'a Value)> {
    to.iter().filter(move |(col, v)| from.get(*col) != Some(*v))
}

// --- what a change touched -------------------------------------------------

/// How one change touched one row: made or removed it (`whole`), changed some
/// of its values (`cols`), or only leaned on it being there (`read`).
#[derive(Default)]
struct Touch {
    whole: bool,
    cols: BTreeSet<String>,
    read: bool,
}

impl Touch {
    fn add(&mut self, before: Option<&Row>, after: Option<&Row>) {
        match (before, after) {
            (Some(b), Some(a)) if b == a => self.read = true,
            (Some(b), Some(a)) => self.cols.extend(changed(b, a).map(|(c, _)| c.clone())),
            _ => self.whole = true,
        }
    }

    fn writes(&self) -> bool {
        self.whole || !self.cols.is_empty()
    }

    /// Whether the two changes can't be undone or redone out of order.
    fn overlaps(&self, other: &Touch) -> bool {
        (self.whole && (other.writes() || other.read))
            || (other.whole && (self.writes() || self.read))
            || !self.cols.is_disjoint(&other.cols)
    }
}

type Target = (String, String); // (table, row id)

fn parse(snapshot: Option<String>) -> Result<Option<Row>> {
    Ok(match snapshot {
        Some(s) => Some(serde_json::from_str(&s)?),
        None => None,
    })
}

/// Every row one journal entry touched.
fn touches(conn: &Connection, txn: &str) -> Result<BTreeMap<Target, Touch>> {
    let mut stmt =
        conn.prepare_cached("SELECT tbl, row_id, before, after FROM txn_row WHERE txn_id = ?1")?;
    let mut rows = stmt.query([txn])?;
    let mut out: BTreeMap<Target, Touch> = BTreeMap::new();
    while let Some(r) = rows.next()? {
        let (before, after) = (parse(r.get(2)?)?, parse(r.get(3)?)?);
        out.entry((r.get(0)?, r.get(1)?))
            .or_default()
            .add(before.as_ref(), after.as_ref());
    }
    Ok(out)
}

/// How one journal entry touched one row.
fn touch_on(conn: &Connection, txn: &str, (tbl, row_id): &Target) -> Result<Touch> {
    let mut stmt = conn.prepare_cached(
        "SELECT before, after FROM txn_row WHERE txn_id = ?1 AND tbl = ?2 AND row_id = ?3",
    )?;
    let mut rows = stmt.query(params![txn, tbl, row_id])?;
    let mut touch = Touch::default();
    while let Some(r) = rows.next()? {
        touch.add(parse(r.get(0)?)?.as_ref(), parse(r.get(1)?)?.as_ref());
    }
    Ok(touch)
}

/// A journal entry's header.
struct Entry {
    id: String,
    label: String,
    room: Room,
}

fn entry(r: &rusqlite::Row<'_>) -> rusqlite::Result<(String, String, String)> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
}

fn to_entry((id, label, room): (String, String, String)) -> Result<Entry> {
    let room = Room::parse(&room)
        .ok_or_else(|| Error::Corrupt(format!("a journal entry names no room: {room}")))?;
    Ok(Entry { id, label, room })
}

/// Entries in `state` that touched `target`, with ids after (`later`) or
/// before `than`, nearest first.
fn neighbours(
    conn: &Connection,
    target: &Target,
    state: &str,
    later: bool,
    than: &str,
) -> Result<Vec<Entry>> {
    let (cmp, order) = if later { (">", "ASC") } else { ("<", "DESC") };
    let sql = format!(
        "SELECT DISTINCT t.id, t.label, t.room FROM txn_row r JOIN txn t ON t.id = r.txn_id
         WHERE r.tbl = ?1 AND r.row_id = ?2 AND t.state = ?3 AND t.id {cmp} ?4 ORDER BY t.id {order}"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let found = stmt.query_map(params![target.0, target.1, state, than], entry)?;
    found.map(|e| to_entry(e?)).collect()
}

/// The entry in `state` nearest `of`, on its later or earlier side, that
/// overlaps it.
fn overlapping(conn: &Connection, of: &Entry, state: &str, later: bool) -> Result<Option<Entry>> {
    let mut nearest: Option<Entry> = None;
    for (target, touch) in touches(conn, &of.id)? {
        for other in neighbours(conn, &target, state, later, &of.id)? {
            if touch.overlaps(&touch_on(conn, &other.id, &target)?) {
                if nearest
                    .as_ref()
                    .map_or(true, |n| (other.id < n.id) == later)
                {
                    nearest = Some(other);
                }
                break; // the rest on this row are farther away
            }
        }
    }
    Ok(nearest)
}

// --- the transaction ------------------------------------------------------

/// One labelled change. Everything done through it undoes together; dropping
/// it without [`Txn::commit`] leaves no trace.
pub struct Txn<'s> {
    tx: Transaction<'s>,
    tables: &'s Tables,
    room: Room,
    label: String,
    actor: Actor,
    /// (table, row id, before, after) in the order they happened.
    rows: Vec<(&'static str, String, Option<Row>, Option<Row>)>,
    touched: HashSet<(&'static str, String)>,
}

impl<'s> Txn<'s> {
    pub(crate) fn begin(
        conn: &'s mut Connection,
        tables: &'s Tables,
        room: Room,
        label: &str,
        actor: Actor,
    ) -> Result<Txn<'s>> {
        Ok(Txn {
            tx: conn.transaction_with_behavior(TransactionBehavior::Immediate)?,
            tables,
            room,
            label: label.to_string(),
            actor,
            rows: Vec::new(),
            touched: HashSet::new(),
        })
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.tx
    }

    pub(crate) fn get(&self, tbl: &str, id: &str) -> Result<Option<Row>> {
        read_row(&self.tx, tbl, id)
    }

    pub(crate) fn insert(&mut self, tbl: &'static str, row: Row) -> Result<()> {
        let id = row
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        insert_row(&self.tx, tbl, &row)?;
        self.wrote(tbl, id, None)
    }

    /// Sets some values of a row; false if the row isn't there.
    pub(crate) fn update(
        &mut self,
        tbl: &'static str,
        id: &str,
        set: &[(&str, Value)],
    ) -> Result<bool> {
        let Some(before) = self.get(tbl, id)? else {
            return Ok(false);
        };
        let set: Vec<(&str, &Value)> = set.iter().map(|(c, v)| (*c, v)).collect();
        update_cols(&self.tx, tbl, id, &set)?;
        self.wrote(tbl, id.to_string(), Some(before))?;
        Ok(true)
    }

    /// Removes a row; false if it wasn't there.
    pub(crate) fn delete(&mut self, tbl: &'static str, id: &str) -> Result<bool> {
        let Some(before) = self.get(tbl, id)? else {
            return Ok(false);
        };
        delete_row(&self.tx, tbl, id)?;
        self.wrote(tbl, id.to_string(), Some(before))?;
        Ok(true)
    }

    /// Notes that this change leans on a row it doesn't change, so undoing
    /// the row's creation waits until this change is undone.
    pub(crate) fn lean_on(&mut self, tbl: &'static str, id: &str) -> Result<()> {
        if self.touched.contains(&(tbl, id.to_string())) {
            return Ok(());
        }
        if let Some(row) = self.get(tbl, id)? {
            self.touched.insert((tbl, id.to_string()));
            self.rows
                .push((tbl, id.to_string(), Some(row.clone()), Some(row)));
        }
        Ok(())
    }

    fn wrote(&mut self, tbl: &'static str, id: String, before: Option<Row>) -> Result<()> {
        let after = self.get(tbl, &id)?;
        if before == after {
            return Ok(()); // nothing changed; don't pollute the journal
        }
        // The rows this one's references point at, before and after.
        let mut parents = Vec::new();
        for (col, parent) in &self.tables.refs[tbl] {
            let (b, a) = (
                before.as_ref().map(|r| &r[col]),
                after.as_ref().map(|r| &r[col]),
            );
            if before.is_some() && after.is_some() && a == b {
                continue;
            }
            parents.extend(
                [b, a]
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|v| (*parent, v.to_string())),
            );
        }
        self.touched.insert((tbl, id.clone()));
        self.rows.push((tbl, id, before, after));
        for (parent, pid) in parents {
            self.lean_on(parent, &pid)?;
        }
        Ok(())
    }

    /// Writes the change and its journal entry together. Returns the entry's
    /// id, or None when nothing changed (and nothing is journaled).
    pub fn commit(self) -> Result<Option<String>> {
        let mut mine: BTreeMap<Target, Touch> = BTreeMap::new();
        for (tbl, id, before, after) in &self.rows {
            mine.entry((tbl.to_string(), id.clone()))
                .or_default()
                .add(before.as_ref(), after.as_ref());
        }
        if !mine.values().any(Touch::writes) {
            self.tx.commit()?;
            return Ok(None);
        }
        discard_redo(&self.tx, self.room, &mine)?;
        let id = fresh_id(&self.tx)?;
        let (actor, tool, reason) = match &self.actor {
            Actor::You => ("you", None, None),
            Actor::Claude { tool, reason } => {
                ("claude", Some(tool.as_str()), Some(reason.as_str()))
            }
        };
        self.tx.execute(
            "INSERT INTO txn (id, label, room, state, actor, tool, reason) VALUES (?1, ?2, ?3, 'done', ?4, ?5, ?6)",
            params![id, self.label, self.room.as_str(), actor, tool, reason],
        )?;
        let snapshot =
            |row: &Option<Row>| row.as_ref().map(|r| Value::Object(r.clone()).to_string());
        for (seq, (tbl, row_id, before, after)) in self.rows.iter().enumerate() {
            self.tx
                .prepare_cached(
                    "INSERT INTO txn_row (txn_id, seq, tbl, row_id, before, after) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?
                .execute(params![id, seq, tbl, row_id, snapshot(before), snapshot(after)])?;
        }
        crate::clips::drop_stale_parts(&self.tx)?;
        self.tx.commit()?;
        Ok(Some(id))
    }
}

/// A journal id later than every one before it, even if the clock stepped
/// back since they were made.
fn fresh_id(conn: &Connection) -> Result<String> {
    let id = wwav_ids::ulid();
    let last: Option<String> = conn.query_row(
        "SELECT max(id) FROM (SELECT id FROM txn UNION ALL SELECT id FROM outward)",
        [],
        |r| r.get(0),
    )?;
    match last.as_deref().and_then(wwav_ids::decode_ulid) {
        Some(n) if last.as_deref() >= Some(id.as_str()) => {
            let next = n + 1;
            Ok(wwav_ids::encode_ulid((next >> 80) as u64, next))
        }
        _ => Ok(id),
    }
}

/// Rule 3: a new change ends its room's redo branch, and any undone change it
/// overlaps, and then whatever undone change came later and leaned on those.
fn discard_redo(conn: &Connection, room: Room, mine: &BTreeMap<Target, Touch>) -> Result<()> {
    let mut stmt =
        conn.prepare_cached("SELECT id FROM txn WHERE room = ?1 AND state = 'undone'")?;
    let mut gone: BTreeSet<String> = stmt
        .query_map([room.as_str()], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for (target, touch) in mine {
        // Every undone change on this row: any id is later than "".
        for other in neighbours(conn, target, "undone", true, "")? {
            if !gone.contains(&other.id) && touch.overlaps(&touch_on(conn, &other.id, target)?) {
                gone.insert(other.id);
            }
        }
    }
    let mut work: Vec<String> = gone.iter().cloned().collect();
    while let Some(id) = work.pop() {
        for (target, touch) in touches(conn, &id)? {
            for other in neighbours(conn, &target, "undone", true, &id)? {
                if !gone.contains(&other.id) && touch.overlaps(&touch_on(conn, &other.id, &target)?)
                {
                    gone.insert(other.id.clone());
                    work.push(other.id);
                }
            }
        }
    }
    for id in gone {
        conn.execute("DELETE FROM txn_row WHERE txn_id = ?1", [&id])?;
        conn.execute("DELETE FROM txn WHERE id = ?1", [&id])?;
    }
    Ok(())
}

/// Notes work that left the machine, with the room's newest done change: ⌘Z
/// there is held until a change is made, or redone, on top of it.
pub(crate) fn record_outward(conn: &Connection, room: Room, what: &str) -> Result<()> {
    let after = first(conn, room, "done", true)?.map(|e| e.id);
    conn.execute(
        "INSERT INTO outward (id, room, what, after_txn) VALUES (?1, ?2, ?3, ?4)",
        params![fresh_id(conn)?, room.as_str(), what, after],
    )?;
    Ok(())
}

// --- undo, redo and the Edit menu ------------------------------------------

/// One side of the Edit menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Menu {
    /// The label of what would be undone or redone.
    Ready(String),
    /// Why it can't be, as one sentence.
    Held(String),
    Nothing,
}

/// What ⌘Z and ⌘⇧Z would do in one room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    pub undo: Menu,
    pub redo: Menu,
}

impl History {
    /// "Undo move clip", the sentence saying why not, or "Nothing to undo."
    pub fn undo_text(&self) -> String {
        match &self.undo {
            Menu::Ready(label) => format!("Undo {label}"),
            Menu::Held(why) => why.clone(),
            Menu::Nothing => "Nothing to undo.".to_string(),
        }
    }

    pub fn redo_text(&self) -> String {
        match &self.redo {
            Menu::Ready(label) => format!("Redo {label}"),
            Menu::Held(why) => why.clone(),
            Menu::Nothing => "Nothing to redo.".to_string(),
        }
    }
}

enum Next {
    Ready(Entry),
    Held(String),
    Nothing,
}

impl Next {
    fn menu(self) -> Menu {
        match self {
            Next::Ready(e) => Menu::Ready(e.label),
            Next::Held(why) => Menu::Held(why),
            Next::Nothing => Menu::Nothing,
        }
    }
}

fn first(conn: &Connection, room: Room, state: &str, newest: bool) -> Result<Option<Entry>> {
    let order = if newest { "DESC" } else { "ASC" };
    let sql = format!("SELECT id, label, room FROM txn WHERE room = ?1 AND state = ?2 ORDER BY id {order} LIMIT 1");
    let found = conn
        .query_row(&sql, params![room.as_str(), state], entry)
        .optional()?;
    found.map(to_entry).transpose()
}

fn next_undo(conn: &Connection, room: Room) -> Result<Next> {
    let newest = first(conn, room, "done", true)?;
    let outward: Option<(Option<String>, String)> = conn
        .query_row(
            "SELECT after_txn, what FROM outward WHERE room = ?1 ORDER BY id DESC LIMIT 1",
            [room.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((after, what)) = outward {
        // Held while the newest done change is still one made before it.
        let held = match (&newest, &after) {
            (None, _) => true,
            (Some(e), Some(after)) => e.id <= *after,
            (Some(_), None) => false,
        };
        if held {
            return Ok(Next::Held(format!("Can't undo {what}.")));
        }
    }
    let Some(entry) = newest else {
        return Ok(Next::Nothing);
    };
    if let Some(title) = unpublishes_what_the_server_has(conn, &entry)? {
        return Ok(Next::Held(format!(
            "Can't undo a publish. Unpublish '{title}'…"
        )));
    }
    if let Some(later) = overlapping(conn, &entry, "done", true)? {
        let (label, other, room) = (&entry.label, &later.label, later.room.name());
        return Ok(Next::Held(if later.room == Room::Sync {
            format!("Can't undo {label}: it changed on another device since.")
        } else {
            format!("Can't undo {label} yet. Undo {other} in {room} first.")
        }));
    }
    Ok(Next::Ready(entry))
}

fn next_redo(conn: &Connection, room: Room) -> Result<Next> {
    let Some(entry) = first(conn, room, "undone", false)? else {
        return Ok(Next::Nothing);
    };
    if let Some(earlier) = overlapping(conn, &entry, "undone", false)? {
        let (label, other, room) = (&entry.label, &earlier.label, earlier.room.name());
        return Ok(Next::Held(format!(
            "Can't redo {label} yet. Redo {other} in {room} first."
        )));
    }
    Ok(Next::Ready(entry))
}

/// The title of a clip the server has whose publish this undo would take
/// back: the entry itself set `published_at` on the clip. Publishing is
/// undoable only while the upload is queued. A change the entry made to the
/// clip otherwise (a rename, a tag, its import) publishes nothing, so it isn't
/// refused here; if it can't come back before the publish, rule 1 holds it.
fn unpublishes_what_the_server_has(conn: &Connection, entry: &Entry) -> Result<Option<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT row_id, before, after FROM txn_row WHERE txn_id = ?1 AND tbl = 'clips' ORDER BY seq",
    )?;
    let published = |row: Option<Row>| {
        row.and_then(|mut r| r.remove("published_at"))
            .unwrap_or(Value::Null)
    };
    // Each row's published_at before the entry (its first `before`) and after
    // it (its last `after`).
    let mut span: BTreeMap<String, (Value, Value)> = BTreeMap::new();
    let found = stmt.query_map([&entry.id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })?;
    for r in found {
        let (row_id, before, after) = r?;
        let after = published(parse(after)?);
        match span.get_mut(&row_id) {
            Some((_, last)) => *last = after,
            None => {
                span.insert(row_id, (published(parse(before)?), after));
            }
        }
    }
    for (row_id, (then, set)) in span {
        if then == set {
            continue;
        }
        let Some(now) = read_row(conn, "clips", &row_id)? else {
            continue; // the entry removed the clip: undo restores it as it was
        };
        if !now.get("remote_id").map_or(true, Value::is_null) {
            let title = now.get("title").and_then(Value::as_str).unwrap_or_default();
            return Ok(Some(title.to_string()));
        }
    }
    Ok(None)
}

/// The server has the clip: every journal snapshot of its row now says so,
/// so no undo or redo (of its delete, say) can write back a `remote_id` from
/// before, put the clip back in the queue, or make its publish undoable.
pub(crate) fn acknowledge(conn: &Connection, clip_id: &str, remote_id: &str) -> Result<()> {
    let snapshots = conn
        .prepare_cached(
            "SELECT txn_id, seq, before, after FROM txn_row WHERE tbl = 'clips' AND row_id = ?1",
        )?
        .query_map([clip_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let stamp = |snapshot: Option<String>| -> Result<Option<String>> {
        Ok(parse(snapshot)?.map(|mut row| {
            row.insert("remote_id".into(), remote_id.into());
            Value::Object(row).to_string()
        }))
    };
    let mut update = conn.prepare_cached(
        "UPDATE txn_row SET before = ?3, after = ?4 WHERE txn_id = ?1 AND seq = ?2",
    )?;
    for (txn, seq, before, after) in snapshots {
        update.execute(params![txn, seq, stamp(before)?, stamp(after)?])?;
    }
    Ok(())
}

/// Who made a change. Claude's changes come through the MCP helper and carry
/// the tool and Claude's one-sentence reason (docs/SPEC.md 3.13, 8.8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Actor {
    #[default]
    You,
    Claude {
        tool: String,
        reason: String,
    },
}

/// One journal entry as Settings → Claude lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryInfo {
    pub id: String,
    /// "Claude's estimate": the Edit menu reads "Undo " + this.
    pub label: String,
    pub room: Room,
    /// Done, or undone since.
    pub done: bool,
    pub actor: Actor,
    /// Milliseconds since the epoch, from the entry's ULID.
    pub at_ms: u64,
}

/// The newest entries first, only Claude's when `claude_only`.
pub(crate) fn entries(
    conn: &Connection,
    claude_only: bool,
    limit: usize,
) -> Result<Vec<EntryInfo>> {
    let filter = if claude_only {
        "WHERE actor = 'claude'"
    } else {
        ""
    };
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT id, label, room, state, actor, tool, reason FROM txn {filter} ORDER BY id DESC LIMIT ?1"
    ))?;
    let rows = stmt.query_map([limit as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<String>>(6)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, label, room, state, actor, tool, reason) = row?;
        let room = Room::parse(&room)
            .ok_or_else(|| Error::Corrupt(format!("a journal entry names no room: {room}")))?;
        let actor = match actor.as_str() {
            "claude" => Actor::Claude {
                tool: tool.unwrap_or_default(),
                reason: reason.unwrap_or_default(),
            },
            _ => Actor::You,
        };
        let at_ms = wwav_ids::ulid_ms(&id).unwrap_or(0);
        out.push(EntryInfo {
            id,
            label,
            room,
            done: state == "done",
            actor,
            at_ms,
        });
    }
    Ok(out)
}

/// One record a journal entry changed: its kind and key, and the record
/// before and after (None: it wasn't there).
#[derive(Clone, Debug, PartialEq)]
pub struct DocChange {
    pub kind: String,
    pub key: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

/// A journal entry with the records it changed. Whoever watches the journal
/// (the core noticing the MCP helper's writes) reads these, newer than the
/// last it saw.
#[derive(Clone, Debug, PartialEq)]
pub struct EntryDocs {
    pub id: String,
    pub label: String,
    pub room: Room,
    pub actor: Actor,
    pub docs: Vec<DocChange>,
}

/// A `docs` row as the journal snapshots it, as the record it holds.
fn doc_record(snapshot: Option<Row>) -> Result<Option<Value>> {
    let Some(row) = snapshot else { return Ok(None) };
    match row.get("json").and_then(Value::as_str) {
        Some(text) => Ok(Some(serde_json::from_str(text)?)),
        None => Err(Error::Corrupt("a journaled docs row has no json".into())),
    }
}

fn docs_of(conn: &Connection, txn: &str) -> Result<Vec<DocChange>> {
    let mut stmt = conn.prepare_cached(
        "SELECT row_id, before, after FROM txn_row WHERE txn_id = ?1 AND tbl = 'docs' ORDER BY seq",
    )?;
    let rows = stmt
        .query_map([txn], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // A row touched twice in one change: the first `before`, the last `after`.
    let mut out: Vec<DocChange> = Vec::new();
    for (row_id, before, after) in rows {
        let Some((kind, key)) = row_id.split_once('/') else {
            continue;
        };
        let after = doc_record(parse(after)?)?;
        match out.iter_mut().find(|d| d.kind == kind && d.key == key) {
            Some(d) => d.after = after,
            None => out.push(DocChange {
                kind: kind.to_string(),
                key: key.to_string(),
                before: doc_record(parse(before)?)?,
                after,
            }),
        }
    }
    Ok(out)
}

fn entry_docs_from(
    conn: &Connection,
    (id, label, room, actor, tool, reason): (
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    ),
) -> Result<EntryDocs> {
    let room = Room::parse(&room)
        .ok_or_else(|| Error::Corrupt(format!("a journal entry names no room: {room}")))?;
    let actor = match actor.as_str() {
        "claude" => Actor::Claude {
            tool: tool.unwrap_or_default(),
            reason: reason.unwrap_or_default(),
        },
        _ => Actor::You,
    };
    let docs = docs_of(conn, &id)?;
    Ok(EntryDocs {
        id,
        label,
        room,
        actor,
        docs,
    })
}

type EntryRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
);

fn entry_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<EntryRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
    ))
}

/// The entries newer than `after` (all of them for None), oldest first, each
/// with the records it changed.
pub(crate) fn entries_after(
    conn: &Connection,
    after: Option<&str>,
    limit: usize,
) -> Result<Vec<EntryDocs>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, label, room, actor, tool, reason FROM txn WHERE id > ?1 ORDER BY id LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![after.unwrap_or(""), limit as i64], entry_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|row| entry_docs_from(conn, row))
        .collect()
}

/// One entry with the records it changed.
pub(crate) fn entry_docs(conn: &Connection, id: &str) -> Result<Option<EntryDocs>> {
    let row = conn
        .query_row(
            "SELECT id, label, room, actor, tool, reason FROM txn WHERE id = ?1",
            [id],
            entry_row,
        )
        .optional()?;
    row.map(|row| entry_docs_from(conn, row)).transpose()
}

/// The newest entry's id: where a watcher starts from.
pub(crate) fn last_entry(conn: &Connection) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT max(id) FROM txn", [], |r| r.get(0))?)
}

/// Undoes one entry out of order, for Settings → Claude's list. Refused when
/// it was undone already, when it took back a publish the server has, or
/// when a later done entry changed the same values since.
pub(crate) fn undo_entry(conn: &mut Connection, id: &str) -> Result<String> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let found = tx
        .query_row(
            "SELECT id, label, room, state FROM txn WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((id, label, room, state)) = found else {
        return Err(Error::Refused(
            "That change isn't in the journal any more.".into(),
        ));
    };
    if state != "done" {
        return Err(Error::Refused(format!("{label} is already undone.")));
    }
    let entry = to_entry((id, label, room))?;
    if let Some(title) = unpublishes_what_the_server_has(&tx, &entry)? {
        return Err(Error::Refused(format!(
            "Can't undo a publish. Unpublish '{title}'…"
        )));
    }
    if overlapping(&tx, &entry, "done", true)?.is_some() {
        return Err(Error::Refused(
            "This changed again since. Undo the later change first.".into(),
        ));
    }
    replay(&tx, &entry.id, true)?;
    tx.execute("UPDATE txn SET state = 'undone' WHERE id = ?1", [&entry.id])?;
    crate::clips::drop_stale_parts(&tx)?;
    tx.commit()?;
    Ok(entry.label)
}

pub(crate) fn history(conn: &Connection, room: Room) -> Result<History> {
    Ok(History {
        undo: next_undo(conn, room)?.menu(),
        redo: next_redo(conn, room)?.menu(),
    })
}

pub(crate) fn undo(conn: &mut Connection, room: Room) -> Result<Option<String>> {
    step(conn, room, true)
}

pub(crate) fn redo(conn: &mut Connection, room: Room) -> Result<Option<String>> {
    step(conn, room, false)
}

fn step(conn: &mut Connection, room: Room, back: bool) -> Result<Option<String>> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let next = if back {
        next_undo(&tx, room)?
    } else {
        next_redo(&tx, room)?
    };
    let entry = match next {
        Next::Ready(entry) => entry,
        Next::Held(why) => return Err(Error::Refused(why)),
        Next::Nothing => return Ok(None),
    };
    replay(&tx, &entry.id, back)?;
    tx.execute(
        "UPDATE txn SET state = ?1 WHERE id = ?2",
        params![if back { "undone" } else { "done" }, entry.id],
    )?;
    crate::clips::drop_stale_parts(&tx)?;
    tx.commit()?;
    Ok(Some(entry.label))
}

/// Writes one entry's `before` snapshots back in reverse order, or its
/// `after` snapshots forward. Only the values the entry changed are written,
/// so a later change to another value of the same row stays.
fn replay(conn: &Connection, txn: &str, back: bool) -> Result<()> {
    let order = if back { "DESC" } else { "ASC" };
    let mut stmt = conn.prepare(&format!(
        "SELECT tbl, row_id, before, after FROM txn_row WHERE txn_id = ?1 ORDER BY seq {order}"
    ))?;
    let rows = stmt
        .query_map([txn], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get(2)?,
                r.get(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<(String, String, Option<String>, Option<String>)>>>()?;
    for (tbl, row_id, before, after) in rows {
        let tbl = journaled(&tbl)?;
        let (from, to) = if back {
            (parse(after)?, parse(before)?)
        } else {
            (parse(before)?, parse(after)?)
        };
        match (from, to) {
            (_, None) => delete_row(conn, tbl, &row_id)?,
            (None, Some(row)) => insert_row(conn, tbl, &row)?,
            (Some(from), Some(to)) => {
                let set: Vec<(&str, &Value)> =
                    changed(&from, &to).map(|(c, v)| (c.as_str(), v)).collect();
                if !set.is_empty() {
                    update_cols(conn, tbl, &row_id, &set)?;
                }
            }
        }
    }
    Ok(())
}
