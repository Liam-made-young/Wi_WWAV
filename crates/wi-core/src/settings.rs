//! The settings object (docs/SPEC.md 2.13). Settings apply at once and are
//! not undo steps: the preferences window has no ⌘Z. Secrets (the account,
//! the Brightspace link) are never here; they live in the keychain (9.8).

use serde_json::{json, Map, Value};

use crate::args::Args;
use crate::{CoreError, Inner};

const KEY: &str = "settings";

/// Every setting and where it starts. A patch may only change these.
fn defaults() -> Value {
    json!({
        "appearance": "system",
        "textSize": 13,
        "reduceMotion": false,
        "library": {"watchedFolders": [], "leaveInPlace": false},
        "audio": {"device": null, "buffer": 128, "pluginFolders": []},
        "video": {"hardwareEncode": true, "proxyMedia": false},
        "claude": {"scoring": "unasked", "mail": "unasked", "feedback": "unasked", "clerk": "unasked"},
        "heat": {"timeZone": null, "school": null},
        "privacy": {"location": false},
    })
}

/// `patch` laid over `base`, key by key. A key `base` doesn't have is
/// refused, so a typo can't hide a setting.
fn merge(base: &mut Value, patch: &Value, path: &str) -> Result<(), CoreError> {
    let (Some(base), Some(patch)) = (base.as_object_mut(), patch.as_object()) else {
        return Err(CoreError::new("bad_args", "Settings change by an object."));
    };
    for (k, v) in patch {
        let here = if path.is_empty() {
            k.clone()
        } else {
            format!("{path}.{k}")
        };
        match base.get_mut(k) {
            None => {
                return Err(CoreError::new(
                    "no_such_setting",
                    format!("There is no setting called '{here}'."),
                ))
            }
            Some(b) if b.is_object() => merge(b, v, &here)?,
            Some(b) => *b = v.clone(),
        }
    }
    Ok(())
}

fn check(s: &Value) -> Result<(), CoreError> {
    let bad = |what: &str| Err(CoreError::new("bad_setting", what.to_string()));
    if !matches!(s["appearance"].as_str(), Some("light" | "dark" | "system")) {
        return bad("Appearance is light, dark or system.");
    }
    if !s["textSize"].as_f64().is_some_and(|n| (13.0..=20.0).contains(&n)) {
        return bad("Text size is from 13 to 20 pt.");
    }
    let buffer = s["audio"]["buffer"].as_u64().unwrap_or(0);
    if !(64..=1024).contains(&buffer) || !buffer.is_power_of_two() {
        return bad("The buffer is 64, 128, 256, 512 or 1024 samples.");
    }
    let claude = s["claude"].as_object().into_iter().flat_map(|m| m.values());
    if claude.clone().any(|v| !matches!(v.as_str(), Some("unasked" | "on" | "off"))) {
        return bad("Each Claude feature is on, off, or not asked yet.");
    }
    for flag in ["reduceMotion"] {
        if !s[flag].is_boolean() {
            return bad("Reduce Motion is on or off.");
        }
    }
    Ok(())
}

pub(crate) fn read(i: &Inner) -> Result<Value, CoreError> {
    let mut s = defaults();
    if let Some(stored) = i.kv.get(KEY)? {
        // Stored settings from an older or newer app: keep what still fits.
        if let Some(stored) = stored.as_object() {
            for (k, v) in stored {
                let one: Map<String, Value> = [(k.clone(), v.clone())].into_iter().collect();
                let _ = merge(&mut s, &Value::Object(one), "");
            }
        }
    }
    Ok(s)
}

pub(crate) fn get(i: &Inner) -> Result<Value, CoreError> {
    read(i)
}

pub(crate) fn set(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let patch = a
        .get("patch")
        .ok_or_else(|| CoreError::new("bad_args", "app.settings.set needs patch, an object."))?;
    let mut s = read(i)?;
    merge(&mut s, patch, "")?;
    check(&s)?;
    i.kv.set(KEY, &s)?;
    i.bus.emit("settings", s.clone());
    Ok(s)
}
