//! Framing (`docs/ENGINE.md` §2): a u32 little-endian payload length from 1
//! to 16 MiB, then the payload, one JSON object in UTF-8. Anything else is a
//! broken connection, and the reader closes it.

use serde::Serialize;
use serde_json::{Map, Value};
use std::io::{self, Read, Write};

/// The largest payload, 16 MiB. Large data goes through a file whose path is
/// in the frame, never through a frame.
pub const MAX_PAYLOAD: usize = 16 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("a frame of {0} bytes is over the 16 MiB limit")]
    TooLong(usize),
    #[error("an empty frame")]
    Empty,
    #[error("the connection ended inside a frame")]
    Short,
    #[error("a frame that isn't JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("a frame that isn't a JSON object")]
    NotObject,
    #[error(transparent)]
    Io(io::Error),
}

/// One frame: the length prefix and the payload, ready to write in one go.
pub fn encode<T: Serialize + ?Sized>(msg: &T) -> Result<Vec<u8>, FrameError> {
    let mut out = vec![0u8; 4];
    serde_json::to_writer(&mut out, msg)?;
    let len = out.len() - 4;
    // serde_json writes an object as `{` with no leading space.
    if out.get(4) != Some(&b'{') {
        return Err(FrameError::NotObject);
    }
    if len > MAX_PAYLOAD {
        return Err(FrameError::TooLong(len));
    }
    out[..4].copy_from_slice(&(len as u32).to_le_bytes());
    Ok(out)
}

/// Writes one frame with a single `write_all`, so callers that share a
/// stream only need to hold their lock around this call.
pub fn write<W: Write, T: Serialize + ?Sized>(w: &mut W, msg: &T) -> Result<(), FrameError> {
    let bytes = encode(msg)?;
    w.write_all(&bytes).map_err(FrameError::Io)?;
    w.flush().map_err(FrameError::Io)
}

/// Reads one frame. `Ok(None)` is the other side closing cleanly between
/// frames; any error means the connection is broken and should be closed.
pub fn read<R: Read>(r: &mut R) -> Result<Option<Map<String, Value>>, FrameError> {
    let mut prefix = [0u8; 4];
    let got = read_full(r, &mut prefix)?;
    if got == 0 {
        return Ok(None);
    }
    if got < 4 {
        return Err(FrameError::Short);
    }
    let len = u32::from_le_bytes(prefix) as usize;
    if len == 0 {
        return Err(FrameError::Empty);
    }
    if len > MAX_PAYLOAD {
        return Err(FrameError::TooLong(len));
    }
    let mut payload = vec![0u8; len];
    if read_full(r, &mut payload)? < len {
        return Err(FrameError::Short);
    }
    match serde_json::from_slice(&payload)? {
        Value::Object(m) => Ok(Some(m)),
        _ => Err(FrameError::NotObject),
    }
}

