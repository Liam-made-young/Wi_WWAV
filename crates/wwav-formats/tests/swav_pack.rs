//! F2 and the format half of S0.6: `wwav swav pack` and `unpack` against
//! swav_pack.py, and what ffprobe makes of the result.
//!
//! Fails if: the Rust packer's bytes differ from swav_pack.py pack's for
//! the same film and film.txt (or the tools print different lines);
//! unpacking doesn't return the MP4 byte for byte, or writes another
//! film.txt or warning than the tool; the two refuse differently; ffprobe
//! -v error prints anything for a packed file; swav_pack.py info can't read
//! the wmet and wlin of a film the library packs with a song as its parent.

mod common;

use std::path::{Path, PathBuf};

use common::*;
use wwav_formats::meta::{Kind, Lineage};
use wwav_formats::swav::{self, FilmMeta};

const FILM_ID: &str = "00112233445566778899aabbccddeeff";
const FILM_TXT: &str = "title = Film \"One\" — é\nartist = Mi\nfilm_id = 00112233445566778899aabbccddeeff\ncreated = 2026-10-03\ncreator = liam_made_young\n";

/// A copy of a corpus film in its own folder, with this film.txt beside it.
fn film(dir: &Path, name: &str, txt: Option<&str>) -> PathBuf {
    let sub = dir.join(name.replace('.', "-"));
    std::fs::create_dir_all(&sub).unwrap();
    let path = sub.join(name);
    std::fs::copy(corpus().join(name), &path).unwrap();
    if let Some(t) = txt {
        std::fs::write(
            sub.join(format!("{}.txt", name.rsplit_once('.').unwrap().0)),
            t,
        )
        .unwrap();
    }
    path
}

/// Packs with both tools, the same arguments; checks the printed lines and
/// the bytes. Returns the Rust .swav.
fn pack_both(src: &Path, args: &[&str]) -> PathBuf {
    let (py, rs) = (src.with_extension("py.swav"), src.with_extension("rs.swav"));
    let reference = python(
        &swav_pack(),
        &[&["pack", s(src), "-o", s(&py)], args].concat(),
    );
    let ours = wwav(&[&["swav", "pack", s(src), "-o", s(&rs)], args].concat());
    let (mut want, mut got) = (said(&reference), said(&ours));
    want.0 = want.0.replace(".py.swav", ".?.swav");
    got.0 = got.0.replace(".rs.swav", ".?.swav");
    same_run(got, want, &src.display().to_string());
    if reference.status.success() {
        same_bytes(&rs, &py);
    }
    rs
}

#[test]
fn pack_writes_the_bytes_swav_pack_writes() {
    if !have_references("pack_writes_the_bytes_swav_pack_writes") {
        return;
    }
    let dir = tmp("swav-pack");
    for name in ["plain.mp4", "fast.mp4", "zero.mp4", "large.mp4"] {
        pack_both(&film(&dir, name, Some(FILM_TXT)), &[]);
    }
    let src = film(&dir, "plain.mp4", Some(FILM_TXT));
    pack_both(
        &src,
        &["--title", "Other", "--artist", "Ü", "--creator", "x"],
    );
    pack_both(&src, &["--title", "", "--creator", ""]);
    // created with no date stays unknown; film.txt as people write it
    let blank = film(
        &dir,
        "fast.mp4",
        Some(&format!(
            "TITLE= a \r\ncreated =\r\n# film_id = no\r\nfilm_id = {FILM_ID}\r\n"
        )),
    );
    pack_both(&blank, &[]);
}

#[test]
fn pack_without_film_txt_names_the_film_after_its_file() {
    if !have_references("pack_without_film_txt_names_the_film_after_its_file") {
        return;
    }
    // no film.txt: the title is the file's name, the id is new and created
    // is today; with python's id in place of ours the bytes are the same
    let dir = tmp("swav-no-txt");
    let src = film(&dir, "plain.mp4", None);
    let (py, rs) = (dir.join("py.swav"), dir.join("rs.swav"));
    ok(python(&swav_pack(), &["pack", s(&src), "-o", s(&py)]));
    let line = ok(wwav(&["swav", "pack", s(&src), "-o", s(&rs)]));
    assert!(line.contains(": plain, "), "{line}");
    let id = line.trim_end().rsplit(' ').next().unwrap();
    let theirs = std::fs::read(&py).unwrap();
    let py_id = &theirs[theirs
        .windows(12)
        .position(|w| w == b"\"film_id\": \"")
        .unwrap()
        + 12..][..32];
    let mut ours = std::fs::read(&rs).unwrap();
    while let Some(i) = ours.windows(32).position(|w| w == id.as_bytes()) {
        ours[i..i + 32].copy_from_slice(py_id);
    }
    assert!(ours == theirs, "{}", first_difference(&ours, &theirs));
    // the default output name: the film's, with .swav
    let reference = python(&swav_pack(), &["pack", s(&src)]);
    let py_default = std::fs::read(src.with_extension("swav")).unwrap();
    std::fs::remove_file(src.with_extension("swav")).unwrap();
    let mine = wwav(&["swav", "pack", s(&src)]);
    assert_eq!(said(&mine).2, said(&reference).2);
    assert_eq!(
        std::fs::read(src.with_extension("swav")).unwrap().len(),
        py_default.len()
    );
}

