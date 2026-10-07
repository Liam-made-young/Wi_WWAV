//! Adversarial review of F1, F2 and F3 (docs/PLAN.md): files and command
//! lines the corpus and the fuzz don't make, read and written by this
//! crate against what the reference tools do with them.
//!
//! Each test here failed when it was written, on a real defect. The two
//! still marked #[ignore] fail in wwav_pack.py itself, which this crate
//! matches byte for byte on purpose: they wait on a decision about the
//! reference (F1 stays failed until then). Run them with
//! `cargo test -p wwav-formats --test review_findings -- --ignored`.

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::*;
use wwav_formats::meta::{Kind, Lineage, SongMeta, Wrmx};
use wwav_formats::writer::WwavWriter;
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
#[ignore = "open question on the reference: wwav_pack.py's num() writes 128.00, which unpack and pack give back as 128 (mirrored on purpose)"]
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
#[ignore = "open question on the reference: wwav_pack.py's song.txt is read trimmed, so a folder title's trailing space is lost (mirrored on purpose)"]
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

/// The same finding for films: a film.txt swav_pack.py can't read (here a
/// folder) makes it die without writing; the Rust pack has to refuse too,
/// and only after the refusals swav_pack.py makes first.
#[test]
fn swav_pack_refuses_a_film_txt_it_cannot_read() {
    let dir = tmp("review-film-txt");
    let film = dir.join("film.mp4");
    std::fs::copy(corpus().join("plain.mp4"), &film).unwrap();
    std::fs::create_dir_all(dir.join("film.txt")).unwrap();
    if swav_pack().exists() && has("python3") {
        let o = python(
            &swav_pack(),
            &["pack", s(&film), "-o", s(&dir.join("py.swav"))],
        );
        assert_eq!(o.status.code(), Some(1));
        assert!(!dir.join("py.swav").exists());
    }
    let out = dir.join("rs.swav");
    let r = swav::pack_original(s(&film), s(&out), "", "", "");
    assert!(r.is_err(), "packed {:?}", r.map(|(m, _)| m.film_id));
    assert!(!out.exists());
    // a film that's already a .swav is refused for that first, as python does
    let packed = dir.join("packed.mp4");
    std::fs::copy(corpus().join("plain.swav"), &packed).unwrap();
    std::fs::create_dir_all(dir.join("packed.txt")).unwrap();
    let e = swav::pack_original(s(&packed), s(&out), "", "", "").unwrap_err();
    assert!(
        e.to_string()
            .ends_with("already has a wmet or wlin box (unpack it first)"),
        "{e}"
    );
}

