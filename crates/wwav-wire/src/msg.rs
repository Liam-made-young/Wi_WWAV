//! The three kinds of payload (`docs/ENGINE.md` §2):
//!
//! ```text
//! -> {"id": 412, "op": "param.set", "args": {…}}
//! <- {"id": 412, "ok": true, "result": {}}
//! <- {"id": 413, "ok": false, "error": {"code": "no_such_node", "message": "No node 01JC5R…"}}
//! <- {"ev": "plugin.latency", "node": "01JC5T…", "samples": 2048}
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A request from the app. `id` is positive and unique per connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Map<String, Value>>,
}

impl Request {
    /// A request from a frame, refusing an id that isn't a positive integer,
    /// a missing op and args that aren't an object.
    pub fn from_object(m: Map<String, Value>) -> Result<Request, String> {
        if request_id(&m).is_none() {
            return Err("A request's id is a positive integer.".into());
        }
        serde_json::from_value(Value::Object(m)).map_err(|e| format!("A bad request: {e}."))
    }

    /// The args, or an empty object when the op was sent without them.
    pub fn args(&self) -> Map<String, Value> {
        self.args.clone().unwrap_or_default()
    }
}

/// The id of a frame that claims to be a request, if it is a positive
/// integer: enough to answer a request whose op or args are bad.
pub fn request_id(m: &Map<String, Value>) -> Option<u64> {
    m.get("id").and_then(Value::as_u64).filter(|&id| id > 0)
}

/// Why the engine refused a request: a snake_case code and a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

