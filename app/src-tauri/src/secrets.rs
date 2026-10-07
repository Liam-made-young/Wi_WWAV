//! The secret store (docs/SPEC.md 9.8): the account and Google tokens and
//! the Brightspace calendar link, which carries a private token, live in the
//! operating system's store and nowhere else. That is the Keychain on macOS,
//! the Credential Manager on Windows and the Secret Service (GNOME Keyring,
//! KWallet) on Linux. On macOS an item belongs to the app's signing
//! identity, so the engine, which runs other people's plugins, can't read it.

use crate::bridge::CoreError;

/// Every secret is filed under the app's identifier, by name.
pub const SERVICE: &str = "com.mi-wwav.wi-wwav";

#[cfg(target_os = "macos")]
const STORE: &str = "Keychain";
#[cfg(windows)]
const STORE: &str = "Credential Manager";
#[cfg(not(any(target_os = "macos", windows)))]
const STORE: &str = "keyring";

/// What the app says when there is no store to talk to. On Linux that means
/// no Secret Service is running, which a bare window manager can leave out.
#[cfg(not(any(target_os = "macos", windows)))]
pub const NO_STORE: &str = "No keyring is running, so Wi_WWAV can't keep you signed in. \
Start GNOME Keyring, KWallet or another Secret Service, then try again.";
#[cfg(any(target_os = "macos", windows))]
pub const NO_STORE: &str =
    "Wi_WWAV couldn't reach the system's password store, so it can't keep you signed in.";

#[derive(Clone, Debug)]
pub struct KeyringStore {
    service: String,
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new(SERVICE)
    }
}

impl KeyringStore {
    pub fn new(service: &str) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, name: &str) -> Result<keyring::Entry, CoreError> {
        keyring::Entry::new(&self.service, name).map_err(refused)
    }

    /// The secret filed as `name`, if there is one.
    pub fn get(&self, name: &str) -> Result<Option<String>, CoreError> {
        match self.entry(name)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(refused(e)),
        }
    }

    pub fn set(&self, name: &str, secret: &str) -> Result<(), CoreError> {
        self.entry(name)?.set_password(secret).map_err(refused)
    }

    /// Forgets `name`. Forgetting one that isn't there is not an error.
    pub fn delete(&self, name: &str) -> Result<(), CoreError> {
        match self.entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(refused(e)),
        }
    }
}

/// The store's errors as sentences. The platform's own words follow where
/// they help; no secret is ever in them.
fn refused(e: keyring::Error) -> CoreError {
    match e {
        keyring::Error::PlatformFailure(why) => {
            eprintln!("wi-wwav: the {STORE} didn't answer: {why}");
            CoreError::new("keyring_unavailable", NO_STORE)
        }
        keyring::Error::NoStorageAccess(_) => CoreError::new(
            "keyring_locked",
            format!("Your {STORE} is locked. Unlock it, then try again."),
        ),
        other => CoreError::new("keyring_failed", format!("The {STORE} refused: {other}.")),
    }
}