/// Fills `buf` unless the stream ends first; returns how much it filled.
fn read_full<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<usize, FrameError> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(FrameError::Io(e)),
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::{ErrorBody, Event, Request, Response};
    use serde_json::{json, Map, Value};
    use std::io::Cursor;

    fn obj(v: Value) -> Map<String, Value> {
        match v {
            Value::Object(m) => m,
            _ => panic!("not an object"),
        }
    }

    fn framed(payload: &[u8]) -> Vec<u8> {
        let mut out = (payload.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn the_length_is_u32_le_and_the_payload_is_compact_json() {
        let req = Request {
            id: 412,
            op: "ping".into(),
            args: None,
        };
        let bytes = encode(&req).unwrap();
        assert_eq!(bytes, framed(br#"{"id":412,"op":"ping"}"#));
    }

    #[test]
    fn every_kind_of_payload_round_trips() {
        let req = Request {
            id: 412,
            op: "param.set".into(),
            args: Some(obj(
                json!({"node": "01JC5R", "param": "gain_db", "value": -3.0}),
            )),
        };
        let ok = Response {
            id: 412,
            outcome: Ok(Map::new()),
        };
        let fail = Response {
            id: 413,
            outcome: Err(ErrorBody {
                code: "no_such_node".into(),
                message: "No node 01JC5R…".into(),
            }),
        };
        let ev = Event {
            ev: "plugin.latency".into(),
            fields: obj(json!({"node": "01JC5T…", "samples": 2048})),
        };

        let mut stream = Vec::new();
        write(&mut stream, &req).unwrap();
        write(&mut stream, &ok).unwrap();
        write(&mut stream, &fail).unwrap();
        write(&mut stream, &ev).unwrap();

        let mut r = Cursor::new(stream);
        let got: Vec<Map<String, Value>> = std::iter::from_fn(|| read(&mut r).unwrap()).collect();
        assert_eq!(got.len(), 4);
        assert_eq!(
            serde_json::from_value::<Request>(Value::Object(got[0].clone())).unwrap(),
            req
        );
        assert_eq!(
            serde_json::from_value::<Response>(Value::Object(got[1].clone())).unwrap(),
            ok
        );
        assert_eq!(
            serde_json::from_value::<Response>(Value::Object(got[2].clone())).unwrap(),
            fail
        );
        assert_eq!(
            serde_json::from_value::<Event>(Value::Object(got[3].clone())).unwrap(),
            ev
        );
        // Key order survives too, so a frame re-encodes to the same bytes.
        assert_eq!(encode(&got[0]).unwrap(), encode(&req).unwrap());
    }

    #[test]
    fn every_f64_crosses_bit_for_bit() {
        // serde_json's default float parser is off by one ULP on about one
        // value in ten; the workspace turns on `float_roundtrip` (F8).
        let mut s: u64 = 0x2545_F491_4F6C_DD1D;
        for _ in 0..200_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            // Any finite f64 (JSON has no NaN or infinity), then a pan in -1..1.
            let any = f64::from_bits(s);
            let pan = -1.0 + 2.0 * ((s >> 11) as f64 / (1u64 << 53) as f64);
            for x in [any, pan] {
                if !x.is_finite() {
                    continue;
                }
                let mut m = Map::new();
                m.insert("value".into(), json!(x));
                let back = read(&mut Cursor::new(encode(&m).unwrap()))
                    .unwrap()
                    .unwrap();
                let got = back["value"].as_f64().unwrap();
                assert_eq!(got.to_bits(), x.to_bits(), "sent {x:?}, read {got:?}");
            }
        }
    }

    #[test]
    fn unicode_survives() {
        let ev = Event {
            ev: "key".into(),
            fields: obj(json!({"key": "é 音 🎚", "code": "KeyE"})),
        };
        let bytes = encode(&ev).unwrap();
        let back = read(&mut Cursor::new(bytes)).unwrap().unwrap();
        assert_eq!(back["key"], "é 音 🎚");
    }

    #[test]
    fn a_clean_end_between_frames_is_none() {
        assert!(read(&mut Cursor::new(Vec::new())).unwrap().is_none());
    }

    #[test]
    fn exactly_16_mib_is_accepted() {
        let filler = MAX_PAYLOAD - r#"{"s":""}"#.len();
        let payload = format!(r#"{{"s":"{}"}}"#, "a".repeat(filler));
        assert_eq!(payload.len(), MAX_PAYLOAD);
        let got = read(&mut Cursor::new(framed(payload.as_bytes())))
            .unwrap()
            .unwrap();
        assert_eq!(got["s"].as_str().unwrap().len(), filler);
        let back: Map<String, Value> = got;
        assert_eq!(encode(&back).unwrap().len(), 4 + MAX_PAYLOAD);
    }

    #[test]
    fn over_16_mib_is_refused_both_ways() {
        // Only the prefix is sent: the reader must refuse before it reads (or
        // allocates) the payload.
        let prefix = ((MAX_PAYLOAD + 1) as u32).to_le_bytes();
        assert!(
            matches!(read(&mut Cursor::new(prefix.to_vec())), Err(FrameError::TooLong(n)) if n == MAX_PAYLOAD + 1)
        );
        let huge = u32::MAX.to_le_bytes();
        assert!(matches!(
            read(&mut Cursor::new(huge.to_vec())),
            Err(FrameError::TooLong(_))
        ));

        let mut m = Map::new();
        m.insert("s".into(), Value::String("a".repeat(MAX_PAYLOAD)));
        assert!(matches!(encode(&m), Err(FrameError::TooLong(_))));
        let mut sink = Vec::new();
        assert!(write(&mut sink, &m).is_err());
        assert!(sink.is_empty(), "nothing of a refused frame is written");
    }

    #[test]
    fn an_empty_frame_is_refused() {
        assert!(matches!(
            read(&mut Cursor::new(framed(b""))),
            Err(FrameError::Empty)
        ));
    }

    #[test]
    fn payloads_that_are_not_objects_are_refused() {
        for payload in [&b"[1,2]"[..], b"42", br#""text""#, b"null", b"true"] {
            assert!(
                matches!(
                    read(&mut Cursor::new(framed(payload))),
                    Err(FrameError::NotObject)
                ),
                "{}",
                String::from_utf8_lossy(payload)
            );
        }
        // And nothing that serialises to a non-object can be sent.
        assert!(matches!(encode(&vec![1, 2]), Err(FrameError::NotObject)));
        assert!(matches!(encode(&"text"), Err(FrameError::NotObject)));
    }

    #[test]
    fn garbage_is_refused() {
        for payload in [
            &b"{"[..],
            b"not json",
            b"{\"a\":1} trailing",
            b"{\"a\":1}{\"b\":2}",
            b"{\"a\":\"\xff\xfe\"}", // not UTF-8
            b"\x00\x00\x00\x00",
        ] {
            assert!(
                matches!(
                    read(&mut Cursor::new(framed(payload))),
                    Err(FrameError::Json(_))
                ),
                "{}",
                String::from_utf8_lossy(payload)
            );
        }
    }

    #[test]
    fn short_reads_are_refused() {
        // In the prefix.
        assert!(matches!(
            read(&mut Cursor::new(vec![7u8, 0])),
            Err(FrameError::Short)
        ));
        // In the payload.
        let mut bytes = framed(br#"{"id":1,"op":"ping"}"#);
        bytes.truncate(bytes.len() - 3);
        assert!(matches!(
            read(&mut Cursor::new(bytes)),
            Err(FrameError::Short)
        ));
    }
}
