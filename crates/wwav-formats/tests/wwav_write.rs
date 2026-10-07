//! The library's writers: the streaming writer the Console's export feeds,
//! remixes, the master-only product, wrapping a plain WAV, and reading
//! stems back in runs.
//!
//! Fails if: the streaming writer, fed in any block sizes and order,
//! writes other bytes than wwav_pack.py; a remix isn't the corpus remix
//! byte for byte (PRANA's chunk order, wrmx last); wrmx's text differs from
//! PRANA's writeWrmx for any of 3,000 mixes; the master-only product isn't
//! the song without wstm, with the same wmet and wlin bytes; wrap_wav
//! writes other bytes, or refuses in other words, than Wi's wrapWav; a
//! song over 4 GB isn't refused with the spec's sentence, or one at the
//! limit is; stems read in runs aren't the frames written.

mod common;

use std::path::Path;
use std::process::Command;

use common::*;
use wwav_formats::meta::{Kind, Lineage, Mode, SongMeta, Track, Wrmx};
use wwav_formats::writer::{master_only, wrap_wav, WwavWriter};
use wwav_formats::wwav::{Verdict, Wwav};

const SONG_ID: &str = "0123456789abcdef0123456789abcdef";
const REMIX_ID: &str = "fedcba9876543210fedcba9876543210";

fn corpus_song() -> SongMeta {
    SongMeta {
        song_id: SONG_ID.into(),
        title: "Tést \"Song\"".into(),
        artist: "Mi".into(),
        bpm: 120.125,
        key: "A minor".into(),
        kind: Kind::Original,
        splitter: String::new(),
        created: "2026-10-03".into(),
    }
}

/// The corpus song's master and stems (one Vec per stem), read back.
fn corpus_audio() -> (Vec<i16>, [Vec<i16>; 4]) {
    let mut w = Wwav::open(&corpus().join("original.wwav")).unwrap();
    let frames = w.master_frames() as usize;
    let mut master = vec![0; frames * 2];
    assert_eq!(w.read_master(0, &mut master).unwrap(), frames);
    let mut all = vec![0; frames * 8];
    assert_eq!(w.read_stems(0, &mut all).unwrap(), frames);
    let stem = |s: usize| {
        all.chunks(8)
            .flat_map(|f| [f[s * 2], f[s * 2 + 1]])
            .collect::<Vec<i16>>()
    };
    (master, [stem(0), stem(1), stem(2), stem(3)])
}

#[test]
fn the_streaming_writer_writes_what_pack_writes_whatever_the_blocks() {
    let (master, stems) = corpus_audio();
    let frames = master.len() / 2;
    let dir = tmp("stream");
    for (n, (mblock, sblock, stems_first)) in [
        (600, 600, false),
        (7, 13, false),
        (1, 1, true),
        (599, 2, true),
    ]
    .into_iter()
    .enumerate()
    {
        let out = dir.join(format!("{n}.wwav"));
        let mut w = WwavWriter::create(
            &out,
            frames as u64,
            &corpus_song(),
            &Lineage::original(SONG_ID, "liam_made_young"),
            None,
        )
        .unwrap();
        let (mut m, mut s) = (0, 0);
        while m < frames || s < frames {
            if s < frames && (stems_first || m == frames) {
                let e = (s + sblock).min(frames);
                w.write_stems([
                    &stems[0][s * 2..e * 2],
                    &stems[1][s * 2..e * 2],
                    &stems[2][s * 2..e * 2],
                    &stems[3][s * 2..e * 2],
                ])
                .unwrap();
                s = e;
            }
            if m < frames {
                let e = (m + mblock).min(frames);
                w.write_master(&master[m * 2..e * 2]).unwrap();
                m = e;
            }
        }
        assert_eq!(w.finish().unwrap(), std::fs::metadata(&out).unwrap().len());
        same_bytes(&out, &corpus().join("original.wwav"));
    }
}

