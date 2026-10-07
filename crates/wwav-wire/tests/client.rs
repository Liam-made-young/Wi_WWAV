//! The app's side of the socket against a scripted fake engine.

use serde_json::{json, Map, Value};
use std::os::unix::net::UnixStream;
use std::thread;
use std::time::{Duration, Instant};
use wwav_wire::client::{CallError, Client};
use wwav_wire::frame;
use wwav_wire::msg::{ErrorBody, Event, Request, Response};

const T: Duration = Duration::from_secs(5);

fn pair() -> (Client, std::sync::mpsc::Receiver<Event>, UnixStream) {
    let (app, engine) = UnixStream::pair().unwrap();
    let (client, events) = Client::new(app).unwrap();
    (client, events, engine)
}

fn next_request(engine: &mut UnixStream) -> Request {
    Request::from_object(frame::read(engine).unwrap().unwrap()).unwrap()
}

fn ok(id: u64, result: Value) -> Response {
    let Value::Object(m) = result else { panic!() };
    Response { id, outcome: Ok(m) }
}

fn event(ev: &str, fields: Value) -> Event {
    let Value::Object(m) = fields else { panic!() };
    Event {
        ev: ev.into(),
        fields: m,
    }
}

#[test]
fn a_call_gets_its_result() {
    let (client, _events, mut engine) = pair();
    let fake = thread::spawn(move || {
        let req = next_request(&mut engine);
        assert_eq!(req.op, "ping");
        assert_eq!(req.args, None, "no args are sent for Null");
        frame::write(&mut engine, &ok(req.id, json!({"t": 42}))).unwrap();
        engine
    });
    let result = client.call("ping", Value::Null, T).unwrap();
    assert_eq!(result["t"], 42);
    fake.join().unwrap();
}

#[test]
fn ids_are_positive_and_unique() {
    let (client, _events, mut engine) = pair();
    let fake = thread::spawn(move || {
        let ids: Vec<u64> = (0..3)
            .map(|_| {
                let req = next_request(&mut engine);
                frame::write(&mut engine, &ok(req.id, json!({}))).unwrap();
                req.id
            })
            .collect();
        ids
    });
    for _ in 0..3 {
        client.call("ping", json!({}), T).unwrap();
    }
    let ids = fake.join().unwrap();
    assert!(ids.iter().all(|&id| id > 0));
    assert!(ids[0] < ids[1] && ids[1] < ids[2], "{ids:?}");
}

#[test]
fn responses_in_any_order_reach_their_callers() {
    let (client, _events, mut engine) = pair();
    let client = std::sync::Arc::new(client);
    let fake = thread::spawn(move || {
        let a = next_request(&mut engine);
        let b = next_request(&mut engine);
        // Answer the second first.
        for req in [&b, &a] {
            let echo = req.args.clone().unwrap()["n"].clone();
            frame::write(&mut engine, &ok(req.id, json!({"n": echo}))).unwrap();
        }
        engine
    });
    let callers: Vec<_> = (0..2)
        .map(|n| {
            let client = client.clone();
            thread::spawn(move || client.call("echo", json!({"n": n}), T).unwrap()["n"].clone())
        })
        .collect();
    let got: Vec<Value> = callers.into_iter().map(|c| c.join().unwrap()).collect();
    assert_eq!(got, [json!(0), json!(1)]);
    fake.join().unwrap();
}

#[test]
fn events_arrive_between_responses() {
    let (client, events, mut engine) = pair();
    let fake = thread::spawn(move || {
        let req = next_request(&mut engine);
        frame::write(
            &mut engine,
            &event("transport", json!({"state": "playing", "sample": 0})),
        )
        .unwrap();
        frame::write(&mut engine, &ok(req.id, json!({"sample": 0}))).unwrap();
        frame::write(
            &mut engine,
            &event("plugin.latency", json!({"node": "D", "samples": 2048})),
        )
        .unwrap();
        engine
    });
    assert_eq!(
        client.call("transport.play", Value::Null, T).unwrap()["sample"],
        0
    );
    let first = events.recv_timeout(T).unwrap();
    assert_eq!(
        first,
        event("transport", json!({"state": "playing", "sample": 0}))
    );
    let second = events.recv_timeout(T).unwrap();
    assert_eq!(second.ev, "plugin.latency");
    assert_eq!(second.fields["samples"], 2048);
    fake.join().unwrap();
}

