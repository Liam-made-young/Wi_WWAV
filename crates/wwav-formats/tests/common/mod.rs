//! What the integration tests share: where things are, the corpus, running
//! the reference tools beside this crate's `wwav`, and making WAV files.
#![allow(dead_code)] // each test file uses its own share of these

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

use wwav_formats::json::{self, Value};

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

pub fn corpus() -> PathBuf {
    root().join("tests/corpus")
}

pub fn wwav_pack() -> PathBuf {
    root().join("formats/prana/tools/wwav_pack.py")
}

pub fn swav_pack() -> PathBuf {
    root().join("formats/formats/swav/swav_pack.py")
}

/// Whether a program runs here (ffmpeg's tools take -version, the rest
/// --version).
pub fn has(cmd: &str) -> bool {
    ["--version", "-version"].iter().any(|v| {
        Command::new(cmd)
            .arg(v)
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

/// The reference tools need python3 and formats/, a private submodule CI
/// may not have (docs/QUESTIONS.md 60). Without them a test says so and
/// passes without checking anything: the criterion stays open.
pub fn have_references(test: &str) -> bool {
    let ok = wwav_pack().exists() && swav_pack().exists() && has("python3");
    if !ok {
        eprintln!("{test}: skipped, it needs python3 and formats/ (tools/local_formats.sh)");
    }
    ok
}

/// A new empty folder under the target directory.
pub fn tmp(name: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn run<S: AsRef<OsStr>>(program: impl AsRef<OsStr>, args: &[S]) -> Output {
    Command::new(program).args(args).output().unwrap()
}

/// `python3 <script> args...`
pub fn python<S: AsRef<OsStr>>(script: &Path, args: &[S]) -> Output {
    let mut all: Vec<&OsStr> = vec![script.as_os_str()];
    all.extend(args.iter().map(|a| a.as_ref()));
    run("python3", &all)
}

/// This crate's `wwav` binary.
pub fn wwav<S: AsRef<OsStr>>(args: &[S]) -> Output {
    run(env!("CARGO_BIN_EXE_wwav"), args)
}

/// stdout, stderr and the exit code, for comparing two runs.
pub fn said(o: &Output) -> (String, String, Option<i32>) {
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        o.status.code(),
    )
}

pub fn ok(o: Output) -> String {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

pub struct Entry {
    pub file: String,
    pub verdict: String,
}

/// tests/corpus/manifest.json's files.
pub fn manifest() -> Vec<Entry> {
    let text = std::fs::read(corpus().join("manifest.json")).unwrap();
    let Some(Value::Dict(top)) = json::loads(&text, true) else {
        panic!("manifest.json isn't an object")
    };
    let Some(Value::List(files)) = json::get(&top, "files") else {
        panic!("manifest.json has no files")
    };
    files
        .iter()
        .map(|f| {
            let Value::Dict(f) = f else {
                panic!("a manifest entry isn't an object")
            };
            let text = |k| match json::get(f, k) {
                Some(Value::Str(s)) => s.clone(),
                _ => panic!("a manifest entry has no {k}"),
            };
            Entry {
                file: text("file"),
                verdict: text("verdict"),
            }
        })
        .collect()
}

pub fn is_film(name: &str) -> bool {
    [".mp4", ".swav"].iter().any(|e| name.ends_with(e))
}

/// A canonical 44-byte-head WAV of `frames` frames whose samples are an
/// integer pattern `seed` picks (full scale, so every byte value occurs).
pub fn wav(frames: u32, seed: u32) -> Vec<u8> {
    wav_as(frames, seed, 44100, 16, 2)
}

pub fn wav_as(frames: u32, seed: u32, rate: u32, bits: u16, channels: u16) -> Vec<u8> {
    let bytes = (bits / 8) as u32;
    let block = channels as u32 * bytes;
    let data = frames * block;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * block).to_le_bytes());
    out.extend_from_slice(&(block as u16).to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    for _ in 0..frames * channels as u32 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        out.extend_from_slice(&x.to_le_bytes()[..bytes as usize]);
    }
    out
}

/// A song folder: master.wav and the four stems (or another set of names),
/// each a different pattern, and song.txt.
pub fn song_folder(dir: &Path, name: &str, frames: u32, names: &[&str], song_txt: &str) -> PathBuf {
    let folder = dir.join(name);
    std::fs::create_dir_all(&folder).unwrap();
    for (i, n) in names.iter().enumerate() {
        std::fs::write(folder.join(n), wav(frames, i as u32 + 1)).unwrap();
    }
    std::fs::write(folder.join("song.txt"), song_txt).unwrap();
    folder
}

pub const NAMED: [&str; 5] = [
    "master.wav",
    "vocals.wav",
    "drums.wav",
    "other.wav",
    "bass.wav",
];

/// Where two byte strings first differ, for a failure message.
pub fn first_difference(a: &[u8], b: &[u8]) -> String {
    match a.iter().zip(b).position(|(x, y)| x != y) {
        Some(i) => format!(
            "they differ first at byte {i} ({} and {} bytes long)",
            a.len(),
            b.len()
        ),
        None => format!(
            "one is a prefix of the other ({} and {} bytes long)",
            a.len(),
            b.len()
        ),
    }
}

pub fn same_bytes(a: &Path, b: &Path) {
    let (x, y) = (std::fs::read(a).unwrap(), std::fs::read(b).unwrap());
    assert!(
        x == y,
        "{} and {}: {}",
        a.display(),
        b.display(),
        first_difference(&x, &y)
    );
}

pub fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// Our run against the reference's, as `said` gives them: the same
/// stdout, stderr and exit code. Where python dies with a traceback (a
/// missing file, a wmet no writer makes) there is no message to match:
/// then the exit code and stdout must, and ours must say something.
pub fn same_run(
    got: (String, String, Option<i32>),
    want: (String, String, Option<i32>),
    what: &str,
) {
    if want.1.contains("Traceback (most recent call last)") {
        assert_eq!((&got.0, got.2), (&want.0, want.2), "{what}");
        assert!(
            !got.1.is_empty(),
            "{what}: python failed with a traceback, and Rust said nothing"
        );
    } else {
        assert_eq!(got, want, "{what}");
    }
}