#[test]
fn the_streaming_writer_refuses_too_much_or_too_little() {
    let dir = tmp("stream-counts");
    let lin = Lineage::original(SONG_ID, "");
    let mut w = WwavWriter::create(&dir.join("a.wwav"), 2, &corpus_song(), &lin, None).unwrap();
    assert!(w.write_master(&[0; 6]).is_err(), "3 frames into 2");
    assert!(w.write_master(&[0; 3]).is_err(), "half a frame");
    w.write_master(&[0; 4]).unwrap();
    assert!(w.finish().is_err(), "no stems written");
}

#[test]
fn a_song_over_4_gb_is_refused_in_the_specs_words() {
    let dir = tmp("too-long");
    let (meta, lin) = (corpus_song(), Lineage::original(SONG_ID, "liam_made_young"));
    let limit = (0..u32::MAX as u64 / 20)
        .rev()
        .find(|&f| WwavWriter::size(f, &meta, &lin, None) <= u32::MAX as u64)
        .unwrap();
    assert!((214_000_000..215_000_000).contains(&limit), "{limit}");
    let path = dir.join("long.wwav");
    let e = WwavWriter::create(&path, limit + 1, &meta, &lin, None)
        .err()
        .unwrap();
    assert_eq!(
        e.to_string(),
        "A .wwav holds about 81 minutes. This session is 82."
    );
    assert!(!path.exists());
    let e = WwavWriter::create(&path, 94 * 60 * 44100, &meta, &lin, None)
        .err()
        .unwrap();
    assert_eq!(
        e.to_string(),
        "A .wwav holds about 81 minutes. This session is 94."
    );
    // at the limit it starts (a sparse file: nothing is written in between)
    drop(WwavWriter::create(&path, limit, &meta, &lin, None).unwrap());
    std::fs::remove_file(&path).unwrap();
}

fn corpus_wrmx() -> Wrmx {
    let track = |vol, mute, reverb| Track {
        vol,
        mute,
        fx: [reverb, 0.0, 0.0, 0.0],
    };
    Wrmx {
        tracks: [
            track(0.8, false, 0.25),
            track(0.65, true, 0.0),
            track(0.8, false, 0.0),
            track(1.0, false, 0.125),
        ],
        pitch: -2,
        speed: 1.0,
        time: 1.0,
        lpf: 1.0,
        hpf: 0.0,
        mode: Mode::Stems,
        start: 0,
        length: 600,
    }
}

#[test]
fn a_remix_is_written_as_prana_writes_one() {
    let (master, stems) = corpus_audio();
    let out = tmp("remix").join("remix.wwav");
    let meta = SongMeta {
        song_id: REMIX_ID.into(),
        title: "Tést \"Song\" (remix-001)".into(),
        kind: Kind::Remix,
        ..corpus_song()
    };
    let parent = Lineage::original(SONG_ID, "liam_made_young");
    let lineage = Lineage::child_of(SONG_ID, &parent, "");
    assert_eq!(
        (
            lineage.parent_id.as_deref(),
            lineage.root_id.as_str(),
            lineage.generation
        ),
        (Some(SONG_ID), SONG_ID, 1)
    );
    let mut w = WwavWriter::create(&out, 600, &meta, &lineage, Some(&corpus_wrmx())).unwrap();
    w.write_master(&master).unwrap();
    w.write_stems([&stems[0], &stems[1], &stems[2], &stems[3]])
        .unwrap();
    w.finish().unwrap();
    // the corpus remix: wmet, wstm, wlin, then wrmx as the device's text
    same_bytes(&out, &corpus().join("remix.wwav"));
    let r = Wwav::open(&out).unwrap();
    assert_eq!(r.verdict(), Verdict::Stems);
    let ids: Vec<_> = r.chunks.iter().map(|c| c.id).collect();
    assert_eq!(
        ids,
        [*b"fmt ", *b"data", *b"wmet", *b"wstm", *b"wlin", *b"wrmx"]
    );
}

