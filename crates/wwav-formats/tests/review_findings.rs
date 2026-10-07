//! Adversarial review of F1, F2 and F3 (docs/PLAN.md): files and command
//! lines the corpus and the fuzz don't make, read and written by this
//! crate against what the reference tools do with them.
//!
//! Each test here failed when it was written, on a real defect, and is
//! marked #[ignore] with the finding it shows. Run them with
//! `cargo test -p wwav-formats --test review_findings -- --ignored`.

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::*;
use wwav_formats::{pack, swav, wwav};

/// A chunk: id, size, payload and the pad byte.
fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
    out
}

/// A .wwav of `frames` silent frames with this wmet text, laid out as
/// wwav_pack.py lays one out (wstm's audio 512-aligned), and a plain wlin.
fn song_with_wmet(frames: u32, wmet: &[u8]) -> Vec<u8> {
    let mut fmt = Vec::new();
    for v in [1u16, 2] {
        fmt.extend_from_slice(&v.to_le_bytes());
    }
    fmt.extend_from_slice(&44_100u32.to_le_bytes());
    fmt.extend_from_slice(&176_400u32.to_le_bytes());
    fmt.extend_from_slice(&4u16.to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());
    let mut body = chunk(b"fmt ", &fmt);
    body.extend(chunk(b"data", &vec![0; frames as usize * 4]));
    body.extend(chunk(b"wmet", wmet));
    let payload = 12 + body.len() as u32 + 8;
    let pad = (512 - (payload + 16) % 512) % 512;
    let mut wstm = Vec::new();
    wstm.extend_from_slice(&1u16.to_le_bytes());
    wstm.extend_from_slice(&[4, 2, 16, 0]);
    wstm.extend_from_slice(&(pad as u16).to_le_bytes());
    wstm.extend_from_slice(&44_100u32.to_le_bytes());
    wstm.extend_from_slice(&frames.to_le_bytes());
    wstm.resize(wstm.len() + pad as usize + frames as usize * 16, 0);
    body.extend(chunk(b"wstm", &wstm));
    body.extend(chunk(
        b"wlin",
        br#"{"parent_id": null, "root_id": "x", "generation": 0, "creator": "", "device_id": ""}"#,
    ));
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(4 + body.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend(body);
    out
}

/// The last line of the reference tool's info, if python3 and formats/ are here.
fn reference_verdict(script: &Path, file: &Path) -> Option<String> {
    if !wwav_pack().exists() || !has("python3") {
        return None;
    }
    let out = ok(python(script, &["info", s(file)]));
    out.lines().last().map(|l| l.trim().to_string())
}

/// Finding: json.rs refuses JSON nested deeper than 500 (MAX_DEPTH), so a
/// wmet that python's json.loads, Wi's JSON.parse and PRANA's reader all
/// read is "not JSON" to the Rust reader alone. wwav_pack.py, Wi and PRANA
/// say "4 stems, and the master" for this file; Rust says "the master only:
/// no wmet and wlin, so a plain WAV". (tools/parity/check.py --corpus on a
/// folder holding it reports only the Rust reader as differing.)
#[test]
fn a_deeply_nested_wmet_gets_the_references_verdict() {
    let dir = tmp("review-deep");
    let path = dir.join("deep.wwav");
    let wmet = format!(
        r#"{{"wwav": "0.1", "frames": 4, "x": {}{}}}"#,
        "[".repeat(600),
        "]".repeat(600)
    );
    std::fs::write(&path, song_with_wmet(4, wmet.as_bytes())).unwrap();
    let ours = wwav::Wwav::open(&path).unwrap().verdict().to_string();
    if let Some(want) = reference_verdict(&wwav_pack(), &path) {
        assert_eq!(format!("on PRANA: {ours}"), want);
    }
    assert_eq!(ours, "4 stems, and the master");
}

/// Finding: the same depth limit in a film's wmet. swav_pack.py and Wi say
/// "a .swav: the film, its wmet and wlin"; Rust says "a plain MP4".
#[test]
fn a_deeply_nested_swav_wmet_gets_the_references_verdict() {
    let dir = tmp("review-deep-swav");
    let path = dir.join("deep.swav");
    let mut film = std::fs::read(corpus().join("plain.mp4")).unwrap();
    let wmet = format!(
        r#"{{"swav": "0.1", "x": {}{}}}"#,
        "[".repeat(600),
        "]".repeat(600)
    );
    let wlin =
        br#"{"parent_id": null, "root_id": "x", "generation": 0, "creator": "", "device_id": ""}"#;
    for (kind, body) in [(b"wmet", wmet.as_bytes()), (b"wlin", &wlin[..])] {
        film.extend_from_slice(&(8 + body.len() as u32).to_be_bytes());
        film.extend_from_slice(kind);
        film.extend_from_slice(body);
    }
    std::fs::write(&path, film).unwrap();
    let ours = swav::Swav::open(&path).unwrap().verdict().to_string();
    if let Some(want) = reference_verdict(&swav_pack(), &path) {
        assert_eq!(format!("reads as: {ours}"), want);
    }
    assert_eq!(ours, "a .swav: the film, its wmet and wlin");
}

/// Finding: json.rs keeps an object's keys in a Vec and searches it for
/// every new key (a repeated key keeps its first place), so reading an
/// object of n keys takes n²/2 comparisons. Every Wwav::open parses wmet,
/// so a 1.3 MB wmet of 100,000 keys holds the reader for about 20 s in a
/// release build (python's info: 0.1 s), and a 13 MB one for about half an
/// hour: a library scan or the file inspector stalls on one crafted file.
/// Here 40,000 keys (about 500 KB) must open in under 2 s.
#[test]
fn a_wmet_with_many_keys_opens_in_linear_time() {
    let dir = tmp("review-keys");
    let path = dir.join("keys.wwav");
    let keys: Vec<String> = (0..40_000).map(|i| format!(r#""k{i}": 1"#)).collect();
    let wmet = format!(r#"{{"wwav": "0.1", "frames": 4, {}}}"#, keys.join(", "));
    std::fs::write(&path, song_with_wmet(4, wmet.as_bytes())).unwrap();
    let start = Instant::now();
    let verdict = wwav::Wwav::open(&path).unwrap().verdict().to_string();
    let took = start.elapsed();
    assert_eq!(verdict, "4 stems, and the master");
    assert!(
        took < Duration::from_secs(2),
        "opening a 40,000-key wmet took {took:?}"
    );
}

/// Finding: text::key_values treats a song.txt it can't read as no
/// song.txt. wwav_pack.py dies on it (here song.txt is a folder:
/// IsADirectoryError, exit 1, nothing written); Rust packs the song under a
/// new random song_id and today's date, so the song silently becomes a new
/// work. The same holds for an unreadable song.txt (EACCES, EIO) and for
/// film.txt in swav pack.
#[test]
#[ignore = "review finding: an unreadable song.txt is ignored, not refused (text.rs key_values)"]
fn pack_refuses_a_song_txt_it_cannot_read() {
    let dir = tmp("review-song-txt");
    let folder = dir.join("01 Song");
    std::fs::create_dir_all(folder.join("song.txt")).unwrap();
    for (i, n) in NAMED.iter().enumerate() {
        std::fs::write(folder.join(n), wav(4, i as u32 + 1)).unwrap();
    }
    if wwav_pack().exists() && has("python3") {
        let o = python(
            &wwav_pack(),
            &["pack", s(&folder), "-o", s(&dir.join("py.wwav"))],
        );
        assert_eq!(o.status.code(), Some(1));
        assert!(!dir.join("py.wwav").exists());
    }
    let out = dir.join("rs.wwav");
    let r = pack::pack(s(&folder), s(&out), "", "");
    assert!(
        r.is_err(),
        "packed {:?} from a song.txt it couldn't read",
        r.map(|p| p.song_id)
    );
    assert!(!out.exists());
}

/// Finding: the help flags take no value in argparse, so `--help=x` and
/// `-h=x` are usage errors (exit 2, "ignored explicit argument 'x'");
/// `wwav` prints the usage and exits 0.
#[test]
#[ignore = "review finding: wwav takes --help=x as --help (bin/wwav.rs option)"]
fn help_with_a_value_is_a_usage_error() {
    for arg in ["--help=x", "-h=x"] {
        let o = wwav(&["info", arg]);
        assert_eq!(o.status.code(), Some(2), "wwav info {arg}");
        if wwav_pack().exists() && has("python3") {
            let p = python(&wwav_pack(), &["info", arg]);
            assert_eq!(p.status.code(), o.status.code(), "wwav info {arg}");
        }
    }
}

/// Packs `folder` with `wwav pack`, unpacks it with `wwav unpack`, packs
/// that folder again, and returns both .wwav files' bytes.
fn rust_round_trip(dir: &Path, folder: &Path) -> (Vec<u8>, Vec<u8>) {
    let (first, back, again) = (
        dir.join("first.wwav"),
        dir.join("back"),
        dir.join("again.wwav"),
    );
    ok(wwav(&["pack", s(folder), "-o", s(&first)]));
    ok(wwav(&["unpack", s(&first), "-o", s(&back)]));
    ok(wwav(&["pack", s(&back), "-o", s(&again)]));
    (
        std::fs::read(&first).unwrap(),
        std::fs::read(&again).unwrap(),
    )
}

/// Finding (F1, "Rust unpack then pack changes a byte"): a bpm that isn't
/// whole but rounds to a whole number at two decimals, as a tempo detector
/// gives (127.9988), is written "bpm": 128.00 by num(); unpack writes
/// "bpm = 128" (num() of the float 128.0), and pack then writes "bpm": 128.
/// wwav_pack.py does exactly the same, so the Rust crate matches it byte
/// for byte and still fails F1: the cause is the reference's num(), and the
/// criterion can't pass for these files until that is decided. 20.001
/// loses its bpm altogether ("20.00", then "20", which pack drops).
#[test]
#[ignore = "review finding: unpack then pack changes \"bpm\": 128.00 to 128 (num() in wwav_pack.py, mirrored)"]
fn unpack_then_pack_keeps_a_bpm_that_rounds_to_whole() {
    for bpm in ["127.9988", "120.001", "20.001"] {
        let dir = tmp("review-bpm");
        let txt = format!(
            "title = Song\nbpm = {bpm}\nsong_id = 0123456789abcdef0123456789abcdef\ncreated = 2026-10-07\n"
        );
        let folder = song_folder(&dir, "01 Song", 4, &NAMED, &txt);
        let (first, again) = rust_round_trip(&dir, &folder);
        assert!(
            first == again,
            "bpm {bpm}: {}",
            first_difference(&first, &again)
        );
    }
}

/// Finding (F1, the same criterion): a title taken from the folder's name
/// keeps the name's trailing space ("01 Song " is "Song "), unpack writes
/// "title = Song " and pack reads song.txt's values trimmed, so the second
/// pack's title is "Song". wwav_pack.py does the same.
#[test]
#[ignore = "review finding: unpack then pack trims a folder title's trailing space (song.txt is trimmed)"]
fn unpack_then_pack_keeps_a_title_from_the_folder() {
    let dir = tmp("review-title");
    let folder = song_folder(
        &dir,
        "01 Song ",
        4,
        &NAMED,
        "song_id = 0123456789abcdef0123456789abcdef\ncreated = 2026-10-07\n",
    );
    let (first, again) = rust_round_trip(&dir, &folder);
    assert!(first == again, "{}", first_difference(&first, &again));
}

/// Finding: `wwav unpack` and `wwav pack` don't check whether what they
/// write is what they read (swav pack and unpack, master_only and wrap_wav
/// now do). Unpacking a .wwav named master.wav into its own folder
/// truncates it to the 44-byte head before reading the master, and fails
/// with "failed to fill whole buffer" after the song is gone; a hard link
/// to it loses the song too. wwav_pack.py loses it the same way.
#[test]
#[ignore = "review finding: wwav unpack and pack write over their input (pack.rs has no not_same)"]
fn unpack_never_writes_over_its_input() {
    let dir = tmp("review-over");
    let song = dir.join("master.wav");
    std::fs::copy(corpus().join("original.wwav"), &song).unwrap();
    let before = std::fs::read(&song).unwrap();
    let o = wwav(&["unpack", s(&song), "-o", s(&dir)]);
    let after = std::fs::read(&song).unwrap();
    assert!(
        after == before,
        "exit {:?}, {}; the .wwav went from {} to {} bytes",
        o.status.code(),
        String::from_utf8_lossy(&o.stderr).trim(),
        before.len(),
        after.len()
    );
}

/// A RIFF WAVE of these chunks, each padded to even.
fn riff(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
    let body: Vec<u8> = chunks.iter().flat_map(|(id, b)| chunk(id, b)).collect();
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(4 + body.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend(body);
    out
}

/// Finding: info kept the chunk ids it had printed in a Vec and searched
/// it for every chunk, and repr of each float formatted 800 digits to find
/// a halfway tie, so a crafted file of many chunks or many floats held
/// `wwav info` for seconds where python took a fraction of one. Here
/// 100,000 distinct empty chunks and a wmet of 100,000 floats must each
/// read and print in well under the time a quadratic walk takes.
#[test]
fn many_chunks_and_many_floats_read_in_linear_time() {
    let dir = tmp("review-many");
    let fmt: Vec<u8> = [1u16, 2]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .chain(44_100u32.to_le_bytes())
        .chain(176_400u32.to_le_bytes())
        .chain(4u16.to_le_bytes())
        .chain(16u16.to_le_bytes())
        .collect();
    let ids: Vec<[u8; 4]> = (0..100_000u32).map(|i| i.to_le_bytes()).collect();
    let mut chunks: Vec<(&[u8; 4], &[u8])> = vec![(b"fmt ", &fmt), (b"data", &[0; 16])];
    chunks.extend(ids.iter().map(|id| (id, &[][..])));
    let floats: Vec<String> = (0..100_000)
        .map(|i| format!("{}", f64::from(i) * 1.37 + 0.001))
        .collect();
    let wmet = format!(r#"{{"wwav": "0.1", "x": [{}]}}"#, floats.join(", "));
    for (name, data) in [
        ("chunks.wwav", riff(&chunks)),
        (
            "floats.wwav",
            riff(&[
                (b"fmt ", &fmt),
                (b"data", &[0; 16]),
                (b"wmet", wmet.as_bytes()),
                (b"wlin", b"{}"),
            ]),
        ),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, data).unwrap();
        let start = Instant::now();
        let info = wwav::Wwav::open(&path).unwrap().info(s(&path));
        let took = start.elapsed();
        assert!(took < Duration::from_secs(3), "{name}: info took {took:?}");
        if wwav_pack().exists() && has("python3") {
            assert_eq!(
                info,
                ok(python(&wwav_pack(), &["info", s(&path)])),
                "{name}"
            );
        }
    }
}
