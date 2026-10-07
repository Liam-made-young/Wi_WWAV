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
    for kind in ["space", "task", "term", "course", "grade", "focusSession", "mailThread"] {
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
    assert_eq!(h.tools().len(), 8);

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
    assert_eq!(h.tools().len(), 8);
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
