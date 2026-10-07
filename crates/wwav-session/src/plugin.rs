//! Plugin state (`docs/SPEC.md` 6.5): up to 256 KB inline as base64, larger
//! states (a sampler's 40 MB of mappings) as ULID-named files in
//! `plugin-state/`, "so an autosave never rewrites them unchanged".

use std::cmp::Ordering;
use std::fs;
use std::path::{Component, Path};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

use crate::model::PluginState;
use crate::{fsx, sha256_hex, Error, Package, Result};

/// The largest state kept inline.
pub const INLINE_LIMIT: usize = 256 * 1024;

impl Package {
    /// Stores a plugin's state. Given the state it replaces, an unchanged
    /// state returns that reference and writes nothing. A replaced file is
    /// left in place, because undo can bring its reference back.
    pub fn store_plugin_state(
        &self,
        state: &[u8],
        previous: Option<&PluginState>,
    ) -> Result<PluginState> {
        let sha256 = sha256_hex(state);
        let bytes = state.len() as u64;
        if let Some(p) = previous.filter(|p| p.sha256 == sha256 && p.bytes == bytes) {
            return Ok(p.clone());
        }
        if state.len() <= INLINE_LIMIT {
            return Ok(PluginState {
                inline: Some(BASE64.encode(state)),
                bytes,
                sha256,
                ..PluginState::default()
            });
        }
        let file = format!("plugin-state/{}.bin", wwav_ids::ulid());
        fsx::write_atomic(&self.dir().join(&file), state)?;
        Ok(PluginState {
            file: Some(file),
            bytes,
            sha256,
            ..PluginState::default()
        })
    }

    /// A stored state's bytes, checked against its sha256.
    pub fn read_plugin_state(&self, s: &PluginState) -> Result<Vec<u8>> {
        let changed = || Error::StateChanged(s.file.clone().unwrap_or_else(|| "inline".into()));
        let bytes = match (&s.inline, &s.file) {
            (Some(b64), _) => BASE64.decode(b64).map_err(|_| changed())?,
            // Only a file inside plugin-state/: a session from someone else
            // must not name a file elsewhere on this Mac.
            (None, Some(file)) if in_plugin_state(file) => fs::read(self.dir().join(file))?,
            _ => return Err(changed()),
        };
        if sha256_hex(&bytes) != s.sha256 {
            return Err(changed());
        }
        Ok(bytes)
    }
}

fn in_plugin_state(file: &str) -> bool {
    let mut parts = Path::new(file).components();
    parts.next() == Some(Component::Normal("plugin-state".as_ref()))
        && matches!(parts.next(), Some(Component::Normal(_)))
        && parts.next().is_none()
}

/// "Saved with 'Tape Echo' 2.1.4. You have 2.0.0, so its settings may not
/// load." when `installed` is older than `saved`; None otherwise.
pub fn older_version_sentence(name: &str, saved: &str, installed: &str) -> Option<String> {
    (compare_versions(installed, saved) == Ordering::Less).then(|| {
        format!("Saved with '{name}' {saved}. You have {installed}, so its settings may not load.")
    })
}

/// Dotted versions part by part, numbers as numbers ("2.10" after "2.9"),
/// a missing part as 0 ("1.3" is "1.3.0").
fn compare_versions(a: &str, b: &str) -> Ordering {
    let (pa, pb): (Vec<&str>, Vec<&str>) = (a.split('.').collect(), b.split('.').collect());
    for i in 0..pa.len().max(pb.len()) {
        let (x, y) = (
            pa.get(i).copied().unwrap_or("0"),
            pb.get(i).copied().unwrap_or("0"),
        );
        let order = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(m), Ok(n)) => m.cmp(&n),
            _ => x.cmp(y),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_files_inside_plugin_state_are_read() {
        assert!(in_plugin_state(
            "plugin-state/01JC5Q8V3M2T7R9X4K6W0YHZNB.bin"
        ));
        assert!(!in_plugin_state("plugin-state/../session.json"));
        assert!(!in_plugin_state("../../etc/passwd"));
        assert!(!in_plugin_state("/etc/passwd"));
        assert!(!in_plugin_state("plugin-state/a/b.bin"));
        assert!(!in_plugin_state("plugin-state"));
    }
}
