//! The app's side of the command socket: a blocking client.
//!
//! Requests go out under a lock, one frame each. A reader thread takes every
//! frame that comes back: a response goes to the caller waiting on its id
//! (responses may come in any order), an event goes on the event channel.
//! When the connection ends, for any reason, every waiting call fails with
//! `Closed` and the event channel ends, which is how the app sees an engine
//! die. `request_ordered` also says how many events came before a response,
//! for a caller (the CLI) that shows both in the wire's order.

use crate::frame::{self, FrameError};
use crate::msg::{ErrorBody, Event, Incoming, Request, Response};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::io;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub enum CallError {
    #[error("{}: {}", .0.code, .0.message)]
    Engine(ErrorBody),
    #[error("no answer within {0:?}")]
    Timeout(Duration),
    #[error("the connection to the engine closed")]
    Closed,
    #[error("a request's args are an object")]
    Args,
    #[error(transparent)]
    Frame(FrameError),
}

pub struct Client {
    stream: UnixStream,
    write: Mutex<()>,
    shared: Arc<Shared>,
    next_id: AtomicU64,
    reader: Option<JoinHandle<()>>,
}

struct Shared {
    state: Mutex<State>,
    closed: Condvar,
}

/// A response and its place among the events.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub response: Response,
    /// How many events the connection had delivered when the response
    /// arrived: the ones that came before it on the wire. They are on the
    /// event channel ahead of any that came after it.
    pub events_before: u64,
}

#[derive(Default)]
struct State {
    waiting: HashMap<u64, SyncSender<Answer>>,
    /// Why the connection ended, once it has.
    closed: Option<String>,
}

/// A write that makes no progress for this long means the engine has
/// stopped reading; the call fails rather than hang the app with it. (The
/// socket's timeout is per write call, so a frame that went out in part
/// first can take up to twice this.)
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

impl Client {
    pub fn connect(path: &Path) -> io::Result<(Client, Receiver<Event>)> {
        Client::new(UnixStream::connect(path)?)
    }

    /// A client over a connected stream, and the channel its events arrive on.
    pub fn new(stream: UnixStream) -> io::Result<(Client, Receiver<Event>)> {
        stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            closed: Condvar::new(),
        });
        let (tx, rx) = mpsc::channel();
        let reader = {
            let stream = stream.try_clone()?;
            let shared = shared.clone();
            thread::Builder::new()
                .name("wwav-wire reader".into())
                .spawn(move || read_loop(stream, shared, tx))?
        };
        let client = Client {
            stream,
            write: Mutex::new(()),
            shared,
            next_id: AtomicU64::new(1),
            reader: Some(reader),
        };
        Ok((client, rx))
    }

    /// Sends `op` and waits up to `timeout` for its result. `args` is an
    /// object, or `Null` for an op that takes none.
    pub fn call(
        &self,
        op: &str,
        args: Value,
        timeout: Duration,
    ) -> Result<Map<String, Value>, CallError> {
        self.request(op, args, timeout)?
            .outcome
            .map_err(CallError::Engine)
    }

    /// Like `call`, but an engine's refusal comes back as the response it is.
    pub fn request(&self, op: &str, args: Value, timeout: Duration) -> Result<Response, CallError> {
        self.request_ordered(op, args, timeout).map(|a| a.response)
    }

    /// Like `request`, with the number of events that came before the
    /// response.
    pub fn request_ordered(
        &self,
        op: &str,
        args: Value,
        timeout: Duration,
    ) -> Result<Answer, CallError> {
        let args = match args {
            Value::Null => None,
            Value::Object(m) => Some(m),
            _ => return Err(CallError::Args),
        };
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::sync_channel(1);
        {
            let mut state = self.shared.lock();
            if state.closed.is_some() {
                return Err(CallError::Closed);
            }
            state.waiting.insert(id, tx);
        }
        let sent = {
            let _w = self.write.lock().unwrap_or_else(|e| e.into_inner());
            frame::write(
                &mut &self.stream,
                &Request {
                    id,
                    op: op.into(),
                    args,
                },
            )
        };
        if let Err(e) = sent {
            self.shared.lock().waiting.remove(&id);
            return Err(match e {
                // A write that failed usually means the engine has gone, and
                // it may have left half a frame on the stream: either way the
                // connection can carry nothing more (§2).
                FrameError::Io(e) => {
                    self.shared
                        .close(format!("Writing to the engine failed: {e}."));
                    self.close();
                    CallError::Closed
                }
                // Nothing was written: this request can't be framed.
                e => CallError::Frame(e),
            });
        }
        match rx.recv_timeout(timeout) {
            Ok(answer) => Ok(answer),
            Err(RecvTimeoutError::Timeout) => {
                // A late answer finds no one waiting and is dropped.
                self.shared.lock().waiting.remove(&id);
                Err(CallError::Timeout(timeout))
            }
            Err(RecvTimeoutError::Disconnected) => Err(CallError::Closed),
        }
    }

    pub fn is_closed(&self) -> bool {
        self.shared.lock().closed.is_some()
    }

    /// Why the connection ended, once it has.
    pub fn closed_reason(&self) -> Option<String> {
        self.shared.lock().closed.clone()
    }

    /// Waits up to `timeout` for the connection to end; true if it has.
    pub fn wait_closed(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut state = self.shared.lock();
        while state.closed.is_none() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            state = self
                .shared
                .closed
                .wait_timeout(state, left)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        true
    }

    /// Says this side has nothing more to send (a half close). The engine
    /// reads the end of its input and closes the connection, and everything
    /// it sent before that still arrives.
    pub fn close_writes(&self) {
        let _ = self.stream.shutdown(Shutdown::Write);
    }

    /// Ends the connection from this side.
    pub fn close(&self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.close();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn close(&self, why: String) {
        let mut state = self.lock();
        state.closed.get_or_insert(why);
        // Dropping the senders wakes every waiting call with Closed.
        state.waiting.clear();
        self.closed.notify_all();
    }
}

fn read_loop(mut stream: UnixStream, shared: Arc<Shared>, events: Sender<Event>) {
    // Events handed to the channel so far, whether or not anyone listens.
    let mut delivered: u64 = 0;
    let why = loop {
        let m = match frame::read(&mut stream) {
            Ok(Some(m)) => m,
            Ok(None) => break "The engine closed the connection.".to_string(),
            Err(e) => break format!("The connection broke: {e}."),
        };
        match Incoming::from_object(m) {
            Ok(Incoming::Response(response)) => {
                if let Some(tx) = shared.lock().waiting.remove(&response.id) {
                    let _ = tx.send(Answer {
                        response,
                        events_before: delivered,
                    });
                }
            }
            Ok(Incoming::Event(e)) => {
                let _ = events.send(e);
                delivered += 1;
            }
            Err(e) => break e,
        }
    };
    // A broken frame closes the connection (§2), so writes fail fast too.
    let _ = stream.shutdown(Shutdown::Both);
    shared.close(why);
}
