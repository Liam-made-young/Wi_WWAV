//! S1.11, Linux part: the sign-in token lands in the operating system's
//! secret store and nowhere else (docs/SPEC.md 9.8). On Linux that store is
//! the Secret Service; the Mac's Keychain is checked by hand on a Mac.
//!
//! Fails if: a secret set through the store isn't in the Secret Service when
//! another client (secret-tool) looks; reading it back gives anything else;
//! or forgetting it leaves it there. tests/keyring_missing.rs covers a
//! machine with no Secret Service.
//!
//! It runs its own session bus with GNOME Keyring on it, in a throwaway home,
//! so it never touches a real keyring. It needs dbus-daemon,
//! gnome-keyring-daemon and secret-tool (Debian and Ubuntu: dbus,
//! gnome-keyring, libsecret-tools).

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use wi_wwav_app::{KeyringStore, SERVICE};

/// Kills what it holds when the test ends, pass or fail.
struct Reaper(Vec<Child>);

impl Drop for Reaper {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn spawn(cmd: &mut Command, what: &str) -> Child {
    cmd.spawn().unwrap_or_else(|e| {
        panic!("couldn't start {what} ({e}); install dbus, gnome-keyring and libsecret-tools")
    })
}

/// A private session bus, and GNOME Keyring on it with an unlocked login
/// keyring. Returns the bus address.
fn keyring_on_a_private_bus(home: &Path, reaper: &mut Reaper) -> String {
    let socket = home.join("bus");
    let mut bus = spawn(
        Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address"])
            .arg(format!("--address=unix:path={}", socket.display()))
            .stdout(Stdio::piped()),
        "dbus-daemon",
    );
    let mut address = String::new();
    BufReader::new(bus.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    reaper.0.push(bus);
    let address = address.trim().to_string();

    let mut keyring = spawn(
        Command::new("gnome-keyring-daemon")
            .args(["--foreground", "--unlock", "--components=secrets"])
            .env("DBUS_SESSION_BUS_ADDRESS", &address)
            .env("HOME", home)
            .env("XDG_RUNTIME_DIR", home)
            .env("XDG_DATA_HOME", home.join("data"))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
        "gnome-keyring-daemon",
    );
    // --unlock reads the new login keyring's password from stdin.
    keyring.stdin.take().unwrap().write_all(b"test").unwrap();
    reaper.0.push(keyring);

    let start = Instant::now();
    while !has_owner(&address, "org.freedesktop.secrets") {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "GNOME Keyring never joined the bus"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    address
}

fn has_owner(address: &str, name: &str) -> bool {
    let out = Command::new("dbus-send")
        .args([
            "--print-reply",
            "--dest=org.freedesktop.DBus",
            "/org/freedesktop/DBus",
        ])
        .args([
            "org.freedesktop.DBus.NameHasOwner",
            &format!("string:{name}"),
        ])
        .env("DBUS_SESSION_BUS_ADDRESS", address)
        .output()
        .expect("dbus-send runs");
    String::from_utf8_lossy(&out.stdout).contains("boolean true")
}

/// What another client of the Secret Service finds under our name.
fn secret_tool_lookup(address: &str, name: &str) -> Option<String> {
    let out = Command::new("secret-tool")
        .args(["lookup", "service", SERVICE, "username", name])
        .env("DBUS_SESSION_BUS_ADDRESS", address)
        .output()
        .unwrap_or_else(|e| panic!("couldn't run secret-tool ({e}); install libsecret-tools"));
    out.status
        .success()
        .then(|| String::from_utf8(out.stdout).unwrap())
}

// One test per process, because libdbus reads the bus address from the
// environment once and keeps it.
#[test]
fn the_token_lives_in_the_secret_service() {
    let home = tempfile::tempdir().unwrap();
    let store = KeyringStore::default();
    let token = "eyJhbGciOiJIUzI1NiJ9.test-token.signature";

    // A running Secret Service holds it, and another client sees it there.
    let mut reaper = Reaper(Vec::new());
    let address = keyring_on_a_private_bus(home.path(), &mut reaper);
    std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &address);
    assert_eq!(store.get("account"), Ok(None));
    store.set("account", token).unwrap();
    assert_eq!(
        secret_tool_lookup(&address, "account").as_deref(),
        Some(token)
    );
    assert_eq!(store.get("account"), Ok(Some(token.to_string())));

    // A second secret is kept apart from the first.
    store
        .set(
            "brightspace-ical",
            "https://school.example/d2l/le/calendar/feed/user/feed.ics?token=abc",
        )
        .unwrap();
    assert_eq!(store.get("account"), Ok(Some(token.to_string())));

    // Signing out forgets it, and forgetting twice is fine.
    store.delete("account").unwrap();
    assert_eq!(secret_tool_lookup(&address, "account"), None);
    assert_eq!(store.get("account"), Ok(None));
    store.delete("account").unwrap();
    assert!(secret_tool_lookup(&address, "brightspace-ical").is_some());
}
