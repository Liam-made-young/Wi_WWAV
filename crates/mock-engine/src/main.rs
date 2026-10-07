//! mock-engine: a stand-in for wwav-engine that speaks docs/ENGINE.md
//! exactly, with a fake but deterministic audio model. The app's UI suite
//! and its supervisor tests run against it (docs/SPEC.md 9.11).
//!
//! ```text
//! mock-engine --socket <path> --shm <name> [--device <name|null>] [--rate N] [--block N] [--test]
//! ```
//!
//! What it pretends to have, for tests that need names to ask for:
//!
//! - Devices: `null` (the default: no inputs, no latency) and `Mock
//!   Interface` (2 in, 2 out, output latency 312, input latency 296). Both
//!   run on the timer; neither makes sound.
//! - Audio (`model`): every track plays its role's tone (vocals 440 Hz,
//!   drums 110, other 330, bass 55) at its role's level, through its fader,
//!   pan, mute and solo; each stem bus is its tracks' sum, the master the
//!   buses' sum. Meters show those levels while playing and silence while
//!   stopped. Devices pass audio through.
//! - Plugins: any `vst3` or `au` device is the mock plugin, with params
//!   `p0`–`p3` (Mix, Time, Feedback, Lookahead), 0 to 1, unset ones at 0.
//!   Its latency is 0, 64, 128 or 192 samples by its id, plus 2048 ×
//!   Lookahead. Its state is its params as JSON; a `mock.state_bytes`
//!   entry in the device's graph `params` pads it to that size, to test the
//!   256 KB file rule. A state it didn't write is kept and handed back as it
//!   came.
//! - MIDI: one input, `mock-keys` ("Mock Keys").
//! - `session.load` refuses a graph whose `sample_rate` isn't the device's
//!   (`rate_mismatch`): the app opens the device at the session's rate first.

mod engine;
mod model;
mod wav;

use engine::Engine;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use wwav_wire::shm::{self, Shm};

const USAGE: &str =
    "usage: mock-engine --socket <path> --shm <name> [--device <name|null>] [--rate N] [--block N] [--test]";

struct Args {
    socket: PathBuf,
    shm: String,
    device: String,
    rate: u32,
    block: u32,
    test: bool,
}

fn parse(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut socket, mut shm) = (None, None);
    let mut args = Args {
        socket: PathBuf::new(),
        shm: String::new(),
        device: "null".into(),
        rate: 48000,
        block: 128,
        test: false,
    };
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--socket" => socket = Some(PathBuf::from(value()?)),
            "--shm" => shm = Some(value()?),
            "--device" => args.device = value()?,
            "--rate" => args.rate = value()?.parse().map_err(|_| "--rate is a number of Hz")?,
            "--block" => {
                args.block = value()?
                    .parse()
                    .map_err(|_| "--block is a number of frames")?
            }
            "--test" => args.test = true,
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    args.socket = socket.ok_or("--socket is required")?;
    args.shm = shm.ok_or("--shm is required")?;
    Ok(args)
}

fn die(message: String) -> ! {
    eprintln!("mock-engine: {message}");
    std::process::exit(1)
}

fn main() {
    let args = parse(std::env::args().skip(1)).unwrap_or_else(|e| die(format!("{e}\n{USAGE}")));
    let device = engine::device_named(&args.device)
        .unwrap_or_else(|| die(format!("no device named {:?}", args.device)));
    engine::valid_format(device, args.rate, args.block).unwrap_or_else(|e| die(e));
    let region =
        Shm::open(&args.shm).unwrap_or_else(|e| die(format!("can't map {} ({e})", args.shm)));
    let engine = Arc::new(Engine::new(
        region,
        args.socket.clone(),
        args.test,
        device,
        args.rate,
        args.block,
    ));

    // A socket file left by an engine that died before us.
    let _ = std::fs::remove_file(&args.socket);
    let listener = UnixListener::bind(&args.socket)
        .unwrap_or_else(|e| die(format!("can't listen on {} ({e})", args.socket.display())));
    // The directory is already 0700; this keeps the socket owner-only too.
    let _ = std::fs::set_permissions(&args.socket, std::fs::Permissions::from_mode(0o600));

    spawn("audio", {
        let engine = engine.clone();
        move || audio(&engine)
    });
    spawn("stdin", {
        let engine = engine.clone();
        move || {
            // The app's pipe: end-of-file means the app has gone (§1).
            let mut buf = [0u8; 64];
            while matches!(std::io::stdin().read(&mut buf), Ok(n) if n > 0) {}
            engine.exit()
        }
    });
    spawn("housekeeping", {
        let engine = engine.clone();
        move || loop {
            thread::sleep(Duration::from_secs(1));
            engine.tick();
        }
    });

    println!("wwav-engine listening {}", args.socket.display());
    let _ = std::io::stdout().flush();
    // The message thread: one client at a time.
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => engine.serve(stream),
            Err(e) => eprintln!("mock-engine: accept failed ({e})"),
        }
    }
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) {
    thread::Builder::new()
        .name(name.into())
        .spawn(f)
        .unwrap_or_else(|e| die(format!("can't start a thread ({e})")));
}

/// The null device: a timer that runs one block every block's worth of time.
/// Each block is stamped with its nominal time, not when the thread woke, so
/// the clock moves at exactly the rate however late a wake-up is; a thread
/// that falls far behind (a suspended process) counts a dropout and starts
/// over from now.
fn audio(engine: &Engine) {
    let mut format = engine.format();
    let mut epoch = shm::monotonic_ns();
    let mut n: u64 = 0;
    loop {
        let now_format = engine.format();
        if now_format != format {
            (format, epoch, n) = (now_format, shm::monotonic_ns(), 0);
        }
        let (rate, block) = (format.0 as u128, format.1 as u128);
        let mut due = epoch + (n as u128 * block * 1_000_000_000 / rate) as u64;
        let now = shm::monotonic_ns();
        if due > now {
            thread::sleep(Duration::from_nanos(due - now));
        } else if now - due > 200_000_000 {
            engine.dropout();
            (epoch, n, due) = (now, 0, now);
        }
        engine.process_block(due);
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Result<Args, String> {
        parse(s.split_whitespace().map(String::from))
    }

    #[test]
    fn flags_parse_with_the_contracts_defaults() {
        let a = args("--socket /tmp/e.sock --shm /wwav-1-0").unwrap();
        assert_eq!(
            (a.device.as_str(), a.rate, a.block, a.test),
            ("null", 48000, 128, false)
        );
        let a = args("--socket /s --shm /m --device null --rate 44100 --block 256 --test").unwrap();
        assert_eq!((a.rate, a.block, a.test), (44100, 256, true));
        assert!(args("--shm /m").is_err());
        assert!(args("--socket /s --shm /m --rate fast").is_err());
        assert!(args("--socket /s --shm /m --colour blue").is_err());
    }
}
