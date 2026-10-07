//! Mail accounts (docs/SPEC.md 3.10): the addresses whose mail Claude reads
//! and records, and how each one reaches the mailbox Claude's Gmail
//! connector is signed in to. Learn reads no mail itself, so an account
//! here holds no password and no token: only the address, and where its
//! mail is forwarded if it is. From that the store works out the Gmail
//! search that finds one account's mail and no other's.

use serde_json::{json, Map, Value};
use wi_store::Store;

use crate::{all, kind, num, one, parse_instant, refused, set_setting, setting, Clock, Result};

/// The most messages kept for one thread, and the most text for one message.
pub const TEXT_MESSAGES: usize = 50;
pub const TEXT_CHARS: usize = 100_000;

/// The most accounts kept.
pub const ACCOUNT_LIMIT: usize = 8;

/// How pressing a thread is, most first.
pub const PRIORITIES: [&str; 4] = ["urgent", "high", "normal", "low"];

/// What a thread is about.
pub const CATEGORIES: [&str; 7] = [
    "school",
    "work",
    "money",
    "people",
    "updates",
    "promotions",
    "other",
];

/// An address as it is kept and compared: trimmed, in lower case.
pub fn address(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

fn is_address(s: &str) -> bool {
    let Some((name, host)) = s.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && s.len() <= 254
        && !s.contains(|c: char| c.is_whitespace() || c == '"' || c == '(' || c == ')')
}

/// The accounts, in the order they were given: `{address, name, via,
/// forwardTo?}`. `via` is `connector` for the mailbox Claude's Gmail
/// connector reads, or `forward` for one whose mail is forwarded into it.
pub fn accounts(store: &Store) -> Result<Vec<Value>> {
    Ok(setting(store, "mail.accounts")?
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default())
}

/// `heat.mail.accounts.set`: the whole list, checked, kept.
pub fn set_accounts(store: &mut Store, list: &[Value]) -> Result<Vec<Value>> {
    if list.len() > ACCOUNT_LIMIT {
        return refused(format!(
            "Learn keeps at most {ACCOUNT_LIMIT} mail accounts."
        ));
    }
    let mut kept: Vec<Value> = Vec::new();
    for item in list {
        let text = |k: &str| item.get(k).and_then(Value::as_str).unwrap_or("").trim();
        let at = address(text("address"));
        if !is_address(&at) {
            return refused("A mail account needs its address, such as name@school.edu.");
        }
        if kept.iter().any(|k| k["address"] == at.as_str()) {
            return refused(format!("{at} is in the list twice."));
        }
        let mut account = Map::new();
        account.insert("address".into(), json!(at));
        account.insert(
            "name".into(),
            json!(if text("name").is_empty() {
                at.as_str()
            } else {
                text("name")
            }),
        );
        match text("via") {
            "" | "connector" => {
                account.insert("via".into(), json!("connector"));
            }
            "forward" => {
                let to = address(text("forwardTo"));
                if !is_address(&to) {
                    return refused(format!(
                        "Say where {at} is forwarded to, such as you+school@gmail.com."
                    ));
                }
                if to == at {
                    return refused(format!("{at} can't be forwarded to itself."));
                }
                account.insert("via".into(), json!("forward"));
                account.insert("forwardTo".into(), json!(to));
            }
            _ => return refused("Mail arrives through Claude's connector, or is forwarded."),
        }
        kept.push(Value::Object(account));
    }
    let forwards: Vec<&str> = kept
        .iter()
        .filter_map(|a| a["forwardTo"].as_str())
        .collect();
    for (n, to) in forwards.iter().enumerate() {
        if forwards[..n].contains(to) {
            return refused(format!(
                "Two accounts are forwarded to {to}, so their mail can't be told apart. Give each its own address, such as you+school@gmail.com."
            ));
        }
    }
    set_setting(store, "mail.accounts", &Value::Array(kept.clone()))?;
    Ok(kept)
}

/// The Gmail search that finds this account's mail and no other's, in the
/// mailbox Claude's connector reads. A forwarded account's mail is what was
/// delivered to its forwarding address; the connector's own account is
/// everything that wasn't delivered to one of those. Empty when there is
/// nothing to tell apart.
pub fn gmail_query(account: &Value, all: &[Value]) -> String {
    match account["forwardTo"].as_str() {
        Some(to) => format!("deliveredto:{to}"),
        None => all
            .iter()
            .filter_map(|a| a["forwardTo"].as_str())
            .map(|to| format!("-deliveredto:{to}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// The account a thread is recorded under: the one named, which must be in
/// the list when there is a list; else the only one there is; else none.
pub fn account_for(named: Option<&str>, all: &[Value]) -> Result<Option<String>> {
    let known: Vec<&str> = all.iter().filter_map(|a| a["address"].as_str()).collect();
    match named.map(address) {
        Some(at) if known.is_empty() || known.contains(&at.as_str()) => Ok(Some(at)),
        Some(at) => refused(format!(
            "{at} isn't one of the mail accounts in Settings → Learn. They are: {}.",
            known.join(", ")
        )),
        None if known.len() == 1 => Ok(Some(known[0].to_string())),
        None => Ok(None),
    }
}

/// What a message of `save_mail_text` may hold.
const MESSAGE_FIELDS: [&str; 5] = ["message_id", "from", "to", "sent_at", "text"];

/// `save_mail_text`: a thread's messages as plain text, for Mail's reader.
/// The thread has to be recorded first. The whole thread is saved each time,
/// so reading it again replaces what was there. It is kept outside the
/// journal, on this Mac only (`kind::MAIL_TEXT` never syncs or exports).
pub fn save_text(
    store: &mut Store,
    clock: &Clock,
    thread_id: &str,
    messages: &[Value],
) -> Result<Value> {
    let thread_id = thread_id.trim();
    if !all(store, kind::MAIL)?
        .iter()
        .any(|t| t["gmailThreadId"].as_str() == Some(thread_id))
    {
        return refused("Record the thread with record_mail_thread first, then save its text.");
    }
    if messages.is_empty() {
        return refused("Give at least one message.");
    }
    if messages.len() > TEXT_MESSAGES {
        return refused(format!(
            "A thread keeps at most {TEXT_MESSAGES} messages. Save the newest {TEXT_MESSAGES}."
        ));
    }
    let mut kept: Vec<Value> = Vec::new();
    let mut characters = 0;
    for (n, m) in messages.iter().enumerate() {
        let which = n + 1;
        let Some(m) = m.as_object() else {
            return refused(format!("Message {which} must be an object."));
        };
        if let Some(k) = m.keys().find(|k| !MESSAGE_FIELDS.contains(&k.as_str())) {
            return refused(format!("A message takes no \"{k}\"."));
        }
        let field = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("");
        let from = field("from").trim();
        if from.is_empty() || from.chars().count() > 300 {
            return refused(format!(
                "Message {which} needs its sender, in 300 characters or fewer."
            ));
        }
        let text = field("text").replace("\r\n", "\n");
        let text = text.trim_matches(|c: char| c == '\n' || c == ' ');
        if text.is_empty() {
            return refused(format!("Message {which} has no text."));
        }
        let length = text.chars().count();
        if length > TEXT_CHARS {
            return refused(format!(
                "Message {which} is longer than {TEXT_CHARS} characters. Leave out quoted replies and footers."
            ));
        }
        let sent = parse_instant(field("sent_at"))?;
        let mut message = json!({"from": from, "sentAt": num(sent), "text": text});
        for (arg, key, max) in [("to", "to", 1000), ("message_id", "id", 200)] {
            let v = field(arg).trim();
            if v.chars().count() > max {
                return refused(format!(
                    "Message {which}'s \"{arg}\" is longer than {max} characters."
                ));
            }
            if !v.is_empty() {
                message[key] = json!(v);
            }
        }
        characters += length;
        kept.push(message);
    }
    kept.sort_by(|a, b| {
        a["sentAt"]
            .as_f64()
            .partial_cmp(&b["sentAt"].as_f64())
            .unwrap()
    });
    let count = kept.len();
    store.set_doc(
        kind::MAIL_TEXT,
        thread_id,
        &json!({"threadId": thread_id, "savedAt": num(clock.now_ms), "messages": kept}),
        "",
    )?;
    Ok(json!({"thread_id": thread_id, "messages": count, "characters": characters}))
}

/// A thread's saved text, oldest message first, or None before Claude has
/// saved it: `{threadId, savedAt, messages: [{id?, from, to?, sentAt, text}]}`.
pub fn text_of(store: &Store, thread_id: &str) -> Result<Option<Value>> {
    one(store, kind::MAIL_TEXT, thread_id)
}

/// The threads whose text is saved.
pub fn with_text(store: &Store) -> Result<Vec<String>> {
    Ok(all(store, kind::MAIL_TEXT)?
        .iter()
        .filter_map(|d| d["threadId"].as_str().map(String::from))
        .collect())
}

// ----- the mailbox: what is unread, what is archived, and the outbox -----

/// What the outbox takes. `send` is a new mail; `reply` answers a thread.
/// The rest change where a thread sits in Gmail.
pub const ACTIONS: [&str; 6] = [
    "send",
    "reply",
    "archive",
    "unarchive",
    "markRead",
    "markUnread",
];

/// The most text one mail of yours holds.
pub const BODY_CHARS: usize = 50_000;

fn recorded(store: &Store, thread_id: &str) -> Result<bool> {
    Ok(all(store, kind::MAIL)?
        .iter()
        .any(|t| t["gmailThreadId"].as_str() == Some(thread_id)))
}

/// Each thread's place: `{<threadId>: {unread, archived}}`. A thread with no
/// entry is read and in the inbox.
pub fn states(store: &Store) -> Result<Value> {
    let mut out = Map::new();
    for d in all(store, kind::MAIL_STATE)? {
        if let Some(id) = d["threadId"].as_str() {
            out.insert(
                id.to_string(),
                json!({"unread": d["unread"] == true, "archived": d["archived"] == true}),
            );
        }
    }
    Ok(Value::Object(out))
}

/// Sets a thread's place here, on this Mac. Gmail's own copy changes when
/// the outbox's action is done.
pub fn set_flags(
    store: &mut Store,
    thread_id: &str,
    unread: Option<bool>,
    archived: Option<bool>,
) -> Result<()> {
    let mut doc = one(store, kind::MAIL_STATE, thread_id)?
        .unwrap_or_else(|| json!({"threadId": thread_id, "unread": false, "archived": false}));
    if let Some(u) = unread {
        doc["unread"] = json!(u);
    }
    if let Some(a) = archived {
        doc["archived"] = json!(a);
    }
    store.set_doc(kind::MAIL_STATE, thread_id, &doc, "")?;
    Ok(())
}

/// One or more addresses, separated by commas, as they will be sent to.
fn addresses(list: &str, what: &str) -> Result<String> {
    let each: Vec<String> = list
        .split([',', ';'])
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(String::from)
        .collect();
    for a in &each {
        // "Name <a@b.c>" or the bare address.
        let bare = match (a.rfind('<'), a.rfind('>')) {
            (Some(i), Some(j)) if i < j => &a[i + 1..j],
            _ => a.as_str(),
        };
        if !is_address(&address(bare)) {
            return refused(format!("\"{a}\" isn't a mail address ({what})."));
        }
    }
    Ok(each.join(", "))
}

/// Puts one action in the outbox, and does its part that is local at once:
/// an archived thread leaves the inbox now, a read one is read now.
pub fn queue(store: &mut Store, clock: &Clock, action: &Map<String, Value>) -> Result<Value> {
    let text = |k: &str| action.get(k).and_then(Value::as_str).unwrap_or("").trim();
    let what = text("kind");
    if !ACTIONS.contains(&what) {
        return refused("Mail can send, reply, archive and mark a thread read or unread.");
    }
    let thread = text("threadId");
    if what != "send" && !recorded(store, thread)? {
        return refused("That thread isn't in Mail.");
    }
    let mut doc = json!({
        "id": crate::ulid(),
        "kind": what,
        "status": "queued",
        "createdAt": num(clock.now_ms),
    });
    if what != "send" {
        doc["threadId"] = json!(thread);
    }
    if matches!(what, "send" | "reply") {
        let body = action
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .replace("\r\n", "\n");
        let body = body.trim_end();
        if body.trim().is_empty() {
            return refused("Write the mail first.");
        }
        if body.chars().count() > BODY_CHARS {
            return refused(format!("A mail holds at most {BODY_CHARS} characters."));
        }
        doc["body"] = json!(body);
        let to = addresses(text("to"), "To")?;
        if what == "send" {
            if to.is_empty() {
                return refused("Say who the mail is to.");
            }
            if text("subject").is_empty() {
                return refused("Give the mail a subject.");
            }
            if text("subject").chars().count() > 300 {
                return refused("A subject holds at most 300 characters.");
            }
            doc["subject"] = json!(text("subject"));
        }
        if !to.is_empty() {
            doc["to"] = json!(to);
        }
        let cc = addresses(text("cc"), "Cc")?;
        if !cc.is_empty() {
            doc["cc"] = json!(cc);
        }
        if !text("account").is_empty() {
            doc["account"] = json!(address(text("account")));
        }
    }
    match what {
        "archive" => set_flags(store, thread, None, Some(true))?,
        "unarchive" => set_flags(store, thread, None, Some(false))?,
        "markRead" => set_flags(store, thread, Some(false), None)?,
        "markUnread" => set_flags(store, thread, Some(true), None)?,
        _ => {}
    }
    // A later change of the same kind to the same thread replaces one still waiting.
    let opposite = |k: &str| match k {
        "archive" | "unarchive" => ["archive", "unarchive"],
        _ => ["markRead", "markUnread"],
    };
    if !matches!(what, "send" | "reply") {
        for old in all(store, kind::MAIL_ACTION)? {
            let same = old["status"] == "queued"
                && old["threadId"].as_str() == Some(thread)
                && old["kind"]
                    .as_str()
                    .is_some_and(|k| opposite(what).contains(&k));
            if let (true, Some(id)) = (same, old["id"].as_str()) {
                store.remove_doc(kind::MAIL_ACTION, id)?;
            }
        }
    }
    store.set_doc(
        kind::MAIL_ACTION,
        doc["id"].as_str().unwrap_or_default(),
        &doc,
        "",
    )?;
    Ok(doc)
}

/// The actions, oldest first: every one waiting or failed, and the last
/// twenty that were done.
pub fn actions(store: &Store) -> Result<Vec<Value>> {
    let mut list = all(store, kind::MAIL_ACTION)?;
    list.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let done = list.iter().filter(|a| a["status"] == "done").count();
    let mut skip = done.saturating_sub(20);
    list.retain(|a| {
        if a["status"] == "done" && skip > 0 {
            skip -= 1;
            return false;
        }
        true
    });
    Ok(list)
}

/// The actions still waiting, oldest first.
pub fn outbox(store: &Store) -> Result<Vec<Value>> {
    Ok(actions(store)?
        .into_iter()
        .filter(|a| a["status"] == "queued")
        .collect())
}

/// `finish_mail_action`: an action done, or why it couldn't be. A failed
/// one stays in the outbox, to be tried again or thrown away by the person.
pub fn finish(store: &mut Store, clock: &Clock, id: &str, done: bool, why: &str) -> Result<Value> {
    let Some(mut doc) = one(store, kind::MAIL_ACTION, id)? else {
        return refused("No action in the outbox has that id.");
    };
    if doc["status"] == "done" {
        return refused("That action is done already. Don't do it twice.");
    }
    doc["status"] = json!(if done { "done" } else { "failed" });
    doc["doneAt"] = num(clock.now_ms);
    match (done, why.trim()) {
        (false, "") => doc["error"] = json!("Gmail didn't take it."),
        (false, why) => doc["error"] = json!(why.chars().take(300).collect::<String>()),
        (true, _) => {
            doc.as_object_mut().map(|m| m.remove("error"));
        }
    }
    store.set_doc(kind::MAIL_ACTION, id, &doc, "")?;
    Ok(doc)
}

/// A failed action waits again; or any action not yet done is thrown away.
pub fn retry_or_discard(store: &mut Store, id: &str, retry: bool) -> Result<()> {
    let Some(mut doc) = one(store, kind::MAIL_ACTION, id)? else {
        return refused("No action in the outbox has that id.");
    };
    if doc["status"] == "done" {
        return refused("That one was done already.");
    }
    if retry {
        doc["status"] = json!("queued");
        doc.as_object_mut().map(|m| m.remove("error"));
        store.set_doc(kind::MAIL_ACTION, id, &doc, "")?;
    } else {
        store.remove_doc(kind::MAIL_ACTION, id)?;
    }
    Ok(())
}

/// The threads whose subject, sender, Claude's reason or saved text hold
/// every word of `query`, by Gmail's thread id.
pub fn search(store: &Store, query: &str) -> Result<Vec<String>> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let texts = all(store, kind::MAIL_TEXT)?;
    let mut found = Vec::new();
    for t in all(store, kind::MAIL)? {
        let Some(id) = t["gmailThreadId"].as_str() else {
            continue;
        };
        let mut hay = String::new();
        for k in ["subject", "from", "reason", "course", "category", "account"] {
            hay.push_str(t[k].as_str().unwrap_or(""));
            hay.push('\n');
        }
        if let Some(doc) = texts.iter().find(|d| d["threadId"].as_str() == Some(id)) {
            for m in doc["messages"].as_array().into_iter().flatten() {
                for k in ["from", "to", "text"] {
                    hay.push_str(m[k].as_str().unwrap_or(""));
                    hay.push('\n');
                }
            }
        }
        let hay = hay.to_lowercase();
        if words.iter().all(|w| hay.contains(w)) {
            found.push(id.to_string());
        }
    }
    Ok(found)
}

/// When mail was last read, and the line that says how it went:
/// `{at, line}`, or null before the first read.
pub fn sync_status(store: &Store) -> Result<Value> {
    Ok(setting(store, "mail.sync")?.unwrap_or(Value::Null))
}

pub fn set_sync_status(store: &mut Store, at_ms: f64, line: &str) -> Result<()> {
    set_setting(store, "mail.sync", &json!({"at": num(at_ms), "line": line}))
}

/// Whether mail is read on its own while the app is open. On until the
/// person turns it off.
pub fn background(store: &Store) -> Result<bool> {
    Ok(setting(store, "mail.background")?
        .and_then(|v| v.as_bool())
        .unwrap_or(true))
}

pub fn set_background(store: &mut Store, on: bool) -> Result<()> {
    set_setting(store, "mail.background", &json!(on))
}