impl ErrorBody {
    pub fn new(code: &str, message: impl Into<String>) -> ErrorBody {
        ErrorBody {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// The engine's answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawResponse", into = "RawResponse")]
pub struct Response {
    pub id: u64,
    pub outcome: Result<Map<String, Value>, ErrorBody>,
}

/// The response as it is on the wire, before `ok` decides which half counts.
#[derive(Serialize, Deserialize)]
struct RawResponse {
    id: u64,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<Map<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<ErrorBody>,
}

impl TryFrom<RawResponse> for Response {
    type Error = String;

    fn try_from(r: RawResponse) -> Result<Response, String> {
        let outcome = if r.ok {
            Ok(r.result.unwrap_or_default())
        } else {
            Err(r.error.ok_or("A failed response needs its error.")?)
        };
        Ok(Response { id: r.id, outcome })
    }
}

impl From<Response> for RawResponse {
    fn from(r: Response) -> RawResponse {
        match r.outcome {
            Ok(result) => RawResponse {
                id: r.id,
                ok: true,
                result: Some(result),
                error: None,
            },
            Err(error) => RawResponse {
                id: r.id,
                ok: false,
                result: None,
                error: Some(error),
            },
        }
    }
}

/// Something the engine says unasked: `ev` names it, the rest of the object
/// is its fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub ev: String,
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

/// A frame from the engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Response(Response),
    Event(Event),
}

impl Incoming {
    /// An event has `ev` and no `id`; a response has `id` and no `ev`.
    pub fn from_object(m: Map<String, Value>) -> Result<Incoming, String> {
        let parsed = match (m.contains_key("ev"), m.contains_key("id")) {
            (true, false) => serde_json::from_value(Value::Object(m)).map(Incoming::Event),
            (false, true) => serde_json::from_value(Value::Object(m)).map(Incoming::Response),
            _ => return Err("A frame from the engine is a response (id) or an event (ev).".into()),
        };
        parsed.map_err(|e| format!("A bad frame from the engine: {e}."))
    }
}

/// `hello`'s result (§3.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelloResult {
    pub protocol: u32,
    pub engine: String,
    pub pid: u32,
    pub sample_rate: u32,
    pub block: u32,
    /// The open device's name, or null when none is open.
    pub device: Option<String>,
    pub shm_layout: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> {
        match v {
            Value::Object(m) => m,
            _ => panic!("not an object"),
        }
    }

    #[test]
    fn requests_look_like_the_contract() {
        let req = Request {
            id: 412,
            op: "param.set".into(),
            args: Some(obj(
                json!({"node": "01JC5R", "param": "gain_db", "value": -3.0}),
            )),
        };
        assert_eq!(
            serde_json::to_string(&req).unwrap(),
            r#"{"id":412,"op":"param.set","args":{"node":"01JC5R","param":"gain_db","value":-3.0}}"#
        );
        // args may be omitted when an op takes none.
        let ping = Request {
            id: 1,
            op: "ping".into(),
            args: None,
        };
        assert_eq!(
            serde_json::to_string(&ping).unwrap(),
            r#"{"id":1,"op":"ping"}"#
        );
        assert_eq!(
            Request::from_object(obj(json!({"id": 1, "op": "ping"}))).unwrap(),
            ping
        );
    }

    #[test]
    fn bad_requests_are_named() {
        let bad = [
            json!({"op": "ping"}),
            json!({"id": 0, "op": "ping"}),
            json!({"id": -4, "op": "ping"}),
            json!({"id": 1.5, "op": "ping"}),
            json!({"id": "7", "op": "ping"}),
            json!({"id": 7}),
            json!({"id": 7, "op": 3}),
            json!({"id": 7, "op": "ping", "args": [1]}),
        ];
        for b in bad {
            assert!(Request::from_object(obj(b.clone())).is_err(), "{b}");
        }
        // The id of a request with a bad op or args is still known, so the
        // engine can answer it.
        assert_eq!(request_id(&obj(json!({"id": 7, "op": 3}))), Some(7));
        assert_eq!(request_id(&obj(json!({"id": 0, "op": "ping"}))), None);
    }

    #[test]
    fn responses_look_like_the_contract() {
        let ok = Response {
            id: 412,
            outcome: Ok(Map::new()),
        };
        assert_eq!(
            serde_json::to_string(&ok).unwrap(),
            r#"{"id":412,"ok":true,"result":{}}"#
        );
        let fail = Response {
            id: 413,
            outcome: Err(ErrorBody {
                code: "no_such_node".into(),
                message: "No node 01JC5R.".into(),
            }),
        };
        assert_eq!(
            serde_json::to_string(&fail).unwrap(),
            r#"{"id":413,"ok":false,"error":{"code":"no_such_node","message":"No node 01JC5R."}}"#
        );
    }

    #[test]
    fn a_missing_result_reads_as_empty() {
        // docs/SPEC.md 9.2 shows `{"id": 412, "ok": true}`; ENGINE.md says
        // result is always an object. Read the first as the second.
        let r: Response = serde_json::from_str(r#"{"id":412,"ok":true}"#).unwrap();
        assert_eq!(r.outcome, Ok(Map::new()));
    }

    #[test]
    fn a_failure_needs_its_error() {
        assert!(serde_json::from_str::<Response>(r#"{"id":1,"ok":false}"#).is_err());
        assert!(serde_json::from_str::<Response>(r#"{"id":1,"ok":true,"result":[]}"#).is_err());
        assert!(serde_json::from_str::<Response>(r#"{"id":1,"result":{}}"#).is_err());
    }

    #[test]
    fn events_are_flat() {
        let ev = Event {
            ev: "transport".into(),
            fields: obj(json!({"state": "playing", "sample": 96000})),
        };
        assert_eq!(
            serde_json::to_string(&ev).unwrap(),
            r#"{"ev":"transport","state":"playing","sample":96000}"#
        );
        let back: Event =
            serde_json::from_str(r#"{"ev":"transport","state":"playing","sample":96000}"#).unwrap();
        assert_eq!(back, ev);
    }

    #[test]
    fn incoming_frames_are_told_apart() {
        let r =
            Incoming::from_object(obj(json!({"id": 3, "ok": true, "result": {"t": 5}}))).unwrap();
        assert!(matches!(
            r,
            Incoming::Response(Response {
                id: 3,
                outcome: Ok(_)
            })
        ));
        let e = Incoming::from_object(obj(json!({"ev": "key", "key": " "}))).unwrap();
        assert!(matches!(e, Incoming::Event(Event { ref ev, .. }) if ev == "key"));
        // An event has ev and no id.
        assert!(Incoming::from_object(obj(json!({"ev": "key", "id": 3, "ok": true}))).is_err());
        assert!(Incoming::from_object(obj(json!({"hello": 1}))).is_err());
    }

    #[test]
    fn hello_round_trips() {
        let h = HelloResult {
            protocol: 1,
            engine: "wwav-engine 0.1.0".into(),
            pid: 1234,
            sample_rate: 48000,
            block: 128,
            device: Some("null".into()),
            shm_layout: 1,
        };
        let v = serde_json::to_value(&h).unwrap();
        assert_eq!(
            v,
            json!({"protocol": 1, "engine": "wwav-engine 0.1.0", "pid": 1234, "sample_rate": 48000,
                   "block": 128, "device": "null", "shm_layout": 1})
        );
        assert_eq!(serde_json::from_value::<HelloResult>(v).unwrap(), h);
    }
}
