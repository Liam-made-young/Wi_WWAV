//! S2.7 (docs/PLAN.md): Heat sync fails if a slower older write overwrites a
//! newer one on any field.
//!
//! "Newer" is judged here without trusting the code under test: the harness
//! keeps a vector clock per device, so write B is newer than write A exactly
//! when B's device had seen A (or made it) before writing B. Writes neither
//! device had seen are concurrent, and either may win, as long as every
//! replica picks the same one.

use proptest::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use wi_heat::sync::{Change, Replica, Server};

#[test]
fn a_slow_older_push_never_overwrites_a_newer_one() {
    let mut mac = Replica::new("mac");
    let mut server = Server::default();

    mac.write("task", "t1", "title", json!("Mix the first verse"));
    let slow = mac.push_batch();
    mac.write("task", "t1", "title", json!("Mix the second verse"));
    let fast = mac.push_batch();

    server.push(&fast);
    mac.acked(&fast);
    server.push(&slow); // the older request lands last
    mac.acked(&slow);

    let page = server.pull(0, 100);
    assert_eq!(page.changes.len(), 1);
    assert_eq!(page.changes[0].value, json!("Mix the second verse"));
    assert!(!mac.has_pending());
}

#[test]
fn a_late_ack_keeps_a_newer_local_write_pending() {
    let mut mac = Replica::new("mac");
    mac.write("task", "t1", "notes", json!("a"));
    let batch = mac.push_batch();
    mac.write("task", "t1", "notes", json!("b"));
    mac.acked(&batch);
    assert!(mac.has_pending(), "\"b\" has not gone up yet");
    assert_eq!(mac.push_batch()[0].value, json!("b"));
}

#[test]
fn a_pulled_older_value_never_replaces_a_newer_local_one() {
    let mut mac = Replica::new("mac");
    // "zed" sorts after "mac", so only the counter can make mac's later write win.
    let mut zed = Replica::new("zed");
    let mut server = Server::default();

    zed.write("task", "t1", "due", json!("2026-10-08"));
    let b = zed.push_batch();
    server.push(&b);
    zed.acked(&b);

    pull_all(&mut mac, &server, 10);
    mac.write("task", "t1", "due", json!("2026-10-09")); // made after seeing zed's
    let m = mac.push_batch();
    server.push(&m);
    mac.acked(&m);

    pull_all(&mut zed, &server, 10);
    pull_all(&mut mac, &server, 10);
    assert_eq!(zed.value("task", "t1", "due"), Some(&json!("2026-10-09")));
    assert_eq!(mac.value("task", "t1", "due"), Some(&json!("2026-10-09")));
}

#[test]
fn concurrent_writes_with_equal_numbers_go_to_the_higher_device_id() {
    let mut a = Replica::new("device-a");
    let mut b = Replica::new("device-b");
    a.write("habit", "h1", "title", json!("Practise kanji"));
    b.write("habit", "h1", "title", json!("Practise kanji, 20m"));
    let (pa, pb) = (a.push_batch(), b.push_batch());
    assert_eq!(pa[0].seq, pb[0].seq);

    for order in [[&pa, &pb], [&pb, &pa]] {
        let mut server = Server::default();
        server.push(order[0]);
        server.push(order[1]);
        assert_eq!(
            server.pull(0, 10).changes[0].value,
            json!("Practise kanji, 20m")
        );
    }
}

#[test]
fn changes_come_in_pages_after_the_cursor() {
    let mut mac = Replica::new("mac");
    let mut server = Server::default();
    for n in 0..5 {
        mac.write("task", &format!("t{n}"), "title", json!(n));
    }
    let batch = mac.push_batch();
    server.push(&batch);

    let first = server.pull(0, 2);
    assert_eq!(first.changes.len(), 2);
    assert!(first.more);
    let second = server.pull(first.cursor, 2);
    let third = server.pull(second.cursor, 2);
    assert_eq!(third.changes.len(), 1);
    assert!(!third.more);
    assert!(server.pull(third.cursor, 2).changes.is_empty());
}

#[test]
fn pushes_go_up_in_batches() {
    let mut mac = Replica::new("mac");
    let n = wi_heat::sync::PUSH_BATCH * 2 + 3;
    for i in 0..n {
        mac.write("task", &format!("t{i}"), "title", json!(i));
    }
    let mut server = Server::default();
    let mut batches = 0;
    while mac.has_pending() {
        let batch = mac.push_batch();
        assert!(batch.len() <= wi_heat::sync::PUSH_BATCH);
        server.push(&batch);
        mac.acked(&batch);
        batches += 1;
    }
    assert_eq!(batches, 3);
    assert_eq!(server.pull(0, n + 1).changes.len(), n);

    // A long field still goes up, even alone over the byte budget.
    let mut big = Replica::new("big");
    big.write(
        "note",
        "2026-10-06",
        "markdown",
        json!("x".repeat(200 * 1024)),
    );
    big.write("note", "2026-10-07", "markdown", json!("y"));
    assert_eq!(big.push_batch().len(), 1);
}

fn pull_all(r: &mut Replica, server: &Server, limit: usize) {
    loop {
        let page = server.pull(r.cursor(), limit);
        let more = page.more;
        r.pulled(page);
        if !more {
            break;
        }
    }
}

// ----- the property test -----

const DEVICES: [&str; 3] = ["air", "mac", "studio"];

