//! F1, writing: `wwav pack` and `wwav unpack` against wwav_pack.py.
//!
//! Fails if: a .wwav packed by Rust differs by any byte from wwav_pack.py
//! pack of the same folder and song.txt (or the tools print different
//! lines); Rust unpack then Rust pack changes a byte; Python unpack of a
//! Rust pack, packed again by Python, changes a byte; Rust unpack writes a
//! folder that differs from Python's; the two refuse differently; ffprobe
//! -v error prints anything for a packed file.

mod common;

use std::path::{Path, PathBuf};

use common::*;

const SONG_ID: &str = "0123456789abcdef0123456789abcdef";

fn song_txt(extra: &str) -> String {
    format!("title = Tést \"Song\"\nartist = Mi\nbpm = 120.125\nkey = A minor\nsong_id = {SONG_ID}\ncreated = 2026-10-03\n{extra}")
}

/// Packs `folder` with both tools (the same extra arguments) and checks
/// the bytes and the printed line are the same. Returns the Rust .wwav.
fn pack_both(folder: &Path, args: &[&str]) -> PathBuf {
    let dir = folder.parent().unwrap();
    let name = folder.file_name().unwrap().to_str().unwrap();
    let (py, rs) = (
        dir.join(format!("{name}.py.wwav")),
        dir.join(format!("{name}.rs.wwav")),
    );
    let reference = python(
        &wwav_pack(),
        &[&["pack", s(folder), "-o", s(&py)], args].concat(),
    );
    let ours = wwav(&[&["pack", s(folder), "-o", s(&rs)], args].concat());
    let (mut want, mut got) = (said(&reference), said(&ours));
    // the printed line names the output, which differs on purpose
    want.0 = want.0.replace(".py.wwav", ".?.wwav");
    got.0 = got.0.replace(".rs.wwav", ".?.wwav");
    same_run(got, want, name);
    if reference.status.success() {
        same_bytes(&rs, &py);
    }
    rs
}

#[test]
fn pack_writes_the_bytes_wwav_pack_writes() {
    if !have_references("pack_writes_the_bytes_wwav_pack_writes") {
        return;
    }
    let dir = tmp("pack-bytes");
    let folders: Vec<(PathBuf, Vec<&str>)> = vec![
        (song_folder(&dir, "01 Test Song", 3000, &NAMED, &song_txt("")), vec!["--creator", "liam_made_young"]),
        (song_folder(&dir, "02 Split", 3001, &NAMED, &song_txt("")), vec!["--splitter", "demucs", "--creator", "a b"]),
        // three passes of 65536 frames, the last one short
        (song_folder(&dir, "03 Long", 140_001, &NAMED, &song_txt("")), vec![]),
        // a breadboard folder: 1.wav is the master, 2-5 the stems
        (song_folder(&dir, "04 Legacy", 777, &["1.wav", "2.wav", "3.wav", "4.wav", "5.wav"], &song_txt("")), vec![]),
        (song_folder(&dir, "05 Upper", 100, &["MASTER.WAV", "Vocals.wav", "DRUMS.wav", "other.WAV", "Bass.Wav"], &song_txt("")), vec![]),
        // song.txt as people write it: CRLF, comments, odd spacing, keys in
        // capitals, no title (the folder names it), a bpm python's float reads
        (
            song_folder(
                &dir,
                "06 - Folder Title",
                500,
                &NAMED,
                &format!("# a comment\r\n  BPM =  1_20.5 \r\nKey= C#\r\nnot a line\r\nSONG_ID = {SONG_ID}\r\ncreated = 2026-01-02\r\n"),
            ),
            vec![],
        ),
        (song_folder(&dir, "07", 10, &NAMED, &format!("title = Ünïcödé 🎵\ttab \\ \u{1}\nartist = Ωmega\nbpm = ١٢٠\nsong_id = {SONG_ID}\ncreated = x\n")), vec!["--creator", "é"]),
        // values the device would skip: bpm out of range or not a number
        (song_folder(&dir, "08 Bpm", 10, &NAMED, &format!("bpm = 400\nsong_id = {SONG_ID}\ncreated = c\n")), vec![]),
        (song_folder(&dir, "09 Bpm", 10, &NAMED, &format!("bpm = nan\nsong_id = {SONG_ID}\ncreated = c\n")), vec![]),
        (song_folder(&dir, "10 Bpm", 10, &NAMED, &format!("bpm = 20.000001\nsong_id = {SONG_ID}\ncreated = c\n")), vec![]),
        (song_folder(&dir, "11 Bpm", 10, &NAMED, &format!("bpm = 97.335\nsong_id = {SONG_ID}\ncreated = c\n")), vec![]),
        (song_folder(&dir, "12 Odd", 10, &NAMED, &format!("title = Odds\nsong_id = {SONG_ID}\ncreated = 2026-10-03\n")), vec!["--creator", "od"]),
        // a BOM and bytes that aren't UTF-8, which python reads as U+FFFD
        (song_folder(&dir, "13 Bytes", 10, &NAMED, ""), vec![]),
        // an empty song: no frames at all
        (song_folder(&dir, "14 Empty", 0, &NAMED, &song_txt("")), vec![]),
    ];
    std::fs::write(
        dir.join("13 Bytes/song.txt"),
        [
            &b"\xef\xbb\xbftitle = x\nartist = a\xffb\xe2\x82\nsong_id = "[..],
            SONG_ID.as_bytes(),
            b"\ncreated = 2026-10-03",
        ]
        .concat(),
    )
    .unwrap();
    for (folder, args) in folders {
        pack_both(&folder, &args);
    }
}

