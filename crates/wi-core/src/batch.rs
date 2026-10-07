//! Many writes, one ⌘Z (docs/ASK.md).
//!
//! Learn's writes are each one command, one store function and one journal
//! entry, so a rule exists once. A paste over forty cells or the prompt box
//! moving five tasks runs those same commands one after another inside
//! [`one_step`], which notes the entries they make on this thread and then
//! joins them into one (`wi_store::Store::merge_entries`). Nothing is
//! written a second way, and one undo takes the whole change back.

use std::cell::RefCell;

use crate::{history, CoreError, Inner};

thread_local! {
    /// The entries made on this thread while a batch runs; None outside one.
    static NOTED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Called for every journal entry this process makes (`Inner::note_own`).
pub(crate) fn noted(id: &str) {
    NOTED.with(|n| {
        if let Some(list) = n.borrow_mut().as_mut() {
            list.push(id.to_string());
        }
    });
}

/// What a batch left in the journal.
pub(crate) struct Step {
    /// "Undo paste", or None when nothing changed.
    pub undo: Option<String>,
    /// How many writes made an entry.
    pub entries: usize,
    /// The entry they became, when there is one.
    pub txn: Option<String>,
}

/// Runs `f`, then joins every journal entry it made on this thread into one
/// labelled `label`. With `keep_single`, a batch that made exactly one
/// entry keeps that entry's own label ("rename task" says more than "edit
/// cell"). Entries that can't be joined (another process wrote in between)
/// stay as separate steps, which is still correct, only more presses of ⌘Z.
pub(crate) fn one_step<T>(
    i: &Inner,
    label: &str,
    keep_single: bool,
    f: impl FnOnce() -> T,
) -> Result<(T, Step), CoreError> {
    // A batch inside a batch joins into the outer one.
    let nested = NOTED.with(|n| n.borrow().is_some());
    if nested {
        let out = f();
        return Ok((
            out,
            Step {
                undo: None,
                entries: 0,
                txn: None,
            },
        ));
    }
    NOTED.with(|n| *n.borrow_mut() = Some(Vec::new()));
    let out = f();
    let ids = NOTED.with(|n| n.borrow_mut().take()).unwrap_or_default();
    let step = match ids.len() {
        0 => Step {
            undo: None,
            entries: 0,
            txn: None,
        },
        1 if keep_single => {
            let label = i
                .store()
                .entry_docs(&ids[0])?
                .map(|e| e.label)
                .unwrap_or_else(|| label.to_string());
            Step {
                undo: Some(format!("Undo {label}")),
                entries: 1,
                txn: Some(ids[0].clone()),
            }
        }
        n => {
            let merged = i.store().merge_entries(&ids, label);
            match merged {
                Ok(id) => {
                    if let Some(id) = &id {
                        i.note_own(id);
                    }
                    // The Edit menu's words changed without a new change.
                    history::records_changed(i, Vec::new());
                    Step {
                        undo: Some(format!("Undo {label}")),
                        entries: n,
                        txn: id,
                    }
                }
                Err(wi_store::Error::Refused(_)) => Step {
                    undo: None,
                    entries: n,
                    txn: None,
                },
                Err(e) => return Err(e.into()),
            }
        }
    };
    Ok((out, step))
}
