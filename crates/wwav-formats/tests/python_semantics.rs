//! The python behaviour the tools' bytes and sentences rest on, checked
//! against python itself on many values: float repr and json.dumps, num()
//! and the .2f and .1f formats (binary ties like 120.125 included),
//! json.dumps of strings, json.loads and str() of tricky texts, str.strip,
//! float() of what a song.txt might hold, title_of, and the U+FFFD that
//! errors="replace" puts in for bytes that aren't UTF-8.
//!
//! Fails if: any of these differs from python on any value tried.

mod common;

use common::*;
use wwav_formats::json::{self, Value};
use wwav_formats::text;

fn python_lines(code: &str, input: &str) -> Vec<String> {
    let dir = tmp("py-semantics");
    let (script, data) = (dir.join("t.py"), dir.join("in.txt"));
    std::fs::write(&script, code).unwrap();
    std::fs::write(&data, input).unwrap();
    let mut args = vec![s(&script).to_string(), s(&data).to_string()];
    args.push(s(&wwav_pack()).to_string());
    ok(run("python3", &args))
        .lines()
        .map(String::from)
        .collect()
}

/// xorshift64*: the same values on every run.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

#[test]
fn numbers_print_as_python_prints_them() {
    if !have_references("numbers_print_as_python_prints_them") {
        return;
    }
    let mut values: Vec<f64> = vec![
        0.0,
        -0.0,
        1.0,
        0.1,
        1e16,
        1e15,
        1e-4,
        1e-5,
        123456789012345678.0,
        5e-324,
        f64::MAX,
        2.5,
        0.125,
        120.125,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e22,
        1e300,
    ];
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..20_000 {
        values.push(f64::from_bits(rng.next()));
    }
    for c in 0..40_000u32 {
        values.push(c as f64 / 8.0); // every exact tie at two decimals up to 5,000
        values.push(c as f64 / 1000.0);
        values.push(20.0 + (rng.next() % 380_000) as f64 / 1000.0); // bpm-like
    }
    let input: String = values
        .iter()
        .map(|v| format!("{:016x}\n", v.to_bits()))
        .collect();
    let code = r#"
import json, struct, sys
sys.path.insert(0, sys.argv[2].rsplit('/', 1)[0])
from wwav_pack import num
for line in open(sys.argv[1]):
    v = struct.unpack('>d', bytes.fromhex(line.strip()))[0]
    try:
        n = num(v)
    except (ValueError, OverflowError) as e:
        n = 'error: ' + str(e)
    print('\t'.join([repr(v), json.dumps(v), n, f'{v:.2f}' if v == v and abs(v) < 1e300 else '-', f'{v:.1f}' if v == v and abs(v) < 1e300 else '-']))
"#;
    let lines = python_lines(code, &input);
    assert_eq!(lines.len(), values.len());
    for (v, line) in values.iter().zip(lines) {
        let n = match json::num(*v) {
            Ok(n) => n,
            Err(e) => format!("error: {e}"),
        };
        let fixed = |p: usize| {
            if v.is_nan() || v.abs() >= 1e300 {
                "-".to_string()
            } else {
                format!("{v:.p$}")
            }
        };
        let ours = [
            json::float_repr(*v),
            json::dumps(&Value::Float(*v)),
            n,
            fixed(2),
            fixed(1),
        ]
        .join("\t");
        assert_eq!(ours, line, "{v:e}");
    }
}

#[test]
fn strings_are_quoted_as_json_dumps_quotes_them() {
    if !have_references("strings_are_quoted_as_json_dumps_quotes_them") {
        return;
    }
    let mut rng = Rng(42);
    let pick = |r: &mut Rng| -> char {
        let cp = match r.next() % 6 {
            0 => r.next() % 0x80,
            1 => r.next() % 0x20,
            2 => [0x22, 0x5c, 0x7f, 0x2028, 0x2029, 0xfeff, 0x85, 0xa0][(r.next() % 8) as usize],
            3 => r.next() % 0x800,
            4 => r.next() % 0x10000,
            _ => r.next() % 0x110000,
        } as u32;
        char::from_u32(cp).unwrap_or('\u{fffd}')
    };
    let strings: Vec<String> = (0..5000)
        .map(|_| (0..rng.next() % 12).map(|_| pick(&mut rng)).collect())
        .collect();
    let input: String = strings
        .iter()
        .map(|s| {
            s.chars()
                .map(|c| format!("{:x} ", c as u32))
                .collect::<String>()
                + "\n"
        })
        .collect();
    let code = r#"
import json, sys, unicodedata
for line in open(sys.argv[1]):
    s = ''.join(chr(int(h, 16)) for h in line.split())
    unassigned = any(unicodedata.category(c) == 'Cn' for c in s)
    print(json.dumps([json.dumps(s, ensure_ascii=False), None if unassigned else repr([s]), s.strip()]))
"#;
    let lines = python_lines(code, &input);
    for (s, line) in strings.iter().zip(lines) {
        // python's answers come back as a JSON list, so newlines survive
        let Some(Value::List(ref v)) = json::loads(line.as_bytes(), false) else {
            panic!("{line}")
        };
        let text = |i: usize| match &v[i] {
            Value::Str(t) => t.clone(),
            _ => panic!(),
        };
        assert_eq!(json::quote(s), text(0), "{s:?}");
        // python escapes what its Unicode (15.1) leaves unassigned, and
        // Rust's tables are newer: those strings are compared without repr
        if v[1] != Value::Null {
            assert_eq!(
                json::py_str(&Value::List(vec![Value::Str(s.clone())])),
                text(1),
                "{s:?}"
            );
        }
        assert_eq!(text::py_strip(s), text(2), "{s:?}");
    }
}

