//! What a change touched: the parts of the session that differ, each as a
//! JSON pointer (RFC 6901) with its value before and after, like a
//! `txn_row` with a pointer for its table and row (`docs/SPEC.md` 9.6).
//!
//! Rows are applied in order to redo and in reverse to undo. A row
//! without `before` is an insert (the key or array element didn't exist);
//! one without `after` is a removal. Arrays that change length record only
//! the elements that came or went, so adding a note to a long clip records
//! one note, not the clip.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub path: String,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub before: Option<Value>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub after: Option<Value>,
}

/// A present key is Some, even when its value is null; only an absent key
/// is None.
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

/// A row that doesn't fit the document it is applied to: the journal and
/// the session have parted.
#[derive(Debug, Clone, PartialEq)]
pub struct Mismatch(pub String);

/// The rows that turn `a` into `b`.
pub fn diff(a: &Value, b: &Value) -> Vec<Row> {
    let mut rows = Vec::new();
    walk(String::new(), a, b, &mut rows);
    rows
}

fn walk(path: String, a: &Value, b: &Value, rows: &mut Vec<Row>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, va) in x {
                let p = format!("{path}/{}", escape(k));
                match y.get(k) {
                    Some(vb) => walk(p, va, vb, rows),
                    None => rows.push(row(p, Some(va), None)),
                }
            }
            for (k, vb) in y {
                if !x.contains_key(k) {
                    rows.push(row(format!("{path}/{}", escape(k)), None, Some(vb)));
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            // Keep what both ends share; pair up the middle element by
            // element, then insert or remove the rest at the first unpaired
            // index. Removals all name that index: each one shifts the next
            // into its place, and undo puts them back in reverse.
            let prefix = x.iter().zip(y).take_while(|(p, q)| p == q).count();
            let room = x.len().min(y.len()) - prefix;
            let suffix = x
                .iter()
                .rev()
                .zip(y.iter().rev())
                .take(room)
                .take_while(|(p, q)| p == q)
                .count();
            let (mx, my) = (&x[prefix..x.len() - suffix], &y[prefix..y.len() - suffix]);
            let paired = mx.len().min(my.len());
            for i in 0..paired {
                walk(format!("{path}/{}", prefix + i), &mx[i], &my[i], rows);
            }
            let at = prefix + paired;
            for (j, v) in my[paired..].iter().enumerate() {
                rows.push(row(format!("{path}/{}", at + j), None, Some(v)));
            }
            for v in &mx[paired..] {
                rows.push(row(format!("{path}/{at}"), Some(v), None));
            }
        }
        _ => rows.push(row(path, Some(a), Some(b))),
    }
}

fn row(path: String, before: Option<&Value>, after: Option<&Value>) -> Row {
    Row {
        path,
        before: before.cloned(),
        after: after.cloned(),
    }
}

/// Applies rows forward: what was `before` becomes `after`.
pub fn redo(doc: &mut Value, rows: &[Row]) -> Result<(), Mismatch> {
    rows.iter()
        .try_for_each(|r| put(doc, &r.path, r.before.as_ref(), r.after.as_ref()))
}

/// Applies rows backward: what is `after` goes back to `before`.
pub fn undo(doc: &mut Value, rows: &[Row]) -> Result<(), Mismatch> {
    rows.iter()
        .rev()
        .try_for_each(|r| put(doc, &r.path, r.after.as_ref(), r.before.as_ref()))
}