#[test]
fn pack_refuses_what_swav_pack_refuses() {
    if !have_references("pack_refuses_what_swav_pack_refuses") {
        return;
    }
    let dir = tmp("swav-refuses");
    let mp4 = std::fs::read(corpus().join("plain.mp4")).unwrap();
    let write = |name: &str, bytes: &[u8]| {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    };
    let cases = [
        film(&dir, "plain.swav", Some(FILM_TXT)),
        write("text.mp4", b"just some text, long enough"),
        write("cut.mp4", &mp4[..mp4.len() - 100]),
        write("ftyp.mp4", &mp4[..32]),
        write("empty.mp4", b""),
        write(
            "broken.mp4",
            &[&mp4[..], b"\x00\x00\x00\x0awmet{}"].concat(),
        ),
        dir.join("missing.mp4"),
    ];
    for src in &cases {
        pack_both(src, &[]);
    }
    // writing over the film itself
    let src = film(&dir, "fast.mp4", Some(FILM_TXT));
    let reference = python(&swav_pack(), &["pack", s(&src), "-o", s(&src)]);
    let ours = wwav(&["swav", "pack", s(&src), "-o", s(&src)]);
    same_run(said(&ours), said(&reference), "writing over the film");
    same_bytes(&src, &corpus().join("fast.mp4"));
    // or over another name for it: os.path.samefile knows a hard link
    let link = src.with_extension("swav");
    std::fs::hard_link(&src, &link).unwrap();
    for cmd in [&["swav", "pack"][..], &["swav", "unpack"][..]] {
        let reference = python(&swav_pack(), &[cmd[1], s(&src), "-o", s(&link)]);
        let ours = wwav(&[cmd, &[s(&src), "-o", s(&link)]].concat());
        same_run(said(&ours), said(&reference), "writing over a hard link");
        same_bytes(&src, &corpus().join("fast.mp4"));
    }
}

#[test]
fn pack_refuses_a_sizeless_last_box_over_4_gb() {
    if !have_references("pack_refuses_a_sizeless_last_box_over_4_gb") {
        return;
    }
    // ftyp, moov, then an mdat of size 0 that runs 5 GB to the end: a
    // sparse file, of which only the headers are read
    let dir = tmp("swav-4gb");
    let path = dir.join("huge.mp4");
    let mut head = vec![0u8; 32];
    head[..4].copy_from_slice(&16u32.to_be_bytes());
    head[4..12].copy_from_slice(b"ftypisom");
    head[16..20].copy_from_slice(&8u32.to_be_bytes());
    head[20..24].copy_from_slice(b"moov");
    head[28..32].copy_from_slice(b"mdat");
    std::fs::write(&path, &head).unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(5 << 30)
        .unwrap();
    std::fs::write(dir.join("huge.txt"), FILM_TXT).unwrap();
    pack_both(&path, &[]);
}

#[test]
fn unpack_gives_back_the_mp4_and_film_txt_the_tool_writes() {
    if !have_references("unpack_gives_back_the_mp4_and_film_txt_the_tool_writes") {
        return;
    }
    let dir = tmp("swav-unpack");
    for e in manifest().into_iter().filter(|e| is_film(&e.file)) {
        let src = corpus().join(&e.file);
        let (py, rs) = (
            dir.join(format!("{}.py.mp4", e.file)),
            dir.join(format!("{}.rs.mp4", e.file)),
        );
        let reference = python(&swav_pack(), &["unpack", s(&src), "-o", s(&py)]);
        let ours = wwav(&["swav", "unpack", s(&src), "-o", s(&rs)]);
        let (mut want, mut got) = (said(&reference), said(&ours));
        for t in [&mut want.0, &mut want.1] {
            *t = t.replace(".py.", ".?.");
        }
        for t in [&mut got.0, &mut got.1] {
            *t = t.replace(".rs.", ".?.");
        }
        same_run(got, want, &e.file);
        for (a, b) in [
            (&rs, &py),
            (&rs.with_extension("txt"), &py.with_extension("txt")),
        ] {
            assert_eq!(a.exists(), b.exists(), "{}", e.file);
            if b.exists() {
                same_bytes(a, b);
            }
        }
    }
    // the MP4 comes back byte for byte (the size-0 fix stays)
    for (swav_file, mp4) in [
        ("plain.swav", "plain.mp4"),
        ("zero.swav", "fast.mp4"),
        ("large.swav", "large.mp4"),
    ] {
        same_bytes(
            &dir.join(format!("{swav_file}.rs.mp4")),
            &corpus().join(mp4),
        );
    }
}

