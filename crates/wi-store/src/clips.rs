//! Clips: any one thing in the library (docs/SPEC.md 2.5). A `.wwav`, a
//! `.swav`, plain audio or video, an image, or text. Each file is copied into
//! `media/` under a new ULID, so renaming a song never renames anything on
//! disk and two songs called "untitled" never collide.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::journal::{self, object, Row};
use crate::organise::Pin;
use crate::{refused, Result, Room, Store, Txn};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Wwav,
    Swav,
    Audio,
    Video,
    Image,
    Text,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Wwav => "wwav",
            Kind::Swav => "swav",
            Kind::Audio => "audio",
            Kind::Video => "video",
            Kind::Image => "image",
            Kind::Text => "text",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        [
            Kind::Wwav,
            Kind::Swav,
            Kind::Audio,
            Kind::Video,
            Kind::Image,
            Kind::Text,
        ]
        .into_iter()
        .find(|k| k.as_str() == s)
    }
}

impl FromSql for Kind {
    fn column_result(v: ValueRef<'_>) -> FromSqlResult<Kind> {
        let s = v.as_str()?;
        Kind::parse(s).ok_or_else(|| FromSqlError::Other(format!("not a clip kind: {s}").into()))
    }
}

/// The six colour labels, for sorting only (2.5). Their colours are tokens,
/// so they live in `design/tokens.json` (8.11), not here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Colour {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
}

impl Colour {
    pub fn as_str(self) -> &'static str {
        match self {
            Colour::Red => "red",
            Colour::Orange => "orange",
            Colour::Yellow => "yellow",
            Colour::Green => "green",
            Colour::Blue => "blue",
            Colour::Purple => "purple",
        }
    }

    pub fn parse(s: &str) -> Option<Colour> {
        [
            Colour::Red,
            Colour::Orange,
            Colour::Yellow,
            Colour::Green,
            Colour::Blue,
            Colour::Purple,
        ]
        .into_iter()
        .find(|c| c.as_str() == s)
    }
}

