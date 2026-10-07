//! Noticing another handle's writes (docs/SPEC.md 8.8). The app and the MCP
//! helper write one file; the app asks SQLite whether another handle
//! committed (`data_version`), then reads the journal entries newer than the
//! last it saw, with the records each one changed. Fails if: a commit by
//! another handle doesn't change the answer, or a commit by the same handle
//! does; an entry doesn't name every record it changed, before and after;
//! entries come back in any order but oldest first; or `Txn::doc` reads
//! anything but the record as the open change finds it.

use serde_json::json;
use wi_store::*;

fn library() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Wi_WWAV");
    Store::open(&root).unwrap();
    (dir, root)
}

fn put(
    store: &mut Store,
    label: &str,
    actor: Actor,
    kind: &str,
    key: &str,
    value: serde_json::Value,
) -> String {
    let mut txn = store.begin_by(Room::Heat, label, actor).unwrap();
    txn.put_doc(kind, key, &value, "").unwrap();
    txn.commit().unwrap().expect("a change is journaled")
}

#[test]
fn another_handles_commit_changes_data_version_and_ones_own_does_not() {
    let (_dir, root) = library();
    let mut app = Store::open(&root).unwrap();
    let mut helper = Store::open(&root).unwrap();
    let seen = app.data_version().unwrap();
    put(
        &mut app,
        "add task",
        Actor::You,
        "task",
        "t1",
        json!({"title": "own"}),
    );
    assert_eq!(
        app.data_version().unwrap(),
        seen,
        "a commit through this handle is not news to it"
    );
    put(
        &mut helper,
        "Claude's task",
        Actor::Claude {
            tool: "add_task".into(),
            reason: "r".into(),
        },
        "task",
        "t2",
        json!({"title": "theirs"}),
    );
    assert_ne!(
        app.data_version().unwrap(),
        seen,
        "another handle's commit is"
    );
    let after = app.data_version().unwrap();
    assert_eq!(
        app.data_version().unwrap(),
        after,
        "and it stays put until the next one"
    );
}

#[test]
fn entries_after_name_the_records_each_changed_oldest_first() {
    let (_dir, root) = library();
    let mut store = Store::open(&root).unwrap();
    assert_eq!(store.last_entry().unwrap(), None);
    let a = put(
        &mut store,
        "add task",
        Actor::You,
        "task",
        "t1",
        json!({"title": "one", "estMin": null}),
    );
    let b = put(
        &mut store,
        "Claude's estimate",
        Actor::Claude {
            tool: "update_task".into(),
            reason: "Two pages.".into(),
        },
        "task",
        "t1",
        json!({"title": "one", "estMin": 45}),
    );
    let mut txn = store.begin(Room::Heat, "add two").unwrap();
    txn.put_doc("note", "n1", &json!({"markdown": "x"}), "")
        .unwrap();
    txn.put_doc("capture", "c1", &json!({"text": "y"}), "")
        .unwrap();
    let c = txn.commit().unwrap().unwrap();
    assert_eq!(store.last_entry().unwrap().as_deref(), Some(c.as_str()));

    let all = store.entries_after(None, 100).unwrap();
    assert_eq!(
        all.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        [a.as_str(), b.as_str(), c.as_str()]
    );
    assert_eq!(
        all[0].docs,
        [DocChange {
            kind: "task".into(),
            key: "t1".into(),
            before: None,
            after: Some(json!({"title": "one", "estMin": null}))
        }]
    );
    assert_eq!(
        all[1].docs[0].before,
        Some(json!({"title": "one", "estMin": null}))
    );
    assert_eq!(
        all[1].docs[0].after,
        Some(json!({"title": "one", "estMin": 45}))
    );
    assert_eq!(
        all[1].actor,
        Actor::Claude {
            tool: "update_task".into(),
            reason: "Two pages.".into()
        }
    );
    let kinds: Vec<&str> = all[2].docs.iter().map(|d| d.kind.as_str()).collect();
    assert_eq!(kinds, ["note", "capture"]);

    // Only what is newer than the last seen, and a limit.
    let newer = store.entries_after(Some(&a), 100).unwrap();
    assert_eq!(newer.len(), 2);
    assert_eq!(store.entries_after(Some(&a), 1).unwrap().len(), 1);
    assert!(store.entries_after(Some(&c), 100).unwrap().is_empty());
    assert_eq!(
        store.entry_docs(&b).unwrap().unwrap().label,
        "Claude's estimate"
    );
    assert!(store.entry_docs("nope").unwrap().is_none());
}

