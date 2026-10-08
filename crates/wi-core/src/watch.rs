//! Noticing the MCP helper (docs/SPEC.md 8.8). The app and `wi-mcp` write one
//! SQLite file, each through the same journal. SQLite tells a connection when
//! another has committed (`PRAGMA data_version`), so every half second the
//! core asks; when the answer changed it reads the journal entries newer than
//! the last it saw, skips the ones it wrote itself, and for the rest tells the
//! views which kinds changed and carries the records to sync, which only the
//! app does. At start it does the same for whatever the helper wrote while the
//! app was closed.

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::heat_cmd::announce;
use crate::{heat, Inner};

const EVERY: Duration = Duration::from_millis(500);

/// Looks at the journal now: what an entry by another process changed is
/// announced and synced. Returns the last entry seen.
fn look(i: &Inner, after: Option<String>) -> Option<String> {
    let entries = match i.store().entries_after(after.as_deref(), 1000) {
        Ok(e) => e,
        Err(_) => return after,
    };
    let mut last = after;
    let mut docs = Vec::new();
    for e in entries {
        last = Some(e.id.clone());
        if !i.take_own(&e.id) {
            docs.extend(e.docs);
        }
    }
    if !docs.is_empty() {
        crate::focus_cmd::noticed(i, &docs);
        let _ = announce(i, &docs, &[]);
    }
    last
}

pub(crate) fn start(inner: &Arc<Inner>) -> JoinHandle<()> {
    let i = inner.clone();
    std::thread::Builder::new()
        .name("heat watch".into())
        .spawn(move || {
            // The journal as it stands, before anything is read: what comes
            // after it is news. What came before it, and isn't in sync yet
            // (the helper wrote while the app was closed), goes up now.
            let mut last = i.store().last_entry().ok().flatten();
            let mut version = i.store().data_version().ok();
            let _ = heat::journal_moved(&i);
            while !i.closing() {
                std::thread::sleep(EVERY);
                let now = i.store().data_version().ok();
                if now != version {
                    version = now;
                    last = look(&i, last);
                }
            }
        })
        .expect("a thread for watching the journal")
}
