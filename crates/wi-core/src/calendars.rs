//! Calendars in the core (docs/SPEC.md 3.11, 8.7): the iCal addresses live in
//! the Keychain, and nowhere else; the core fetches each one itself, on open
//! if the last read was over 15 minutes ago and then hourly, and hands the
//! parsed feed to `wi-heat-store`, which turns it into tasks (Brightspace) or
//! read-only events (other calendars). An address is read from the Keychain
//! for the request and is never logged, kept in the library, exported or put
//! in an error.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use jiff::Timestamp;
use serde_json::{json, Value};
use wi_heat::brightspace::{classes_types, feed_items, sync_line, Counts, SYNC_EVERY, SYNC_ON_OPEN_AFTER};
use wi_heat::ical;
use wi_heat_store::{feed, Clock};

use crate::args::Args;
use crate::bus::lock;
use crate::heat_cmd::{announce, calendars_changed, core_error, open_heat, wrote_outside};
use crate::{CoreError, Inner};

/// One calendar is read at a time.
static ONE_SYNC: Mutex<()> = Mutex::new(());

/// The address as it will be fetched, or why it can't be. `webcal://` is
/// `https://`. The address itself is never in the sentence.
fn clean_address(url: &str) -> Result<String, CoreError> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    let address = if let Some(rest) = lower.strip_prefix("webcals://").map(|_| &url["webcals://".len()..]) {
        format!("https://{rest}")
    } else if let Some(rest) = lower.strip_prefix("webcal://").map(|_| &url["webcal://".len()..]) {
        format!("https://{rest}")
    } else {
        url.to_string()
    };
    let scheme_ok = ["https://", "http://"].iter().any(|s| address.to_ascii_lowercase().starts_with(s));
    if !scheme_ok || address.contains(char::is_whitespace) || address.len() < 12 {
        return Err(CoreError::new(
            "bad_address",
            "That doesn't look like a calendar address. Paste the iCal link, which starts with https:// or webcal://.",
        ));
    }
    Ok(address)
}

fn keychain_error(why: String) -> CoreError {
    CoreError::new("keychain", format!("The keychain refused the calendar's address: {why}"))
}

/// Keeps a new calendar: the address in the Keychain first, then the record.
fn make(i: &Inner, name: &str, kind: &str, url: &str) -> Result<Value, CoreError> {
    let address = clean_address(url)?;
    let record = feed::new_calendar(&i.store(), name, kind).map_err(core_error)?;
    let item = record["keychainRef"].as_str().unwrap_or_default().to_string();
    i.secrets.set(&item, &address).map_err(keychain_error)?;
    if let Err(e) = feed::save_calendar(&mut i.store(), &record) {
        let _ = i.secrets.delete(&item);
        return Err(core_error(e));
    }
    Ok(record)
}

/// `heat.calendars.add {name, kind, url}`
pub(crate) fn add(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let record = make(i, a.str("name")?, a.str("kind")?, a.str("url")?)?;
    calendars_changed(i);
    i.poke();
    Ok(json!({ "calendar": record }))
}

/// `heat.calendars.remove {id}`
pub(crate) fn remove(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let record = feed::remove_calendar(&mut i.store(), a.str("id")?).map_err(core_error)?;
    if let Some(item) = record["keychainRef"].as_str() {
        let _ = i.secrets.delete(item);
    }
    calendars_changed(i);
    Ok(json!({}))
}

/// `heat.school.set`: the School sheet. Its Brightspace address, if it comes
/// with one, goes to the Keychain like any other.
pub(crate) fn set_school(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    feed::set_school(&mut i.store(), &a.object()).map_err(core_error)?;
    if let Some(url) = a.opt_str("icalUrl").map(str::trim).filter(|u| !u.is_empty()) {
        let existing = feed::calendars(&i.store()).map_err(core_error)?.into_iter().find(|c| c["kind"] == "brightspace");
        match existing {
            // A new link for the calendar already here: the same calendar, read again.
            Some(mut cal) => {
                let address = clean_address(url)?;
                let item = cal["keychainRef"].as_str().unwrap_or_default().to_string();
                i.secrets.set(&item, &address).map_err(keychain_error)?;
                cal["lastSyncedAt"] = Value::Null;
                feed::save_calendar(&mut i.store(), &cal).map_err(core_error)?;
            }
            None => {
                make(i, "Brightspace", "brightspace", url)?;
            }
        }
        calendars_changed(i);
        i.poke();
    }
    wrote_outside(i, &["heatSetting"]);
    Ok(json!({}))
}