#[test]
fn wrmx_is_the_text_prana_writes() {
    if !has("c++")
        || !Path::new(&format!(
            "{}/formats/prana/core/app/remix.cpp",
            root().display()
        ))
        .exists()
    {
        eprintln!("wrmx_is_the_text_prana_writes: skipped, it needs c++ and formats/");
        return;
    }
    let core = root().join("formats/prana/core");
    let exe = tmp("prana-wrmx").join("prana_wrmx");
    let mut cmd = Command::new(std::env::var("CXX").unwrap_or("c++".into()));
    cmd.args([
        "-std=c++17",
        "-O1",
        "-ffp-contract=off",
        "-w",
        "-I",
        s(&core),
        s(&root().join("tools/parity/prana_wrmx.cpp")),
    ]);
    for src in [
        "app/remix.cpp",
        "app/mix.cpp",
        "base/dmath.cpp",
        "base/text.cpp",
        "base/json.cpp",
    ] {
        cmd.arg(core.join(src));
    }
    assert!(cmd.args(["-o", s(&exe)]).status().unwrap().success());
    let lines = ok(run(&exe, &[] as &[&str]));
    let f = |h: &str| f32::from_bits(u32::from_str_radix(h, 16).unwrap());
    let mut n = 0;
    for line in lines.lines() {
        let (values, text) = line.split_once('\t').unwrap();
        let v: Vec<&str> = values.split(' ').collect();
        let track = |s: usize| Track {
            vol: f(v[s * 6]),
            mute: v[s * 6 + 1] == "1",
            fx: [
                f(v[s * 6 + 2]),
                f(v[s * 6 + 3]),
                f(v[s * 6 + 4]),
                f(v[s * 6 + 5]),
            ],
        };
        let w = Wrmx {
            tracks: [track(0), track(1), track(2), track(3)],
            pitch: v[24].parse().unwrap(),
            speed: f(v[25]),
            time: f(v[26]),
            lpf: f(v[27]),
            hpf: f(v[28]),
            mode: if v[29] == "master" {
                Mode::Master
            } else {
                Mode::Stems
            },
            start: v[30].parse().unwrap(),
            length: v[31].parse().unwrap(),
        };
        assert_eq!(w.json(), text);
        n += 1;
    }
    assert_eq!(n, 3000);
}

#[test]
fn master_only_is_the_song_without_its_stems() {
    let dir = tmp("master-only");
    let out = dir.join("master-only.wwav");
    master_only(&corpus().join("original.wwav"), &out).unwrap();
    same_bytes(&out, &corpus().join("master-only.wwav"));
    assert_eq!(
        Wwav::open(&out).unwrap().verdict().to_string(),
        "the master only: no wstm"
    );
    // only a song with stems has stems to leave out
    for name in [
        "master-only.wwav",
        "plain.wav",
        "version-1.0.wwav",
        "48k.wav",
    ] {
        assert!(
            master_only(&corpus().join(name), &dir.join("x.wwav")).is_err(),
            "{name}"
        );
    }
}

/// Wi's wrapWav on each input, under Node: Ok(bytes) or Err(message).
fn wi_wrap(inputs: &[(&str, std::path::PathBuf)], dir: &Path) -> Vec<Result<Vec<u8>, String>> {
    let script = dir.join("wrap.mjs");
    std::fs::write(
        &script,
        format!(
            "import {{ openAsBlob, writeFileSync }} from 'node:fs';\n\
             import {{ wrapWav }} from '{}';\n\
             const meta = {{ songId: 'fedcba9876543210fedcba9876543210', title: 'Wrapped \"é\"', artist: 'Mi', creator: 'liam_made_young', created: '2026-10-03' }};\n\
             for (const src of process.argv.slice(2)) {{\n\
               try {{ writeFileSync(src + '.wi', new Uint8Array(await (await wrapWav(await openAsBlob(src), meta)).arrayBuffer())); console.log('ok'); }}\n\
               catch (e) {{ console.log(e.message); }}\n\
             }}\n",
            root().join("formats/wi/src/formats/wwav.js").display()
        ),
    )
    .unwrap();
    let mut args = vec![s(&script).to_string()];
    args.extend(inputs.iter().map(|(_, p)| s(p).to_string()));
    let said = ok(run("node", &args));
    said.lines()
        .zip(inputs)
        .map(|(line, (_, p))| {
            if line == "ok" {
                Ok(std::fs::read(format!("{}.wi", s(p))).unwrap())
            } else {
                Err(line.to_string())
            }
        })
        .collect()
}

