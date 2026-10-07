//! The wire between Wi_WWAV.app and wwav-engine (`docs/ENGINE.md`).
//!
//! - `frame`: a u32 little-endian length, then one JSON object.
//! - `msg`: requests, responses and events, the three kinds of payload.
//! - `graph`: the session as the engine receives it in `session.load`.
//! - `client`: the app's side of the socket, matching responses to requests.
//! - `shm`: the shared-memory region, mirrored from `engine/include/wwav_shm.h`.
//! - `process`: the app's side of starting the engine (§1).

pub mod frame;
pub mod graph;
pub mod msg;
pub mod shm;

// Windows (Stage 6): TODO the socket becomes a named pipe, so `client` and
// `process` are Unix-only until then.
#[cfg(unix)]
pub mod client;
#[cfg(unix)]
pub mod process;

/// The protocol version both sides send in `hello`.
pub const PROTOCOL: u32 = 1;