#[test]
fn json_reads_as_pythons_json_reads() {
    if !have_references("json_reads_as_pythons_json_reads") {
        return;
    }
    let mut texts: Vec<String> = [
        r#"{"a": 1, "b": [true, false, null], "c": {"d": "e"}}"#,
        r#"{"a": 1, "a": 2, "b": 3, "a": 4}"#,
        r#"{"wwav": "0.1", "frames": 600.0}"#,
        r#"{"frames": 600}"#,
        r#"[1, 2, ]"#,
        r#"{"a": 1, }"#,
        r#"{"a": 01}"#,
        r#"[1.]"#,
        r#"[.5]"#,
        r#"[-]"#,
        r#"[-0, -0.0, 0.0, 1E5, 1e-7, 2.50, 1e400, -1e400]"#,
        r#"[NaN, Infinity, -Infinity]"#,
        r#"[nan]"#,
        r#"[-NaN]"#,
        "\u{feff}{}",
        " \t\r\n{} \n",
        "\u{c}{}",
        "{}\u{a0}",
        r#"["\ud83d\ude00", "\u00e9\u0041", "\/\b\f\n\r\t\"\\"]"#,
        r#"["\x41"]"#,
        r#"["\u12"]"#,
        "[\"tab\there\"]",
        "[\"\u{7f}\"]",
        r#"{"k": [1, {"deep": [[[]]]}], "e": {}}"#,
        r#"[123456789012345678901234567890, -123456789012345678901234567890]"#,
        r#"[1.5, true, "x", null, ["y", 'z']]"#,
        r#"{1: 2}"#,
        r#"[1 2]"#,
        r#""a string""#,
        r#"12"#,
        r#"null"#,
        "",
        "   ",
        r#"[tru]"#,
        r#"{"a"  :  1  ,  "b":2}"#,
        r#"{"wwav": ["0.1"], "frames": {"n": 'x'}}"#,
        r#"{"frames": [1, "a'b", "a\"b", "a'b\"c", "\u0001\u00a0\u00ad\u2028"]}"#,
        r#"{"frames": {"x": 1.5e300, "y": -2.5e-7, "z": true, "w": null}}"#,
    ]
    .iter()
    .map(|t| t.to_string())
    .collect();
    texts.push(format!("[{}]", "1".repeat(4300)));
    texts.push(format!("[{}]", "1".repeat(4301)));
    texts.push(format!("[-{}]", "1".repeat(4301)));
    texts.push(format!("{}{}", "[".repeat(200), "]".repeat(200)));
    let input: String = texts
        .iter()
        .map(|t| t.bytes().map(|b| format!("{b:02x}")).collect::<String>() + "\n")
        .collect();
    let code = r#"
import json, sys
for line in open(sys.argv[1]):
    t = bytes.fromhex(line.strip())
    try:
        v = json.loads(t.decode('utf-8'))
        print(json.dumps([json.dumps(v, ensure_ascii=False), str(v)], ensure_ascii=False))
    except ValueError:
        print(json.dumps(None))
"#;
    let lines = python_lines(code, &input);
    for (t, line) in texts.iter().zip(lines) {
        let ours = match json::loads(t.as_bytes(), true) {
            Some(v) => json::dumps(&Value::List(vec![
                Value::Str(json::dumps(&v)),
                Value::Str(json::py_str(&v)),
            ])),
            None => "null".into(),
        };
        assert_eq!(ours, line, "{t}");
    }
    // NaN and Infinity are .swav's not_json
    assert!(json::loads(b"[NaN]", false).is_none() && json::loads(b"[1e400]", false).is_some());
}

