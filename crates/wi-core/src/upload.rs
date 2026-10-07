//! Publishing is a drop, and the upload queue is a query (docs/SPEC.md 2.8,
//! 9.7): a drop sets `published_at`, and the queue is every clip with
//! `published_at IS NOT NULL AND remote_id IS NULL`, wi-store's
//! [`wi_store::UPLOAD_QUEUE`]. There is no queue file to lose.
//!
//! The worker takes one clip at a time. It signs just before each upload,
//! because presigned URLs last 300 s. A file over 250 MB goes up in 8 MiB
//! parts through `/api/upload/parts`, and each finished part is recorded
//! (`upload_part`), so a resumed upload skips it. The `trackId` the first
//! sign gave is kept across retries, and the publish carries `settings:
//! {origin: "wi_wwav", clipId}`, so a retry never posts twice (#70).
//! Failures back off from 2 s to 5 min with jitter.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wi_store::{Clip, Kind, Room, TagKind};

use crate::args::Args;
use crate::bus::lock;
use crate::net::{encode, Body, Fail};
use crate::{history, CoreError, Inner};

/// `/api/upload/sign` takes one PUT of up to 250 MB; bigger goes in parts.
pub const SINGLE_MAX: u64 = 250 * 1024 * 1024;
pub const PART: u64 = 8 * 1024 * 1024;
const FIRST_WAIT: Duration = Duration::from_secs(2);
const LONGEST_WAIT: Duration = Duration::from_secs(300);
/// How often an idle worker looks again on its own.
const IDLE: Duration = Duration::from_secs(60);

/// How long to wait before attempt `n + 1` after `n` failures: 2 s doubling
/// to 5 min, plus up to a quarter more of jitter, never past 5 min.
pub fn backoff(failures: u32, rng: &mut impl Rng) -> Duration {
    let base = FIRST_WAIT
        .saturating_mul(1u32 << failures.saturating_sub(1).min(16))
        .min(LONGEST_WAIT);
    let jitter = rng.gen_range(0..=base.as_millis() as u64 / 4);
    (base + Duration::from_millis(jitter)).min(LONGEST_WAIT)
}

/// What the server gave for one clip's upload, kept across retries and
/// relaunches beside the library.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Upload {
    track_id: Option<String>,
    s3_key: Option<String>,
    upload_id: Option<String>,
    parts: Option<u32>,
    failures: u32,
    /// When to try again, in ms since 1970.
    next_at: u64,
    /// A refusal that retrying won't fix, in the server's words.
    refused: Option<String>,
}

fn key(clip: &str) -> String {
    format!("upload/{clip}")
}

