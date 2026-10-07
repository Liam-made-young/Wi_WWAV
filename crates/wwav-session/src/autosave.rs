//! When to save: "`session.json` is rewritten 2 s after the last change and
//! at every transport stop" (`docs/SPEC.md` 6.5). The app owns the timer
//! and the transport; this holds the rule, on the package's clock.

use crate::{Package, Result};

/// Milliseconds since the Unix epoch. The package reads time only through
/// this, so tests can move it by hand.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
}

/// The wall clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        wwav_ids::now_ms()
    }
}

/// Quiet after the last change before an autosave.
pub const AUTOSAVE_AFTER_MS: u64 = 2_000;

#[derive(Debug, Default)]
pub struct Autosaver {
    stopped: bool,
}

impl Autosaver {
    pub fn new() -> Autosaver {
        Autosaver::default()
    }

    /// The transport stopped: unsaved changes are written at the next tick.
    pub fn transport_stopped(&mut self) {
        self.stopped = true;
    }

    /// How long until a save is due: None with nothing to save, Some(0) when
    /// it is due now. The app sets its timer from this.
    pub fn due_in_ms(&self, pkg: &Package) -> Option<u64> {
        if !pkg.is_dirty() {
            return None;
        }
        if self.stopped {
            return Some(0);
        }
        let due = pkg.last_change_ms() + AUTOSAVE_AFTER_MS;
        Some(due.saturating_sub(pkg.now_ms()))
    }

    /// Saves if a save is due; says whether it did.
    pub fn tick(&mut self, pkg: &mut Package) -> Result<bool> {
        let due = self.due_in_ms(pkg) == Some(0);
        if due {
            pkg.save()?;
        }
        self.stopped = false;
        Ok(due)
    }
}