/// Replaces, inserts or removes the value at `path`, checking first that
/// what is there is `from`.
fn put(
    doc: &mut Value,
    path: &str,
    from: Option<&Value>,
    to: Option<&Value>,
) -> Result<(), Mismatch> {
    let bad = || Mismatch(path.to_string());
    let Some((parent, last)) = path.rsplit_once('/') else {
        // The whole document.
        return match (from, to) {
            (Some(f), Some(t)) if path.is_empty() && doc == f => {
                *doc = t.clone();
                Ok(())
            }
            _ => Err(bad()),
        };
    };
    let key = unescape(last);
    match doc.pointer_mut(parent).ok_or_else(bad)? {
        Value::Object(m) => match (from, to) {
            (Some(f), Some(t)) => match m.get_mut(&key) {
                Some(cur) if cur == f => *cur = t.clone(),
                _ => return Err(bad()),
            },
            (None, Some(t)) if !m.contains_key(&key) => {
                m.insert(key, t.clone());
            }
            (Some(f), None) if m.get(&key) == Some(f) => {
                m.remove(&key);
            }
            _ => return Err(bad()),
        },
        Value::Array(a) => {
            let i: usize = key.parse().map_err(|_| bad())?;
            match (from, to) {
                (Some(f), Some(t)) => match a.get_mut(i) {
                    Some(cur) if cur == f => *cur = t.clone(),
                    _ => return Err(bad()),
                },
                (None, Some(t)) if i <= a.len() => a.insert(i, t.clone()),
                (Some(f), None) if a.get(i) == Some(f) => {
                    a.remove(i);
                }
                _ => return Err(bad()),
            }
        }
        _ => return Err(bad()),
    }
    Ok(())
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    #[test]
    fn one_note_added_to_a_long_clip_is_one_row() {
        let notes: Vec<Value> = (0..1000)
            .map(|i| json!({"pitch": i % 128, "at": i}))
            .collect();
        let a = json!({"midi": {"C": notes}});
        let mut b = a.clone();
        b["midi"]["C"]
            .as_array_mut()
            .unwrap()
            .insert(500, json!({"pitch": 1, "at": 0.5}));
        let rows = diff(&a, &b);
        assert_eq!(
            rows,
            vec![Row {
                path: "/midi/C/500".into(),
                before: None,
                after: Some(json!({"pitch": 1, "at": 0.5}))
            }]
        );
    }

    #[test]
    fn a_moved_clip_is_one_number() {
        let a = json!({"tracks": [{"events": [{"at_ms": 0}, {"at_ms": 1000}, {"at_ms": 2000}]}]});
        let mut b = a.clone();
        b["tracks"][0]["events"][1]["at_ms"] = json!(1500);
        let rows = diff(&a, &b);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, "/tracks/0/events/1/at_ms");
    }

    #[test]
    fn keys_with_slashes_and_tildes_are_escaped() {
        let a = json!({"a/b": {"~c": 1}});
        let b = json!({"a/b": {"~c": 2}});
        let rows = diff(&a, &b);
        assert_eq!(rows[0].path, "/a~1b/~0c");
        let mut x = a.clone();
        redo(&mut x, &rows).unwrap();
        assert_eq!(x, b);
    }

    #[test]
    fn null_is_a_value_and_absence_is_not() {
        let a = json!({"frame": null});
        let b = json!({"frame": {"w": 1}, "key": null});
        let rows = diff(&a, &b);
        let line = serde_json::to_string(&rows).unwrap();
        let back: Vec<Row> = serde_json::from_str(&line).unwrap();
        assert_eq!(back, rows);
        assert_eq!(back[0].before, Some(Value::Null));
        assert_eq!(back[1].before, None);
        assert_eq!(back[1].after, Some(Value::Null));
    }

    #[test]
    fn rows_that_dont_fit_are_refused() {
        let a = json!({"x": [1, 2, 3]});
        let rows = diff(&a, &json!({"x": [1, 3]}));
        let mut other = json!({"x": [1, 5, 3]});
        assert!(redo(&mut other, &rows).is_err());
        let mut gone = json!({});
        assert!(redo(&mut gone, &rows).is_err());
    }

    fn json_value() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::from),
            (0..4i64).prop_map(Value::from),
            "[ab/~]{0,2}".prop_map(Value::from),
        ];
        leaf.prop_recursive(4, 40, 6, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
                prop::collection::btree_map("[abc~/]{1,2}", inner, 0..5)
                    .prop_map(|m| Value::Object(m.into_iter().collect())),
            ]
        })
    }

    proptest! {
        #[test]
        fn redo_turns_a_into_b_and_undo_turns_it_back(a in json_value(), b in json_value()) {
            let rows = diff(&a, &b);
            let mut x = a.clone();
            redo(&mut x, &rows).unwrap();
            prop_assert_eq!(&x, &b);
            undo(&mut x, &rows).unwrap();
            prop_assert_eq!(&x, &a);
        }
    }
}
