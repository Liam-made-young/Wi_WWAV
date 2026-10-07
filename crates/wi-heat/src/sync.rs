//! Heat sync (`docs/SPEC.md` 2.8, 9.7): what is local is the truth, and Heat
//! records sync privately to the person's account through
//! `/api/heat/changes`, pulled with a cursor and pushed in batches.
//!
//! Every field of every record carries a stamp: a sequence number from a
//! per-device counter, and the device's id. A store keeps whichever stamp is
//! higher, field by field, with the device id breaking a tie, so a slow older
//! write never overwrites a newer one and every store settles on the same
//! value.
//!
//! The counter is a hybrid logical clock ([`next_seq`]): each write takes the
//! device's clock in microseconds, or one more than the highest number the
//! device has made or pulled if that is higher. So a write made after seeing
//! another always carries the higher number, whatever the clocks say; and of
//! two edits made on different devices without either seeing the other, the
//! later one by the clock wins, not the one from the busier device.
//!
//! [`Replica`] is a device's side and [`Server`] the account's; wi-core keeps
//! the same rule in SQLite (or saves a [`Replica`] whole, [`Saved`]), and the
//! mock server runs [`Server`] itself.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

/// Changes in one push at most, and roughly the bytes they may take, so a
/// push stays well under the server's 100 KB JSON limit. A single field
/// larger than that still goes, alone.
pub const PUSH_BATCH: usize = 100;
pub const PUSH_BATCH_BYTES: usize = 64 * 1024;

/// The highest `seq` a store accepts: the largest whole number a JSON number
/// carries exactly in JavaScript, which the server is written in. The clock
/// stays under it until the year 2255. A change numbered above it can't be
/// real, and is ignored, so no pulled number can run the counter out.
pub const MAX_SEQ: u64 = (1 << 53) - 1;

/// The number for a device's next write, given the highest it has made or
/// pulled (`last`): its clock in microseconds since 1970, or `last + 1` if
/// that is higher. A device saves `last` with each write, or recomputes it as
/// the highest `seq` in its store ([`Replica::restore`] does).
pub fn next_seq(last: u64, now: Timestamp) -> u64 {
    let clock = u64::try_from(now.as_microsecond()).unwrap_or(0);
    last.saturating_add(1).max(clock)
}

/// Which of two writes to a field wins: the higher `seq`, then the higher
/// `device`. Field order makes the derived ordering exactly that.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Stamp {
    pub seq: u64,
    pub device: String,
}

/// One field's value, as it goes over the wire.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub table: String,
    pub id: String,
    pub field: String,
    pub value: Value,
    pub seq: u64,
    pub device: String,
}

impl Change {
    pub fn stamp(&self) -> Stamp {
        Stamp {
            seq: self.seq,
            device: self.device.clone(),
        }
    }

    fn key(&self) -> Key {
        (self.table.clone(), self.id.clone(), self.field.clone())
    }
}

/// `GET /api/heat/changes?cursor=&limit=`: the fields that changed after
/// `cursor`, oldest first, and the cursor to ask from next.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub changes: Vec<Change>,
    pub cursor: u64,
    pub more: bool,
}

/// `POST /api/heat/changes`: one batch from [`Replica::push_batch`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Push {
    pub changes: Vec<Change>,
}

/// The answer to a push: how many changes were newer than the server's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pushed {
    pub applied: usize,
}

/// (table, record id, field).
pub type Key = (String, String, String);

#[derive(Clone, Debug, PartialEq)]
struct Cell {
    value: Value,
    stamp: Stamp,
}

/// Applies `c` if it is newer than what `cells` holds. True if it was.
fn merge(cells: &mut BTreeMap<Key, Cell>, c: &Change) -> bool {
    if c.seq > MAX_SEQ {
        return false;
    }
    let stamp = c.stamp();
    match cells.get(&c.key()) {
        Some(cell) if cell.stamp >= stamp => false,
        _ => {
            cells.insert(
                c.key(),
                Cell {
                    value: c.value.clone(),
                    stamp,
                },
            );
            true
        }
    }
}

fn change(key: &Key, cell: &Cell) -> Change {
    Change {
        table: key.0.clone(),
        id: key.1.clone(),
        field: key.2.clone(),
        value: cell.value.clone(),
        seq: cell.stamp.seq,
        device: cell.stamp.device.clone(),
    }
}

/// A [`Replica`] as it is kept between launches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub device: String,
    /// The highest number the device has made or pulled.
    pub seq: u64,
    pub cursor: u64,
    /// Every field, with its stamp.
    pub cells: Vec<Change>,
    /// Fields written here that the server hasn't acknowledged.
    pub pending: Vec<Key>,
}

/// A device's copy of the person's Heat records.
#[derive(Clone, Debug)]
pub struct Replica {
    device: String,
    seq: u64,
    cells: BTreeMap<Key, Cell>,
    /// Fields written here that the server hasn't acknowledged.
    pending: BTreeSet<Key>,
    cursor: u64,
}

