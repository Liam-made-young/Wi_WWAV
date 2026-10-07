//! The protocol and the door, against a fake backend: what a client sees
//! before any store is involved (docs/SPEC.md 3.13, 8.8, 8.12).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};
use wi_mcp::{check, handle_line, is_date_time, serve, tools, Backend, ToolError};

#[derive(Default)]
struct Fake {
    off: BTreeSet<String>,
    calls: Vec<(String, Map<String, Value>)>,
    fail: BTreeMap<String, String>,
}

impl Backend for Fake {
    fn enabled(&mut self, tool: &str) -> bool {
        !self.off.contains(tool)
    }
    fn call(&mut self, tool: &str, args: &Map<String, Value>) -> Result<Value, ToolError> {
        self.calls.push((tool.to_string(), args.clone()));
        if let Some(sentence) = self.fail.get(tool) {
            return Err(ToolError::new(sentence.clone()));
        }
        Ok(json!({ "tool": tool, "undo_label": tools::undo_label(tool) }))
    }
}

fn ask(b: &mut Fake, message: Value) -> Value {
    handle_line(&message.to_string(), b).expect("a request gets an answer")
}

fn call(b: &mut Fake, tool: &str, args: Value) -> Value {
    ask(b, json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": tool, "arguments": args}}))
}

#[test]
fn initialize_answers_the_version_asked_for_when_known() {
    let mut b = Fake::default();
    let r = ask(&mut b, json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}}));
    assert_eq!(r["id"], 1);
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(r["result"]["serverInfo"]["name"], "wi-wwav");
    assert!(r["result"]["capabilities"]["tools"].is_object());

    let r = ask(&mut b, json!({"jsonrpc": "2.0", "id": 2, "method": "initialize", "params": {"protocolVersion": "1999-01-01"}}));
    assert_eq!(r["result"]["protocolVersion"], wi_mcp::PROTOCOL_VERSIONS[0]);
}

#[test]
fn notifications_and_responses_get_no_answer() {
    let mut b = Fake::default();
    assert!(handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, &mut b).is_none());
    assert!(handle_line(r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":3}}"#, &mut b).is_none());
    assert!(handle_line(r#"{"jsonrpc":"2.0","id":9,"result":{}}"#, &mut b).is_none());
}

#[test]
fn bad_lines_get_json_rpc_errors() {
    let mut b = Fake::default();
    assert_eq!(handle_line("{not json", &mut b).unwrap()["error"]["code"], -32700);
    assert_eq!(handle_line("[1,2]", &mut b).unwrap()["error"]["code"], -32600);
    let r = ask(&mut b, json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"}));
    assert_eq!(r["error"]["code"], -32601);
    let r = ask(&mut b, json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "delete_task"}}));
    assert_eq!(r["error"]["code"], -32602);
    assert_eq!(ask(&mut b, json!({"jsonrpc": "2.0", "id": 5, "method": "ping"}))["result"], json!({}));
}

#[test]
fn the_list_has_the_eight_tools_with_closed_schemas() {
    let mut b = Fake::default();
    let r = ask(&mut b, json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}));
    let list = r["result"]["tools"].as_array().unwrap();
    let names: Vec<_> = list.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, tools::NAMES);
    for t in list {
        let name = t["name"].as_str().unwrap();
        assert_eq!(t["inputSchema"]["type"], "object", "{name}");
        assert_eq!(t["inputSchema"]["additionalProperties"], false, "{name}");
        let props = t["inputSchema"]["properties"].as_object().unwrap();
        for forbidden in ["done", "score", "public", "delete", "due_date"] {
            assert!(!props.contains_key(forbidden), "{name} takes {forbidden}");
        }
        let writes = !matches!(name, "list_tasks" | "get_grades");
        assert_eq!(t["description"].as_str().unwrap().starts_with(tools::WRITE_PREAMBLE), writes, "{name}");
        assert_eq!(t["annotations"]["readOnlyHint"], tools::is_read_only(name), "{name}");
    }
    // Only add_task, update_task, add_pending_grade, log_focus and record_mail_thread journal.
    let labelled: Vec<_> = tools::NAMES.iter().filter(|t| tools::undo_label(t).is_some()).collect();
    assert_eq!(labelled.len(), 5);
}

#[test]
fn a_tool_switched_off_is_missing_and_refused() {
    let mut b = Fake::default();
    b.off.insert("log_focus".into());
    let r = ask(&mut b, json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}));
    let names: Vec<_> = r["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].clone()).collect();
    assert!(!names.contains(&json!("log_focus")));
    assert_eq!(names.len(), 7);
    let r = call(&mut b, "log_focus", json!({"task_id": "t1", "minutes": 25, "reason": "Worked on it."}));
    assert_eq!(r["result"]["isError"], true);
    assert_eq!(r["result"]["content"][0]["text"], "This tool is switched off in Wi_WWAV.");
    assert!(b.calls.is_empty(), "a switched-off tool never reaches the store");
}