#[test]
fn a_refusal_comes_back_as_the_engines_error() {
    let (client, _events, mut engine) = pair();
    let fake = thread::spawn(move || {
        let req = next_request(&mut engine);
        let fail = Response {
            id: req.id,
            outcome: Err(ErrorBody::new("no_such_node", "No node X.")),
        };
        frame::write(&mut engine, &fail).unwrap();
        engine
    });
    match client.call("param.set", json!({"node": "X"}), T) {
        Err(CallError::Engine(e)) => assert_eq!(e, ErrorBody::new("no_such_node", "No node X.")),
        other => panic!("{other:?}"),
    }
    fake.join().unwrap();
}

#[test]
fn an_unanswered_call_times_out_and_a_late_answer_is_dropped() {
    let (client, _events, mut engine) = pair();
    let started = Instant::now();
    let fake = thread::spawn(move || {
        let late = next_request(&mut engine);
        let next = next_request(&mut engine);
        frame::write(&mut engine, &ok(late.id, json!({"late": true}))).unwrap();
        frame::write(&mut engine, &ok(next.id, json!({"late": false}))).unwrap();
        engine
    });
    match client.call(
        "debug.hang",
        json!({"in": "message"}),
        Duration::from_millis(200),
    ) {
        Err(CallError::Timeout(_)) => {}
        other => panic!("{other:?}"),
    }
    let waited = started.elapsed();
    assert!(
        waited >= Duration::from_millis(200) && waited < Duration::from_secs(2),
        "{waited:?}"
    );
    // The late answer to the first call must not be taken for the second's.
    assert_eq!(client.call("ping", Value::Null, T).unwrap()["late"], false);
    fake.join().unwrap();
}

#[test]
fn the_engine_closing_ends_waiting_calls_and_the_event_channel() {
    let (client, events, mut engine) = pair();
    let fake = thread::spawn(move || {
        next_request(&mut engine);
        drop(engine); // killed: the socket closes with a call outstanding
    });
    let started = Instant::now();
    match client.call("ping", Value::Null, T) {
        Err(CallError::Closed) => {}
        other => panic!("{other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(1));
    fake.join().unwrap();
    assert!(client.wait_closed(T));
    assert!(client.is_closed());
    assert!(
        events.recv_timeout(T).is_err(),
        "the event channel ends with the connection"
    );
    assert!(matches!(
        client.call("ping", Value::Null, T),
        Err(CallError::Closed)
    ));
}

#[test]
fn a_bad_frame_from_the_engine_closes_the_connection() {
    let (client, _events, mut engine) = pair();
    use std::io::Write;
    engine.write_all(&[2, 0, 0, 0, b'[', b']']).unwrap();
    assert!(client.wait_closed(T));
    assert!(
        client.closed_reason().unwrap().contains("object"),
        "{:?}",
        client.closed_reason()
    );
}

#[test]
fn args_must_be_an_object() {
    let (client, _events, _engine) = pair();
    assert!(matches!(
        client.call("ping", json!([1, 2]), T),
        Err(CallError::Args)
    ));
    assert!(matches!(
        client.call("ping", json!(3), T),
        Err(CallError::Args)
    ));
}

#[test]
fn request_returns_the_response_itself() {
    let (client, _events, mut engine) = pair();
    let fake = thread::spawn(move || {
        let req = next_request(&mut engine);
        let fail = Response {
            id: req.id,
            outcome: Err(ErrorBody::new("protocol", "No.")),
        };
        frame::write(&mut engine, &fail).unwrap();
        engine
    });
    let r = client.request("hello", json!({"protocol": 2}), T).unwrap();
    assert_eq!(r.outcome, Err(ErrorBody::new("protocol", "No.")));
    let _: Map<String, Value> = Map::new();
    fake.join().unwrap();
}