fn riff(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut out = b"RIFF\0\0\0\0WAVE".to_vec();
    for (id, body) in chunks {
        out.extend_from_slice(*id);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
        if body.len() & 1 == 1 {
            out.push(0);
        }
    }
    let n = out.len() as u32 - 8;
    out[4..8].copy_from_slice(&n.to_le_bytes());
    out
}

fn fmt(tag: u16, rate: u32, size: usize) -> Vec<u8> {
    let mut b = wav_as(0, 0, rate, 16, 2)[20..36].to_vec();
    b[..2].copy_from_slice(&tag.to_le_bytes());
    b.resize(size, 0);
    if size >= 26 {
        b[24..26].copy_from_slice(&1u16.to_le_bytes()); // EXTENSIBLE: PCM in the subformat
    }
    b
}

#[test]
fn wrap_wav_writes_what_wis_wrap_wav_writes() {
    if !has("node") || !root().join("formats/wi/src/formats/wwav.js").exists() {
        eprintln!("wrap_wav_writes_what_wis_wrap_wav_writes: skipped, it needs node and formats/");
        return;
    }
    let dir = tmp("wrap");
    let pcm = wav(100, 7)[44..].to_vec();
    let plain = wav(1000, 3);
    let junk = |at: usize| vec![0u8; at - 12 - 24 - 16]; // the data chunk at `at`, after a 16-byte fmt
    let inputs: Vec<(&str, Vec<u8>)> = vec![
        ("plain", plain.clone()),
        (
            "odd LIST, no pad",
            [&plain[..], b"LIST\x03\x00\x00\x00abc"].concat(),
        ),
        (
            "FLLR filler",
            riff(&[
                (b"fmt ", fmt(1, 44100, 16)),
                (b"FLLR", vec![0; 4096 - 12 - 24 - 16]),
                (b"data", pcm.clone()),
            ]),
        ),
        (
            "bext and iXML",
            riff(&[
                (b"bext", vec![0; 602]),
                (b"iXML", vec![32; 500]),
                (b"fmt ", fmt(1, 44100, 16)),
                (b"data", pcm.clone()),
            ]),
        ),
        (
            "data at 1026",
            riff(&[
                (b"fmt ", fmt(1, 44100, 16)),
                (b"JUNK", junk(1026)),
                (b"data", pcm.clone()),
            ]),
        ),
        (
            "data at 1024",
            riff(&[
                (b"fmt ", fmt(1, 44100, 16)),
                (b"JUNK", junk(1024)),
                (b"data", pcm.clone()),
            ]),
        ),
        (
            "fmt after data",
            riff(&[(b"data", pcm.clone()), (b"fmt ", fmt(1, 44100, 16))]),
        ),
        (
            "extensible, 26 bytes",
            riff(&[(b"fmt ", fmt(0xfffe, 44100, 26)), (b"data", pcm.clone())]),
        ),
        (
            "extensible, 40 bytes",
            riff(&[(b"fmt ", fmt(0xfffe, 44100, 40)), (b"data", pcm.clone())]),
        ),
        (
            "48 kHz, then 44.1 kHz",
            riff(&[
                (b"fmt ", fmt(1, 48000, 16)),
                (b"fmt ", fmt(1, 44100, 16)),
                (b"data", pcm.clone()),
            ]),
        ),
        (
            "48 kHz before data, 44.1 after",
            riff(&[
                (b"fmt ", fmt(1, 48000, 16)),
                (b"data", pcm.clone()),
                (b"fmt ", fmt(1, 44100, 16)),
            ]),
        ),
        (
            "a .wwav",
            std::fs::read(corpus().join("original.wwav")).unwrap(),
        ),
        ("48 kHz", wav_as(100, 1, 48000, 16, 2)),
        ("no frames", wav(0, 1)),
        ("no data chunk", riff(&[(b"fmt ", fmt(1, 44100, 16))])),
        ("cut off", plain[..1000].to_vec()),
        ("stray bytes", [&plain[..], b"xyz"].concat()),
        (
            "a wmet that isn't one",
            [&plain[..], b"wmet\x02\x00\x00\x00{}"].concat(),
        ),
        ("not a WAV", b"not a wav at all".to_vec()),
    ];
    let paths: Vec<_> = inputs
        .iter()
        .enumerate()
        .map(|(i, (name, bytes))| {
            let p = dir.join(format!("{i}.wav"));
            std::fs::write(&p, bytes).unwrap();
            (*name, p)
        })
        .collect();
    let wi = wi_wrap(&paths, &dir);
    let meta = SongMeta {
        song_id: REMIX_ID.into(),
        title: "Wrapped \"é\"".into(),
        artist: "Mi".into(),
        created: "2026-10-03".into(),
        ..SongMeta::default()
    };
    let mut wrapped = 0;
    for ((name, src), want) in paths.iter().zip(wi) {
        let out = dir.join(format!("{name}.wwav"));
        let got = wrap_wav(src, &out, &meta, "liam_made_young")
            .map(|_| std::fs::read(&out).unwrap())
            .map_err(|e| e.to_string());
        match (&got, &want) {
            (Ok(a), Ok(b)) => assert!(a == b, "{name}: {}", first_difference(a, b)),
            _ => assert_eq!(
                got.as_ref().map(|_| ()),
                want.as_ref().map(|_| ()),
                "{name}"
            ),
        }
        if got.is_ok() {
            wrapped += 1;
            assert_eq!(
                Wwav::open(&out).unwrap().verdict().to_string(),
                "the master only: no wstm",
                "{name}"
            );
        }
    }
    assert_eq!(wrapped, 10);
}

