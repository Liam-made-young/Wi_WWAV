//! Commitments in the core (docs/COMMITMENTS.md): the commands, and what
//! needs a clock, a network or Claude.
//!
//! - A pasted schedule or a photo of one is left as a draft that is
//!   `reading`, and the worker asks Claude once: text with no tools at all, a
//!   photo with the one right to read that one file. Either way the answer is
//!   one JSON object, checked field by field, and the draft is then `ready`
//!   to preview or `failed` with one sentence. Nothing is a commitment until
//!   the draft is accepted.
//! - An `.ics` file is read here, in Rust. A subscribed address is kept in
//!   the Keychain and read again every hour.
//! - While the app is open the worker says when it is time to leave for
//!   something, once, and looks at new mail for a class that is canceled or
//!   moved. What it finds is offered, never applied.

use std::path::Path;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use wi_heat::commitments::{self as rules, Mode};
use wi_heat::ical;
use wi_heat_store::{commit, notes};
use wi_store::Actor;

use crate::args::Args;
use crate::claude_cli::{self, Ask};
use crate::heat_cmd::{core_error, open_heat, write, wrote_outside};
use crate::{CoreError, Inner};

/// The `heat` event's kinds for what this module writes outside the journal.
const DRAFT: &str = "commitmentDraft";
const PENDING: &str = "pendingException";
const NOTICE: &str = "notice";

/// A typed schedule is short: the smallest model reads it.
const TEXT_MODEL: &str = "haiku";
const TEXT_TIMEOUT: Duration = Duration::from_secs(120);
/// A photographed table takes the larger one.
const IMAGE_MODEL: &str = "sonnet";
const IMAGE_TIMEOUT: Duration = Duration::from_secs(240);

/// How often the worker looks: for a leave time, and for new mail.
const LOOK_EVERY: Duration = Duration::from_secs(20);
/// The least time between two looks, however often the worker is woken.
const LOOK_SOONEST: Duration = Duration::from_secs(2);
/// How often a subscribed calendar is read again.
const FEED_EVERY_MS: f64 = 3_600_000.0;

/// The most a pasted schedule may hold, and the largest photo of one.
const TEXT_MAX: usize = 40_000;
const IMAGE_MAX_BYTES: u64 = 30 * 1024 * 1024;

pub(crate) const IMAGE_TYPES: [&str; 9] = [
    "jpg", "jpeg", "png", "heic", "heif", "gif", "webp", "tif", "tiff",
];

fn object<'a>(a: &'a Args, key: &str) -> Result<&'a Map<String, Value>, CoreError> {
    a.get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| CoreError::new("bad_args", format!("This command needs {key}, an object.")))
}

