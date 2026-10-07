//! Where the core speaks unasked: events as JSON (`{event, payload}`), and
//! the meters as raw bytes on their own channel (docs/COMMANDS.md). Anyone
//! may listen; a listener that has gone is dropped at the next send.

use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TrySendError};
use std::sync::Mutex;

use serde::Serialize;
use serde_json::{json, Value};

/// One event, as the UI receives it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Event {
    pub event: String,
    pub payload: Value,
}

#[derive(Default)]
pub(crate) struct Bus {
    events: Mutex<Vec<Sender<Event>>>,
    meters: Mutex<Vec<SyncSender<Vec<u8>>>>,
}

impl Bus {
    pub fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = mpsc::channel();
        lock(&self.events).push(tx);
        rx
    }

    pub fn emit(&self, event: &str, payload: Value) {
        let e = Event {
            event: event.to_string(),
            payload,
        };
        lock(&self.events).retain(|tx| tx.send(e.clone()).is_ok());
    }

    /// The status bar's line for one area: "Saved on this Mac", "Uploading
    /// World Ending · part 14 of 27".
    pub fn status(&self, area: &str, sentence: &str) {
        self.emit("status", json!({"area": area, "sentence": sentence}));
    }

    /// A meters listener. The channel holds one frame: a listener that falls
    /// behind gets the newest, never a backlog.
    pub fn meters(&self) -> Receiver<Vec<u8>> {
        let (tx, rx) = mpsc::sync_channel(1);
        lock(&self.meters).push(tx);
        rx
    }

    pub fn wants_meters(&self) -> bool {
        !lock(&self.meters).is_empty()
    }

    pub fn send_meters(&self, bytes: &[u8]) {
        lock(&self.meters).retain(|tx| match tx.try_send(bytes.to_vec()) {
            Ok(()) | Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });
    }
}

/// A poisoned lock still holds good data here: every writer leaves it whole.
pub(crate) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
