//! Several entries become one undo step (merge.rs): a batch of Learn's
//! one-entry writes is a single ⌘Z, and undoing and redoing it is exact.

use serde_json::{json, Value};
use wi_store::*;

fn library() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("Wi_WWAV")).unwrap();
    (dir, store)
}

fn put(store: &mut Store, room: Room, label: &str, key: &str, value: Value) -> String {
    let mut txn = store.begin(room, label).unwrap();
    txn.put_doc("task", key, &value, "").unwrap();
    txn.commit().unwrap().expect("a change is journaled")
}

fn due(store: &Store, key: &str) -> Option<Value> {
    store
        .doc("task", key)
        .unwrap()
        .map(|d| d.json["due"].clone())
}

fn refusal(r: Result<Option<String>>) -> String {
    match r {
        Err(Error::Refused(why)) => why,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_batch_is_one_undo_and_one_redo() {
    let (_dir, mut store) = library();
    put(&mut store, Room::Heat, "add task", "a", json!({"due": 1}));
    put(&mut store, Room::Heat, "add task", "b", json!({"due": 1}));
    let ids = vec![
        put(&mut store, Room::Heat, "edit task", "a", json!({"due": 2})),
        put(&mut store, Room::Heat, "edit task", "b", json!({"due": 2})),
        put(&mut store, Room::Heat, "add task", "c", json!({"due": 2})),
        // The same record twice in one batch: its first before and last after are what count.
        put(&mut store, Room::Heat, "edit task", "a", json!({"due": 3})),
    ];
    let merged = store
        .merge_entries(&ids, "Claude: 2 tasks moved, 1 added")
        .unwrap();
    assert_eq!(merged.as_deref(), Some(ids[3].as_str()));
    assert_eq!(
        store.history(Room::Heat).unwrap().undo_text(),
        "Undo Claude: 2 tasks moved, 1 added"
    );

    assert_eq!(
        store.undo(Room::Heat).unwrap().as_deref(),
        Some("Claude: 2 tasks moved, 1 added")
    );
    assert_eq!(due(&store, "a"), Some(json!(1)));
    assert_eq!(due(&store, "b"), Some(json!(1)));
    assert_eq!(due(&store, "c"), None);
    // What came before the batch is the next thing ⌘Z reaches.
    assert_eq!(
        store.history(Room::Heat).unwrap().undo_text(),
        "Undo add task"
    );

    assert_eq!(
        store.redo(Room::Heat).unwrap().as_deref(),
        Some("Claude: 2 tasks moved, 1 added")
    );
    assert_eq!(due(&store, "a"), Some(json!(3)));
    assert_eq!(due(&store, "b"), Some(json!(2)));
    assert_eq!(due(&store, "c"), Some(json!(2)));
}

#[test]
fn the_joined_entry_lists_every_record_it_changed() {
    let (_dir, mut store) = library();
    let ids = vec![
        put(&mut store, Room::Heat, "add task", "a", json!({"due": 1})),
        put(&mut store, Room::Heat, "add task", "b", json!({"due": 1})),
    ];
    let merged = store.merge_entries(&ids, "paste").unwrap().unwrap();
    let entry = store.entry_docs(&merged).unwrap().unwrap();
    assert_eq!(entry.label, "paste");
    let keys: Vec<&str> = entry.docs.iter().map(|d| d.key.as_str()).collect();
    assert_eq!(keys, ["a", "b"]);
    assert!(store.entry_docs(&ids[0]).unwrap().is_none());
}

#[test]
fn one_entry_is_only_relabelled_and_none_is_nothing() {
    let (_dir, mut store) = library();
    let id = put(&mut store, Room::Heat, "edit task", "a", json!({"due": 1}));
    assert_eq!(store.merge_entries(&[], "x").unwrap(), None);
    assert_eq!(
        store.merge_entries(&[id.clone()], "fill down").unwrap(),
        Some(id)
    );
    assert_eq!(
        store.history(Room::Heat).unwrap().undo_text(),
        "Undo fill down"
    );
}

#[test]
fn entries_with_something_between_them_stay_separate() {
    let (_dir, mut store) = library();
    let first = put(&mut store, Room::Heat, "edit task", "a", json!({"due": 1}));
    let _other = put(&mut store, Room::Heat, "edit task", "z", json!({"due": 9}));
    let last = put(&mut store, Room::Heat, "edit task", "b", json!({"due": 1}));
    let why = refusal(store.merge_entries(&[first, last], "paste"));
    assert!(why.contains("Something else changed in between"), "{why}");
    // Nothing was joined: three steps, as before.
    for _ in 0..3 {
        assert_eq!(
            store.undo(Room::Heat).unwrap().as_deref(),
            Some("edit task")
        );
    }
    assert_eq!(store.undo(Room::Heat).unwrap(), None);
}

#[test]
fn undone_entries_and_entries_from_two_views_are_refused() {
    let (_dir, mut store) = library();
    let a = put(&mut store, Room::Heat, "edit task", "a", json!({"due": 1}));
    let b = put(&mut store, Room::Space, "edit task", "b", json!({"due": 1}));
    let why = refusal(store.merge_entries(&[a.clone(), b], "paste"));
    assert!(why.contains("different views"), "{why}");

    let c = put(&mut store, Room::Heat, "edit task", "c", json!({"due": 1}));
    store.undo(Room::Heat).unwrap();
    let why = refusal(store.merge_entries(&[a.clone(), c], "paste"));
    assert!(why.contains("undone already"), "{why}");
    let why = refusal(store.merge_entries(&[a, "01ZZZZZZZZZZZZZZZZZZZZZZZZ".into()], "paste"));
    assert!(why.contains("isn't in the journal"), "{why}");
}

#[test]
fn undoing_everything_after_a_merge_returns_the_first_state() {
    let (_dir, mut store) = library();
    let mut ids = Vec::new();
    for n in 0..25 {
        let key = format!("t{}", n % 5);
        ids.push(put(
            &mut store,
            Room::Heat,
            "edit task",
            &key,
            json!({"due": n}),
        ));
    }
    store.merge_entries(&ids[5..20], "move 15").unwrap();
    let mut steps = 0;
    while store.undo(Room::Heat).unwrap().is_some() {
        steps += 1;
    }
    // Five before, the joined one, five after.
    assert_eq!(steps, 11);
    assert!(store.docs("task").unwrap().is_empty());
    while store.redo(Room::Heat).unwrap().is_some() {}
    for k in 0..5 {
        assert_eq!(due(&store, &format!("t{k}")), Some(json!(20 + k)));
    }
}