/// The same finding for pack: `wwav pack F -o F/master.wav` (or -o a hard
/// link to any of its WAVs) writes the .wwav over the master it is
/// reading. It has to refuse, in swav_pack.py's words, and leave the
/// folder as it was; so does unpack into a folder holding a hard link to
/// the .wwav under a stem's name.
#[test]
fn pack_never_writes_over_its_input() {
    let dir = tmp("review-over-pack");
    let folder = song_folder(&dir, "01 Song", 600, &NAMED, "title = x\n");
    let before = std::fs::read(folder.join("master.wav")).unwrap();
    std::fs::hard_link(folder.join("bass.wav"), dir.join("bass-link.wwav")).unwrap();
    for out in [folder.join("master.wav"), dir.join("bass-link.wwav")] {
        let o = wwav(&["pack", s(&folder), "-o", s(&out)]);
        assert_eq!(o.status.code(), Some(1), "pack -o {}", out.display());
        let said = String::from_utf8_lossy(&o.stderr);
        assert!(said.contains("would write over"), "{said}");
    }
    assert!(std::fs::read(folder.join("master.wav")).unwrap() == before);
    assert!(std::fs::read(folder.join("bass.wav")).unwrap() == wav(600, 5));

    let song = dir.join("song.wwav");
    std::fs::copy(corpus().join("original.wwav"), &song).unwrap();
    let back = dir.join("back");
    std::fs::create_dir_all(&back).unwrap();
    std::fs::hard_link(&song, back.join("drums.wav")).unwrap();
    let whole = std::fs::read(&song).unwrap();
    let o = wwav(&["unpack", s(&song), "-o", s(&back)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(std::fs::read(&song).unwrap() == whole);
    assert!(
        !back.join("master.wav").exists(),
        "unpack wrote before refusing"
    );
}

/// An original the Console exports: four frames of silence with `meta`.
fn export(path: &Path, meta: &SongMeta, lineage: &Lineage) -> Result<u64, wwav_formats::Error> {
    let mut w = WwavWriter::create(path, 4, meta, lineage, None)?;
    w.write_master(&[0; 8])?;
    w.write_stems([&[1; 8], &[2; 8], &[3; 8], &[4; 8]])?;
    w.finish()
}

/// Finding: WwavWriter wrote whatever metadata it was given, so an
/// original exported with a title wwav_pack.py's song.txt can't hold
/// ("Low Tide "), a bpm its num() can't give back (127.9988 is written
/// 128.00 and comes back 128; 400 is dropped) or no created date came back
/// from `wwav_pack.py unpack` then `pack` with another sha256 (6.13.1).
/// Such an original is refused, and `SongMeta::normalized()` of it is
/// written and survives the round trip byte for byte.
#[test]
fn an_original_export_survives_the_reference_round_trip_or_is_refused() {
    let dir = tmp("review-export");
    let good = SongMeta {
        song_id: "0123456789abcdef0123456789abcdef".into(),
        title: "Low Tide".into(),
        artist: "LMY".into(),
        bpm: 120.125,
        key: "A minor".into(),
        kind: Kind::Original,
        splitter: String::new(),
        created: "2026-10-07".into(),
    };
    let lineage = Lineage::original(&good.song_id, "liam_made_young");
    let path = dir.join("good.wwav");
    export(&path, &good, &lineage).unwrap();
    let mut cases: Vec<SongMeta> = Vec::new();
    for (title, artist, key, created) in [
        ("Low Tide ", "LMY", "", "2026-10-07"),
        ("Low\nTide", "LMY", "", "2026-10-07"),
        ("Low Tide", " LMY", "", "2026-10-07"),
        ("Low Tide", "LMY", "A minor\r", "2026-10-07"),
        ("Low Tide", "LMY", "", " 2026-10-07"),
    ] {
        cases.push(SongMeta {
            title: title.into(),
            artist: artist.into(),
            key: key.into(),
            created: created.into(),
            ..good.clone()
        });
    }
    for bpm in [127.9988, 120.001, 20.001, 400.0, 399.996, 10.0, -5.0] {
        cases.push(SongMeta {
            bpm,
            ..good.clone()
        });
    }
    for (i, meta) in cases.iter().enumerate() {
        let path = dir.join(format!("{i}.wwav"));
        let e = export(&path, meta, &lineage);
        assert!(e.is_err(), "{meta:?} was written");
        assert!(!path.exists());
        let fixed = meta.normalized();
        export(&path, &fixed, &lineage).unwrap_or_else(|e| panic!("{fixed:?}: {e}"));
        if wwav_pack().exists() && has("python3") {
            let (back, again) = (
                dir.join(format!("{i}")),
                dir.join(format!("{i}.again.wwav")),
            );
            ok(python(&wwav_pack(), &["unpack", s(&path), "-o", s(&back)]));
            let creator = "--creator=liam_made_young";
            ok(python(
                &wwav_pack(),
                &["pack", s(&back), "-o", s(&again), creator],
            ));
            let (a, b) = (
                std::fs::read(&path).unwrap(),
                std::fs::read(&again).unwrap(),
            );
            assert!(a == b, "{fixed:?}: {}", first_difference(&a, &b));
        }
    }
    // what pack writes in place of a missing title, song_id or date, and an
    // original's lineage and lack of wrmx, can't come back either
    let refused = [
        (
            SongMeta {
                title: " ".into(),
                ..good.clone()
            },
            lineage.clone(),
        ),
        (
            SongMeta {
                created: String::new(),
                ..good.clone()
            },
            lineage.clone(),
        ),
        (
            SongMeta {
                song_id: "0123".into(),
                ..good.clone()
            },
            Lineage::original("0123", ""),
        ),
        (
            good.clone(),
            Lineage {
                generation: 1,
                ..lineage.clone()
            },
        ),
        (
            good.clone(),
            Lineage::child_of("fedcba9876543210fedcba9876543210", &lineage, ""),
        ),
        (
            good.clone(),
            Lineage {
                device_id: "x".into(),
                ..lineage.clone()
            },
        ),
    ];
    for (i, (meta, lineage)) in refused.iter().enumerate() {
        let path = dir.join(format!("refused-{i}.wwav"));
        assert!(
            export(&path, meta, lineage).is_err(),
            "{meta:?} {lineage:?}"
        );
        assert!(!path.exists());
    }
    let wrmx = Wrmx::default();
    let path = dir.join("wrmx.wwav");
    assert!(WwavWriter::create(&path, 4, &good, &lineage, Some(&wrmx)).is_err());
}

/// Finding: WwavWriter truncated the file at its final name and wrote it
/// in place, so an export over an existing song destroyed it at once, and
/// one cut short (an error, a cancelled or killed export) left a song with
/// the right RIFF size and no wlin, which reads as "the master only".
/// Until finish(), the old file has to stay as it was, and a writer
/// dropped part way has to leave nothing behind.
#[test]
fn an_export_cut_short_leaves_the_old_file() {
    let dir = tmp("review-staged");
    let path = dir.join("Low Tide.wwav");
    let old = std::fs::read(corpus().join("original.wwav")).unwrap();
    std::fs::write(&path, &old).unwrap();
    let meta = SongMeta {
        song_id: "0123456789abcdef0123456789abcdef".into(),
        title: "Low Tide".into(),
        created: "2026-10-07".into(),
        ..SongMeta::default()
    };
    let lineage = Lineage::original(&meta.song_id, "");
    let mut w = WwavWriter::create(&path, 4, &meta, &lineage, None).unwrap();
    w.write_master(&[0; 8]).unwrap();
    assert!(
        std::fs::read(&path).unwrap() == old,
        "the old song changed before finish()"
    );
    drop(w);
    assert!(
        std::fs::read(&path).unwrap() == old,
        "a dropped export changed the old song"
    );
    let names = |d: &Path| -> Vec<String> {
        let mut n: Vec<String> = std::fs::read_dir(d)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        n.sort();
        n
    };
    assert_eq!(
        names(&dir),
        ["Low Tide.wwav"],
        "a dropped export left a file"
    );
    assert_eq!(
        export(&path, &meta, &lineage).unwrap(),
        std::fs::metadata(&path).unwrap().len()
    );
    assert_eq!(
        wwav::Wwav::open(&path).unwrap().verdict().to_string(),
        "4 stems, and the master"
    );
    assert_eq!(names(&dir), ["Low Tide.wwav"]);
    // the film writer too
    let film = dir.join("film.swav");
    let film_meta = swav::FilmMeta {
        film_id: meta.song_id.clone(),
        ..Default::default()
    };
    swav::pack(&corpus().join("plain.mp4"), &film, &film_meta, &lineage).unwrap();
    assert_eq!(names(&dir), ["Low Tide.wwav", "film.swav"]);
}
