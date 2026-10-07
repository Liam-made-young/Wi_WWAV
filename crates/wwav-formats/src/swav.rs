//! `.swav` 0.1 (formats/swav/SPEC.md): any ISO BMFF film with two boxes
//! after its own, wmet then wlin, read, packed and unpacked as
//! `swav_pack.py` does.
//!
//! ```text
//! [ftyp] [moov] [mdat] …   the film, untouched
//! [wmet]                   {"swav": "0.1", "film_id", "title", "artist", "type", "created"}
//! [wlin]                   exactly a .wwav's wlin
//! ```
//!
//! Appending moves nothing (MP4 chunk offsets are absolute); the one change
//! to the film is a last box of size 0 ("to the end of the file"), which
//! gets its real size written first. Readers take the first wmet and wlin;
//! NaN and Infinity aren't JSON here.

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::json::{self, Value};
use crate::meta::{Kind, Lineage};
use crate::pack::object_or_empty;
use crate::text::{basename, key_values, splitext, today};
use crate::wwav::read_at;
use crate::{msg, not_same, Error, Staged};

const OWN: [&[u8; 4]; 2] = [b"wmet", b"wlin"];

/// A top-level box: its type, where it starts, its whole size, and its
/// header's size (16 when a 64-bit size follows the type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopBox {
    pub kind: [u8; 4],
    pub at: u64,
    pub size: u64,
    pub header: u64,
}

/// What a reader makes of a film (formats/swav/SPEC.md, Reading).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// "a plain MP4: no wmet and wlin"
    Plain,
    /// "the film only: " and why.
    FilmOnly(String),
    /// "a .swav: the film, its wmet and wlin"
    Swav,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::Plain => f.write_str("a plain MP4: no wmet and wlin"),
            Verdict::FilmOnly(why) => write!(f, "the film only: {why}"),
            Verdict::Swav => f.write_str("a .swav: the film, its wmet and wlin"),
        }
    }
}

/// An open film.
pub struct Swav {
    file: File,
    pub size: u64,
    pub boxes: Vec<TopBox>,
    /// The first wmet and wlin as JSON (None: missing, not JSON, or null).
    pub wmet: Option<Value>,
    pub wlin: Option<Value>,
    /// The first ftyp's major brand.
    pub brand: Option<[u8; 4]>,
}

impl Swav {
    /// Walks the top-level boxes, which must run cleanly to the end of the
    /// file and include a moov.
    pub fn open(path: &Path) -> Result<Swav, Error> {
        let shown = path.to_string_lossy();
        let mut file = File::open(path).map_err(|e| msg(format!("{shown}: {e}")))?;
        let size = file.metadata()?.len();
        let mut boxes = Vec::new();
        let mut at = 0;
        while at < size {
            let head = read_at(&mut file, at, 16)?;
            let cut = || msg(format!("{shown}: not an MP4, or cut off (at byte {at})"));
            if head.len() < 8 {
                return Err(cut());
            }
            let mut n = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as u64;
            let mut header = 8;
            if n == 1 && head.len() == 16 {
                n = u64::from_be_bytes(head[8..16].try_into().unwrap_or_default());
                header = 16;
            } else if n == 0 {
                n = size - at;
            }
            if n < header || n > size - at {
                return Err(cut());
            }
            boxes.push(TopBox {
                kind: [head[4], head[5], head[6], head[7]],
                at,
                size: n,
                header,
            });
            at += n;
        }
        if !boxes.iter().any(|b| &b.kind == b"moov") {
            return Err(msg(format!("{shown}: not a film (no moov box)")));
        }
        let mut s = Swav {
            file,
            size,
            boxes,
            wmet: None,
            wlin: None,
            brand: None,
        };
        s.wmet = s.json(b"wmet")?;
        s.wlin = s.json(b"wlin")?;
        if let Some(b) = s.first(b"ftyp").copied().filter(|b| b.size >= b.header + 4) {
            let h = read_at(&mut s.file, b.at + b.header, 4)?;
            s.brand = Some([h[0], h[1], h[2], h[3]]);
        }
        Ok(s)
    }