#[test]
fn song_txt_values_read_as_python_reads_them() {
    if !have_references("song_txt_values_read_as_python_reads_them") {
        return;
    }
    let floats = [
        "120",
        "120.5",
        " 97.333 ",
        "abc",
        "1_20",
        "1__20",
        "_120",
        "120_",
        "1_2.5_0",
        "nan",
        "-inf",
        "Infinity",
        "+1e2",
        ".5",
        "5.",
        "",
        "١٢٠",
        "１２０.５",
        "1e400",
        "0x10",
        "1,5",
        "12 0",
        "\u{2003}42\u{3000}",
        "٣.٥",
        "1_٢",
    ];
    let names = [
        "01 Song",
        "1-Song",
        "0001_.Song",
        "12345 Song",
        "Song",
        "01",
        "01 ",
        "01  ",
        "٠٣ Arabic",
        "01 . - _x",
        "a/b/02 Two/",
        "02 Two/.",
        "x/./03 Three",
        "../04 Four/..",
        "/",
        "",
        "۱۲۳۴ Persian",
        "05-",
        "05--",
        "06 \u{2003}x",
    ];
    let input = format!("{}\n{}\n", floats.join("\u{1}"), names.join("\u{1}"));
    let code = r#"
import json, sys
sys.path.insert(0, sys.argv[2].rsplit('/', 1)[0])
from wwav_pack import title_of
floats, names = open(sys.argv[1], encoding='utf-8').read().split('\n')[:2]
out = []
for f in floats.split('\x01'):
    try:
        out.append(repr(float(f)))
    except ValueError:
        out.append(None)
print(json.dumps(out, ensure_ascii=False))
print(json.dumps([title_of(n) for n in names.split('\x01')], ensure_ascii=False))
print(json.dumps([c for c in range(0x110000) if chr(c).isspace()]))
print(json.dumps([c for c in range(0x110000) if chr(c).isdecimal()]))
"#;
    let lines = python_lines(code, &input);
    let floats_ours: Vec<Value> = floats
        .iter()
        .map(|f| text::py_float(f).map_or(Value::Null, |v| Value::Str(json::float_repr(v))))
        .collect();
    assert_eq!(json::dumps(&Value::List(floats_ours)), lines[0]);
    let names_ours: Vec<Value> = names
        .iter()
        .map(|n| Value::Str(text::title_of(n)))
        .collect();
    assert_eq!(json::dumps(&Value::List(names_ours)), lines[1]);
    let space: Vec<String> = (0..0x110000u32)
        .filter_map(char::from_u32)
        .filter(|&c| text::is_py_space(c))
        .map(|c| (c as u32).to_string())
        .collect();
    assert_eq!(format!("[{}]", space.join(", ")), lines[2]);
    let digits: Vec<String> = (0..0x110000u32)
        .filter_map(char::from_u32)
        .filter(|&c| text::decimal(c).is_some())
        .map(|c| (c as u32).to_string())
        .collect();
    assert_eq!(format!("[{}]", digits.join(", ")), lines[3]);
}

#[test]
fn bytes_that_arent_utf8_read_as_python_replaces_them() {
    if !have_references("bytes_that_arent_utf8_read_as_python_replaces_them") {
        return;
    }
    let mut rng = Rng(7);
    let samples: Vec<Vec<u8>> = (0..3000)
        .map(|_| {
            (0..rng.next() % 10)
                .map(|_| match rng.next() % 4 {
                    0 => (rng.next() % 0x80) as u8,
                    1 => 0x80 + (rng.next() % 0x40) as u8,
                    2 => 0xc0 + (rng.next() % 0x40) as u8,
                    _ => rng.next() as u8,
                })
                .collect()
        })
        .collect();
    let input: String = samples
        .iter()
        .map(|b| b.iter().map(|x| format!("{x:02x}")).collect::<String>() + "\n")
        .collect();
    let code = r#"
import sys
for line in open(sys.argv[1]):
    print(' '.join('%x' % ord(c) for c in bytes.fromhex(line.strip()).decode('utf-8', errors='replace')))
"#;
    for (b, line) in samples.iter().zip(python_lines(code, &input)) {
        let ours: Vec<String> = String::from_utf8_lossy(b)
            .chars()
            .map(|c| format!("{:x}", c as u32))
            .collect();
        assert_eq!(ours.join(" "), line, "{b:02x?}");
    }
}