impl FromSql for Colour {
    fn column_result(v: ValueRef<'_>) -> FromSqlResult<Colour> {
        let s = v.as_str()?;
        Colour::parse(s)
            .ok_or_else(|| FromSqlError::Other(format!("not a colour label: {s}").into()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub id: String,
    pub kind: Kind,
    /// `media/<id>.<ext>`, `purchases/<id>.<ext>`, or an absolute path for a
    /// file left in place. [`Store::path_of`] resolves it.
    pub file: String,
    pub sha256: String,
    pub bytes: u64,
    pub title: String,
    pub artist: String,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub duration_ms: i64,
    /// The reader's sentence, word for word ("4 stems, and the master").
    pub verdict: String,
    pub text: Option<String>,
    pub colour: Option<Colour>,
    pub published_at: Option<i64>,
    pub remote_id: Option<String>,
    pub from_sequence: Option<String>,
}

impl Clip {
    /// Published and waiting to go up: ⌘Z can still take it back.
    pub fn is_queued(&self) -> bool {
        self.published_at.is_some() && self.remote_id.is_none()
    }

    /// The server has it: it has left the machine and can't be undone.
    pub fn is_up(&self) -> bool {
        self.remote_id.is_some()
    }
}

pub(crate) const CLIP_COLS: &str =
    "c.id, c.kind, c.file, c.sha256, c.bytes, c.title, c.artist, c.bpm, c.key, \
     c.duration_ms, c.verdict, c.text, c.colour, c.published_at, c.remote_id, c.from_sequence";

pub(crate) fn clip_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<Clip> {
    Ok(Clip {
        id: r.get(0)?,
        kind: r.get(1)?,
        file: r.get(2)?,
        sha256: r.get(3)?,
        bytes: r.get::<_, i64>(4)? as u64,
        title: r.get(5)?,
        artist: r.get(6)?,
        bpm: r.get(7)?,
        key: r.get(8)?,
        duration_ms: r.get(9)?,
        verdict: r.get(10)?,
        text: r.get(11)?,
        colour: r.get(12)?,
        published_at: r.get(13)?,
        remote_id: r.get(14)?,
        from_sequence: r.get(15)?,
    })
}

fn clips_where(conn: &Connection, filter: &str) -> Result<Vec<Clip>> {
    let mut stmt = conn.prepare_cached(&format!("SELECT {CLIP_COLS} FROM clips c {filter}"))?;
    let clips = stmt
        .query_map([], clip_from)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(clips)
}

/// What a reader found in a file.
#[derive(Clone, Debug, PartialEq)]
pub struct Inspection {
    pub kind: Kind,
    pub title: String,
    pub artist: String,
    /// None, or a value that isn't finite, means the reader couldn't measure one.
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub duration_ms: i64,
    /// The sentence Get Info shows for the file.
    pub verdict: String,
    pub text: Option<String>,
}

impl Inspection {
    pub fn new(kind: Kind, title: &str) -> Inspection {
        Inspection {
            kind,
            title: title.to_string(),
            artist: String::new(),
            bpm: None,
            key: None,
            duration_ms: 0,
            verdict: String::new(),
            text: None,
        }
    }
}

/// Reads a file before it comes in. wi-core plugs in the formats crate's
/// readers, whose verdicts match `wwav_pack.py info` word for word.
pub trait Inspector {
    fn inspect(&self, path: &Path) -> Result<Inspection>;
}

/// Knows a file only by its extension, and says no more than that tells it.
pub struct ByExtension;

impl Inspector for ByExtension {
    fn inspect(&self, path: &Path) -> Result<Inspection> {
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = extension(path);
        let kind = match ext.as_str() {
            "wwav" => Kind::Wwav,
            "swav" => Kind::Swav,
            "wav" | "aif" | "aiff" | "flac" | "mp3" | "m4a" | "aac" | "ogg" | "opus" => Kind::Audio,
            "mp4" | "mov" | "m4v" | "mkv" | "webm" => Kind::Video,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "tif" | "tiff" => Kind::Image,
            "txt" | "md" => Kind::Text,
            "" => {
                return refused(format!(
                    "Wi_WWAV can't tell what '{title}' is without its extension."
                ))
            }
            other => return refused(format!("Wi_WWAV can't open .{other} files.")),
        };
        let mut found = Inspection::new(kind, &title);
        match kind {
            Kind::Audio => found.verdict = "Plain audio comes in as master only.".to_string(),
            Kind::Text => {
                found.text = Some(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
            }
            _ => {}
        }
        Ok(found)
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Copy or reference (Open #3, built at its recommendation): copy songs,
/// films and anything under 2 GB, so the library and its export are complete;
/// leave anything bigger (long raw video, say) where it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Copy,
    LeaveInPlace,
}

impl Placement {
    pub fn for_file(kind: Kind, bytes: u64) -> Placement {
        const TWO_GB: u64 = 2_000_000_000;
        if matches!(kind, Kind::Wwav | Kind::Swav) || bytes < TWO_GB {
            Placement::Copy
        } else {
            Placement::LeaveInPlace
        }
    }
}

/// A file ready to become a clip: copied into place and hashed. A render
/// takes its name from [`Store::reserve_media`], writes the file there itself
/// and fills one in, with `from_sequence`.
#[derive(Clone, Debug, PartialEq)]
pub struct NewClip {
    pub id: String,
    pub file: String,
    pub sha256: String,
    pub bytes: u64,
    pub info: Inspection,
    pub from_sequence: Option<String>,
}

/// Reads `src` through to `to`, returning its sha256 and size.
fn hash_through(src: &Path, to: &mut impl Write) -> Result<(String, u64)> {
    let mut from = File::open(src)?;
    let (mut hash, mut bytes, mut buf) = (Sha256::new(), 0u64, vec![0u8; 1 << 20]);
    loop {
        let n = from.read(&mut buf)?;
        if n == 0 {
            return Ok((hex::encode(hash.finalize()), bytes));
        }
        hash.update(&buf[..n]);
        to.write_all(&buf[..n])?;
        bytes += n as u64;
    }
}

/// Copies `src` to `dir/name` through a temporary name, hashing as it goes,
/// so a crash never leaves half a file under a real name.
pub(crate) fn copy_hashed(src: &Path, dir: &Path, name: &str) -> Result<(String, u64)> {
    let part = dir.join(format!(".{name}.part"));
    let copied = (|| -> Result<(String, u64)> {
        let mut to = File::create(&part)?;
        let hashed = hash_through(src, &mut to)?;
        to.sync_all()?;
        std::fs::rename(&part, dir.join(name))?;
        Ok(hashed)
    })();
    if copied.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    copied
}

/// A new file name: the clip's ULID and the source's extension.
pub(crate) fn file_name(id: &str, src: &Path) -> String {
    match extension(src).as_str() {
        "" => id.to_string(),
        ext => format!("{id}.{ext}"),
    }
}

/// `published_at IS NOT NULL AND remote_id IS NULL`: the upload queue is this
/// query, not a list (2.8), so it survives a crash with no queue file.
pub const UPLOAD_QUEUE: &str = "published_at IS NOT NULL AND remote_id IS NULL";

fn missing_clip<T>() -> Result<T> {
    refused("That clip isn't in the library any more.")
}

/// Upload parts are kept only while their clip is queued. When a clip leaves
/// the queue without the server having it (⌘Z of the publish, a delete), the
/// upload it was part of is over, and the next publish starts a new one.
pub(crate) fn drop_stale_parts(conn: &Connection) -> Result<()> {
    conn.prepare_cached(&format!(
        "DELETE FROM upload_part WHERE clip_id NOT IN (SELECT id FROM clips WHERE {UPLOAD_QUEUE})"
    ))?
    .execute([])?;
    Ok(())
}

/// How long a reservation in `media/` holds: one older than this is what a
/// crash left, and Clean up media… may take its file.
pub(crate) const RESERVATION_MS: i64 = 24 * 60 * 60 * 1000;

impl Store {
    /// Reads `src` and brings the file into the library, unrecorded: copied
    /// to `media/` under a new ULID, or left in place (see [`Placement`]).
    /// [`Txn::add_clip`] records it.
    pub fn bring_in(&self, src: &Path, inspector: &dyn Inspector) -> Result<NewClip> {
        let info = inspector.inspect(src)?;
        let id = wwav_ids::ulid();
        let size = std::fs::metadata(src)?.len();
        let (file, (sha256, bytes)) = match Placement::for_file(info.kind, size) {
            Placement::Copy => {
                let name = file_name(&id, src);
                let file = format!("media/{name}");
                // Reserved before the first byte lands, so Clean up media…
                // never takes it before `add_clip` records it.
                self.reserve(&file)?;
                let hashed = copy_hashed(src, &self.root.join("media"), &name);
                if hashed.is_err() {
                    self.release(&file)?;
                }
                (file, hashed?)
            }
            Placement::LeaveInPlace => {
                let path = src.canonicalize()?;
                let Some(file) = path.to_str() else {
                    return refused(format!(
                        "Wi_WWAV can't keep track of '{}': its path isn't UTF-8.",
                        path.display()
                    ));
                };
                (file.to_string(), hash_through(&path, &mut std::io::sink())?)
            }
        };
        Ok(NewClip {
            id,
            file,
            sha256,
            bytes,
            info,
            from_sequence: None,
        })
    }

    /// A new name in `media/` for a file about to be written there, such as a
    /// render: returns the clip id and the file (`media/<id>.<ext>`), reserved
    /// so Clean up media… leaves it alone until [`Txn::add_clip`] records it.
    pub fn reserve_media(&self, ext: &str) -> Result<(String, String)> {
        if ext.is_empty() || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
            return refused(format!(
                "A file in the library ends in letters and digits, as .wwav, not '.{ext}'."
            ));
        }
        let id = wwav_ids::ulid();
        let file = format!("media/{id}.{}", ext.to_ascii_lowercase());
        self.reserve(&file)?;
        Ok((id, file))
    }

    fn reserve(&self, file: &str) -> Result<()> {
        self.conn
            .prepare_cached("INSERT OR REPLACE INTO media_pending (file, made_ms) VALUES (?1, ?2)")?
            .execute(params![file, wwav_ids::now_ms() as i64])?;
        Ok(())
    }

    fn release(&self, file: &str) -> Result<()> {
        self.conn
            .prepare_cached("DELETE FROM media_pending WHERE file = ?1")?
            .execute([file])?;
        Ok(())
    }

    /// Brings one file in and records it as one change: "import 'Low Tide'".
    pub fn import(&mut self, room: Room, src: &Path, inspector: &dyn Inspector) -> Result<Clip> {
        let new = self.bring_in(src, inspector)?;
        let mut tx = self.begin(room, &format!("import '{}'", new.info.title))?;
        let id = tx.add_clip(new)?;
        tx.commit()?;
        self.clip(&id)?.map_or_else(missing_clip, Ok)
    }

    pub fn clip(&self, id: &str) -> Result<Option<Clip>> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!("SELECT {CLIP_COLS} FROM clips c WHERE c.id = ?1"))?;
        Ok(stmt.query_row([id], clip_from).optional()?)
    }

    /// Every clip, newest first.
    pub fn clips(&self) -> Result<Vec<Clip>> {
        clips_where(&self.conn, "ORDER BY c.n DESC")
    }

    /// Where a clip's file is on disk.
    pub fn path_of(&self, clip: &Clip) -> PathBuf {
        self.root.join(&clip.file) // joining an absolute path gives that path
    }

    /// Published clips the server doesn't have yet, first published first.
    pub fn upload_queue(&self) -> Result<Vec<Clip>> {
        clips_where(
            &self.conn,
            &format!("WHERE {UPLOAD_QUEUE} ORDER BY c.published_at, c.id"),
        )
    }

    /// Notes that part `n` of a clip's upload is up, so a resumed upload
    /// skips it. Not a journal entry: it records what the server has. Returns
    /// false (and records nothing) when the clip isn't queued any more, so
    /// wi-core knows to abort that upload.
    pub fn record_upload_part(&mut self, clip_id: &str, n: u32, etag: &str) -> Result<bool> {
        let recorded = self.conn.execute(
            &format!(
                "INSERT OR REPLACE INTO upload_part (clip_id, n, etag)
                 SELECT ?1, ?2, ?3 WHERE EXISTS (SELECT 1 FROM clips WHERE id = ?1 AND {UPLOAD_QUEUE})"
            ),
            params![clip_id, n, etag],
        )?;
        Ok(recorded == 1)
    }

    /// Forgets the parts recorded for a clip's upload, so the next attempt
    /// starts a new one (when the server says the old one is gone, say).
    pub fn discard_upload_parts(&mut self, clip_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM upload_part WHERE clip_id = ?1", [clip_id])?;
        Ok(())
    }

    /// The parts already up, in order.
    pub fn upload_parts(&self, clip_id: &str) -> Result<Vec<(u32, String)>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT n, etag FROM upload_part WHERE clip_id = ?1 ORDER BY n")?;
        let parts = stmt.query_map([clip_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(parts.collect::<rusqlite::Result<_>>()?)
    }

    /// The server has the clip: it leaves the queue, and from now on undoing
    /// its publish is refused. Returns false (and records nothing) when the
    /// clip was unpublished or deleted first, so wi-core knows to take it down
    /// again. Not a journal entry, but every journal snapshot of the clip
    /// learns its `remote_id`, so no undo or redo forgets the server has it.
    pub fn mark_uploaded(&mut self, clip_id: &str, remote_id: &str) -> Result<bool> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            &format!("UPDATE clips SET remote_id = ?1 WHERE id = ?2 AND {UPLOAD_QUEUE}"),
            params![remote_id, clip_id],
        )?;
        if changed == 1 {
            journal::acknowledge(&tx, clip_id, remote_id)?;
        }
        tx.execute("DELETE FROM upload_part WHERE clip_id = ?1", [clip_id])?;
        tx.commit()?;
        Ok(changed == 1)
    }
}

/// The row a [`NewClip`] becomes, in the table's column order.
pub(crate) fn clip_row(new: NewClip) -> Row {
    let info = new.info;
    let row = json!({
        "id": new.id,
        "kind": info.kind.as_str(),
        "file": new.file,
        "sha256": new.sha256,
        "bytes": new.bytes,
        "title": info.title,
        "artist": info.artist,
        "bpm": info.bpm.filter(|bpm| bpm.is_finite()),
        "key": info.key,
        "duration_ms": info.duration_ms,
        "verdict": info.verdict,
        "text": info.text,
        "from_sequence": new.from_sequence,
    });
    object(row)
}

impl Txn<'_> {
    /// Records a clip whose file is in place; returns its id.
    pub fn add_clip(&mut self, new: NewClip) -> Result<String> {
        if let Some(seq) = &new.from_sequence {
            if self.get("sequences", seq)?.is_none() {
                return refused("That session isn't in the library any more.");
            }
        }
        let id = new.id.clone();
        // Recorded now, so its reservation in media/ ends with this change.
        self.conn()
            .prepare_cached("DELETE FROM media_pending WHERE file = ?1")?
            .execute([&new.file])?;
        self.insert("clips", clip_row(new))?;
        Ok(id)
    }

    fn set_clip(&mut self, id: &str, col: &str, value: Value) -> Result<()> {
        if self.update("clips", id, &[(col, value)])? {
            Ok(())
        } else {
            missing_clip()
        }
    }

    /// Changes the title in the library. The file keeps its name and bytes.
    pub fn rename_clip(&mut self, id: &str, title: &str) -> Result<()> {
        self.set_clip(id, "title", title.into())
    }

    pub fn set_artist(&mut self, id: &str, artist: &str) -> Result<()> {
        self.set_clip(id, "artist", artist.into())
    }

    pub fn set_colour(&mut self, id: &str, colour: Option<Colour>) -> Result<()> {
        self.set_clip(id, "colour", colour.map(Colour::as_str).into())
    }

    pub fn set_bpm(&mut self, id: &str, bpm: Option<f64>) -> Result<()> {
        if bpm.is_some_and(|bpm| !bpm.is_finite()) {
            return refused("A BPM is a number, such as 128.");
        }
        self.set_clip(id, "bpm", bpm.into())
    }

    pub fn set_key(&mut self, id: &str, key: Option<&str>) -> Result<()> {
        self.set_clip(id, "key", key.into())
    }

    /// The drop: sets `published_at`, which puts the clip in the upload queue.
    pub fn publish(&mut self, id: &str) -> Result<()> {
        let Some(clip) = self.get("clips", id)? else {
            return missing_clip();
        };
        if clip["published_at"].is_null() {
            self.set_clip(id, "published_at", wwav_ids::now_ms().into())?;
        }
        Ok(())
    }

    /// Removes the clip's rows (its tags, its pin, the clip), never its file,
    /// so undo brings it back with its sound.
    pub fn delete_clip(&mut self, id: &str) -> Result<()> {
        if self.get("clips", id)?.is_none() {
            return missing_clip();
        }
        self.unpin(&Pin::Clip(id.to_string()))?;
        let links: Vec<String> = {
            let mut stmt = self
                .conn()
                .prepare_cached("SELECT id FROM clip_tags WHERE clip_id = ?1 ORDER BY id")?;
            let links = stmt.query_map([id], |r| r.get(0))?;
            links.collect::<rusqlite::Result<_>>()?
        };
        for link in links {
            self.delete("clip_tags", &link)?;
        }
        self.delete("clips", id)?;
        Ok(())
    }
}

/// Used where a clip row must exist for the change to make sense.
pub(crate) fn require_clip(tx: &Txn, id: &str) -> Result<Row> {
    tx.get("clips", id)?.map_or_else(missing_clip, Ok)
}
