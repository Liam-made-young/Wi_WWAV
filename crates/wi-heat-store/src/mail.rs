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