fn read_upload(i: &Inner, clip: &str) -> Result<Upload, CoreError> {
    Ok(i.kv
        .get(&key(clip))?
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

fn write_upload(i: &Inner, clip: &str, u: &Upload) -> Result<(), CoreError> {
    i.kv.set(&key(clip), &serde_json::to_value(u).unwrap_or_default())
}

/// What the status bar last said about uploads, for `publish.queue`.
#[derive(Default)]
pub(crate) struct Status {
    offline: bool,
    sentence: Option<String>,
}

fn say(i: &Inner, sentence: String) {
    i.bus.status("upload", &sentence);
    lock(&i.uploads).sentence = Some(sentence);
}

fn works(n: usize) -> String {
    if n == 1 {
        "1 work waits to go up; it leaves".to_string()
    } else {
        format!("{n} works wait to go up; they leave")
    }
}

pub(crate) fn start(inner: &Arc<Inner>) -> JoinHandle<()> {
    let i = inner.clone();
    std::thread::Builder::new()
        .name("upload queue".into())
        .spawn(move || work(&i))
        .expect("a thread for the upload queue")
}

enum Ended {
    Up,
    Interrupted,
    Failed(Fail),
}

fn work(i: &Inner) {
    while !i.closing() {
        let queue = i.store().upload_queue().unwrap_or_default();
        if queue.is_empty() || !i.net.signed_in() {
            i.nap(IDLE);
            continue;
        }
        let now = wwav_ids::now_ms();
        let mut soonest = IDLE;
        for clip in &queue {
            if i.closing() {
                return;
            }
            let Ok(mut u) = read_upload(i, &clip.id) else {
                continue;
            };
            if u.refused.is_some() {
                continue;
            }
            if u.next_at > now {
                soonest = soonest.min(Duration::from_millis(u.next_at - now));
                continue;
            }
            match upload(i, clip) {
                Ended::Up | Ended::Interrupted => {}
                Ended::Failed(f) => {
                    let Ok(fresh) = read_upload(i, &clip.id) else {
                        continue;
                    };
                    u = fresh;
                    let permanent = matches!(f.status(), Some(400 | 403 | 404 | 409 | 413 | 422));
                    if permanent {
                        u.refused = Some(f.sentence());
                        say(
                            i,
                            format!("'{}' didn't go up: {}", clip.title, f.sentence()),
                        );
                    } else {
                        u.failures += 1;
                        let wait = backoff(u.failures, &mut rand::thread_rng());
                        u.next_at = wwav_ids::now_ms() + wait.as_millis() as u64;
                        soonest = soonest.min(wait);
                        if matches!(f, Fail::Offline) {
                            lock(&i.uploads).offline = true;
                            let n = i.store().upload_queue().map_or(0, |q| q.len());
                            say(i, format!("Offline. {} when you're back.", works(n)));
                            // The rest would fail the same way: wait it out.
                            let _ = write_upload(i, &clip.id, &u);
                            break;
                        }
                    }
                    let _ = write_upload(i, &clip.id, &u);
                }
            }
        }
        i.nap(soonest);
    }
}

fn file_type(clip: &Clip) -> &'static str {
    match clip.kind {
        Kind::Wwav => "audio/wav",
        _ => match clip.file.rsplit('.').next().unwrap_or("") {
            "mp3" => "audio/mpeg",
            "flac" => "audio/flac",
            "m4a" | "aac" => "audio/mp4",
            "ogg" | "opus" => "audio/ogg",
            _ => "audio/wav",
        },
    }
}

/// The server answered, so whatever the status bar said about being
/// offline is over.
fn reached(i: &Inner) {
    lock(&i.uploads).offline = false;
}

fn failed(f: Fail) -> Ended {
    Ended::Failed(f)
}

/// The file couldn't be read here: tried again later, like a network fault.
fn disk(e: std::io::Error) -> Ended {
    failed(Fail::Status {
        status: 0,
        body: json!({"error": format!("Reading the file failed: {e}")}),
    })
}

fn api(i: &Inner, method: &str, path: &str, body: Option<&Value>) -> Result<Value, Ended> {
    i.net.api(method, path, body).map_err(failed)
}

fn put(i: &Inner, url: &str, bytes: &[u8], kind: &str) -> Result<String, Ended> {
    let r = i
        .net
        .expect("PUT", url, Body::Bytes(bytes, kind), None)
        .map_err(failed)?;
    Ok(r.etag.unwrap_or_default())
}

/// One clip, from wherever it stopped to "Up.".
fn upload(i: &Inner, clip: &Clip) -> Ended {
    match send(i, clip) {
        Ok(ended) => ended,
        Err(ended) => ended,
    }
}

fn send(i: &Inner, clip: &Clip) -> Result<Ended, Ended> {
    let path = i.store().path_of(clip);
    let size = std::fs::metadata(&path)
        .map_err(|e| {
            failed(Fail::Status {
                status: 410,
                body: json!({"error": format!("'{}' isn't on disk: {e}", clip.title)}),
            })
        })?
        .len();
    let mut u = read_upload(i, &clip.id).unwrap_or_default();
    let kind = file_type(clip);
    let (track_id, s3_key) = if size > SINGLE_MAX {
        match parts(i, clip, &path, size, kind, &mut u)? {
            Some(done) => done,
            None => return Ok(Ended::Interrupted),
        }
    } else {
        // Sign just before the PUT. The first sign mints the trackId; a
        // retry signs again under the same one.
        let signed = match &u.track_id {
            None => api(
                i,
                "GET",
                &format!("/api/upload/sign?fileType={}&size={size}", encode(kind)),
                None,
            )?,
            Some(t) => api(
                i,
                "GET",
                &format!(
                    "/api/upload/sign-replace?trackId={}&fileType={}&size={size}",
                    encode(t),
                    encode(kind)
                ),
                None,
            )?,
        };
        reached(i);
        say(i, format!("Uploading {}", clip.title));
        if let Some(t) = signed["trackId"].as_str() {
            u.track_id = Some(t.to_string());
        }
        u.s3_key = signed["s3Key"].as_str().map(String::from);
        let _ = write_upload(i, &clip.id, &u);
        let bytes = std::fs::read(&path).map_err(disk)?;
        put(
            i,
            signed["signedUrl"].as_str().unwrap_or_default(),
            &bytes,
            kind,
        )?;
        (
            u.track_id.clone().unwrap_or_default(),
            u.s3_key.clone().unwrap_or_default(),
        )
    };
    let body = json!({
        "trackId": track_id,
        "s3Key": s3_key,
        "title": clip.title,
        "settings": {"origin": "wi_wwav", "clipId": clip.id},
    });
    api(i, "POST", "/api/publish", Some(&body))?;
    let kept = i.store().mark_uploaded(&clip.id, &track_id).map_err(|e| {
        failed(Fail::Status {
            status: 500,
            body: json!({"error": e.to_string()}),
        })
    })?;
    let _ = i.kv.delete(&key(&clip.id));
    if !kept {
        // Unpublished while it went up: take it down again.
        api(
            i,
            "POST",
            "/api/unpublish",
            Some(&json!({"trackId": track_id})),
        )?;
        say(i, format!("'{}' isn't published.", clip.title));
        return Ok(Ended::Up);
    }
    place(i, clip, &track_id)?;
    history::changed(i, std::slice::from_ref(&clip.id));
    say(i, format!("Up. {} is in your galaxy.", clip.title));
    Ok(Ended::Up)
}

/// The multipart path: create (or resume) the upload, then each part not
/// yet recorded, signed just before it goes. None if the app is closing.
fn parts(
    i: &Inner,
    clip: &Clip,
    path: &std::path::Path,
    size: u64,
    kind: &str,
    u: &mut Upload,
) -> Result<Option<(String, String)>, Ended> {
    if u.upload_id.is_none() {
        let mut body = json!({"size": size, "fileType": kind});
        if let Some(t) = &u.track_id {
            body["trackId"] = json!(t);
        }
        let made = api(i, "POST", "/api/upload/parts", Some(&body))?;
        u.upload_id = made["uploadId"].as_str().map(String::from);
        u.track_id = made["trackId"].as_str().map(String::from);
        u.s3_key = made["s3Key"].as_str().map(String::from);
        u.parts = made["parts"].as_u64().map(|n| n as u32);
        // A fresh upload starts with no parts.
        let _ = i.kv.clear_upload_parts(&clip.id);
        let _ = write_upload(i, &clip.id, u);
    }
    let upload_id = u.upload_id.clone().unwrap_or_default();
    let total = u.parts.unwrap_or_else(|| size.div_ceil(PART) as u32);
    let done = i.store().upload_parts(&clip.id).unwrap_or_default();
    let mut file = File::open(path).map_err(disk)?;
    for n in 1..=total {
        if done.iter().any(|(m, _)| *m == n) {
            continue;
        }
        if i.closing() {
            return Ok(None);
        }
        let signed = match i.net.api(
            "GET",
            &format!("/api/upload/parts/{}/{n}", encode(&upload_id)),
            None,
        ) {
            Ok(v) => {
                reached(i);
                say(i, format!("Uploading {} · part {n} of {total}", clip.title));
                v
            }
            // The server forgot the upload (or finished it): start again under the same trackId.
            Err(
                f @ Fail::Status {
                    status: 404 | 409, ..
                },
            ) => {
                u.upload_id = None;
                let _ = write_upload(i, &clip.id, u);
                return Err(failed(f));
            }
            Err(f) => return Err(failed(f)),
        };
        let at = (n as u64 - 1) * PART;
        let len = PART.min(size - at) as usize;
        let mut bytes = vec![0u8; len];
        file.seek(SeekFrom::Start(at))
            .and_then(|_| file.read_exact(&mut bytes))
            .map_err(disk)?;
        let etag = put(
            i,
            signed["signedUrl"].as_str().unwrap_or_default(),
            &bytes,
            "",
        )?;
        i.store()
            .record_upload_part(&clip.id, n, &etag)
            .map_err(|e| {
                failed(Fail::Status {
                    status: 500,
                    body: json!({"error": e.to_string()}),
                })
            })?;
    }
    let parts: Vec<Value> = i
        .store()
        .upload_parts(&clip.id)
        .unwrap_or_default()
        .into_iter()
        .map(|(n, etag)| json!({"n": n, "etag": etag}))
        .collect();
    let done = api(
        i,
        "POST",
        &format!("/api/upload/parts/{}/complete", encode(&upload_id)),
        Some(&json!({"parts": parts})),
    )?;
    Ok(Some((
        done["trackId"].as_str().unwrap_or_default().to_string(),
        done["s3Key"].as_str().unwrap_or_default().to_string(),
    )))
}

/// Where the drop put it: a system in the galaxy, or the shelf with a price.
fn place(i: &Inner, clip: &Clip, track_id: &str) -> Result<(), Ended> {
    let tags = i.store().tags_of(&clip.id).unwrap_or_default();
    for t in tags.into_iter().filter(|t| t.kind == TagKind::System) {
        if let Some(system) = t.name.strip_prefix("system ") {
            let body = json!({"kind": "song", "trackId": track_id});
            match i.net.api(
                "POST",
                &format!("/api/v2/systems/{}/planets", encode(system)),
                Some(&body),
            ) {
                Ok(_) => {}
                Err(f) if f.code() == Some("already_placed") => {}
                Err(f) => return Err(failed(f)),
            }
        } else if t.name == "shelf" {
            let terms = i
                .store()
                .doc("publish.terms", &clip.id)
                .ok()
                .flatten()
                .map(|d| d.json)
                .unwrap_or(Value::Null);
            if let Some(price) = terms["price"].as_f64() {
                api(
                    i,
                    "PUT",
                    &format!("/api/tracks/{}/set-price", encode(track_id)),
                    Some(&json!({"price": price})),
                )?;
            }
        }
    }
    Ok(())
}

/// `publish.drop`: one change that tags the clip with its place, keeps its
/// terms and sets `published_at`. ⌘Z takes all three back while it waits.
pub(crate) fn drop_on(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let label = a.label()?;
    let id = a.str("clip")?;
    let target = a
        .get("target")
        .ok_or_else(|| CoreError::new("bad_args", "publish.drop needs target, {kind, id}."))?;
    let (room, tag) = match (target["kind"].as_str(), &target["id"]) {
        (Some("system"), id) if !id.is_null() => {
            let id = id.as_str().map_or_else(|| id.to_string(), String::from);
            (Room::Space, format!("system {id}"))
        }
        (Some("shelf"), _) => (Room::Unquantized, "shelf".to_string()),
        _ => {
            return Err(CoreError::new(
                "bad_args",
                "A drop lands on a system {kind: \"system\", id} or a shelf {kind: \"shelf\"}.",
            ))
        }
    };
    {
        let mut store = i.store();
        let clip = store
            .clip(id)?
            .ok_or_else(|| CoreError::new("refused", "That clip isn't in the library any more."))?;
        if !matches!(clip.kind, Kind::Wwav | Kind::Audio) {
            return Err(CoreError::new(
                "not_yet",
                "Only songs go up for now: mi-wwav.com doesn't take films from the app yet.",
            ));
        }
        let mut tx = store.begin(room, label)?;
        tx.add_tag(id, &tag, TagKind::System)?;
        if let Some(terms) = a.get("terms") {
            tx.put_doc("publish.terms", id, terms, "")?;
        }
        tx.publish(id)?;
        tx.commit()?;
    }
    history::changed(i, &[id.to_string()]);
    i.poke();
    Ok(json!({"queued": true}))
}

/// `publish.queue`: what waits, and the status bar's sentence for it.
pub(crate) fn queue(i: &Inner) -> Result<Value, CoreError> {
    let waiting = i.store().upload_queue()?;
    let mut rows = Vec::new();
    for c in &waiting {
        let u = read_upload(i, &c.id)?;
        rows.push(json!({"clip": c.id, "title": c.title, "refused": u.refused}));
    }
    let n = waiting.len();
    let status = lock(&i.uploads);
    let sentence = if n == 0 {
        "Nothing waits to go up.".to_string()
    } else if !i.net.signed_in() {
        format!("{} once you sign in.", works(n))
    } else if status.offline {
        format!("Offline. {} when you're back.", works(n))
    } else {
        status
            .sentence
            .clone()
            .unwrap_or_else(|| format!("{} shortly.", works(n)))
    };
    Ok(json!({"waiting": rows, "sentence": sentence}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn retries_back_off_from_two_seconds_to_five_minutes() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        for _ in 0..200 {
            let first = backoff(1, &mut rng);
            assert!(
                first >= Duration::from_secs(2) && first <= Duration::from_millis(2500),
                "{first:?}"
            );
            let fourth = backoff(4, &mut rng);
            assert!(
                fourth >= Duration::from_secs(16) && fourth <= Duration::from_secs(20),
                "{fourth:?}"
            );
            for n in 9..40 {
                let late = backoff(n, &mut rng);
                assert!(
                    late >= Duration::from_secs(256) && late <= LONGEST_WAIT,
                    "{n}: {late:?}"
                );
            }
        }
        // The jitter spreads retries out.
        let spread: std::collections::BTreeSet<Duration> =
            (0..50).map(|_| backoff(3, &mut rng)).collect();
        assert!(spread.len() > 10);
    }
}