#[test]
fn pack_takes_the_title_from_the_folder_as_the_tool_does() {
    if !have_references("pack_takes_the_title_from_the_folder_as_the_tool_does") {
        return;
    }
    let dir = tmp("pack-titles");
    let txt = format!("song_id = {SONG_ID}\ncreated = 2026-10-03\n");
    for name in [
        "01 Song",
        "1-Song",
        "0001_.Song",
        "12345 Song",
        "Song",
        "01",
        "01  ",
        "٠٣ Arabic",
        "01 . - _x",
    ] {
        let folder = song_folder(&dir, name, 10, &NAMED, &txt);
        pack_both(&folder, &[]);
        // the default output path: next to the folder, from its normalized name
        let trailing = format!("{}/", s(&folder));
        let reference = python(&wwav_pack(), &["pack", &trailing]);
        let py = std::fs::read(format!("{}.wwav", s(&folder))).unwrap();
        let ours = wwav(&["pack", &trailing]);
        same_run(said(&ours), said(&reference), name);
        assert!(
            std::fs::read(format!("{}.wwav", s(&folder))).unwrap() == py,
            "{name}"
        );
    }
}

#[test]
fn pack_makes_an_id_when_song_txt_has_none() {
    if !have_references("pack_makes_an_id_when_song_txt_has_none") {
        return;
    }
    let dir = tmp("pack-new-id");
    let folder = song_folder(
        &dir,
        "01 No Id",
        10,
        &NAMED,
        "song_id = 0123\ncreated = 2026-10-03\n",
    );
    let (py, rs) = (dir.join("py.wwav"), dir.join("rs.wwav"));
    ok(python(&wwav_pack(), &["pack", s(&folder), "-o", s(&py)]));
    let line = ok(wwav(&["pack", s(&folder), "-o", s(&rs)]));
    let id = line.trim_end().rsplit(' ').next().unwrap().to_string();
    assert!(
        id.len() == 32
            && id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "{id}"
    );
    let theirs = std::fs::read(&py).unwrap();
    let py_id = &theirs[theirs
        .windows(12)
        .position(|w| w == b"\"song_id\": \"")
        .unwrap()
        + 12..][..32];
    // with python's random id in place of ours (in wmet and wlin), the
    // files are the same
    let mut ours = std::fs::read(&rs).unwrap();
    let mut swapped = 0;
    while let Some(i) = ours.windows(32).position(|w| w == id.as_bytes()) {
        ours[i..i + 32].copy_from_slice(py_id);
        swapped += 1;
    }
    assert_eq!(swapped, 2);
    assert!(ours == theirs, "{}", first_difference(&ours, &theirs));
}

#[test]
fn pack_today_is_the_tools_today() {
    if !have_references("pack_today_is_the_tools_today") {
        return;
    }
    // created missing or empty: today, by the local clock
    let dir = tmp("pack-today");
    let folder = song_folder(
        &dir,
        "01 Today",
        10,
        &NAMED,
        &format!("song_id = {SONG_ID}\ncreated =\n"),
    );
    pack_both(&folder, &[]);
}