    pub fn first(&self, kind: &[u8; 4]) -> Option<&TopBox> {
        self.boxes.iter().find(|b| &b.kind == kind)
    }

    fn json(&mut self, kind: &[u8; 4]) -> io::Result<Option<Value>> {
        let Some(b) = self.first(kind).copied() else {
            return Ok(None);
        };
        let v = json::loads(
            &read_at(&mut self.file, b.at + b.header, b.size - b.header)?,
            false,
        );
        Ok(v.filter(|v| *v != Value::Null))
    }

    pub fn verdict(&self) -> Verdict {
        let (Some(Value::Dict(wmet)), Some(Value::Dict(_))) = (&self.wmet, &self.wlin) else {
            return Verdict::Plain;
        };
        // a string that starts with a digit 0-9; its digits are the major
        let version = match json::get(wmet, "swav") {
            Some(Value::Str(v)) if v.starts_with(|c: char| c.is_ascii_digit()) => v,
            _ => return Verdict::FilmOnly("wmet has no version".into()),
        };
        if version
            .chars()
            .take_while(char::is_ascii_digit)
            .any(|c| c != '0')
        {
            return Verdict::FilmOnly(format!("version {version} is newer than this reader (0.x)"));
        }
        Verdict::Swav
    }

    /// `swav_pack.py info`'s printout, `shown` being the path as given.
    pub fn info(&self, shown: &str) -> String {
        let latin1 = |b: &[u8; 4]| b.iter().map(|&c| c as char).collect::<String>();
        let mut out = format!("{shown}: {} bytes", self.size);
        if let Some(b) = &self.brand {
            out += &format!(", brand {}", latin1(b));
        }
        out.push('\n');
        for b in &self.boxes {
            out += &format!("  {}  at {}, {} bytes", latin1(&b.kind), b.at, b.size);
            if OWN.contains(&&b.kind) {
                let j = if &b.kind == b"wmet" {
                    &self.wmet
                } else {
                    &self.wlin
                };
                out += ": ";
                out += &j.as_ref().map_or("not JSON".into(), json::dumps);
            }
            out.push('\n');
        }
        out + &format!("  reads as: {}\n", self.verdict())
    }
}

/// A film's wmet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilmMeta {
    /// 32 lowercase hex, in the same id space as songs.
    pub film_id: String,
    pub title: String,
    pub artist: String,
    /// Original in 0.1; Remix is 6.10's 0.2.
    pub kind: Kind,
    /// YYYY-MM-DD, or "" when unknown.
    pub created: String,
}

impl FilmMeta {
    pub fn wmet(&self) -> String {
        format!(
            "{{\"swav\": \"0.1\", \"film_id\": {}, \"title\": {}, \"artist\": {}, \"type\": {}, \"created\": {}}}",
            json::quote(&self.film_id),
            json::quote(&self.title),
            json::quote(&self.artist),
            json::quote(self.kind.as_str()),
            json::quote(&self.created)
        )
    }
}