/// What one calendar's sync brought, or the sentence for why it didn't.
fn sync_one(i: &Inner, cal: &Value, clock: &Clock) -> Result<Counts, String> {
    let id = cal["id"].as_str().unwrap_or_default();
    let name = cal["name"].as_str().unwrap_or("calendar");
    let failed = || format!("Couldn't read the {name} calendar. Heat will try again in an hour.");
    let item = cal["keychainRef"].as_str().unwrap_or_default();
    let address = match i.secrets.get(item) {
        Ok(Some(a)) => a,
        _ => return Err(format!("The {name} calendar has no address in the Keychain. Add it again in Settings → Heat.")),
    };
    let bytes = i.net.fetch_feed(&address).map_err(|_| failed())?;
    let events = ical::parse(&bytes, &clock.zone).map_err(|_| failed())?;
    let mut counts = Counts::default();
    let mut docs = Vec::new();
    {
        let mut store = i.store();
        if cal["kind"] == "brightspace" {
            let school = feed::school(&store, &clock.zone).map_err(|e| e.to_string())?;
            let items = feed_items(&events, &school, &classes_types());
            let changes = feed::apply_brightspace(&mut store, clock, id, &items).map_err(|e| e.to_string())?;
            counts = Counts { new_tasks: changes.new_tasks, date_changes: changes.date_changes, new_grades: 0 };
            if let Some(txn) = &changes.txn {
                i.note_own(txn);
                docs = store.entry_docs(txn).ok().flatten().map(|e| e.docs).unwrap_or_default();
            }
        } else {
            feed::replace_events(&mut store, clock, id, &events).map_err(|e| e.to_string())?;
        }
        feed::mark_synced(&mut store, id, clock.now_ms).map_err(|e| e.to_string())?;
    }
    announce(i, &docs, &["calendar", "calendarEvent"]).map_err(|e| e.message)?;
    Ok(counts)
}

/// Reads the calendars (one, or all) now, and says how it went in the line
/// 3.11 gives: "Synced 3:41 PM: 2 new tasks, 1 date change", or why not.
pub(crate) fn sync_calendars(i: &Inner, only: Option<&str>) -> Result<String, CoreError> {
    open_heat(i)?;
    let _one = lock(&ONE_SYNC);
    let mut cals = feed::calendars(&i.store()).map_err(core_error)?;
    if let Some(id) = only {
        cals.retain(|c| c["id"] == id);
        if cals.is_empty() {
            return Err(CoreError::new("refused", "That calendar isn't in Heat any more."));
        }
    }
    if cals.is_empty() {
        return Ok("No calendars yet. Add one in Settings → Heat.".to_string());
    }
    let clock = i.clock();
    let (mut counts, mut read, mut failures) = (Counts::default(), false, Vec::new());
    for cal in &cals {
        match sync_one(i, cal, &clock) {
            Ok(c) => {
                read = true;
                counts.new_tasks += c.new_tasks;
                counts.date_changes += c.date_changes;
            }
            Err(sentence) => failures.push(sentence),
        }
    }
    let mut line = String::new();
    if read {
        let at = Timestamp::from_millisecond(clock.now_ms as i64).unwrap_or(Timestamp::UNIX_EPOCH);
        line = sync_line(at, &clock.zone, counts);
    }
    for f in failures {
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&f);
    }
    i.bus.status("calendar", &line);
    Ok(line)
}

/// `heat.calendars.sync {id?}`
pub(crate) fn sync(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    Ok(json!({ "line": sync_calendars(i, a.opt_str("id"))? }))
}

/// How long a calendar added while the app is open is left for the person's
/// own Sync (the sheet reads it as it is added) before it is read by itself.
const ADDED_GRACE: f64 = 10_000.0;

/// Reads each calendar on open if the last read was over 15 minutes ago, and
/// then once an hour (3.11). A read that failed is tried again an hour on.
pub(crate) fn start(inner: &Arc<Inner>) -> JoinHandle<()> {
    let i = inner.clone();
    // The calendars there are as the app opens: the ones "on open" is about.
    let at_open: BTreeSet<String> = feed::calendars(&i.store())
        .unwrap_or_default()
        .iter()
        .filter_map(|c| c["id"].as_str().map(String::from))
        .collect();
    std::thread::Builder::new()
        .name("heat calendars".into())
        .spawn(move || {
            let mut tried: BTreeMap<String, f64> = BTreeMap::new();
            let mut first_seen: BTreeMap<String, f64> = BTreeMap::new();
            let mut opening = true;
            let (open_after, every) = (SYNC_ON_OPEN_AFTER.as_secs_f64() * 1000.0, SYNC_EVERY.as_secs_f64() * 1000.0);
            while !i.closing() {
                let now = i.clock().now_ms;
                let cals = feed::calendars(&i.store()).unwrap_or_default();
                for cal in cals {
                    let id = cal["id"].as_str().unwrap_or_default().to_string();
                    let last = cal["lastSyncedAt"].as_f64();
                    let due = if opening && at_open.contains(&id) && !tried.contains_key(&id) {
                        last.map_or(true, |t| now - t > open_after)
                    } else if last.is_none() && !tried.contains_key(&id) {
                        // Added since: given a moment for the Sync that comes with adding it.
                        now - *first_seen.entry(id.clone()).or_insert(now) >= ADDED_GRACE
                    } else {
                        let newest = [last, tried.get(&id).copied()].into_iter().flatten().fold(f64::MIN, f64::max);
                        newest == f64::MIN || now - newest >= every
                    };
                    if due && !i.closing() {
                        tried.insert(id.clone(), now);
                        let _ = sync_calendars(&i, Some(&id));
                    }
                }
                opening = false;
                i.nap(Duration::from_secs(60));
            }
        })
        .expect("a thread for Heat's calendars")
}