impl Replica {
    pub fn new(device: &str) -> Self {
        Replica {
            device: device.to_string(),
            seq: 0,
            cells: BTreeMap::new(),
            pending: BTreeSet::new(),
            cursor: 0,
        }
    }

    /// A device's copy as [`Replica::save`] left it. The counter restarts
    /// at least past every stamp in the store, so a write after a restart
    /// beats every write made before it even if the counter wasn't saved.
    pub fn restore(saved: Saved) -> Self {
        let mut cells = BTreeMap::new();
        for c in &saved.cells {
            merge(&mut cells, c);
        }
        let top = cells.values().map(|c| c.stamp.seq).max().unwrap_or(0);
        let pending = saved
            .pending
            .into_iter()
            .filter(|k| cells.contains_key(k))
            .collect();
        Replica {
            device: saved.device,
            seq: saved.seq.max(top),
            cells,
            pending,
            cursor: saved.cursor,
        }
    }

    /// Everything a device must keep across a restart: save it with each
    /// write (or keep the same rows in SQLite) and [`Replica::restore`] it
    /// on launch.
    pub fn save(&self) -> Saved {
        Saved {
            device: self.device.clone(),
            seq: self.seq,
            cursor: self.cursor,
            cells: self
                .cells
                .iter()
                .map(|(key, cell)| change(key, cell))
                .collect(),
            pending: self.pending.iter().cloned().collect(),
        }
    }

    /// A local edit, stamped by the system clock. It applies at once and
    /// waits to go up.
    pub fn write(&mut self, table: &str, id: &str, field: &str, value: Value) {
        self.write_at(table, id, field, value, Timestamp::now());
    }

    /// A local edit made at `now` by this device's clock.
    pub fn write_at(&mut self, table: &str, id: &str, field: &str, value: Value, now: Timestamp) {
        self.seq = next_seq(self.seq, now);
        let key: Key = (table.to_string(), id.to_string(), field.to_string());
        let stamp = Stamp {
            seq: self.seq,
            device: self.device.clone(),
        };
        self.cells.insert(key.clone(), Cell { value, stamp });
        self.pending.insert(key);
    }

    pub fn value(&self, table: &str, id: &str, field: &str) -> Option<&Value> {
        self.cells
            .get(&(table.to_string(), id.to_string(), field.to_string()))
            .map(|c| &c.value)
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// The next batch to push. The fields stay pending until [`Self::acked`].
    pub fn push_batch(&self) -> Vec<Change> {
        let mut batch = Vec::new();
        let mut bytes = 0;
        for key in &self.pending {
            let c = change(key, &self.cells[key]);
            let size = serde_json::to_vec(&c).map_or(0, |v| v.len());
            if !batch.is_empty() && (batch.len() == PUSH_BATCH || bytes + size > PUSH_BATCH_BYTES) {
                break;
            }
            bytes += size;
            batch.push(c);
        }
        batch
    }

    /// The server has `batch`. A field written again since it was taken stays
    /// pending, because the server doesn't have that value yet.
    pub fn acked(&mut self, batch: &[Change]) {
        for c in batch {
            let key = c.key();
            if self
                .cells
                .get(&key)
                .is_some_and(|cell| cell.stamp == c.stamp())
            {
                self.pending.remove(&key);
            }
        }
    }

    /// Merges a page pulled from the server and moves the cursor past it.
    pub fn pulled(&mut self, page: Page) {
        for c in &page.changes {
            if c.seq <= MAX_SEQ {
                self.seq = self.seq.max(c.seq);
                merge(&mut self.cells, c);
            }
        }
        self.cursor = self.cursor.max(page.cursor);
    }

    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Every field's value, for comparing stores.
    pub fn snapshot(&self) -> BTreeMap<Key, Value> {
        self.cells
            .iter()
            .map(|(k, c)| (k.clone(), c.value.clone()))
            .collect()
    }
}

/// The account's copy on mi-wwav.com. Each accepted change takes the next
/// number in the account's own sequence, which is what a cursor counts.
#[derive(Clone, Debug, Default)]
pub struct Server {
    cells: BTreeMap<Key, Cell>,
    /// Where each field last changed in the server's sequence, both ways.
    changed_at: BTreeMap<Key, u64>,
    by_number: BTreeMap<u64, Key>,
    last: u64,
}

impl Server {
    /// `POST /api/heat/changes`. Returns how many changes were newer than
    /// what the server held ([`Pushed::applied`]); the rest were already
    /// beaten, and the device acks the whole batch either way.
    pub fn push(&mut self, changes: &[Change]) -> usize {
        let mut applied = 0;
        for c in changes {
            if merge(&mut self.cells, c) {
                self.last += 1;
                let key = c.key();
                if let Some(old) = self.changed_at.insert(key.clone(), self.last) {
                    self.by_number.remove(&old);
                }
                self.by_number.insert(self.last, key);
                applied += 1;
            }
        }
        applied
    }

