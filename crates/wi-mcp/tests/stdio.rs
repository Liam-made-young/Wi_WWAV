//! The MCP suite of docs/SPEC.md 8.12, over real stdio against a real
//! library (PLAN S2.8). It fails if a write leaves anything but one journal
//! entry by Claude with its label; if undoing it doesn't return the library
//! byte for byte; if a read or `plan_day` journals; if a repeated
//! `source_id` or `thread_id` makes a second row; if a switched-off tool is
//! listed or answers; if a kill leaves a change without its entry or the
//! reverse; or if writes from the helper and the app at once lose one.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};
use wi_store::{Actor, Room, Store};

const HELPER: &str = env!("CARGO_BIN_EXE_wi-mcp");

struct Helper {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: i64,
}

impl Helper {
    fn start(root: &Path) -> Helper {
        let mut child = Command::new(HELPER)
            .arg("--library")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("wi-mcp starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut h = Helper { child, stdin, stdout, next: 1 };
        let init = h.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}));
        assert_eq!(init["result"]["serverInfo"]["name"], "wi-wwav");
        h.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        h
    }

    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
        assert_eq!(reply["id"], id);
        reply
    }

    /// A tool's answer: Ok(structuredContent), or Err(the sentence).
    fn call(&mut self, tool: &str, args: Value) -> Result<Value, String> {
        let reply = self.request("tools/call", json!({"name": tool, "arguments": args}));
        let result = &reply["result"];
        if result["isError"] == true {
            Err(result["content"][0]["text"].as_str().unwrap_or_default().to_string())
        } else {
            Ok(result["structuredContent"].clone())
        }
    }

    fn tools(&mut self) -> Vec<String> {
        let reply = self.request("tools/list", json!({}));
        reply["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect()
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A library as the app leaves it after first launch, plus a term with a
/// course and two tasks.
fn seeded() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Wi_WWAV");
    let mut store = Store::open(&root).unwrap();
    let mut txn = store.begin(Room::Heat, "first launch").unwrap();
    for (id, name, kind, types) in [
        ("sp-classes", "Classes", "course", json!(["Homework", "Quiz", "Reading"])),
        ("sp-wwav", "WWAV", "milestone", json!(["Build", "Mix"])),
        ("sp-personal", "Personal", "free", json!(["Errand"])),
    ] {
        let space = json!({"id": id, "name": name, "hue": 210, "groupKind": kind, "groupLabel": "Course", "types": types, "persona": format!("{name} persona")});
        txn.put_doc("space", id, &space, name).unwrap();
    }
    txn.put_doc("term", "term-fall", &json!({"id": "term-fall", "name": "Fall 2026"}), "").unwrap();
    let course = json!({"id": "c-jpn", "termId": "term-fall", "code": "JPN 201", "name": "Japanese 2",
        "categories": [{"id": "cat-quiz", "name": "Quizzes", "weight": 40, "keywords": ["quiz"]},
                       {"id": "cat-hw", "name": "Homework", "weight": 60, "keywords": ["homework", "edfinity"]}],
        "notes": ""});
    txn.put_doc("course", "c-jpn", &course, "JPN 201").unwrap();
    for (id, title, due) in [("t-essay", "Essay draft", 1_791_500_000_000.0), ("t-lab", "Lab 5a", 1_791_400_000_000.0)] {
        let task = json!({"id": id, "spaceId": "sp-classes", "title": title, "type": "Homework", "courseId": "c-jpn",
            "due": due as i64, "difficulty": 3, "estMin": null, "adjustMin": 0, "notes": "", "done": false, "doneAt": null, "source": "you"});
        txn.put_doc("task", id, &task, title).unwrap();
    }
    txn.commit().unwrap();
    (dir, root)
}

/// Every journaled record, for byte-for-byte comparisons.
fn dump(root: &Path) -> String {
    let store = Store::open(root).unwrap();
    let mut out = String::new();
    for kind in ["space", "task", "term", "course", "grade", "focusSession", "mailThread", "project", "milestone", "note", "capture", "habit", "timeBlock"] {
        for doc in store.docs(kind).unwrap() {
            out.push_str(&format!("{kind}/{} {}\n", doc.key, doc.json));
        }
    }
    out
}

fn claude_entries(root: &Path) -> Vec<wi_store::EntryInfo> {
    Store::open(root).unwrap().entries(true, 10_000).unwrap()
}

#[test]
fn every_tool_answers_and_each_write_is_one_entry_by_claude() {
    let (_dir, root) = seeded();
    let mut h = Helper::start(&root);
    assert_eq!(h.tools().len(), 23);

    let before = dump(&root);
    let listed = h.call("list_tasks", json!({})).unwrap();
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(listed["spaces"][0]["name"], "Classes");
    assert!(listed["tasks"][0]["heat"]["level"].is_string());
    let grades = h.call("get_grades", json!({"course": "jpn201"})).unwrap();
    assert_eq!(grades["term"], "Fall 2026");
    assert_eq!(grades["courses"][0]["code"], "JPN 201");
    let plan = h.call("plan_day", json!({})).unwrap();
    assert!(plan["drafts"].is_array() && plan["minutes_left"].is_number(), "{plan}");
    assert_eq!(dump(&root), before, "reads and plan_day change no record");
    assert!(claude_entries(&root).is_empty(), "reads and plan_day journal nothing");

    let writes = [
        ("add_task", json!({"title": "Grammar quiz 4", "course": "JPN 201", "due": "2026-10-09T23:59:00-04:00", "source_id": "gm-1", "reason": "The notice gives a Friday deadline."}), "Claude's task"),
        ("update_task", json!({"id": "t-essay", "difficulty": 4, "estimate_min": 90, "reason": "Two thousand words."}), "Claude's estimate"),
        ("add_pending_grade", json!({"course": "JPN 201", "item": "Quiz 3", "reason": "A grade was posted."}), "Claude's pending grade"),
        ("log_focus", json!({"task_id": "t-lab", "minutes": 25, "reason": "They said they worked on it."}), "Claude's focus log"),
        ("record_mail_thread", json!({"thread_id": "th-1", "subject": "Quiz 3 graded", "from": "D2L", "received_at": "2026-10-07T09:12:00-04:00", "state": "grade", "course": "JPN 201", "reason": "A grade notice."}), "Claude's mail note"),
        ("add_project", json!({"title": "EP v1", "space": "WWAV", "target_date": "2026-12-01", "reason": "They said the EP is the next thing."}), "Claude's project"),
        ("add_milestone", json!({"title": "Mix the second verse", "date": "2026-10-20", "space": "WWAV", "reason": "They named the date."}), "Claude's milestone"),
        ("add_note", json!({"title": "Essay outline", "text": "# Outline\n\n1. Thesis", "reason": "They asked for an outline to start from."}), "Claude's note"),
        ("add_capture", json!({"text": "Ask the TA about lab 6", "reason": "It came up and isn't clearly a task."}), "Claude's capture"),
    ];
    for (n, (tool, args, label)) in writes.iter().enumerate() {
        let state_before = dump(&root);
        let answer = h.call(tool, args.clone()).unwrap_or_else(|e| panic!("{tool}: {e}"));
        assert_eq!(answer["undo_label"], format!("Undo {label}"), "{tool}");
        let entries = claude_entries(&root);
        assert_eq!(entries.len(), n + 1, "{tool} makes exactly one entry");
        assert_eq!(entries[0].label, *label);
        match &entries[0].actor {
            Actor::Claude { tool: t, reason } => {
                assert_eq!(t, tool);
                assert_eq!(reason, args["reason"].as_str().unwrap());
            }
            Actor::You => panic!("{tool} journaled as the person"),
        }
        // Undo returns the library byte for byte, and redo puts it back.
        let after = dump(&root);
        let mut store = Store::open(&root).unwrap();
        assert_eq!(store.undo(Room::Heat).unwrap().as_deref(), Some(*label));
        drop(store);
        assert_eq!(dump(&root), state_before, "undoing {tool}");
        let mut store = Store::open(&root).unwrap();
        store.redo(Room::Heat).unwrap();
        drop(store);
        assert_eq!(dump(&root), after, "redoing {tool}");
    }
    let estimate = h.call("list_tasks", json!({"status": "all"})).unwrap();
    let essay = estimate["tasks"].as_array().unwrap().iter().find(|t| t["id"] == "t-essay").unwrap().clone();
    assert_eq!(essay["estimate_min"], 90);
    assert_eq!(essay["estimate_by"], "claude");
    assert_eq!(essay["estimate_reason"], "Two thousand words.");
}

/// The ten tools past 3.13's first eight: the reads change nothing, the
/// drafts mark their records as Claude's with the reason, and nothing they
/// make is done, public or triaged.
#[test]
fn the_reads_change_nothing_and_the_drafts_are_marked_as_claudes() {
    let (_dir, root) = seeded();
    {
        let mut store = Store::open(&root).unwrap();
        let mut txn = store.begin(Room::Heat, "seed more").unwrap();
        txn.put_doc("habit", "h-walk", &json!({"id": "h-walk", "title": "Walk", "minutes": 20, "log": {}, "showCounter": false}), "Walk").unwrap();
        txn.put_doc("note", "n-mine", &json!({"id": "n-mine", "title": "Mine", "markdown": "x".repeat(5000)}), "Mine").unwrap();
        txn.put_doc("capture", "cap-done", &json!({"id": "cap-done", "text": "old", "triagedAt": 1}), "old").unwrap();
        txn.put_doc("timeBlock", "b-1", &json!({"id": "b-1", "taskId": "t-lab", "date": "2099-01-05", "start": 600, "minutes": 60, "origin": "you"}), "").unwrap();
        txn.commit().unwrap();
    }
    let mut h = Helper::start(&root);
    let before = dump(&root);
    let habits = h.call("list_habits", json!({})).unwrap();
    assert_eq!(habits["habits"][0]["title"], "Walk");
    assert_eq!(habits["habits"][0]["done_today"], false);
    let notes = h.call("get_notes", json!({})).unwrap();
    assert_eq!(notes["notes"][0]["text"].as_str().unwrap().len(), 4000);
    assert_eq!(notes["notes"][0]["cut_short"], true);
    assert_eq!(notes["notes"][0]["by"], "you");
    assert!(notes["daily_note"].is_null());
    assert_eq!(h.call("list_inbox", json!({})).unwrap()["captures"], json!([]));
    assert_eq!(h.call("list_projects", json!({})).unwrap(), json!({"projects": [], "milestones": []}));
    let day = h.call("get_schedule", json!({"from": "2099-01-05"})).unwrap();
    assert_eq!(day["blocks"].as_array().unwrap().len(), 1, "{day}");
    assert_eq!(day["blocks"][0]["title"], "Lab 5a");
    assert_eq!(day["blocks"][0]["minutes"], 60);
    assert_eq!(dump(&root), before, "the reads change no record");
    assert!(claude_entries(&root).is_empty(), "the reads journal nothing");

    // A draft is not a change: nothing is journaled and no block is made.
    let draft = h.call("draft_block", json!({"task_id": "t-essay", "date": "2099-01-05", "start": "14:10", "minutes": 50, "reason": "They asked for the afternoon."})).unwrap();
    assert_eq!(draft["draft"]["title"], "Essay draft");
    assert_eq!(draft["draft"]["minutes"], 45, "rounded to the 15-minute grid");
    assert!(draft["draft"]["start"].as_str().unwrap().starts_with("2099-01-05T14:15:00"), "{draft}");
    assert_eq!(draft["drafts_waiting"], 1);
    assert_eq!(dump(&root), before, "a draft makes no block");
    assert!(claude_entries(&root).is_empty(), "a draft journals nothing");
    let day = h.call("get_schedule", json!({"from": "2099-01-05"})).unwrap();
    assert_eq!(day["drafts"][0]["reason"], "They asked for the afternoon.");
    // The same task and day again replaces the draft; a held time is refused.
    let moved = h.call("draft_block", json!({"task_id": "t-essay", "date": "2099-01-05", "start": "16:00", "minutes": 30, "reason": "Later."})).unwrap();
    assert_eq!(moved["drafts_waiting"], 1);
    let held = h.call("draft_block", json!({"task_id": "t-essay", "date": "2099-01-05", "start": "10:30", "minutes": 30, "reason": "r"})).unwrap_err();
    assert_eq!(held, "A block already holds that time. Read get_schedule and pick a free one.");
    let on_draft = h.call("draft_block", json!({"task_id": "t-lab", "date": "2099-01-05", "start": "16:15", "minutes": 30, "reason": "r"})).unwrap_err();
    assert_eq!(on_draft, "A draft already holds that time. Read get_schedule and pick a free one.");
    assert_eq!(h.call("draft_block", json!({"task_id": "t-lab", "date": "2020-01-05", "start": "16:15", "minutes": 30, "reason": "r"})).unwrap_err(), "That day is over. Plan today or a day ahead.");
    assert_eq!(h.call("draft_block", json!({"task_id": "t-lab", "date": "2099-01-05", "start": "06:00", "minutes": 30, "reason": "r"})).unwrap_err(), "A block sits between 7 AM and midnight.");
    assert_eq!(h.call("draft_block", json!({"task_id": "nope", "start": "16:15", "minutes": 30, "reason": "r"})).unwrap_err(), "No open task has that id.");

    let project = h.call("add_project", json!({"title": "EP v1", "space": "wwav", "reason": "The EP is next."})).unwrap()["project"].clone();
    assert_eq!(project["source"], "claude");
    assert_eq!(project["claudeReason"], "The EP is next.");
    assert_eq!(project["status"], "active");
    assert_eq!(project["public"], false);
    assert_eq!(h.call("add_project", json!({"title": "ep V1", "space": "WWAV", "reason": "Again."})).unwrap_err(), "WWAV already has a project called ep V1.");
    let milestone = h.call("add_milestone", json!({"title": "Mixed", "date": "2026-11-01", "project_id": project["id"], "reason": "They named it."})).unwrap()["milestone"].clone();
    assert_eq!(milestone["done"], false);
    assert_eq!(milestone["spaceId"], "sp-wwav", "a milestone takes its project's space");
    assert_eq!(milestone["source"], "claude");
    assert_eq!(h.call("add_milestone", json!({"title": "x", "date": "2026-02-30", "reason": "r"})).unwrap_err(), "A milestone needs a day, as YYYY-MM-DD.");
    assert_eq!(h.call("add_milestone", json!({"title": "x", "date": "2026-11-01", "project_id": "nope", "reason": "r"})).unwrap_err(), "No project has that id.");
    let listed = h.call("list_projects", json!({"space": "WWAV"})).unwrap();
    assert_eq!(listed["projects"][0]["milestones"][0]["title"], "Mixed");
    assert_eq!(listed["projects"][0]["open_tasks"], 0);

    h.call("add_note", json!({"title": "Outline", "text": "# One", "project_id": project["id"], "reason": "A place to start."})).unwrap();
    let notes = h.call("get_notes", json!({})).unwrap();
    let outline = notes["notes"].as_array().unwrap().iter().find(|n| n["title"] == "Outline").unwrap();
    assert_eq!(outline["by"], "claude");
    assert_eq!(outline["text"], "# One");
    assert_eq!(h.call("add_note", json!({"title": "x", "text": "y", "project_id": "nope", "reason": "r"})).unwrap_err(), "No project has that id.");

    let captured = h.call("add_capture", json!({"text": "Ask the TA about lab 6", "reason": "Not clearly a task."})).unwrap();
    assert_eq!(captured["in_inbox"], 1, "the triaged capture isn't counted");
    let inbox = h.call("list_inbox", json!({})).unwrap();
    assert_eq!(inbox["captures"], json!([{"id": captured["capture"]["id"], "text": "Ask the TA about lab 6", "by": "claude"}]));

    // Nothing Claude made is done, public or triaged, and the person's records are as they were.
    let store = Store::open(&root).unwrap();
    for kind in ["project", "milestone", "note", "capture"] {
        for doc in store.docs(kind).unwrap() {
            assert_ne!(doc.json["public"], true, "{kind}");
            assert_ne!(doc.json["done"], true, "{kind}");
            if doc.json["source"] == "claude" {
                assert!(doc.json.get("triagedAt").is_none(), "{kind}");
            }
        }
    }
    assert_eq!(store.docs("habit").unwrap()[0].json["log"], json!({}), "no habit was ticked");
    assert_eq!(store.docs("timeBlock").unwrap().len(), 1, "no block was made");
    assert_eq!(claude_entries(&root).len(), 4, "one entry for each of the four records");
}

/// Two accounts in one mailbox: each has a search that finds its mail and no
/// other's, a thread is recorded under its account with a priority and a
/// category, and a thread read again keeps them unless Claude says otherwise.
#[test]
fn mail_is_recorded_per_account_with_a_priority_and_a_category() {
    let (_dir, root) = seeded();
    let mut h = Helper::start(&root);
    let none = h.call("list_mail_accounts", json!({})).unwrap();
    assert_eq!(none["accounts"], json!([]));
    assert!(none["note"].as_str().unwrap().contains("Settings → Learn"));
    // With no accounts, a thread is still recorded, under whatever it names.
    let thread = |id: &str, extra: Value| {
        let mut t = json!({"thread_id": id, "subject": format!("Subject {id}"), "from": "A <a@uri.edu>", "received_at": "2026-10-07T09:00:00-04:00", "state": "nothing", "reason": "Nothing to do."});
        for (k, v) in extra.as_object().unwrap() {
            t[k] = v.clone();
        }
        t
    };
    assert_eq!(h.call("record_mail_thread", thread("th-0", json!({}))).unwrap()["thread"]["priority"], "normal");

    {
        let mut store = Store::open(&root).unwrap();
        let bad = |list: Value| wi_heat_store::mail::set_accounts(&mut Store::open(&root).unwrap(), list.as_array().unwrap()).unwrap_err().to_string();
        assert_eq!(bad(json!([{"address": "nope"}])), "A mail account needs its address, such as name@school.edu.");
        assert_eq!(bad(json!([{"address": "a@uri.edu", "via": "forward"}])), "Say where a@uri.edu is forwarded to, such as you+school@gmail.com.");
        assert_eq!(bad(json!([{"address": "a@uri.edu"}, {"address": "A@URI.edu"}])), "a@uri.edu is in the list twice.");
        assert!(bad(json!([{"address": "a@uri.edu", "via": "forward", "forwardTo": "me+x@gmail.com"}, {"address": "b@uri.edu", "via": "forward", "forwardTo": "me+x@gmail.com"}])).starts_with("Two accounts are forwarded to me+x@gmail.com"));
        wi_heat_store::mail::set_accounts(&mut store, json!([
            {"address": "Liam.Young@uri.edu", "name": "URI", "via": "forward", "forwardTo": "made.liamyoung+uri@gmail.com"},
            {"address": "made.liamyoung@gmail.com", "name": "Personal"}
        ]).as_array().unwrap()).unwrap();
    }
    let listed = h.call("list_mail_accounts", json!({})).unwrap();
    assert!(listed.get("note").is_none());
    assert_eq!(listed["accounts"][0]["address"], "liam.young@uri.edu");
    assert_eq!(listed["accounts"][0]["gmail_query"], "deliveredto:made.liamyoung+uri@gmail.com");
    assert_eq!(listed["accounts"][1]["gmail_query"], "-deliveredto:made.liamyoung+uri@gmail.com");
    assert_eq!(listed["accounts"][1]["name"], "Personal");
    assert_eq!(listed["priorities"], json!(["urgent", "high", "normal", "low"]));

    let before = claude_entries(&root).len();
    let urgent = h.call("record_mail_thread", thread("th-1", json!({"account": "LIAM.YOUNG@uri.edu", "priority": "urgent", "category": "school", "course": "JPN 201", "reason": "The quiz is due tonight."}))).unwrap();
    assert_eq!(urgent["thread"]["account"], "liam.young@uri.edu");
    assert_eq!(urgent["thread"]["priority"], "urgent");
    assert_eq!(urgent["thread"]["category"], "school");
    h.call("record_mail_thread", thread("th-2", json!({"account": "made.liamyoung@gmail.com", "priority": "low", "category": "promotions"}))).unwrap();
    assert_eq!(claude_entries(&root).len(), before + 2);
    assert_eq!(
        h.call("record_mail_thread", thread("th-3", json!({"account": "someone@else.com"}))).unwrap_err(),
        "someone@else.com isn't one of the mail accounts in Settings → Learn. They are: liam.young@uri.edu, made.liamyoung@gmail.com."
    );
    // With two accounts and none named, the thread has no account rather than a guessed one.
    assert!(h.call("record_mail_thread", thread("th-4", json!({}))).unwrap()["thread"].get("account").is_none());
    // Read again with only a new state: the account, priority and category stay.
    let again = h.call("record_mail_thread", thread("th-1", json!({"state": "task", "reason": "It asks for the quiz."}))).unwrap();
    assert_eq!(again["created"], false);
    assert_eq!((again["thread"]["account"].clone(), again["thread"]["priority"].clone(), again["thread"]["category"].clone()), (json!("liam.young@uri.edu"), json!("urgent"), json!("school")));

    let counted = h.call("list_mail_accounts", json!({})).unwrap();
    assert_eq!((counted["accounts"][0]["recorded"].clone(), counted["accounts"][1]["recorded"].clone()), (json!(1), json!(1)));
    let uri = h.call("list_mail", json!({"account": "liam.young@uri.edu"})).unwrap();
    assert_eq!(uri["recorded"], 1);
    assert_eq!(uri["threads"][0]["thread_id"], "th-1");
    assert_eq!(uri["threads"][0]["state"], "task");
    assert!(uri["threads"][0]["received_at"].as_str().unwrap().starts_with("2026-10-07T"));
    assert_eq!(h.call("list_mail", json!({"priority": "urgent"})).unwrap()["recorded"], 1);
    assert_eq!(h.call("list_mail", json!({"priority": "normal"})).unwrap()["recorded"], 2, "a thread with no priority reads as normal");
    assert_eq!(h.call("list_mail", json!({"category": "promotions"})).unwrap()["threads"][0]["thread_id"], "th-2");
    assert_eq!(h.call("list_mail", json!({})).unwrap()["recorded"], 4);
    assert_eq!(h.call("list_mail", json!({"since": "2026-10-08T00:00:00-04:00"})).unwrap()["recorded"], 0);
    let store = Store::open(&root).unwrap();
    assert!(!store.docs("mailThread").unwrap().iter().any(|d| d.json.get("body").is_some()));
    drop(store);

    // The text of a thread, for Mail's reader: saved whole, outside the journal, and replaced when saved again.
    let entries = claude_entries(&root).len();
    let msg = |at: &str, text: &str| json!({"from": "Prof. Collis <ncollis@uri.edu>", "sent_at": at, "text": text});
    assert_eq!(
        h.call("save_mail_text", json!({"thread_id": "th-none", "messages": [msg("2026-10-07T09:00:00-04:00", "x")]})).unwrap_err(),
        "Record the thread with record_mail_thread first, then save its text."
    );
    assert_eq!(h.call("list_mail", json!({"account": "liam.young@uri.edu"})).unwrap()["threads"][0]["has_text"], false);
    let saved = h.call("save_mail_text", json!({"thread_id": "th-1", "messages": [
        msg("2026-10-07T12:00:00-04:00", "Second.\r\n\r\nSee https://brightspace.uri.edu/d2l/home\n"),
        msg("2026-10-07T09:00:00-04:00", "\n\nFirst.  "),
    ]})).unwrap();
    assert_eq!(saved, json!({"thread_id": "th-1", "messages": 2, "characters": 55}));
    let doc = wi_heat_store::mail::text_of(&Store::open(&root).unwrap(), "th-1").unwrap().unwrap();
    let texts: Vec<_> = doc["messages"].as_array().unwrap().iter().map(|m| m["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(texts, ["First.", "Second.\n\nSee https://brightspace.uri.edu/d2l/home"], "oldest first, trimmed, with its paragraphs");
    assert_eq!(h.call("list_mail", json!({"account": "liam.young@uri.edu"})).unwrap()["threads"][0]["has_text"], true);
    h.call("save_mail_text", json!({"thread_id": "th-1", "messages": [msg("2026-10-07T09:00:00-04:00", "Only this now.")]})).unwrap();
    let doc = wi_heat_store::mail::text_of(&Store::open(&root).unwrap(), "th-1").unwrap().unwrap();
    assert_eq!(doc["messages"].as_array().unwrap().len(), 1);
    assert_eq!(claude_entries(&root).len(), entries, "the mail's text is no journal entry");
    for (bad, why) in [
        (json!([{"from": "f", "sent_at": "2026-10-07T09:00:00Z", "text": "   "}]), "Message 1 has no text."),
        (json!([{"from": "f", "sent_at": "2026-10-07T09:00:00Z", "text": "x", "html": "<b>x</b>"}]), "A message takes no \"html\"."),
        (json!([{"from": "", "sent_at": "2026-10-07T09:00:00Z", "text": "x"}]), "Message 1 needs its sender, in 300 characters or fewer."),
        (json!(["Dear Liam"]), "Message 1 must be an object."),
    ] {
        assert_eq!(h.call("save_mail_text", json!({"thread_id": "th-1", "messages": bad})).unwrap_err(), why);
    }
}

/// The outbox: what the person asked for in Mail waits for Claude, is done
/// once, and a thread's place (unread, archived) is kept on this Mac.
#[test]
fn the_outbox_holds_what_to_do_in_gmail_and_each_is_done_once() {
    use wi_heat_store::{mail, Clock};
    let (_dir, root) = seeded();
    let mut h = Helper::start(&root);
    assert_eq!(h.call("list_mail_outbox", json!({})).unwrap(), json!({"actions": []}));
    h.call("record_mail_thread", json!({"thread_id": "th-1", "subject": "Quiz", "from": "Prof <p@uri.edu>", "received_at": "2026-10-07T09:00:00-04:00", "state": "nothing", "unread": true, "reason": "Nothing to do."})).unwrap();
    let clock = Clock::system();
    let queue = |a: Value| mail::queue(&mut Store::open(&root).unwrap(), &clock, a.as_object().unwrap());
    assert_eq!(mail::states(&Store::open(&root).unwrap()).unwrap(), json!({"th-1": {"unread": true, "archived": false}}));
    for (bad, why) in [
        (json!({"kind": "delete", "threadId": "th-1"}), "Mail can send, reply, archive and mark a thread read or unread."),
        (json!({"kind": "archive", "threadId": "nope"}), "That thread isn't in Mail."),
        (json!({"kind": "reply", "threadId": "th-1", "body": "  "}), "Write the mail first."),
        (json!({"kind": "send", "subject": "Hi", "body": "x"}), "Say who the mail is to."),
        (json!({"kind": "send", "to": "p@uri.edu", "body": "x"}), "Give the mail a subject."),
        (json!({"kind": "send", "to": "not an address", "subject": "Hi", "body": "x"}), "\"not an address\" isn't a mail address (To)."),
    ] {
        assert_eq!(queue(bad).unwrap_err().to_string(), why);
    }
    let entries = claude_entries(&root).len();
    let reply = queue(json!({"kind": "reply", "threadId": "th-1", "body": "Thank you.\r\nLiam\n\n"})).unwrap();
    let sent = queue(json!({"kind": "send", "to": "Prof <p@uri.edu>; ta@uri.edu", "subject": "Lab 6", "body": "When is it due?"})).unwrap();
    queue(json!({"kind": "markRead", "threadId": "th-1"})).unwrap();
    queue(json!({"kind": "archive", "threadId": "th-1"})).unwrap();
    // The local part is done at once, whatever Gmail does later.
    assert_eq!(mail::states(&Store::open(&root).unwrap()).unwrap(), json!({"th-1": {"unread": false, "archived": true}}));
    // A change of mind replaces the one still waiting.
    queue(json!({"kind": "unarchive", "threadId": "th-1"})).unwrap();

    let waiting = h.call("list_mail_outbox", json!({})).unwrap();
    let kinds: Vec<_> = waiting["actions"].as_array().unwrap().iter().map(|a| a["kind"].as_str().unwrap().to_string()).collect();
    assert_eq!(kinds, ["reply", "send", "markRead", "unarchive"]);
    assert_eq!(waiting["actions"][0]["body"], "Thank you.\nLiam", "the body as written, less trailing blank lines");
    assert_eq!(waiting["actions"][1]["to"], "Prof <p@uri.edu>, ta@uri.edu");

    let id = reply["id"].as_str().unwrap();
    assert_eq!(h.call("finish_mail_action", json!({"id": id, "result": "done"})).unwrap()["status"], "done");
    assert_eq!(h.call("finish_mail_action", json!({"id": id, "result": "done"})).unwrap_err(), "That action is done already. Don't do it twice.");
    let failed = h.call("finish_mail_action", json!({"id": sent["id"], "result": "failed", "error": "Gmail refused the address."})).unwrap();
    assert_eq!(failed["status"], "failed");
    assert_eq!(h.call("finish_mail_action", json!({"id": "nope", "result": "done"})).unwrap_err(), "No action in the outbox has that id.");
    let left: Vec<_> = h.call("list_mail_outbox", json!({})).unwrap()["actions"].as_array().unwrap().iter().map(|a| a["kind"].clone()).collect();
    assert_eq!(left, [json!("markRead"), json!("unarchive")], "done and failed ones no longer wait");
    // The person tries the failed one again, or throws it away.
    let store = || Store::open(&root).unwrap();
    assert_eq!(mail::actions(&store()).unwrap().iter().find(|a| a["id"] == sent["id"]).unwrap()["error"], "Gmail refused the address.");
    mail::retry_or_discard(&mut store(), sent["id"].as_str().unwrap(), true).unwrap();
    assert_eq!(mail::outbox(&store()).unwrap().len(), 3);
    mail::retry_or_discard(&mut store(), sent["id"].as_str().unwrap(), false).unwrap();
    assert_eq!(mail::outbox(&store()).unwrap().len(), 2);
    assert_eq!(claude_entries(&root).len(), entries, "the outbox is no journal entry");

    // Search reads the subject, the sender and the saved text.
    h.call("save_mail_text", json!({"thread_id": "th-1", "messages": [{"from": "Prof", "sent_at": "2026-10-07T09:00:00-04:00", "text": "The quiz moves to Thursday in Swan Hall."}]})).unwrap();
    assert_eq!(mail::search(&store(), "swan THURSDAY").unwrap(), ["th-1"]);
    assert_eq!(mail::search(&store(), "prof quiz").unwrap(), ["th-1"]);
    assert!(mail::search(&store(), "swan friday").unwrap().is_empty());
    assert!(mail::search(&store(), "   ").unwrap().is_empty());
}

#[test]
fn the_same_source_never_makes_a_second_row() {
    let (_dir, root) = seeded();
    let mut h = Helper::start(&root);
    let task = json!({"title": "Grammar quiz 4", "source_id": "gm-7", "reason": "From mail."});
    assert_eq!(h.call("add_task", task.clone()).unwrap()["created"], true);
    assert_eq!(h.call("add_task", task).unwrap()["created"], false);
    let grade = json!({"course": "JPN 201", "item": "Quiz 3", "reason": "Posted."});
    assert_eq!(h.call("add_pending_grade", grade.clone()).unwrap()["created"], true);
    assert_eq!(h.call("add_pending_grade", json!({"course": "jpn 201", "item": "quiz 3 ", "reason": "Posted again."})).unwrap()["created"], false);
    let thread = json!({"thread_id": "th-9", "subject": "s", "from": "f", "received_at": "2026-10-07T09:00:00Z", "state": "nothing", "reason": "Nothing to do."});
    assert_eq!(h.call("record_mail_thread", thread).unwrap()["created"], true);
    let again = json!({"thread_id": "th-9", "subject": "s", "from": "f", "received_at": "2026-10-07T09:00:00Z", "state": "task", "reason": "It asks for a form."});
    let updated = h.call("record_mail_thread", again).unwrap();
    assert_eq!(updated["created"], false);
    assert_eq!(updated["thread"]["state"], "task");

    let store = Store::open(&root).unwrap();
    let claude_tasks = store.docs("task").unwrap().into_iter().filter(|d| d.json["sourceId"] == "gm-7").count();
    assert_eq!(claude_tasks, 1);
    assert_eq!(store.docs("grade").unwrap().len(), 1);
    assert_eq!(store.docs("mailThread").unwrap().len(), 1);
    assert_eq!(claude_entries(&root).len(), 4, "a repeat that adds nothing journals nothing");
}

#[test]
fn a_switched_off_tool_is_missing_and_refused() {
    let (_dir, root) = seeded();
    {
        let mut store = Store::open(&root).unwrap();
        wi_heat_store::set_tool(&mut store, "log_focus", false).unwrap();
    }
    let mut h = Helper::start(&root);
    assert!(!h.tools().contains(&"log_focus".to_string()));
    let refused = h.call("log_focus", json!({"task_id": "t-lab", "minutes": 25, "reason": "r"})).unwrap_err();
    assert_eq!(refused, "This tool is switched off in Wi_WWAV.");
    assert!(claude_entries(&root).is_empty());
}

#[test]
fn rules_that_need_the_library_answer_in_one_sentence() {
    let (_dir, root) = seeded();
    let mut h = Helper::start(&root);
    assert_eq!(h.call("update_task", json!({"id": "nope", "difficulty": 2, "reason": "r"})).unwrap_err(), "No open task has that id.");
    assert_eq!(h.call("update_task", json!({"id": "t-essay", "reason": "r"})).unwrap_err(), "Give a difficulty, an estimate in minutes, or both.");
    let clamped = h.call("update_task", json!({"id": "t-essay", "estimate_min": 9000, "reason": "Long."})).unwrap();
    assert_eq!(clamped["clamped"], true);
    assert_eq!(clamped["task"]["estimate_min"], 600);
    assert_eq!(h.call("add_task", json!({"title": "x", "course": "BIO 100", "reason": "r"})).unwrap_err(), "No course has the code BIO 100.");
    assert_eq!(h.call("add_task", json!({"title": "x", "space": "Gym", "reason": "r"})).unwrap_err(), "No space is called Gym.");
    assert_eq!(h.call("log_focus", json!({"task_id": "nope", "minutes": 5, "reason": "r"})).unwrap_err(), "No task has that id.");
}

#[test]
fn with_no_library_every_call_says_to_open_the_app_and_nothing_is_made() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Wi_WWAV");
    let mut h = Helper::start(&root);
    assert_eq!(h.tools().len(), 23);
    assert_eq!(h.call("list_tasks", json!({})).unwrap_err(), "No Wi_WWAV library yet. Open the app once.");
    assert!(!root.exists(), "the helper never makes a library");
}

/// Kills the helper at 200 random moments while it writes. Every task Claude
/// added has its entry, and every entry its task: both or neither.
#[test]
fn a_kill_mid_call_leaves_the_change_and_its_entry_or_neither() {
    let (_dir, root) = seeded();
    let mut seed: u64 = 0x5eed_2026_1007;
    let mut rand = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for i in 0..200 {
        let mut h = Helper::start(&root);
        for j in 0..3 {
            let args = json!({"title": format!("task {i}.{j}"), "source_id": format!("kill-{i}-{j}"), "reason": "r"});
            h.send(json!({"jsonrpc": "2.0", "id": 100 + j, "method": "tools/call", "params": {"name": "add_task", "arguments": args}}));
        }
        std::thread::sleep(Duration::from_micros(rand() % 4_000));
        let _ = h.child.kill(); // SIGKILL
        let _ = h.child.wait();
    }
    let store = Store::open(&root).unwrap();
    let made = store.docs("task").unwrap().into_iter().filter(|d| d.json["source"] == "claude").count();
    let journaled = store
        .entries(true, 100_000)
        .unwrap()
        .into_iter()
        .filter(|e| e.done && matches!(&e.actor, Actor::Claude { tool, .. } if tool == "add_task"))
        .count();
    assert_eq!(made, journaled, "{made} tasks by Claude, {journaled} entries");
}

/// The helper and the app write at once, from two processes, 1,000 times
/// in all. None is lost, and every one has its entry.
#[test]
fn a_thousand_interleaved_writes_from_both_lose_none() {
    let (_dir, root) = seeded();
    let app_root = root.clone();
    let app = std::thread::spawn(move || {
        let mut store = Store::open(&app_root).unwrap();
        for i in 0..500 {
            let task = json!({"id": format!("app-{i}"), "spaceId": "sp-personal", "title": format!("errand {i}"), "type": "Errand",
                "due": null, "difficulty": 2, "estMin": null, "adjustMin": 0, "notes": "", "done": false, "doneAt": null, "source": "you"});
            let mut txn = store.begin(Room::Heat, "add task").unwrap();
            txn.put_doc("task", &format!("app-{i}"), &task, "errand").unwrap();
            txn.commit().unwrap();
        }
    });
    let mut h = Helper::start(&root);
    for i in 0..500 {
        h.call("add_task", json!({"title": format!("from mail {i}"), "source_id": format!("mail-{i}"), "reason": "r"})).unwrap();
    }
    app.join().unwrap();
    let store = Store::open(&root).unwrap();
    let tasks = store.docs("task").unwrap();
    assert_eq!(tasks.len(), 2 + 1000);
    let entries = store.entries(false, 100_000).unwrap();
    assert_eq!(entries.iter().filter(|e| e.label == "add task").count(), 500);
    assert_eq!(entries.iter().filter(|e| e.label == "Claude's task").count(), 500);
}