#[test]
fn arguments_outside_the_table_never_reach_the_store() {
    let mut b = Fake::default();
    let refused = [
        ("update_task", json!({"id": "t1", "done": true, "reason": "r"})),
        ("add_pending_grade", json!({"course": "JPN 201", "item": "Quiz 4", "score": 9, "reason": "r"})),
        ("add_task", json!({"title": "x", "public": true, "reason": "r"})),
        ("add_task", json!({"title": "x"})),
        ("add_task", json!({"title": "   ", "reason": "r"})),
        ("add_task", json!({"title": "x", "reason": "r".repeat(201)})),
        ("update_task", json!({"id": "t1", "difficulty": 6, "reason": "r"})),
        ("update_task", json!({"id": "t1", "difficulty": 2.5, "reason": "r"})),
        ("log_focus", json!({"task_id": "t1", "minutes": 0, "reason": "r"})),
        ("log_focus", json!({"task_id": "t1", "minutes": 601, "reason": "r"})),
        ("record_mail_thread", json!({"thread_id": "a", "subject": "s", "from": "f", "received_at": "yesterday", "state": "grade", "reason": "r"})),
        ("record_mail_thread", json!({"thread_id": "a", "subject": "s", "from": "f", "received_at": "2026-10-07T09:00:00-04:00", "state": "spam", "reason": "r"})),
        ("plan_day", json!({"date": "10/07/2026"})),
        ("plan_day", json!({"day_ends": "24:00"})),
        ("list_tasks", json!({"limit": 201})),
        ("list_tasks", json!({"status": "deleted"})),
    ];
    for (tool, args) in refused {
        let r = call(&mut b, tool, args.clone());
        assert_eq!(r["result"]["isError"], true, "{tool} {args}");
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().ends_with('.'), "{tool}: one sentence");
    }
    assert!(b.calls.is_empty(), "{:?}", b.calls);
}

#[test]
fn good_calls_reach_the_store_and_answer_its_json() {
    let mut b = Fake::default();
    let ok = [
        ("list_tasks", json!({})),
        ("list_tasks", json!({"status": "all", "space": "Classes", "due_before": "2026-10-14T00:00:00-04:00", "limit": 200})),
        ("add_task", json!({"title": "Grammar quiz 4", "course": "JPN 201", "due": "2026-10-09T23:59:00-04:00",
                             "notes": "From mail: …", "source_id": "18f2a", "mail_thread_id": "18f29", "reason": "The notice gives a Friday deadline."})),
        ("update_task", json!({"id": "t1", "estimate_min": 9000, "reason": "Long."})),
        ("plan_day", json!({"date": "2026-10-07", "day_ends": "23:00"})),
        ("get_grades", json!({"course": "JPN 201"})),
        ("add_pending_grade", json!({"course": "JPN 201", "item": "Quiz 4", "posted_at": "2026-10-07T09:12:00Z", "reason": "A grade was posted."})),
        ("log_focus", json!({"task_id": "t1", "minutes": 25, "reason": "They said they worked on it."})),
        ("record_mail_thread", json!({"thread_id": "18f29", "subject": "Quiz 4 graded", "from": "D2L", "received_at": "2026-10-07T09:12:00.5-04:00",
                                      "state": "grade", "reason": "A grade notice."})),
    ];
    for (tool, args) in ok {
        let r = call(&mut b, tool, args.clone());
        assert_eq!(r["result"]["isError"], false, "{tool} {args} {r}");
        assert_eq!(r["result"]["structuredContent"]["tool"], tool);
        let text: Value = serde_json::from_str(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(text, r["result"]["structuredContent"]);
    }
    assert_eq!(b.calls.len(), 9);
    // update_task passes 9000 through: clamping 5–600 is the store's rule, in one place.
    assert_eq!(b.calls[3].1["estimate_min"], 9000);
}

#[test]
fn a_store_failure_is_one_sentence_with_the_flag_set() {
    let mut b = Fake::default();
    b.fail.insert("update_task".into(), "No open task has that id.".into());
    let r = call(&mut b, "update_task", json!({"id": "nope", "difficulty": 2, "reason": "r"}));
    assert_eq!(r["result"]["isError"], true);
    assert_eq!(r["result"]["content"][0]["text"], "No open task has that id.");
}

#[test]
fn serve_answers_line_by_line_and_stops_at_the_end_of_input() {
    let mut b = Fake::default();
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#, "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, "\n",
        "\n",
        r#"{"jsonrpc":"2.0","id":"two","method":"tools/list"}"#, "\n",
    );
    let mut out = Vec::new();
    serve(input.as_bytes(), &mut out, &mut b).unwrap();
    let lines: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[1]["id"], "two");
}

#[test]
fn dates_are_iso_8601_with_an_offset() {
    for good in ["2026-10-07T23:59:00-04:00", "2026-10-07T23:59-04:00", "2026-10-07T03:59:00Z", "2026-10-07T03:59:00.123+09:00"] {
        assert!(is_date_time(good), "{good}");
    }
    for bad in ["2026-10-07", "2026-10-07T23:59:00", "2026-10-07 23:59:00Z", "2026-10-07T24:00:00Z", "2026-10-07T23:59:00+0400", "２０２６-10-07T23:59:00Z"] {
        assert!(!is_date_time(bad), "{bad}");
    }
}

#[test]
fn update_task_needs_no_more_than_the_table_says() {
    // "At least one of the first two" is the store's check; the schema only
    // closes the door. An empty update reaches it and is refused there.
    assert!(check(&tools::input_schema("update_task"), json!({"id": "t", "reason": "r"}).as_object().unwrap()).is_ok());
}