fn mode_of(a: &Args) -> Result<Mode, CoreError> {
    match a.opt_str("mode") {
        None => Ok(Mode::Schedule),
        Some(m) => Mode::parse(m).ok_or_else(|| {
            CoreError::new(
                "bad_args",
                "A schedule is read as schedule, week or breaks.",
            )
        }),
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A calendar's bytes as a schedule, or the sentence for why not.
fn schedule_from_ics(i: &Inner, bytes: &[u8], mode: Mode) -> Result<rules::Schedule, CoreError> {
    let zone = i.clock().zone;
    let events = ical::parse(bytes, &zone).map_err(|_| {
        CoreError::new(
            "refused",
            "That isn't a calendar file. Export it as .ics and try again.",
        )
    })?;
    Ok(rules::from_ical(&events, mode, &zone))
}

/// A calendar read as one week's shifts: what falls in that week, each on
/// its own day, as work. What repeats, or is on another week's day, is
/// counted as unread: a week's import never reaches past its week.
fn for_week(
    mut schedule: rules::Schedule,
    mode: Mode,
    week_of: Option<&str>,
    clock: &wi_heat_store::Clock,
) -> rules::Schedule {
    if mode != Mode::Week {
        return schedule;
    }
    let monday = rules::monday_of(week_of.unwrap_or(&clock.today()));
    let sunday = wi_heat::model::zone::add_days(&monday, 6.0);
    let before = schedule.items.len();
    schedule.items.retain(|item| {
        item.rrule.is_none()
            && item
                .date
                .as_deref()
                .is_some_and(|d| d >= monday.as_str() && d <= sunday.as_str())
    });
    schedule.unread += before - schedule.items.len();
    for item in &mut schedule.items {
        if item.kind == rules::Kind::Other {
            item.kind = rules::Kind::Work;
        }
    }
    schedule
}

/// Reads a subscribed calendar now and writes what changed, as one entry.
fn feed_sync(i: &Inner, feed: &Value) -> Result<Value, CoreError> {
    let id = feed["id"].as_str().unwrap_or_default().to_string();
    let item = feed["keychainRef"].as_str().unwrap_or_default();
    let address = i
        .secrets
        .get(item)
        .map_err(|why| {
            CoreError::new(
                "keychain",
                format!("The keychain refused the calendar's address: {why}"),
            )
        })?
        .ok_or_else(|| {
            CoreError::new(
                "refused",
                format!(
                    "{}'s address is gone. Subscribe to it again.",
                    feed["name"].as_str().unwrap_or("That calendar")
                ),
            )
        })?;
    let bytes = i.net.fetch_feed(&address).map_err(|_| {
        CoreError::new(
            "offline",
            format!(
                "Couldn't read {}. Check the connection and the address.",
                feed["name"].as_str().unwrap_or("that calendar")
            ),
        )
    })?;
    let schedule = schedule_from_ics(i, &bytes, Mode::Schedule)?;
    write(i, |s, c| commit::feed_apply(s, c, &id, &schedule))
}

pub(crate) fn invoke(i: &Inner, cmd: &str, a: &Args) -> Result<Value, CoreError> {
    match cmd {
        "heat.commitment.create" => {
            open_heat(i)?;
            let args = a.object();
            write(i, |s, c| commit::create(s, c, &args, Actor::You))
        }
        "heat.commitment.update" => {
            let (id, set) = (a.str("id")?, object(a, "set")?);
            write(i, |s, c| commit::update(s, c, id, set, Actor::You))
        }
        "heat.commitment.addException" => {
            let id = a.str("id")?;
            let mut ex = a.object();
            ex.remove("id");
            ex.entry("source").or_insert(json!("you"));
            let ex = Value::Object(ex);
            write(i, |s, c| commit::add_exception(s, c, id, &ex, Actor::You))
        }
        "heat.commitment.removeException" => {
            let (id, date) = (a.str("id")?, a.str("date")?);
            write(i, |s, _| commit::remove_exception(s, id, date))
        }

        // `{text}` is read by Claude on the worker; `{json}` is a schedule
        // the caller read itself, and is ready at once.
        "heat.commitment.importText" => {
            open_heat(i)?;
            let mode = mode_of(a)?;
            let week = a.opt_str("weekOf");
            let clock = i.clock();
            let draft = match (a.get("json"), a.opt_str("text")) {
                (Some(answer), _) => {
                    let monday = rules::monday_of(week.unwrap_or(&clock.today()));
                    let schedule = rules::parse_schedule(answer, mode, &monday);
                    commit::draft_from(
                        &mut i.store(),
                        &clock,
                        mode,
                        "paste",
                        week,
                        "schedule",
                        &schedule,
                    )
                    .map_err(core_error)?
                }
                (None, Some(text)) => {
                    let text = text.trim();
                    if text.is_empty() {
                        return Err(CoreError::new("refused", "Paste the schedule first."));
                    }
                    if text.len() > TEXT_MAX {
                        return Err(CoreError::new(
                            "refused",
                            "That is too long to be a schedule. Paste the schedule by itself.",
                        ));
                    }
                    // Said now, not after: there is no Claude to ask.
                    claude_cli::binary(i)?;
                    commit::draft_begin(
                        &mut i.store(),
                        &clock,
                        mode,
                        week,
                        Some(text),
                        None,
                        "pasted text",
                    )
                    .map_err(core_error)?
                }
                (None, None) => {
                    return Err(CoreError::new(
                        "bad_args",
                        "heat.commitment.importText needs text, or json.",
                    ))
                }
            };
            answer_draft(i, &draft)
        }
        "heat.commitment.importImage" => {
            open_heat(i)?;
            let mode = mode_of(a)?;
            let file = Path::new(a.str("path")?);
            if !IMAGE_TYPES.contains(&extension(file).as_str()) || !file.is_file() {
                return Err(CoreError::new(
                    "refused",
                    "Drop a photo or a screenshot of the schedule.",
                ));
            }
            if std::fs::metadata(file).map(|m| m.len()).unwrap_or(0) > IMAGE_MAX_BYTES {
                return Err(CoreError::new(
                    "refused",
                    "That image is too large. Take a screenshot of the schedule by itself.",
                ));
            }
            claude_cli::binary(i)?;
            let clock = i.clock();
            let draft = commit::draft_begin(
                &mut i.store(),
                &clock,
                mode,
                a.opt_str("weekOf"),
                None,
                file.to_str(),
                &file_name(file),
            )
            .map_err(core_error)?;
            answer_draft(i, &draft)
        }
        "heat.commitment.importIcs" => {
            open_heat(i)?;
            let mode = mode_of(a)?;
            let subscribe = a.opt_bool("subscribe")?.unwrap_or(false);
            if let Some(url) = a.opt_str("url") {
                let address = crate::calendars::clean_address(url)?;
                if subscribe {
                    let feed = commit::feed_new(&mut i.store(), a.opt_str("name").unwrap_or(""))
                        .map_err(core_error)?;
                    let item = feed["keychainRef"].as_str().unwrap_or_default().to_string();
                    let kept = i.secrets.set(&item, &address).map_err(|why| {
                        CoreError::new(
                            "keychain",
                            format!("The keychain refused the calendar's address: {why}"),
                        )
                    });
                    let done = kept.and_then(|_| feed_sync(i, &feed));
                    return match done {
                        Ok(mut out) => {
                            out["feed"] = json!({"id": feed["id"], "name": feed["name"]});
                            wrote_outside(i, &["commitmentFeed"]);
                            Ok(out)
                        }
                        // A subscription that couldn't be read the first time isn't kept.
                        Err(e) => {
                            let _ = i.secrets.delete(&item);
                            let _ = commit::feed_remove(
                                &mut i.store(),
                                feed["id"].as_str().unwrap_or_default(),
                            );
                            Err(e)
                        }
                    };
                }
                let bytes = i.net.fetch_feed(&address).map_err(|_| {
                    CoreError::new(
                        "offline",
                        "Couldn't read that calendar. Check the connection and the address.",
                    )
                })?;
                let clock = i.clock();
                let week = a.opt_str("weekOf");
                let schedule = for_week(schedule_from_ics(i, &bytes, mode)?, mode, week, &clock);
                let draft = commit::draft_from(
                    &mut i.store(),
                    &clock,
                    mode,
                    "ics",
                    week,
                    "calendar",
                    &schedule,
                )
                .map_err(core_error)?;
                return answer_draft(i, &draft);
            }
            let (bytes, name) = match (a.opt_str("path"), a.opt_str("text")) {
                (Some(path), _) => {
                    let file = Path::new(path);
                    let bytes = std::fs::read(file).map_err(|_| {
                        CoreError::new("refused", "Learn couldn't open that calendar file.")
                    })?;
                    (bytes, file_name(file))
                }
                (None, Some(text)) => (text.as_bytes().to_vec(), "calendar".to_string()),
                (None, None) => {
                    return Err(CoreError::new(
                        "bad_args",
                        "heat.commitment.importIcs needs path, url or text.",
                    ))
                }
            };
            let clock = i.clock();
            let week = a.opt_str("weekOf");
            let schedule = for_week(schedule_from_ics(i, &bytes, mode)?, mode, week, &clock);
            let draft =
                commit::draft_from(&mut i.store(), &clock, mode, "ics", week, &name, &schedule)
                    .map_err(core_error)?;
            answer_draft(i, &draft)
        }
        "heat.commitment.draft.accept" => {
            let id = a.str("draftId")?;
            let skip: Vec<usize> = a
                .get("skip")
                .and_then(Value::as_array)
                .map(|l| {
                    l.iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as usize)
                        .collect()
                })
                .unwrap_or_default();
            write(i, |s, c| commit::draft_accept(s, c, id, &skip, Actor::You))
        }
        "heat.commitment.draft.discard" => {
            let id = a.str("draftId")?;
            write(i, |s, _| commit::draft_discard(s, id))
        }

        "heat.commitment.exception.confirm" => {
            let id = a.str("id")?;
            let out = write(i, |s, c| commit::pending_confirm(s, c, id))?;
            let _ = notes::notice_dismiss(&mut i.store(), &format!("exception:{id}"));
            wrote_outside(i, &[NOTICE]);
            Ok(out)
        }
        "heat.commitment.exception.dismiss" => {
            let id = a.str("id")?;
            let out = write(i, |s, _| commit::pending_dismiss(s, id))?;
            let _ = notes::notice_dismiss(&mut i.store(), &format!("exception:{id}"));
            wrote_outside(i, &[NOTICE]);
            Ok(out)
        }
        // `heat.commitment.mail.check {}`: looks at the mail now, rather than
        // at the worker's next pass. Answers what waits for a tap.
        "heat.commitment.mail.check" => {
            open_heat(i)?;
            look_at_mail(i);
            Ok(json!({"pending": commit::pending(&i.store()).map_err(core_error)?}))
        }

        "heat.commitment.feed.sync" => {
            let only = a.opt_str("id");
            let feeds = commit::feeds(&i.store()).map_err(core_error)?;
            let mut changed = 0;
            for feed in feeds
                .iter()
                .filter(|f| only.map_or(true, |id| f["id"] == id))
            {
                changed += feed_sync(i, feed)?["changed"].as_u64().unwrap_or(0);
            }
            wrote_outside(i, &["commitmentFeed"]);
            Ok(json!({ "changed": changed }))
        }
        "heat.commitment.feed.remove" => {
            let record = commit::feed_remove(&mut i.store(), a.str("id")?).map_err(core_error)?;
            if let Some(item) = record["keychainRef"].as_str() {
                let _ = i.secrets.delete(item);
            }
            wrote_outside(i, &["commitmentFeed"]);
            Ok(json!({}))
        }

        "heat.planner.freeTime" => {
            open_heat(i)?;
            let clock = i.clock();
            let today = clock.today();
            let (from, to) = match (a.opt_str("from"), a.opt_str("to"), a.opt_str("date")) {
                (Some(f), Some(t), _) => (f, t),
                (_, _, Some(d)) => (d, d),
                _ => (today.as_str(), today.as_str()),
            };
            commit::free_time(&i.store(), &clock, from, to).map_err(core_error)
        }
        "heat.sleep.set" => {
            let (from, to) = (a.f64("from")?, a.f64("to")?);
            write(i, |s, _| commit::set_sleep(s, from, to))
        }
        _ => Err(CoreError::new(
            "unknown_command",
            format!("There is no command called '{cmd}'."),
        )),
    }
}

/// A draft just made, as the views show it; the worker is woken to read it.
fn answer_draft(i: &Inner, draft: &Value) -> Result<Value, CoreError> {
    wrote_outside(i, &[DRAFT]);
    i.poke();
    let id = draft["id"].as_str().unwrap_or_default();
    let shown = commit::draft(&i.store(), &i.clock(), id).map_err(core_error)?;
    Ok(json!({ "draft": shown.unwrap_or_else(|| draft.clone()) }))
}

/// The courses Learn holds, by their codes, for Claude to write them the same way.
fn course_codes(i: &Inner) -> Vec<String> {
    wi_heat_store::all(&i.store(), wi_heat_store::kind::COURSE)
        .unwrap_or_default()
        .iter()
        .filter_map(|c| c["code"].as_str().map(String::from))
        .collect()
}

/// Reads one draft: asks Claude, checks the answer. Whatever happens, the
/// draft says so: ready with its preview, or failed with one sentence.
fn read_draft(i: &Inner, draft: &Value) {
    let id = draft["id"].as_str().unwrap_or_default().to_string();
    let fail = |sentence: &str| {
        let _ = commit::draft_fail(&mut i.store(), &id, sentence);
        wrote_outside(i, &[DRAFT]);
    };
    let mode = draft["mode"]
        .as_str()
        .and_then(Mode::parse)
        .unwrap_or(Mode::Schedule);
    let week = draft["weekOf"].as_str().unwrap_or_default().to_string();
    let clock = i.clock();
    let fixed = commit::Fixed::load(&i.store()).unwrap_or_default();
    let term = (fixed.term_start.as_deref(), fixed.term_end.as_deref());
    let courses = course_codes(i);
    let schema = rules::schedule_schema();
    let answer = match draft["path"].as_str() {
        None => {
            let text = draft["text"].as_str().unwrap_or_default();
            let prompt =
                rules::schedule_prompt(mode, text, false, &clock.today(), &week, &courses, term);
            claude_cli::run_json(
                i,
                &Ask {
                    prompt: &prompt,
                    allowed_tools: &[],
                    json_schema: Some(&schema),
                    model: Some(TEXT_MODEL),
                    timeout: TEXT_TIMEOUT,
                },
            )
        }
        Some(path) => {
            // The photo, alone in a folder made for this run: all Claude can read.
            let source = Path::new(path);
            let folder = match look_folder(i, &id) {
                Ok(f) => f,
                Err(e) => return fail(&e.message),
            };
            let name = format!("schedule.{}", extension(source));
            if std::fs::copy(source, folder.join(&name)).is_err() {
                let _ = std::fs::remove_dir_all(&folder);
                return fail("Learn couldn't open that image any more. Drop it again.");
            }
            let prompt =
                rules::schedule_prompt(mode, &name, true, &clock.today(), &week, &courses, term);
            let answer = claude_cli::run_json_looking(
                i,
                &Ask {
                    prompt: &prompt,
                    allowed_tools: &[],
                    json_schema: Some(&schema),
                    model: Some(IMAGE_MODEL),
                    timeout: IMAGE_TIMEOUT,
                },
                &folder,
            );
            let _ = std::fs::remove_dir_all(&folder);
            answer
        }
    };
    let ready = match answer {
        Ok(answer) => {
            let schedule = rules::parse_schedule(&answer, mode, &week);
            commit::draft_ready(&mut i.store(), &id, &schedule).map_err(|e| e.to_string())
        }
        Err(e) => Err(e.message),
    };
    match ready {
        Ok(_) => wrote_outside(i, &[DRAFT]),
        Err(sentence) => fail(&sentence),
    }
}

/// An empty folder for one run of Claude to stand in, inside the library's
/// own cache so nothing else is beside what it is given to read.
pub(crate) fn look_folder(i: &Inner, id: &str) -> Result<std::path::PathBuf, CoreError> {
    let folder = i.root.join("cache").join("look").join(id);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder)?;
    Ok(folder)
}

