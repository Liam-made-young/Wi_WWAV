//! S1.11, Linux part: with no Secret Service running, the store says so in
//! one plain sentence, quickly, and keeps the secret nowhere else.
//!
//! Fails if: setting or reading a secret hangs, succeeds (so the token went
//! somewhere other than the OS's store), or answers anything but that no
//! keyring is running.

use std::time::{Duration, Instant};

use wi_wwav_app::{KeyringStore, NO_STORE};

// Alone in its process: libdbus reads the bus address once.
#[test]
fn with_no_keyring_running_the_store_says_so() {
    let nowhere = tempfile::tempdir().unwrap();
    std::env::set_var(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={}", nowhere.path().join("bus").display()),
    );
    let store = KeyringStore::default();
    let start = Instant::now();
    let refusals = [
        store
            .set("account", "eyJhbGciOiJIUzI1NiJ9.test-token.signature")
            .unwrap_err(),
        store.get("account").unwrap_err(),
        store.delete("account").unwrap_err(),
    ];
    for refused in refusals {
        assert_eq!(
            (refused.code.as_str(), refused.message.as_str()),
            ("keyring_unavailable", NO_STORE)
        );
    }
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "the store took {:?} to give up",
        start.elapsed()
    );
}