    /// `GET /api/heat/changes?cursor=&limit=`. A field that changed twice
    /// since the cursor comes once, with its latest value.
    pub fn pull(&self, cursor: u64, limit: usize) -> Page {
        let mut changes = Vec::new();
        let mut next = cursor;
        let mut rest = self
            .by_number
            .range((Bound::Excluded(cursor), Bound::Unbounded));
        for (&n, key) in rest.by_ref().take(limit.max(1)) {
            changes.push(change(key, &self.cells[key]));
            next = n;
        }
        Page {
            changes,
            cursor: next,
            more: rest.next().is_some(),
        }
    }

    pub fn snapshot(&self) -> BTreeMap<Key, Value> {
        self.cells
            .iter()
            .map(|(k, c)| (k.clone(), c.value.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stamps_order_by_number_then_device() {
        let s = |seq, device: &str| Stamp {
            seq,
            device: device.into(),
        };
        assert!(s(2, "a") > s(1, "z"));
        assert!(s(1, "b") > s(1, "a"));
        assert_eq!(s(1, "a"), s(1, "a"));
    }

    #[test]
    fn a_change_goes_over_the_wire_as_json() {
        let c = Change {
            table: "task".into(),
            id: "1290877-128934@brightspace.uri.edu".into(),
            field: "estMin".into(),
            value: json!(45),
            seq: 12,
            device: "mac-7f3a".into(),
        };
        let wire = serde_json::to_string(&c).unwrap();
        assert_eq!(
            wire,
            r#"{"table":"task","id":"1290877-128934@brightspace.uri.edu","field":"estMin","value":45,"seq":12,"device":"mac-7f3a"}"#
        );
        assert_eq!(serde_json::from_str::<Change>(&wire).unwrap(), c);
    }

    #[test]
    fn pulling_moves_the_counter_past_what_it_saw() {
        let mut mac = Replica::new("mac");
        mac.pulled(Page {
            changes: vec![Change {
                table: "task".into(),
                id: "t1".into(),
                field: "title".into(),
                value: json!("x"),
                seq: 40,
                device: "air".into(),
            }],
            cursor: 1,
            more: false,
        });
        // A clock far behind what was pulled: the counter still moves past it.
        mac.write_at("task", "t1", "title", json!("y"), Timestamp::UNIX_EPOCH);
        assert_eq!(mac.push_batch()[0].seq, 41);
        assert_eq!(mac.cursor(), 1);
        // A clock ahead of it: the number is the clock, in microseconds.
        let now: Timestamp = "2026-10-06T12:40:00Z".parse().unwrap();
        mac.write_at("task", "t1", "title", json!("z"), now);
        assert_eq!(mac.push_batch()[0].seq, 1_791_290_400_000_000);
    }

    #[test]
    fn a_number_past_max_seq_is_ignored() {
        let mut server = Server::default();
        let mut c = Change {
            table: "task".into(),
            id: "t1".into(),
            field: "title".into(),
            value: json!("x"),
            seq: MAX_SEQ + 1,
            device: "air".into(),
        };
        assert_eq!(server.push(std::slice::from_ref(&c)), 0);
        c.seq = MAX_SEQ;
        assert_eq!(server.push(std::slice::from_ref(&c)), 1);
        assert_eq!(MAX_SEQ, 9_007_199_254_740_991);
    }

    #[test]
    fn a_cursor_past_the_end_returns_nothing() {
        let mut server = Server::default();
        server.push(&[Change {
            table: "task".into(),
            id: "t1".into(),
            field: "done".into(),
            value: json!(true),
            seq: 1,
            device: "mac".into(),
        }]);
        let page = server.pull(u64::MAX, 10);
        assert!(page.changes.is_empty());
        assert!(!page.more);
        assert_eq!(page.cursor, u64::MAX);
    }

    #[test]
    fn a_push_and_its_answer_go_over_the_wire_as_json() {
        let push = Push { changes: vec![] };
        assert_eq!(serde_json::to_string(&push).unwrap(), r#"{"changes":[]}"#);
        assert_eq!(
            serde_json::from_str::<Pushed>(r#"{"applied":3}"#).unwrap(),
            Pushed { applied: 3 }
        );
        let page = Page {
            changes: vec![],
            cursor: 7,
            more: false,
        };
        assert_eq!(
            serde_json::to_string(&page).unwrap(),
            r#"{"changes":[],"cursor":7,"more":false}"#
        );
    }

    #[test]
    fn a_repeated_push_changes_nothing() {
        let mut mac = Replica::new("mac");
        mac.write("habit", "h1", "title", json!("Practise kanji"));
        let batch = mac.push_batch();
        let mut server = Server::default();
        assert_eq!(server.push(&batch), 1);
        assert_eq!(server.push(&batch), 0, "a retried push is a no-op");
        assert_eq!(server.pull(0, 10).changes.len(), 1);
    }
}