#[derive(Clone, Debug)]
enum Op {
    Write { dev: usize, rec: u8, field: u8 },
    Push { dev: usize, slow: bool },
    Land { pick: usize },
    Pull { dev: usize, limit: usize },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (0..3usize, 0..3u8, 0..2u8).prop_map(|(dev, rec, field)| Op::Write { dev, rec, field }),
        2 => (0..3usize, any::<bool>()).prop_map(|(dev, slow)| Op::Push { dev, slow }),
        2 => any::<usize>().prop_map(|pick| Op::Land { pick }),
        2 => (0..3usize, 1..4usize).prop_map(|(dev, limit)| Op::Pull { dev, limit }),
    ]
}

type Vc = [u32; 3];
type Key = (String, String, String);

fn before(a: &Vc, b: &Vc) -> bool {
    a != b && a.iter().zip(b).all(|(x, y)| x <= y)
}

struct World {
    replicas: Vec<Replica>,
    server: Server,
    in_flight: Vec<(usize, Vec<Change>)>,
    vc: [Vc; 3],
    /// Every write ever made, by its unique value: its key and vector clock.
    writes: HashMap<String, (Key, Vc)>,
    made: u32,
}

impl World {
    fn new() -> Self {
        World {
            replicas: DEVICES.iter().map(|d| Replica::new(d)).collect(),
            server: Server::default(),
            in_flight: Vec::new(),
            vc: [[0; 3]; 3],
            writes: HashMap::new(),
            made: 0,
        }
    }

    fn vc_of(&self, v: &Value) -> Vc {
        self.writes[v.as_str().unwrap()].1
    }

    /// Whatever a store held before and after a step, no field may move from
    /// a write to one that write had already seen.
    fn check_step(&self, who: &str, old: &BTreeMap<Key, Value>, new: &BTreeMap<Key, Value>) {
        for (key, was) in old {
            let now = new.get(key).expect("a field never disappears");
            if now != was {
                assert!(
                    !before(&self.vc_of(now), &self.vc_of(was)),
                    "{who}: {key:?} went from {was} back to the older {now}"
                );
            }
        }
    }

    fn land(&mut self, dev: usize, batch: Vec<Change>) {
        let old = self.server.snapshot();
        self.server.push(&batch);
        let new = self.server.snapshot();
        self.check_step("server", &old, &new);
        self.replicas[dev].acked(&batch);
    }

    fn pull(&mut self, dev: usize, limit: usize) {
        loop {
            let page = self.server.pull(self.replicas[dev].cursor(), limit);
            for c in &page.changes {
                let seen = self.vc_of(&c.value);
                for (mine, theirs) in self.vc[dev].iter_mut().zip(seen) {
                    *mine = (*mine).max(theirs);
                }
            }
            let more = page.more;
            let old = self.replicas[dev].snapshot();
            self.replicas[dev].pulled(page);
            let new = self.replicas[dev].snapshot();
            self.check_step(DEVICES[dev], &old, &new);
            if !more {
                break;
            }
        }
    }

    fn apply(&mut self, op: &Op) {
        match *op {
            Op::Write { dev, rec, field } => {
                self.vc[dev][dev] += 1;
                self.made += 1;
                let value = format!("{}#{}", DEVICES[dev], self.made);
                let key: Key = ("task".into(), format!("t{rec}"), format!("f{field}"));
                self.writes
                    .insert(value.clone(), (key.clone(), self.vc[dev]));
                let old = self.replicas[dev].snapshot();
                self.replicas[dev].write(&key.0, &key.1, &key.2, json!(value));
                let new = self.replicas[dev].snapshot();
                assert_eq!(
                    new.get(&key),
                    Some(&json!(value)),
                    "a local write shows at once"
                );
                self.check_step(DEVICES[dev], &old, &new);
            }
            Op::Push { dev, slow } => {
                let batch = self.replicas[dev].push_batch();
                if slow {
                    self.in_flight.push((dev, batch));
                } else {
                    self.land(dev, batch);
                }
            }
            Op::Land { pick } => {
                if !self.in_flight.is_empty() {
                    let (dev, batch) = self.in_flight.remove(pick % self.in_flight.len());
                    self.land(dev, batch);
                }
            }
            Op::Pull { dev, limit } => self.pull(dev, limit),
        }
    }

    fn settle(&mut self) {
        while let Some((dev, batch)) = self.in_flight.pop() {
            self.land(dev, batch);
        }
        for dev in 0..3 {
            while self.replicas[dev].has_pending() {
                let batch = self.replicas[dev].push_batch();
                self.land(dev, batch);
            }
        }
        for dev in 0..3 {
            self.pull(dev, 3);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn three_devices_converge_and_never_go_back(ops in prop::collection::vec(op(), 1..120)) {
        let mut world = World::new();
        for op in &ops {
            world.apply(op);
        }
        world.settle();

        let server = world.server.snapshot();
        for (dev, r) in world.replicas.iter().enumerate() {
            prop_assert_eq!(&r.snapshot(), &server, "{} differs from the server", DEVICES[dev]);
        }
        // Each field ends on a write that no other write to it had seen.
        for (key, value) in &server {
            let last = world.vc_of(value);
            for (other, (k, vc)) in &world.writes {
                if k == key {
                    prop_assert!(!before(&last, vc), "{:?} ended on {} though {} is newer", key, value, other);
                }
            }
        }
        // Every field anyone wrote arrived.
        let written: std::collections::BTreeSet<&Key> = world.writes.values().map(|(k, _)| k).collect();
        prop_assert_eq!(written.len(), server.len());
    }
}