#[test]
fn pack_refuses_what_wwav_pack_refuses() {
    if !have_references("pack_refuses_what_wwav_pack_refuses") {
        return;
    }
    let dir = tmp("pack-refuses");
    let txt = song_txt("");
    let missing = song_folder(&dir, "01 Missing", 10, &["master.wav", "vocals.wav"], &txt);
    let legacy_missing = song_folder(&dir, "02 Legacy", 10, &["1.wav", "2.wav", "5.wav"], &txt);
    let lengths = song_folder(&dir, "03 Lengths", 10, &NAMED, &txt);
    std::fs::write(lengths.join("drums.wav"), wav(11, 9)).unwrap();
    let rate = song_folder(&dir, "04 Rate", 10, &NAMED, &txt);
    std::fs::write(rate.join("other.wav"), wav_as(10, 1, 48000, 16, 2)).unwrap();
    let not_wav = song_folder(&dir, "05 Not Wav", 10, &NAMED, &txt);
    std::fs::write(not_wav.join("bass.wav"), b"OggS and so on").unwrap();
    let no_data = song_folder(&dir, "06 No Data", 10, &NAMED, &txt);
    std::fs::write(no_data.join("master.wav"), &wav(10, 1)[..36]).unwrap();
    let no_fmt = song_folder(&dir, "07 No Fmt", 10, &NAMED, &txt);
    std::fs::write(
        no_fmt.join("master.wav"),
        [
            &b"RIFF\x0c\x00\x00\x00WAVEdata\x04\x00\x00\x00"[..],
            &[0; 4],
        ]
        .concat(),
    )
    .unwrap();
    let mono = song_folder(&dir, "08 Mono", 10, &NAMED, &txt);
    std::fs::write(mono.join("vocals.wav"), wav_as(10, 1, 44100, 16, 1)).unwrap();
    // data size 0 means "to the end of the file": these are 10 frames
    let zero = song_folder(&dir, "09 Zero", 10, &NAMED, &txt);
    let mut z = wav(10, 3);
    z[40..44].copy_from_slice(&[0; 4]);
    std::fs::write(zero.join("bass.wav"), z).unwrap();
    std::fs::write(dir.join("a file"), b"").unwrap();
    for folder in [
        &missing,
        &legacy_missing,
        &lengths,
        &rate,
        &not_wav,
        &no_data,
        &no_fmt,
        &mono,
        &zero,
        &dir.join("a file"),
        &dir.join("nothing here"),
    ] {
        pack_both(folder, &[]);
    }
}

#[test]
fn pack_refuses_over_4_gb_as_wwav_pack_does() {
    if !have_references("pack_refuses_over_4_gb_as_wwav_pack_does") {
        return;
    }
    // 214,800,000 frames is 20 bytes a frame over 4 GB. The files are sparse:
    // a header and a hole, which neither tool reads before refusing.
    let dir = tmp("pack-4gb");
    let folder = dir.join("01 Long");
    std::fs::create_dir_all(&folder).unwrap();
    let frames: u64 = 214_800_000;
    for n in NAMED {
        let mut head = wav(0, 0);
        head[4..8].copy_from_slice(&((36 + frames * 4) as u32).to_le_bytes());
        head[40..44].copy_from_slice(&((frames * 4) as u32).to_le_bytes());
        std::fs::write(folder.join(n), &head).unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(folder.join(n))
            .unwrap()
            .set_len(44 + frames * 4)
            .unwrap();
    }
    std::fs::write(folder.join("song.txt"), song_txt("")).unwrap();
    pack_both(&folder, &[]);
    assert!(!dir.join("01 Long.rs.wwav").exists());
}

