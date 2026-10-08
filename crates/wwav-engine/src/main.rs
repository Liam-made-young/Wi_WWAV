//! wwav-engine: see the library's docs, and `docs/ENGINE.md`.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use wwav_engine::args::{self, USAGE};
use wwav_engine::engine::Engine;
use wwav_wire::shm::{self, HeaderFields, Shm};

static SIGNALLED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    SIGNALLED.store(true, Ordering::Relaxed);
}

fn die(message: String) -> ! {
    eprintln!("wwav-engine: {message}");
    std::process::exit(1)
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name(name.into())
        .spawn(f)
        .unwrap_or_else(|e| die(format!("can't start a thread ({e})")));
}

fn main() {
    let args =
        args::parse(std::env::args().skip(1)).unwrap_or_else(|e| die(format!("{e}\n{USAGE}")));
    let region =
        Shm::open(&args.shm).unwrap_or_else(|e| die(format!("can't map {} ({e})", args.shm)));
    region.region().header.write(&HeaderFields {
        sample_rate: args.rate,
        block_size: args.block,
        engine_pid: std::process::id() as u64,
        engine_start_ns: shm::monotonic_ns(),
    });
    let socket = args.socket.clone();
    let (engine, mut worker) = Engine::new(args, region);

    // The device is open, or known not to open, before anyone can ask.
    let (opened, wait) = std::sync::mpsc::channel();
    spawn("worker", move || {
        worker.open_first();
        let _ = opened.send(());
        worker.run()
    });
    let _ = wait.recv();

    // A socket file left by an engine that died before this one.
    let _ = std::fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)
        .unwrap_or_else(|e| die(format!("can't listen on {} ({e})", socket.display())));
    // The directory is already 0700; this keeps the socket owner-only too.
    let _ = std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600));
    engine.bound(&socket);

    spawn("stdin", {
        let engine = engine.clone();
        move || {
            // The app's pipe: end-of-file means the app has gone (§1).
            let mut buf = [0u8; 64];
            while matches!(std::io::stdin().read(&mut buf), Ok(n) if n > 0) {}
            engine.quit("the app's pipe closed");
        }
    });
    // SAFETY: the handler only stores to an atomic.
    unsafe {
        let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }
    spawn("signals", {
        let engine = engine.clone();
        move || loop {
            std::thread::sleep(Duration::from_millis(20));
            if SIGNALLED.load(Ordering::Relaxed) {
                engine.quit("a signal came");
                return;
            }
        }
    });

    println!("wwav-engine listening {}", socket.display());
    let _ = std::io::stdout().flush();
    // One client at a time.
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => engine.serve(stream),
            Err(e) => eprintln!("wwav-engine: accept failed ({e})"),
        }
    }
}