/// Asks the shell to show one line as an interrupt, when the Focus layout's
/// door is there (docs/FOCUS.md). Until it is, the quiet notice is all.
/// `act` is a notice's one act, `{label, cmd, args}`; the shell's own shape
/// for it is `{label, do: "command", cmd, args}`.
pub(crate) fn raise(i: &Inner, id: &str, line: &str, act: Option<&Value>) {
    let mut args = json!({"id": id, "source": "commitments", "line": line, "changesNext": true});
    if let Some(act) = act {
        args["action"] =
            json!({"label": act["label"], "do": "command", "cmd": act["cmd"], "args": act["args"]});
    }
    let _ = crate::heat_cmd::invoke(
        i,
        "heat.interrupt.raise",
        &Args::new("heat.interrupt.raise", &args),
    );
}

/// Looks at the mail Claude recorded for a class that is canceled or moved.
/// Each one found waits for a tap, and is said once.
pub(crate) fn look_at_mail(i: &Inner) {
    let clock = i.clock();
    let found = commit::pending_from_mail(&mut i.store(), &clock).unwrap_or_default();
    if found.is_empty() {
        return;
    }
    for p in &found {
        let id = p["id"].as_str().unwrap_or_default();
        let line = p["line"].as_str().unwrap_or_default();
        let act = json!({"label": p["act"], "cmd": "heat.commitment.exception.confirm", "args": {"id": id}});
        let key = format!("exception:{id}");
        let _ = notes::notice_add(
            &mut i.store(),
            &clock,
            &key,
            "exception",
            line,
            json!({"act": act, "pendingId": id}),
        );
        raise(i, &key, line, Some(&act));
    }
    wrote_outside(i, &[PENDING, NOTICE]);
}