#[test]
fn unpack_writes_the_folder_wwav_pack_writes() {
    if !have_references("unpack_writes_the_folder_wwav_pack_writes") {
        return;
    }
    let dir = tmp("unpack");
    for e in manifest().into_iter().filter(|e| !is_film(&e.file)) {
        let src = corpus().join(&e.file);
        let (py, rs) = (
            dir.join(format!("{}.py", e.file)),
            dir.join(format!("{}.rs", e.file)),
        );
        let reference = python(&wwav_pack(), &["unpack", s(&src), "-o", s(&py)]);
        let ours = wwav(&["unpack", s(&src), "-o", s(&rs)]);
        let (mut want, mut got) = (said(&reference), said(&ours));
        want.0 = want.0.replace(".py/", ".?/");
        got.0 = got.0.replace(".rs/", ".?/");
        same_run(got, want, &e.file);
        let mut names: Vec<_> = std::fs::read_dir(&py)
            .map(|d| d.map(|f| f.unwrap().file_name()).collect())
            .unwrap_or_default();
        names.sort();
        let mut ours: Vec<_> = std::fs::read_dir(&rs)
            .map(|d| d.map(|f| f.unwrap().file_name()).collect())
            .unwrap_or_default();
        ours.sort();
        assert_eq!(ours, names, "{}", e.file);
        for n in names {
            same_bytes(&rs.join(&n), &py.join(&n));
        }
    }
}

#[test]
fn unpack_then_pack_gives_the_same_bytes() {
    if !have_references("unpack_then_pack_gives_the_same_bytes") {
        return;
    }
    let dir = tmp("round-trip");
    let long = song_folder(&dir, "01 Long", 140_001, &NAMED, &song_txt(""));
    let packed = pack_both(&long, &["--creator", ""]);
    let originals = [
        corpus().join("original.wwav"),
        corpus().join("odd-chunks.wwav"),
        corpus().join("unicode.wwav"),
        packed,
    ];
    for (i, src) in originals.iter().enumerate() {
        // Rust unpack, Rust pack
        let folder = dir.join(format!("{i:02} Again"));
        ok(wwav(&["unpack", s(src), "-o", s(&folder)]));
        let creator = creator_of(src);
        let splitter = if s(src).contains("unicode") {
            "demucs"
        } else {
            ""
        };
        let again = dir.join(format!("{i}.again.wwav"));
        ok(wwav(&[
            "pack",
            s(&folder),
            "-o",
            s(&again),
            "--creator",
            &creator,
            "--splitter",
            splitter,
        ]));
        same_bytes(&again, src);

        // Python unpack of the Rust pack, Python pack
        let folder = dir.join(format!("{i:02} Python"));
        ok(python(
            &wwav_pack(),
            &["unpack", s(&again), "-o", s(&folder)],
        ));
        let python_again = dir.join(format!("{i}.python.wwav"));
        ok(python(
            &wwav_pack(),
            &[
                "pack",
                s(&folder),
                "-o",
                s(&python_again),
                "--creator",
                &creator,
                "--splitter",
                splitter,
            ],
        ));
        same_bytes(&python_again, &again);
    }
}

/// wlin's creator, which song.txt doesn't keep: pack takes it as an argument.
fn creator_of(wwav_file: &Path) -> String {
    let w = wwav_formats::wwav::Wwav::open(wwav_file).unwrap();
    match &w.wlin {
        Some(wwav_formats::json::Value::Dict(d)) => match wwav_formats::json::get(d, "creator") {
            Some(wwav_formats::json::Value::Str(c)) => c.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

#[test]
fn ffprobe_reads_a_packed_wwav_without_a_word() {
    if !has("ffprobe") {
        eprintln!("ffprobe_reads_a_packed_wwav_without_a_word: skipped, it needs ffprobe");
        return;
    }
    let dir = tmp("ffprobe-wwav");
    let folder = song_folder(&dir, "01 Probe", 70_000, &NAMED, &song_txt(""));
    let out = dir.join("probe.wwav");
    ok(wwav(&["pack", s(&folder), "-o", s(&out)]));
    for file in [
        out,
        corpus().join("remix.wwav"),
        corpus().join("master-only.wwav"),
    ] {
        let o = run("ffprobe", &["-v", "error", s(&file)]);
        assert!(
            o.status.success() && o.stdout.is_empty() && o.stderr.is_empty(),
            "{}: {}",
            file.display(),
            String::from_utf8_lossy(&o.stderr)
        );
        let o = run(
            "ffmpeg",
            &["-v", "error", "-i", s(&file), "-f", "null", "-"],
        );
        assert!(
            o.status.success() && o.stderr.is_empty(),
            "{}: {}",
            file.display(),
            String::from_utf8_lossy(&o.stderr)
        );
    }
}
