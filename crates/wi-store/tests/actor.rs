//! Who made each change (docs/SPEC.md 3.12, 3.16, 8.8; docs/HEAT.md). A
//! change Claude makes through an MCP tool is one journal entry like any
//! other, labelled as Claude's, with its tool and reason, and ⌘Z undoes it.
//! Settings → Claude undoes one of Claude's entries out of order, unless a
//! later change touched the same values.

use rusqlite::Connection;
use serde_json::json;
use wi_store::*;

fn library() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("Wi_WWAV")).unwrap();
    (dir, store)
}

fn claude(tool: &str, reason: &str) -> Actor {
    Actor::Claude { tool: tool.into(), reason: reason.into() }
}

fn put(store: &mut Store, label: &str, actor: Actor, key: &str, value: serde_json::Value) -> String {
    let mut txn = store.begin_by(Room::Heat, label, actor).unwrap();
    txn.put_doc("task", key, &value, value["title"].as_str().unwrap_or("")).unwrap();
    txn.commit().unwrap().expect("a change is journaled")
}

fn title(store: &Store, key: &str) -> Option<String> {
    store.doc("task", key).unwrap().map(|d| d.json["title"].as_str().unwrap().to_string())
}

#[test]
fn claudes_change_reads_as_claudes_in_the_edit_menu() {
    let (_dir, mut store) = library();
    put(&mut store, "add task", Actor::You, "t1", json!({"title": "Quiz 4", "estMin": null}));
    put(&mut store, "Claude's estimate", claude("update_task", "It read the title and your averages."), "t1",
        json!({"title": "Quiz 4", "estMin": 45}));
    assert_eq!(store.history(Room::Heat).unwrap().undo_text(), "Undo Claude's estimate");
    assert_eq!(store.undo(Room::Heat).unwrap().as_deref(), Some("Claude's estimate"));
    assert_eq!(store.doc("task", "t1").unwrap().unwrap().json["estMin"], json!(null));
}

#[test]
fn entries_name_the_actor_tool_and_reason_newest_first() {
    let (_dir, mut store) = library();
    put(&mut store, "add task", Actor::You, "t1", json!({"title": "Quiz 4"}));
    put(&mut store, "Claude's task", claude("add_task", "The notice gives a Friday deadline."), "t2", json!({"title": "Lab 5a"}));
    put(&mut store, "Claude's estimate", claude("update_task", "Two pages of reading."), "t2", json!({"title": "Lab 5a", "estMin": 30}));

    let all = store.entries(false, 10).unwrap();
    assert_eq!(all.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(), ["Claude's estimate", "Claude's task", "add task"]);
    assert_eq!(all[2].actor, Actor::You);

    let mine = store.entries(true, 10).unwrap();
    assert_eq!(mine.len(), 2);
    assert_eq!(mine[1].actor, claude("add_task", "The notice gives a Friday deadline."));
    assert!(mine.iter().all(|e| e.done && e.room == Room::Heat && e.at_ms > 1_700_000_000_000));
}

#[test]
fn one_of_claudes_entries_undoes_out_of_order_when_nothing_later_touched_it() {
    let (_dir, mut store) = library();
    let added = put(&mut store, "Claude's task", claude("add_task", "From mail."), "t1", json!({"title": "Grammar quiz 4"}));
    put(&mut store, "add task", Actor::You, "t2", json!({"title": "Mix the second verse"}));

    assert_eq!(store.undo_entry(&added).unwrap(), "Claude's task");
    assert_eq!(title(&store, "t1"), None, "Claude's task is gone");
    assert_eq!(title(&store, "t2").as_deref(), Some("Mix the second verse"), "the later change stays");
    assert_eq!(store.history(Room::Heat).unwrap().undo_text(), "Undo add task");

    let again = store.undo_entry(&added).unwrap_err().to_string();
    assert_eq!(again, "Claude's task is already undone.");
    let gone = store.undo_entry("01ZZZZZZZZZZZZZZZZZZZZZZZZ").unwrap_err().to_string();
    assert_eq!(gone, "That change isn't in the journal any more.");
}

#[test]
fn an_entry_changed_again_since_is_refused_with_the_sentence() {
    let (_dir, mut store) = library();
    let estimate = put(&mut store, "Claude's estimate", claude("update_task", "Long."), "t1", json!({"title": "Essay", "estMin": 120}));
    put(&mut store, "estimate", Actor::You, "t1", json!({"title": "Essay", "estMin": 90}));
    let refused = store.undo_entry(&estimate).unwrap_err().to_string();
    assert_eq!(refused, "This changed again since. Undo the later change first.");
    assert_eq!(store.doc("task", "t1").unwrap().unwrap().json["estMin"], json!(90), "nothing moved");
}

#[test]
fn a_library_from_before_reads_every_old_entry_as_yours() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Wi_WWAV");
    {
        let mut store = Store::open(&root).unwrap();
        put(&mut store, "add task", Actor::You, "t1", json!({"title": "Quiz 4"}));
    }
    // Take the file back to the first layout, as an older app left it.
    {
        let conn = Connection::open(root.join("library.sqlite")).unwrap();
        conn.execute_batch(
            "DROP INDEX txn_actor;
             CREATE TABLE txn_old (id TEXT PRIMARY KEY, label TEXT NOT NULL, room TEXT NOT NULL, state TEXT NOT NULL);
             INSERT INTO txn_old SELECT id, label, room, state FROM txn;
             PRAGMA foreign_keys = OFF;
             DROP TABLE txn;
             ALTER TABLE txn_old RENAME TO txn;
             CREATE INDEX txn_room ON txn(room, state, id);
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }
    let store = Store::open(&root).unwrap();
    let all = store.entries(false, 10).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].actor, Actor::You);
    assert_eq!(SCHEMA_VERSION, 2);
}

#[test]
fn the_journal_refuses_an_actor_it_doesnt_know() {
    let (dir, mut store) = library();
    put(&mut store, "add task", Actor::You, "t1", json!({"title": "Quiz 4"}));
    let conn = Connection::open(dir.path().join("Wi_WWAV/library.sqlite")).unwrap();
    let err = conn.execute("UPDATE txn SET actor = 'someone'", []).unwrap_err().to_string();
    assert!(err.contains("CHECK"), "{err}");
}