#[test]
fn stems_come_back_in_runs() {
    let mut w = Wwav::open(&corpus().join("original.wwav")).unwrap();
    let bytes = std::fs::read(corpus().join("original.wwav")).unwrap();
    let stems = w.stems().unwrap();
    assert_eq!((stems.audio % 512, stems.frames), (0, 600));
    let at = |frame: usize, sample: usize| {
        let i = stems.audio as usize + frame * 16 + sample * 2;
        i16::from_le_bytes([bytes[i], bytes[i + 1]])
    };
    let mut run = vec![0i16; 8 * 64];
    for start in [0, 100, 590] {
        let n = w.read_stems(start as u64, &mut run).unwrap();
        assert_eq!(n, 64.min(600 - start));
        for f in 0..n {
            for s in 0..8 {
                assert_eq!(run[f * 8 + s], at(start + f, s));
            }
        }
    }
    assert_eq!(w.read_stems(600, &mut run).unwrap(), 0);
    let mut master = vec![0i16; 2 * 10];
    assert_eq!(w.read_master(595, &mut master).unwrap(), 5);
    assert_eq!(
        master[0],
        i16::from_le_bytes([bytes[44 + 595 * 4], bytes[45 + 595 * 4]])
    );
    // a file whose verdict isn't "4 stems" has no stems to read
    assert!(Wwav::open(&corpus().join("frames-wmet.wwav"))
        .unwrap()
        .stems()
        .is_none());
}
