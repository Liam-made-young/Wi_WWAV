//! The prompts the server offers (docs/SPEC.md 3.12): a job the person
//! starts in one step, written out for Claude. The first is school mail:
//! the app never reads Gmail, so this is how mail gets into Learn. Claude
//! reads it with its own Gmail connector and records it with the tools.
//! A prompt only words the job; the tools' schemas are still the door.

use serde_json::{json, Map, Value};

pub const SCHOOL_MAIL: &str = "school_mail";
pub const READ_MAIL: &str = "read_mail";

/// How far back school mail is read: a week unless the person says.
pub const DAYS: (i64, i64, i64) = (1, 7, 60);

/// The school as Settings → Learn has it, for the prompt to name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct School {
    pub name: String,
    pub host: String,
}

/// `prompts/list`.
pub fn list() -> Value {
    json!([{
        "name": SCHOOL_MAIL,
        "title": "Read school mail",
        "description": "Reads recent school mail with Claude's Gmail connector and records it in Learn: a task for what asks for something, a pending grade for a grade notice, and a row in Mail for every thread.",
        "arguments": [{
            "name": "days",
            "description": "How many days back to read, 1 to 60. Default 7.",
            "required": false
        }]
    }, {
        "name": READ_MAIL,
        "title": "Read and sort my mail",
        "description": "Reads recent mail from every account set up in Settings → Learn, or one of them, with Claude's Gmail connector, and records each thread in Mail with its account, priority and category. School mail also becomes tasks and pending grades.",
        "arguments": [{
            "name": "account",
            "description": "One account's address. Default: every account.",
            "required": false
        }, {
            "name": "days",
            "description": "How many days back to read, 1 to 60. Default 7.",
            "required": false
        }]
    }])
}

/// `prompts/get`: the job in words, or one sentence for why not.
pub fn get(name: &str, args: &Map<String, Value>, school: &School) -> Result<Value, String> {
    if name != SCHOOL_MAIL && name != READ_MAIL {
        return Err(format!("Unknown prompt: {name}"));
    }
    // Clients send prompt arguments as text.
    let days = match args.get("days") {
        None | Some(Value::Null) => DAYS.1,
        Some(v) => match v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())) {
            Some(n) if (DAYS.0..=DAYS.2).contains(&n) => n,
            _ => return Err("\"days\" is a whole number from 1 to 60.".to_string()),
        },
    };
    if name == READ_MAIL {
        let account = args.get("account").and_then(Value::as_str).map(str::trim).filter(|a| !a.is_empty());
        if account.is_some_and(|a| !a.contains('@') || a.chars().any(|c| c.is_whitespace() || c.is_control()) || a.len() > 254) {
            return Err("\"account\" is a mail address.".to_string());
        }
        return Ok(json!({
            "description": "Read mail and sort it into Learn",
            "messages": [{"role": "user", "content": {"type": "text", "text": read_mail(days, account)}}]
        }));
    }
    Ok(json!({
        "description": "Read school mail and record it in Learn",
        "messages": [{"role": "user", "content": {"type": "text", "text": school_mail(days, school)}}]
    }))
}