fn top_box(kind: &[u8; 4], body: &str) -> Vec<u8> {
    let mut out = ((8 + body.len()) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(body.as_bytes());
    out
}

/// The two boxes that go after the film.
fn tail(meta: &FilmMeta, lineage: &Lineage) -> Vec<u8> {
    let mut t = top_box(b"wmet", &meta.wmet());
    t.extend(top_box(b"wlin", &lineage.wlin()));
    t
}

fn copy(f: &mut File, n: u64, o: &mut impl Write, shown: &Path) -> Result<(), Error> {
    f.seek(SeekFrom::Start(0))?;
    if io::copy(&mut f.take(n), o)? != n {
        return Err(msg(format!(
            "{}: got shorter while being read",
            shown.display()
        )));
    }
    Ok(())
}

/// Packs `film` into `out` with this wmet and wlin, as `swav_pack.py pack`
/// writes them. Returns the .swav's size.
///
/// 0.1's pack writes originals only (6.10). This takes any lineage, so the
/// Console's export can write a film whose parent is the song made in the
/// same export ("a film made from a song is its child"); every 0.x reader
/// reads it, and `swav_pack.py unpack` warns that film.txt can't keep its
/// parent. The command line's pack ([`pack_original`]) stays an original.
pub fn pack(film: &Path, out: &Path, meta: &FilmMeta, lineage: &Lineage) -> Result<u64, Error> {
    pack_with(film, out, || Ok((meta.clone(), lineage.clone()))).map(|(_, total)| total)
}

/// pack, with wmet and wlin made by `identity` once the film has passed
/// the tool's checks, as swav_pack.py reads film.txt only then.
fn pack_with(
    film: &Path,
    out: &Path,
    identity: impl FnOnce() -> Result<(FilmMeta, Lineage), Error>,
) -> Result<(FilmMeta, u64), Error> {
    not_same(film, out)?;
    let mut s = Swav::open(film)?;
    if s.boxes.iter().any(|b| OWN.contains(&&b.kind)) {
        return Err(msg(format!(
            "{}: already has a wmet or wlin box (unpack it first)",
            film.display()
        )));
    }
    let (meta, lineage) = identity()?;
    let tail = tail(&meta, &lineage);
    // A last box of size 0 runs "to the end of the file", which would
    // swallow the new boxes: write its real size in first.
    let last = *s.boxes.last().ok_or_else(|| msg("no boxes"))?;
    let patch = read_at(&mut s.file, last.at, 4)? == [0, 0, 0, 0];
    if patch && last.size > u32::MAX as u64 {
        return Err(msg(format!(
            "{}: the last box is over 4 GB and has no size, so nothing can go after it",
            film.display()
        )));
    }
    let mut o = Staged::create(out)?;
    copy(&mut s.file, s.size, &mut *o, film)?;
    if patch {
        o.seek(SeekFrom::Start(last.at))?;
        o.write_all(&(last.size as u32).to_be_bytes())?;
        o.seek(SeekFrom::End(0))?;
    }
    o.write_all(&tail)?;
    o.commit()?;
    Ok((meta, s.size + tail.len() as u64))
}

/// film.txt beside a film: `film.mp4` -> `film.txt`.
pub fn txt_path(film: &str) -> String {
    format!("{}.txt", splitext(film).0)
}

/// The wmet and wlin pack writes for `film` from its film.txt (`info`) and
/// the command line's --title, --artist and --creator (empty means unset).
fn original(
    film: &str,
    info: &HashMap<String, String>,
    title: &str,
    artist: &str,
    creator: &str,
) -> (FilmMeta, Lineage) {
    let text = |k: &str| info.get(k).cloned().unwrap_or_default();
    let or = |given: &str, k: &str| {
        if given.is_empty() {
            text(k)
        } else {
            given.to_string()
        }
    };
    let film_id = Some(text("film_id"))
        .filter(|id| wwav_ids::is_work_id(id))
        .unwrap_or_else(wwav_ids::new_work_id);
    let meta = FilmMeta {
        film_id: film_id.clone(),
        title: Some(or(title, "title"))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| splitext(basename(film)).0.to_string()),
        artist: or(artist, "artist"),
        kind: Kind::Original,
        // "created =" with no date keeps it unknown; no line at all is today
        created: info.get("created").cloned().unwrap_or_else(today),
    };
    let lineage = Lineage::original(&film_id, &or(creator, "creator"));
    (meta, lineage)
}

/// `swav_pack.py pack film -o out [--title T] [--artist A] [--creator C]`:
/// an original, its details from film.txt beside the film unless given.
pub fn pack_original(
    film: &str,
    out: &str,
    title: &str,
    artist: &str,
    creator: &str,
) -> Result<(FilmMeta, u64), Error> {
    pack_with(Path::new(film), Path::new(out), || {
        let info = key_values(Path::new(&txt_path(film)))?;
        Ok(original(film, &info, title, artist, creator))
    })
}