#[test]
fn unpack_then_pack_gives_the_same_swav() {
    if !have_references("unpack_then_pack_gives_the_same_swav") {
        return;
    }
    let dir = tmp("swav-round-trip");
    for name in ["plain.swav", "zero.swav", "large.swav"] {
        let back = dir.join(format!("{name}.mp4"));
        ok(wwav(&[
            "swav",
            "unpack",
            s(&corpus().join(name)),
            "-o",
            s(&back),
        ]));
        let again = dir.join(format!("{name}.again.swav"));
        ok(wwav(&["swav", "pack", s(&back), "-o", s(&again)]));
        same_bytes(&again, &corpus().join(name));
    }
}

/// A film the Console's export would write: its parent is the song made
/// in the same export ("a film made from a song is its child").
fn packed_child(dir: &Path) -> PathBuf {
    let song = Lineage::original("0123456789abcdef0123456789abcdef", "liam_made_young");
    let lineage = Lineage::child_of("0123456789abcdef0123456789abcdef", &song, "liam_made_young");
    let meta = FilmMeta {
        film_id: FILM_ID.into(),
        title: "Low Tide".into(),
        artist: "Mi".into(),
        kind: Kind::Original,
        created: "2026-10-07".into(),
    };
    let out = dir.join("child.swav");
    swav::pack(&corpus().join("zero.mp4"), &out, &meta, &lineage).unwrap();
    out
}

#[test]
fn a_film_with_a_song_for_a_parent_reads_in_swav_pack() {
    if !have_references("a_film_with_a_song_for_a_parent_reads_in_swav_pack") {
        return;
    }
    let dir = tmp("swav-child");
    let out = packed_child(&dir);
    let info = ok(python(&swav_pack(), &["info", s(&out)]));
    assert!(
        info.ends_with("  reads as: a .swav: the film, its wmet and wlin\n"),
        "{info}"
    );
    assert!(info.contains("  wmet  at 7816, 152 bytes: {\"swav\": \"0.1\", \"film_id\": \"00112233445566778899aabbccddeeff\", \"title\": \"Low Tide\""), "{info}");
    assert!(info.contains("bytes: {\"parent_id\": \"0123456789abcdef0123456789abcdef\", \"root_id\": \"0123456789abcdef0123456789abcdef\", \"generation\": 1, \"creator\": \"liam_made_young\", \"device_id\": \"\"}"), "{info}");
    let back = dir.join("back.mp4");
    ok(python(&swav_pack(), &["unpack", s(&out), "-o", s(&back)]));
    same_bytes(&back, &corpus().join("fast.mp4"));
}

#[test]
fn ffprobe_reads_a_packed_swav_without_a_word() {
    if !has("ffprobe") || !has("ffmpeg") {
        eprintln!(
            "ffprobe_reads_a_packed_swav_without_a_word: skipped, it needs ffprobe and ffmpeg"
        );
        return;
    }
    let dir = tmp("swav-ffprobe");
    let mut files = vec![packed_child(&dir)];
    for name in ["plain.mp4", "fast.mp4", "zero.mp4", "large.mp4"] {
        let src = film(&dir, name, Some(FILM_TXT));
        let out = src.with_extension("swav");
        ok(wwav(&["swav", "pack", s(&src), "-o", s(&out)]));
        files.push(out);
    }
    for f in files {
        let o = run("ffprobe", &["-v", "error", s(&f)]);
        assert!(
            o.status.success() && o.stdout.is_empty() && o.stderr.is_empty(),
            "{}: {}",
            f.display(),
            String::from_utf8_lossy(&o.stderr)
        );
        let o = run("ffmpeg", &["-v", "error", "-i", s(&f), "-f", "null", "-"]);
        assert!(
            o.status.success() && o.stderr.is_empty(),
            "{}: {}",
            f.display(),
            String::from_utf8_lossy(&o.stderr)
        );
    }
}