/// Says "time to leave" for each commitment whose travel time has started,
/// once for that day.
fn look_at_the_time(i: &Inner) {
    let clock = i.clock();
    let today = clock.today();
    let fixed = match commit::Fixed::load(&i.store()) {
        Ok(f) => f,
        Err(_) => return,
    };
    let all = fixed.occurrences(&clock, &today, &today);
    let now = wi_heat::model::zone::minute_of_day(clock.now_ms, &clock.zone);
    let mut said = false;
    for o in rules::leaving(&all, &today, now) {
        let key = format!("leave:{}@{today}", o.commitment_id);
        if notes::notice_said(&i.store(), &key).unwrap_or(true) {
            continue;
        }
        let line = rules::leave_line(o);
        let mut store = i.store();
        let _ = notes::notice_remember(&mut store, &key, &today);
        let _ = notes::notice_add(&mut store, &clock, &key, "leave", &line, json!({}));
        drop(store);
        raise(i, &key, &line, None);
        i.bus.status("learn", &line);
        said = true;
    }
    if said {
        wrote_outside(i, &[NOTICE]);
    }
}

/// Reads each subscribed calendar that hasn't been read for an hour.
fn look_at_feeds(i: &Inner) {
    let now = i.clock().now_ms;
    let feeds = commit::feeds(&i.store()).unwrap_or_default();
    for feed in feeds {
        let last = feed["lastSyncedAt"].as_f64().unwrap_or(0.0);
        if now - last >= FEED_EVERY_MS && !i.closing() && feed_sync(i, &feed).is_ok() {
            wrote_outside(i, &["commitmentFeed"]);
        }
    }
}

pub(crate) fn start(inner: &Arc<Inner>, leave_notices: bool) -> JoinHandle<()> {
    let i = inner.clone();
    std::thread::Builder::new()
        .name("heat commitments".into())
        .spawn(move || {
            let mut looked = Instant::now() - LOOK_SOONEST;
            while !i.closing() {
                // A draft left half read, or just dropped: read it now.
                // Read first, then let go of the library: reading a draft takes it again.
                let waiting = commit::drafts_to_read(&i.store()).unwrap_or_default();
                for draft in waiting {
                    if i.closing() {
                        return;
                    }
                    read_draft(&i, &draft);
                }
                // A look every twenty seconds, and soon after anything is
                // written (new mail, a commitment), but never in a tight loop.
                let due = looked.elapsed() >= LOOK_SOONEST;
                if due {
                    looked = Instant::now();
                    look_at_mail(&i);
                    if leave_notices {
                        look_at_the_time(&i);
                        look_at_feeds(&i);
                    }
                }
                i.nap(if due { LOOK_EVERY } else { LOOK_SOONEST });
            }
        })
        .expect("a thread for Learn's commitments")
}
