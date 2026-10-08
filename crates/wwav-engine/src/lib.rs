//! wwav-engine: the audio engine `Wi_WWAV.app` runs as a child process, in
//! Rust. It is a server of `docs/ENGINE.md`: framed JSON over a socket, and
//! the clock, meters and crumb in shared memory.
//!
//! ```text
//!   main      accepts one client at a time and reads its frames: hello,
//!             ping and param.set are answered there, the rest handed on
//!   writer    sends every frame out, so no other thread waits on the client
//!   worker    runs ops in the order they came: devices, sessions, the
//!             transport, renders, takes
//!   audio     the device's callback, or the timer when no device is open
//!   reader    keeps streamed clips ahead of the playhead
//! ```

pub mod args;
pub mod audio;
pub mod build;
pub mod convert;
pub mod device;
pub mod engine;
pub mod export;
pub mod graph;
pub mod media;
pub mod record;
pub mod server;
pub mod wav;