/// `{**a, **b}`: each key's value, b's where both have it, and the keys in
/// the order they first appear.
fn merged<'a>(
    a: &'a [(String, Value)],
    b: &'a [(String, Value)],
) -> (HashMap<&'a str, &'a Value>, Vec<&'a str>) {
    let (mut values, mut keys) = (HashMap::new(), Vec::new());
    for (k, v) in a.iter().chain(b) {
        if values.insert(k.as_str(), v).is_none() {
            keys.push(k.as_str());
        }
    }
    (values, keys)
}

/// What unpack wrote, and the tool's warning if packing again won't give
/// back the same .swav.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unpacked {
    pub txt: String,
    pub warning: Option<String>,
}

/// `swav_pack.py unpack file -o out`: the MP4 as it was before wmet and
/// wlin (a size-0 fix stays), and film.txt beside it.
pub fn unpack(path: &str, out: &str) -> Result<Unpacked, Error> {
    not_same(Path::new(path), Path::new(out))?;
    let mut s = Swav::open(Path::new(path))?;
    let mut keep = s.boxes.len();
    while keep > 1 && OWN.contains(&&s.boxes[keep - 1].kind) {
        keep -= 1;
    }
    if s.boxes[..keep].iter().any(|b| OWN.contains(&&b.kind)) {
        return Err(msg(format!(
            "{path}: a wmet or wlin box isn't at the end, so dropping it would move the film"
        )));
    }
    let end = s.boxes[keep - 1].at + s.boxes[keep - 1].size;
    let mut o = BufWriter::with_capacity(1 << 20, File::create(out)?);
    copy(&mut s.file, end, &mut o, Path::new(path))?;
    o.flush()?;

    let meta = object_or_empty(&s.wmet)?;
    let lin = object_or_empty(&s.wlin)?;
    let mut lines = Vec::new();
    let fields = ["title", "artist", "film_id", "created"].map(|k| (k, json::get(meta, k)));
    for (k, v) in fields
        .into_iter()
        .chain([("creator", json::get(lin, "creator"))])
    {
        match v {
            None | Some(Value::Null) => {}
            // "": not known, which pack keeps (no line would mean today)
            Some(Value::Str(e)) if e.is_empty() && k != "created" => {}
            Some(v) => lines.push(format!("{k} = {}", json::py_str(v))),
        }
    }
    let txt = txt_path(out);
    std::fs::write(&txt, lines.join("\n") + "\n")?;

    // film.txt holds one trimmed line per value, and pack writes originals:
    // say when packing this again won't give back what was dropped, and why
    let (again_meta, again_lin) = original(out, &key_values(Path::new(&txt))?, "", "", "");
    let again = tail(&again_meta, &again_lin);
    let rest = read_at(&mut s.file, end, s.size - end)?;
    let mut warning = None;
    if end < s.size && rest != again {
        let (again_lin, again_meta) = (
            json::loads(again_lin.wlin().as_bytes(), false),
            json::loads(again_meta.wmet().as_bytes(), false),
        );
        fn items(v: &Option<Value>) -> &[(String, Value)] {
            match v {
                Some(Value::Dict(d)) => d,
                _ => &[],
            }
        }
        let (old, old_keys) = merged(lin, meta);
        let (new, new_keys) = merged(items(&again_lin), items(&again_meta));
        let null = Value::Null;
        // dict.fromkeys([*old, *new])
        let keys = old_keys
            .into_iter()
            .chain(new_keys.into_iter().filter(|k| !old.contains_key(k)));
        let changed: Vec<&str> = keys
            .filter(|k| {
                let (was, now) = (old.get(k).copied(), new.get(k).copied());
                !json::py_eq(was.unwrap_or(&null), now.unwrap_or(&null))
            })
            .collect();
        let what = if changed.is_empty() {
            "wmet and wlin as written".to_string()
        } else {
            changed.join(", ")
        };
        warning = Some(format!("{txt} can't hold this film's {what}, so packing {out} again won't give back the same .swav"));
    }
    Ok(Unpacked { txt, warning })
}
