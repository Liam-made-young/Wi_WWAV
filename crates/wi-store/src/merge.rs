//! Several journal entries made one after another become one, so a change
//! made of many writes is a single ⌘Z.
//!
//! Learn's writes are each one store function and one entry, so a rule
//! exists once (docs/HEAT.md). A batch (a paste over forty cells, the
//! prompt box moving five tasks) runs those same functions one by one and
//! then joins what they journaled. Joining is exact: an entry's rows are
//! kept in the order they happened, undo writes `before` back in reverse
//! and redo writes `after` forward, so entries laid end to end undo and redo
//! as the one change they were.

use rusqlite::{params, TransactionBehavior};

use crate::{Error, Result, Store};

impl Store {
    /// Joins the entries `ids` into one, labelled `label`, and returns its
    /// id (the newest of them, so a watcher that has read past the others
    /// still meets it). None when `ids` is empty.
    ///
    /// The entries must be done, in one room, and next to each other in the
    /// journal: if anything else was journaled between the first and the
    /// last, they are left as they are and the answer is `Refused`, since
    /// joining would put their rows out of order with it.
    pub fn merge_entries(&mut self, ids: &[String], label: &str) -> Result<Option<String>> {
        let mut ids: Vec<String> = ids.to_vec();
        ids.sort();
        ids.dedup();
        let (Some(first), Some(last)) = (ids.first().cloned(), ids.last().cloned()) else {
            return Ok(None);
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if ids.len() == 1 {
            let n = tx.execute(
                "UPDATE txn SET label = ?2 WHERE id = ?1 AND state = 'done'",
                params![first, label],
            )?;
            if n == 0 {
                return Err(Error::Refused(
                    "That change isn't in the journal any more.".into(),
                ));
            }
            tx.commit()?;
            return Ok(Some(first));
        }
        let mut room: Option<String> = None;
        for id in &ids {
            let found: Option<(String, String)> = {
                let mut stmt = tx.prepare_cached("SELECT room, state FROM txn WHERE id = ?1")?;
                let mut rows = stmt.query([id])?;
                match rows.next()? {
                    Some(r) => Some((r.get(0)?, r.get(1)?)),
                    None => None,
                }
            };
            let Some((r, state)) = found else {
                return Err(Error::Refused(
                    "One of those changes isn't in the journal any more.".into(),
                ));
            };
            if state != "done" {
                return Err(Error::Refused(
                    "One of those changes was undone already.".into(),
                ));
            }
            match &room {
                None => room = Some(r),
                Some(have) if *have != r => {
                    return Err(Error::Refused(
                        "Those changes were made in different views.".into(),
                    ))
                }
                Some(_) => {}
            }
        }
        // Nothing else between the first and the last: not another process's
        // entry, and not work that left the machine.
        let between: i64 = tx.query_row(
            "SELECT (SELECT count(*) FROM txn WHERE id > ?1 AND id < ?2)
                  + (SELECT count(*) FROM outward WHERE id > ?1 AND id < ?2)",
            params![first, last],
            |r| r.get(0),
        )?;
        if between != ids.len() as i64 - 2 {
            return Err(Error::Refused(
                "Something else changed in between, so these stay separate steps.".into(),
            ));
        }
        // Every row of every entry, oldest entry first, in the order it happened.
        let mut rows: Vec<(String, String, Option<String>, Option<String>)> = Vec::new();
        for id in &ids {
            let mut stmt = tx.prepare_cached(
                "SELECT tbl, row_id, before, after FROM txn_row WHERE txn_id = ?1 ORDER BY seq",
            )?;
            let found =
                stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
            for row in found {
                rows.push(row?);
            }
        }
        for id in &ids {
            tx.execute("DELETE FROM txn_row WHERE txn_id = ?1", [id])?;
        }
        for (seq, (tbl, row_id, before, after)) in rows.iter().enumerate() {
            tx.prepare_cached(
                "INSERT INTO txn_row (txn_id, seq, tbl, row_id, before, after) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?
            .execute(params![last, seq as i64, tbl, row_id, before, after])?;
        }
        for id in ids.iter().filter(|id| **id != last) {
            // Work that left the machine after one of them now follows the joined entry.
            tx.execute(
                "UPDATE outward SET after_txn = ?2 WHERE after_txn = ?1",
                params![id, last],
            )?;
            tx.execute("DELETE FROM txn WHERE id = ?1", [id])?;
        }
        tx.execute(
            "UPDATE txn SET label = ?2 WHERE id = ?1",
            params![last, label],
        )?;
        tx.commit()?;
        Ok(Some(last))
    }
}
