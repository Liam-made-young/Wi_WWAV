//! S2.7 (docs/PLAN.md): Heat sync fails if a slower older write overwrites a
//! newer one on any field.
//!
//! "Newer" is judged here without trusting the code under test, two ways.
//! The harness keeps a vector clock per device, so write B is newer than
//! write A when B's device had seen A (or made it) before writing B: that
//! must hold however wrong the devices' clocks are. And it keeps a true time
//! for every write, so of two writes neither device had seen, the later one
//! is newer: that must hold when the clocks agree and the edits are at least
//! a millisecond apart.

use jiff::{SignedDuration, Timestamp};
use proptest::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use wi_heat::sync::{Change, Replica, Saved, Server};

fn at(s: &str) -> Timestamp {
    s.parse().unwrap()
}

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
    let now = at("2026-10-06T12:40:00Z");
    a.write_at("habit", "h1", "title", json!("Practise kanji"), now);
    b.write_at("habit", "h1", "title", json!("Practise kanji, 20m"), now);
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

#[test]
fn of_two_unseen_edits_the_later_one_wins_whichever_lands_last() {
    let mut studio = Replica::new("studio");
    let mut air = Replica::new("air");
    let mut server = Server::default();
    // The studio is busy all morning and edits the title at 9:00; the Air,
    // which hasn't synced since, edits it at 10:00. The studio's push lands
    // last.
    for n in 0..40 {
        let t = at("2026-10-06T12:00:00Z") + SignedDuration::from_mins(n);
        studio.write_at("task", &format!("t{n}"), "notes", json!(n), t);
    }
    studio.write_at(
        "task",
        "t1",
        "title",
        json!("9:00"),
        at("2026-10-06T13:00:00Z"),
    );
    air.write_at(
        "task",
        "t1",
        "title",
        json!("10:00"),
        at("2026-10-06T14:00:00Z"),
    );
    for r in [&mut air, &mut studio] {
        while r.has_pending() {
            let b = r.push_batch();
            server.push(&b);
            r.acked(&b);
        }
    }
    for r in [&mut air, &mut studio] {
        pull_all(r, &server, 10);
        assert_eq!(r.value("task", "t1", "title"), Some(&json!("10:00")));
    }
}

#[test]
fn a_seen_edit_loses_to_the_next_one_even_on_a_clock_behind() {
    let mut mac = Replica::new("mac");
    let mut air = Replica::new("air");
    let mut server = Server::default();
    // The Air's clock runs an hour fast.
    air.write_at(
        "task",
        "t1",
        "due",
        json!("Oct 8"),
        at("2026-10-06T14:00:00Z"),
    );
    let b = air.push_batch();
    server.push(&b);
    pull_all(&mut mac, &server, 10);
    mac.write_at(
        "task",
        "t1",
        "due",
        json!("Oct 9"),
        at("2026-10-06T13:01:00Z"),
    );
    let b = mac.push_batch();
    assert_eq!(server.push(&b), 1, "made after seeing the Air's, so newer");
    pull_all(&mut air, &server, 10);
    assert_eq!(air.value("task", "t1", "due"), Some(&json!("Oct 9")));
}

#[test]
fn a_saved_replica_comes_back_whole() {
    let mut mac = Replica::new("mac");
    let now = at("2026-10-06T12:40:00Z");
    mac.write_at("task", "t1", "title", json!("Lab 5a"), now);
    mac.write_at("task", "t1", "done", json!(false), now);
    let mut server = Server::default();
    let b = mac.push_batch();
    server.push(&b[..1]);
    mac.acked(&b[..1]);

    let json = serde_json::to_string(&mac.save()).unwrap();
    let back = Replica::restore(serde_json::from_str::<Saved>(&json).unwrap());
    assert_eq!(back.snapshot(), mac.snapshot());
    assert_eq!(back.cursor(), mac.cursor());
    assert_eq!(
        back.push_batch(),
        mac.push_batch(),
        "the unacked field still waits"
    );
    assert_eq!(back.save(), mac.save());
}

