//! The socket's way out (`docs/ENGINE.md` 2): every frame the engine sends
//! goes through a queue to a writer thread, so no thread that has work to do
//! ever waits on a client that has stopped reading.

use serde::Serialize;
use serde_json::{Map, Value};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wwav_wire::frame::{self, FrameError};
use wwav_wire::msg::{ErrorBody, Event, Response};

pub type Reply = Result<Map<String, Value>, ErrorBody>;

/// The longest sentence an error carries. A message that quotes the request
/// (a 16 MiB param name, say) is cut, so no reply is over a frame's limit.
const MAX_MESSAGE: usize = 16 * 1024;
/// A client with this much waiting for it has stopped reading: it is let go.
const MAX_QUEUED: usize = 64 * 1024 * 1024;

pub fn fail<T>(code: &str, message: impl Into<String>) -> Result<T, ErrorBody> {
    Err(error(code, message))
}

pub fn error(code: &str, message: impl Into<String>) -> ErrorBody {
    let mut message = message.into();
    if message.len() > MAX_MESSAGE {
        let mut cut = MAX_MESSAGE;
        while !message.is_char_boundary(cut) {
            cut -= 1;
        }
        message.truncate(cut);
        message.push('…');
    }
    ErrorBody::new(code, message)
}

pub fn done(v: Value) -> Reply {
    match v {
        Value::Object(m) => Ok(m),
        _ => Ok(Map::new()),
    }
}

struct Conn {
    id: u64,
    frames: Sender<Vec<u8>>,
    queued: Arc<AtomicUsize>,
    stream: UnixStream,
}

/// The connected client, if there is one.
#[derive(Default)]
pub struct Outbox {
    conn: Mutex<Option<Conn>>,
}

impl Outbox {
    /// A new client: frames queued from now go to it.
    pub fn connect(&self, id: u64, stream: &UnixStream) -> std::io::Result<()> {
        let (frames, queue) = mpsc::channel::<Vec<u8>>();
        let queued = Arc::new(AtomicUsize::new(0));
        let mut out = stream.try_clone()?;
        let shut = stream.try_clone()?;
        std::thread::Builder::new().name("writer".into()).spawn({
            let queued = queued.clone();
            move || {
                for bytes in queue {
                    if out.write_all(&bytes).is_err() {
                        break;
                    }
                    queued.fetch_sub(bytes.len(), Ordering::Relaxed);
                }
            }
        })?;
        *self.lock() = Some(Conn {
            id,
            frames,
            queued,
            stream: shut,
        });
        Ok(())
    }

    /// The client has gone, or is let go: a write it was blocking fails, and
    /// what was queued for it is dropped.
    pub fn disconnect(&self, id: u64) {
        let mut conn = self.lock();
        if conn.as_ref().is_some_and(|c| c.id == id) {
            if let Some(c) = conn.take() {
                let _ = c.stream.shutdown(std::net::Shutdown::Both);
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Conn>> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Queues one frame for connection `only`, or for whoever is connected.
    fn send(&self, only: Option<u64>, bytes: Vec<u8>) {
        let mut conn = self.lock();
        let Some(c) = conn.as_ref().filter(|c| only.is_none_or(|id| id == c.id)) else {
            return;
        };
        if c.queued.fetch_add(bytes.len(), Ordering::Relaxed) + bytes.len() > MAX_QUEUED {
            let _ = c.stream.shutdown(std::net::Shutdown::Both);
            *conn = None;
            return;
        }
        let _ = c.frames.send(bytes);
    }

    /// An answer, on the connection that asked: it means nothing to the next.
    pub fn reply(&self, conn: u64, id: u64, outcome: Reply) {
        let bytes = match encode(&Response { id, outcome }) {
            Ok(bytes) => bytes,
            Err(FrameError::TooLong(n)) => {
                let outcome = fail(
                    "too_big",
                    format!("The answer is {n} bytes, more than a frame holds (16 MiB)."),
                );
                match encode(&Response { id, outcome }) {
                    Ok(bytes) => bytes,
                    Err(_) => return,
                }
            }
            Err(_) => return,
        };
        self.send(Some(conn), bytes);
    }

    /// Something the engine says unasked, to whoever is connected.
    pub fn event(&self, ev: &str, fields: Value) {
        let Value::Object(fields) = fields else {
            return;
        };
        if let Ok(bytes) = encode(&Event {
            ev: ev.into(),
            fields,
        }) {
            self.send(None, bytes);
        }
    }

    /// Waits, for `limit` at most, until everything queued has been written.
    pub fn flush(&self, limit: Duration) {
        let until = Instant::now() + limit;
        loop {
            let waiting = self
                .lock()
                .as_ref()
                .map_or(0, |c| c.queued.load(Ordering::Relaxed));
            if waiting == 0 || Instant::now() > until {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, FrameError> {
    frame::encode(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_message_is_cut_on_a_character() {
        let e = error("no_such_param", "é".repeat(MAX_MESSAGE));
        assert!(e.message.len() <= MAX_MESSAGE + 3 && e.message.ends_with('…'));
        assert_eq!(error("x", "Short.").message, "Short.");
    }
}