fn school_mail(days: i64, school: &School) -> String {
    let mut senders = vec!["brightspace".to_string(), "d2l".to_string()];
    // A host such as brightspace.uri.edu also names the school's own mail: uri.edu.
    let host = school.host.trim().trim_start_matches("https://").trim_end_matches('/');
    if !host.is_empty() {
        senders.insert(0, format!("from:{host}"));
        let parts: Vec<&str> = host.split('.').collect();
        if parts.len() > 2 {
            senders.insert(1, format!("from:{}", parts[parts.len() - 2..].join(".")));
        }
    }
    let who = if school.name.trim().is_empty() { "my school".to_string() } else { school.name.trim().to_string() };
    let query = format!("newer_than:{days}d ({})", senders.join(" OR "));
    format!(
        "Read my school mail from the last {days} days and record it in Learn, the planner in Wi_WWAV. The school is {who}.

Learn never reads mail itself. You read it with your Gmail tools, then record what you found with the wi-wwav tools. If you have no Gmail tools here, say so and stop.

1. Call list_tasks with status \"all\" and get_grades first, so you know the courses and what is already there.
2. Search Gmail. Start from this query and widen it only if it finds nothing: {query}
3. For each thread, oldest first, read it and pick one state:
   - grade: a grade or feedback was posted. Call add_pending_grade with the course code, the item as the notice names it, posted_at, the Brightspace address as link if the mail gives one, and mail_thread_id. There is no score argument: only I type a score.
   - task: it asks me to do something. Call add_task with a title as I would write it, the course code, due only if the mail states a time (never guess one), notes that start \"From mail:\" and say in a line or two what is asked, source_id set to the Gmail message id, and mail_thread_id.
   - nothing: no deadline and nothing to do.
4. Then call record_mail_thread for every thread you read, whatever its state: thread_id, subject, from, received_at, the course code if it is about one, the state, task_id if you made a task, and a one-sentence reason.
5. Then call save_mail_text for the thread, so I can read it in Learn: every message, oldest first, each with from, sent_at and text. The text is the mail's own words as plain text. Leave out quoted earlier messages and footers, and never summarise or reword.
6. If a course code isn't one get_grades returned, leave course out rather than guess.

Reading the same mail twice is safe: the same source_id, the same course and item, or the same thread_id never makes a second row. Don't mark anything done, don't change a due date, and don't reply to, archive or label any mail.

When you finish, tell me in a few lines how many threads you read and what you made. I can undo each change in Learn with \u{2318}Z."
    )
}

fn read_mail(days: i64, account: Option<&str>) -> String {
    let which = match account {
        Some(a) => format!("Read only the account {a}."),
        None => "Read every account it lists.".to_string(),
    };
    format!(
        "Read my mail from the last {days} days and sort it into Learn, the planner in Wi_WWAV.

Learn never reads mail itself. You read it with your Gmail tools, then record what you found with the wi-wwav tools. If you have no Gmail tools here, say so and stop.

1. Call list_mail_accounts. {which} Each account comes with a gmail_query that finds its mail and no other account's. If it lists no accounts, read the mailbox your Gmail tools reach and leave account out when you record.
2. Call list_tasks with status \"all\", get_grades, and list_mail with since set to {days} days ago, so you know the courses, the tasks and what you have already recorded.
3. For each account, search Gmail with: newer_than:{days}d, then the account's gmail_query. Read each thread you haven't recorded, oldest first.
4. Sort each thread:
   - priority. urgent: I must act today or tomorrow. high: I must act this week, or a person is waiting on me. normal: worth knowing, nothing to do. low: bulk mail, promotions, automatic notices.
   - category: school, work, money, people, updates, promotions or other.
   - state. grade: a grade or feedback was posted; call add_pending_grade with the course code, the item as the notice names it, posted_at, the Brightspace address as link if the mail gives one, and mail_thread_id. There is no score argument: only I type a score. task: it asks me to do something; call add_task with a title as I would write it, the course code if it is school, due only if the mail states a time (never guess one), notes that start \"From mail:\" and say in a line or two what is asked, source_id set to the Gmail message id, and mail_thread_id. nothing: nothing to do.
5. Call record_mail_thread for every thread you read, whatever you decided: thread_id, account, subject, from, received_at, priority, category, state, the course code if it is about one of my courses, task_id if you made a task, and a one-sentence reason that says why it has that priority.
6. Then call save_mail_text for the thread, so I can read it in Learn: every message, oldest first, each with from, sent_at and text. The text is the mail's own words as plain text, with its paragraphs. Leave out quoted earlier messages and unsubscribe footers, keep links as plain addresses, and never summarise or reword. Also do this for any thread list_mail shows with has_text false.
7. If a course code isn't one get_grades returned, leave course out rather than guess. Don't make a task for promotions or automatic notices.

Reading the same mail twice is safe: the same source_id, the same course and item, or the same thread_id never makes a second row. Don't mark anything done, don't change a due date, and don't reply to, archive, label or delete any mail.

When you finish, tell me per account how many threads you read, then list the urgent and high ones with what each needs from me. I can undo each change in Learn with \u{2318}Z."
    )
}