#[test]
fn a_record_touched_twice_in_one_change_shows_its_first_before_and_last_after() {
    let (_dir, root) = library();
    let mut store = Store::open(&root).unwrap();
    put(
        &mut store,
        "add task",
        Actor::You,
        "task",
        "t1",
        json!({"title": "start"}),
    );
    let mut txn = store.begin(Room::Heat, "two edits").unwrap();
    txn.put_doc("task", "t1", &json!({"title": "middle"}), "")
        .unwrap();
    txn.put_doc("task", "t1", &json!({"title": "end"}), "")
        .unwrap();
    txn.delete_doc("task", "t1").unwrap();
    txn.put_doc("task", "t1", &json!({"title": "back"}), "")
        .unwrap();
    let id = txn.commit().unwrap().unwrap();
    let entry = store.entry_docs(&id).unwrap().unwrap();
    assert_eq!(entry.docs.len(), 1);
    assert_eq!(entry.docs[0].before, Some(json!({"title": "start"})));
    assert_eq!(entry.docs[0].after, Some(json!({"title": "back"})));
}

#[test]
fn a_change_reads_a_record_as_it_stands_inside_the_change() {
    let (_dir, root) = library();
    let mut app = Store::open(&root).unwrap();
    let mut helper = Store::open(&root).unwrap();
    put(
        &mut app,
        "add task",
        Actor::You,
        "task",
        "t1",
        json!({"title": "one"}),
    );
    let mut txn = app.begin(Room::Heat, "edit task").unwrap();
    assert_eq!(
        txn.doc("task", "t1").unwrap().unwrap().json,
        json!({"title": "one"})
    );
    assert!(txn.doc("task", "missing").unwrap().is_none());
    txn.put_doc("task", "t1", &json!({"title": "two"}), "two")
        .unwrap();
    assert_eq!(
        txn.doc("task", "t1").unwrap().unwrap().json,
        json!({"title": "two"}),
        "it sees its own writes"
    );
    txn.commit().unwrap();
    // The helper's change is whole before the app's begins, so the app reads it.
    put(
        &mut helper,
        "Claude's task",
        Actor::Claude {
            tool: "add_task".into(),
            reason: "r".into(),
        },
        "task",
        "t1",
        json!({"title": "three"}),
    );
    let txn = app.begin(Room::Heat, "edit again").unwrap();
    assert_eq!(
        txn.doc("task", "t1").unwrap().unwrap().json,
        json!({"title": "three"})
    );
}

#[test]
fn docs_come_back_in_the_order_they_were_made_and_undo_keeps_each_place() {
    let (_dir, root) = library();
    let mut store = Store::open(&root).unwrap();
    for key in ["classes", "wwav", "personal"] {
        put(
            &mut store,
            "add space",
            Actor::You,
            "space",
            key,
            json!({"name": key}),
        );
    }
    let names = |s: &Store| {
        s.docs_oldest_first("space")
            .unwrap()
            .into_iter()
            .map(|d| d.key)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&store), ["classes", "wwav", "personal"]);
    assert_eq!(
        store
            .docs("space")
            .unwrap()
            .into_iter()
            .map(|d| d.key)
            .collect::<Vec<_>>(),
        ["classes", "personal", "wwav"],
        "docs() stays by key"
    );
    let mut txn = store.begin(Room::Heat, "delete space").unwrap();
    txn.delete_doc("space", "wwav").unwrap();
    txn.commit().unwrap();
    assert_eq!(names(&store), ["classes", "personal"]);
    store.undo(Room::Heat).unwrap();
    assert_eq!(
        names(&store),
        ["classes", "wwav", "personal"],
        "undo puts it back where it was"
    );
}