#[test]
fn a_restored_counter_is_past_every_stamp_even_if_its_number_was_lost() {
    let mut mac = Replica::new("mac");
    let now = at("2026-10-06T12:40:00Z");
    mac.write_at("task", "t1", "title", json!("before"), now);
    let mut server = Server::default();
    let b = mac.push_batch();
    server.push(&b);
    mac.acked(&b);

    // The counter's own row was lost; the cells keep their stamps.
    let mut saved = mac.save();
    saved.seq = 0;
    let mut mac = Replica::restore(saved);
    // The clock stepped back a minute while the app was closed.
    mac.write_at(
        "task",
        "t1",
        "title",
        json!("after"),
        now - SignedDuration::from_mins(1),
    );
    let b = mac.push_batch();
    assert_eq!(server.push(&b), 1);
    assert_eq!(server.pull(0, 10).changes[0].value, json!("after"));
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
    Write {
        dev: usize,
        rec: u8,
        field: u8,
    },
    Push {
        dev: usize,
        slow: bool,
    },
    Land {
        pick: usize,
    },
    Pull {
        dev: usize,
        limit: usize,
    },
    /// Time passes, in microseconds.
    Wait {
        us: i64,
    },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (0..3usize, 0..3u8, 0..2u8).prop_map(|(dev, rec, field)| Op::Write { dev, rec, field }),
        2 => (0..3usize, any::<bool>()).prop_map(|(dev, slow)| Op::Push { dev, slow }),
        2 => any::<usize>().prop_map(|pick| Op::Land { pick }),
        2 => (0..3usize, 1..4usize).prop_map(|(dev, limit)| Op::Pull { dev, limit }),
        2 => (0..3_000i64).prop_map(|us| Op::Wait { us }),
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
    /// Every write ever made, by its unique value: its key, vector clock and
    /// true time.
    writes: HashMap<String, (Key, Vc, Timestamp)>,
    made: u32,
    /// The true time.
    now: Timestamp,
    /// How far each device's clock is off.
    skew: [SignedDuration; 3],
    /// Time that passes before each write.
    gap: SignedDuration,
}

impl World {
    fn new(skew: [SignedDuration; 3], gap: SignedDuration) -> Self {
        World {
            replicas: DEVICES.iter().map(|d| Replica::new(d)).collect(),
            server: Server::default(),
            in_flight: Vec::new(),
            vc: [[0; 3]; 3],
            writes: HashMap::new(),
            made: 0,
            now: at("2026-10-06T12:40:00Z"),
            skew,
            gap,
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
                self.now += self.gap;
                self.vc[dev][dev] += 1;
                self.made += 1;
                let value = format!("{}#{}", DEVICES[dev], self.made);
                let key: Key = ("task".into(), format!("t{rec}"), format!("f{field}"));
                self.writes
                    .insert(value.clone(), (key.clone(), self.vc[dev], self.now));
                let old = self.replicas[dev].snapshot();
                let clock = self.now + self.skew[dev];
                self.replicas[dev].write_at(&key.0, &key.1, &key.2, json!(value), clock);
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
            Op::Wait { us } => self.now += SignedDuration::from_micros(us),
        }
    }

    /// Lands everything, then checks every store holds the same values and
    /// each field ends on a write no other write to it had seen.
    fn settle_and_check(&mut self) -> Result<(), TestCaseError> {
        self.settle();
        let server = self.server.snapshot();
        for (dev, r) in self.replicas.iter().enumerate() {
            prop_assert_eq!(
                &r.snapshot(),
                &server,
                "{} differs from the server",
                DEVICES[dev]
            );
        }
        for (key, value) in &server {
            let last = self.vc_of(value);
            for (other, (k, vc, _)) in &self.writes {
                if k == key {
                    prop_assert!(
                        !before(&last, vc),
                        "{:?} ended on {} though {} is newer",
                        key,
                        value,
                        other
                    );
                }
            }
        }
        // Every field anyone wrote arrived.
        let written: std::collections::BTreeSet<&Key> =
            self.writes.values().map(|(k, _, _)| k).collect();
        prop_assert_eq!(written.len(), server.len());
        Ok(())
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

fn skew() -> impl Strategy<Value = SignedDuration> {
    (-600_000_000i64..600_000_000).prop_map(SignedDuration::from_micros)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Clocks up to ten minutes off either way, and writes as close as the
    /// same microsecond: what a device had seen still always wins.
    #[test]
    fn three_devices_converge_and_never_go_back(
        ops in prop::collection::vec(op(), 1..120),
        skews in [skew(), skew(), skew()],
    ) {
        let mut world = World::new(skews, SignedDuration::ZERO);
        for op in &ops {
            world.apply(op);
        }
        world.settle_and_check()?;
    }

    /// Clocks that agree, and edits at least a millisecond apart: each field
    /// also ends on its latest edit, whatever order the pushes land in.
    #[test]
    fn with_clocks_that_agree_the_latest_edit_wins(ops in prop::collection::vec(op(), 1..120)) {
        let mut world = World::new([SignedDuration::ZERO; 3], SignedDuration::from_millis(1));
        for op in &ops {
            world.apply(op);
        }
        world.settle_and_check()?;
        for (key, value) in world.server.snapshot() {
            let latest = world
                .writes
                .iter()
                .filter(|(_, (k, _, _))| *k == key)
                .max_by_key(|(_, (_, _, t))| *t)
                .map(|(v, _)| v.clone());
            prop_assert_eq!(Some(value.as_str().unwrap().to_string()), latest, "{:?}", key);
        }
    }
}
