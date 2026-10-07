//! Export everything (docs/SPEC.md 2.9): one action, to a folder or a zip,
//! never behind a paywall, and working signed out.
//!
//! ```text
//! media/ + manifest.json   every library file byte for byte, with its sha256
//! sessions/                every Console session, with the plugin state it saved
//! heat.json                every record, by kind, each with its Public switch
//! notes/                   Markdown with frontmatter and [[wikilinks]]
//! galaxy.json              the galaxy, as last seen from mi-wwav.com
//! index.html               plays every master and film from disk; after one
//!                          press, "Open this folder", every song apart
//! ```
//!
//! Each file is hashed as it is copied and checked against the sha256 the
//! library recorded when it came in.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use wi_store::Kind;
use wwav_tokens::Color;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::args::Args;
use crate::player::STEMS_VERDICT;
use crate::{CoreError, Inner};

const PAGE: &str = include_str!("export.html");

/// Copies `from` to `to`, returning its sha256 and size.
fn copy_hashed(from: &Path, to: &Path) -> io::Result<(String, u64)> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut src = File::open(from)?;
    let mut dst = File::create(to)?;
    let (mut h, mut n, mut buf) = (Sha256::new(), 0u64, vec![0u8; 1 << 20]);
    loop {
        let got = src.read(&mut buf)?;
        if got == 0 {
            break;
        }
        h.update(&buf[..got]);
        dst.write_all(&buf[..got])?;
        n += got as u64;
    }
    dst.sync_all()?;
    Ok((hex::encode(h.finalize()), n))
}

fn copy_tree(from: &Path, to: &Path, count: &mut Counter) -> io::Result<()> {
    fs::create_dir_all(to)?;
    let mut entries: Vec<_> = fs::read_dir(from)?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_tree(&src, &dst, count)?;
        } else {
            let (_, bytes) = copy_hashed(&src, &dst)?;
            count.add(bytes);
        }
    }
    Ok(())
}

#[derive(Default)]
struct Counter {
    files: u64,
    bytes: u64,
}

impl Counter {
    fn add(&mut self, bytes: u64) {
        self.files += 1;
        self.bytes += bytes;
    }

    fn write(&mut self, path: &Path, text: &str) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, text)?;
        self.add(text.len() as u64);
        Ok(())
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default() + "\n"
}

/// A note's file name: the title, without characters a file system refuses,
/// made unique with " 2", " 3".
fn note_name(title: &str, taken: &mut BTreeSet<String>) -> String {
    let clean: String = title
        .chars()
        .map(|c| {
            if "/\\:*?\"<>|".contains(c) || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let base = match clean.trim() {
        "" => "Untitled".to_string(),
        t => t.to_string(),
    };
    let mut name = base.clone();
    let mut n = 1;
    while !taken.insert(name.to_lowercase()) {
        n += 1;
        name = format!("{base} {n}");
    }
    name
}

/// YAML frontmatter from (key, value) pairs; values written as JSON, which
/// YAML reads.
fn frontmatter(pairs: &[(&str, Value)]) -> String {
    let mut s = String::from("---\n");
    for (k, v) in pairs.iter().filter(|(_, v)| !v.is_null()) {
        s += &format!("{k}: {v}\n");
    }
    s + "---\n\n"
}

fn day_of(ms: u64) -> String {
    jiff::Timestamp::from_millisecond(ms as i64)
        .map(|t| t.strftime("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

fn str_of<'a>(v: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter().find_map(|k| v.get(*k)?.as_str()).unwrap_or("")
}

/// The CSS custom properties the page uses, from the design tokens
/// (docs/SPEC.md 8.11): the night register, as Space and the player wear it.
fn token_css() -> String {
    use wwav_tokens::{night, r#type, radius, stem};
    let rgb = |c: Color| format!("rgb({} {} {})", c.r, c.g, c.b);
    let ink = night::INK;
    let step = |a: f32| format!("rgb({} {} {} / {a})", ink.r, ink.g, ink.b);
    let family = |f: &[&str]| {
        f.iter()
            .map(|n| {
                if n.contains(' ') {
                    format!("\"{n}\"")
                } else {
                    n.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        ":root {{ --ground: {}; --deep: {}; --clay: {}; --ink: {}; --ink-2: {}; --ink-3: {}; \
         --hairline: {}; --accent: {}; --vocals: {}; --drums: {}; --other: {}; --bass: {}; \
         --text: {}; --name: {}; --body: {}px; --small: {}px; --title: {}px; --radius: {}px; }}",
        rgb(night::GROUND),
        rgb(night::DEEP),
        rgb(night::CLAY),
        rgb(ink),
        step(night::text_steps::BODY[1]),
        step(night::text_steps::SECONDARY[0]),
        step(night::INK_STEPS[5]),
        rgb(night::ACCENT),
        rgb(stem::VOCALS),
        rgb(stem::DRUMS),
        rgb(stem::OTHER),
        rgb(stem::BASS),
        family(r#type::text::FAMILY),
        family(r#type::name::FAMILY),
        r#type::text::BODY_PX,
        r#type::text::SMALL_PX,
        r#type::name::HEADER_PX,
        radius::CARD_PX,
    )
}

fn page(songs: &[Value], films: &[Value], exported: &str) -> String {
    let data = json!({"songs": songs, "films": films, "exported": exported});
    // Inline JSON can't close its own script element, and can't start a
    // comment or a nested script that the HTML parser would act on: every
    // "<" is written as the escape JSON.parse reads back.
    let data = data.to_string().replace('<', "\\u003c");
    PAGE.replace("/*TOKENS*/", &token_css())
        .replace("/*LIBRARY*/{}", &data)
}

/// The galaxy as mi-wwav.com has it, when signed in and online; otherwise
/// the copy kept from the last time.
fn galaxy(i: &Inner) -> Value {
    let fetched = (|| -> Option<Value> {
        let mine = i.net.api("GET", "/api/v2/galaxies/mine", None).ok()?;
        let slug = mine["data"]["galaxy"]["slug"].as_str()?.to_string();
        let whole = i
            .net
            .api("GET", &format!("/api/v2/galaxies/{slug}"), None)
            .ok()?;
        let sun = i
            .net
            .api("GET", &format!("/api/v2/galaxies/{slug}/sun"), None)
            .ok();
        let mut systems = Vec::new();
        for s in whole["data"]["systems"].as_array().into_iter().flatten() {
            let Some(sys) = s["slug"].as_str() else {
                continue;
            };
            if let Ok(v) = i.net.api(
                "GET",
                &format!("/api/v2/galaxies/{slug}/systems/{sys}"),
                None,
            ) {
                let id = v["data"]["system"]["id"].clone();
                let links = i
                    .net
                    .api(
                        "GET",
                        &format!("/api/v2/lineage-links?type=system&id={id}"),
                        None,
                    )
                    .map(|l| l["data"]["links"].clone())
                    .unwrap_or(Value::Null);
                systems.push(json!({"system": v["data"], "links": links}));
            }
        }
        Some(json!({
            "galaxy": whole["data"]["galaxy"],
            "sun": sun.map(|s| s["data"]["sun"].clone()),
            "systems": systems,
            "fetched": jiff::Timestamp::now().to_string(),
        }))
    })();
    match fetched {
        Some(g) => {
            let _ = i.kv.set("galaxy", &g);
            g
        }
        None => i.kv.get("galaxy").ok().flatten().unwrap_or_else(|| {
            json!({"galaxy": null, "note": "Not signed in when this was exported, and never seen before. Everything else is here."})
        }),
    }
}

pub(crate) fn everything(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let to = PathBuf::from(a.str("to")?);
    let zip = a.opt_bool("zip")?.unwrap_or(false);
    let folder = if zip {
        let mut part = to.clone().into_os_string();
        part.push(".part");
        PathBuf::from(part)
    } else {
        to.clone()
    };
    if folder.exists() && fs::read_dir(&folder)?.next().is_some() {
        return Err(CoreError::new(
            "not_empty",
            format!(
                "'{}' already holds files. Choose an empty folder.",
                folder.display()
            ),
        ));
    }
    fs::create_dir_all(&folder)?;
    let written = write_all(i, &folder);
    let result = match (written, zip) {
        (Err(e), _) => Err(e),
        (Ok((count, mismatched)), false) => Ok((count, mismatched)),
        (Ok((count, mismatched)), true) => zip_folder(&folder, &to)
            .map(|()| (count, mismatched))
            .map_err(CoreError::from),
    };
    if zip {
        let _ = fs::remove_dir_all(&folder);
    }
    let (count, mismatched) = result?;
    let sentence = if mismatched.is_empty() {
        format!("Exported everything to {}.", to.display())
    } else {
        format!(
            "Exported, but {} changed on disk since it came in: {}.",
            if mismatched.len() == 1 {
                "1 file has"
            } else {
                "some files have"
            },
            mismatched.join(", ")
        )
    };
    i.bus.status("export", &sentence);
    Ok(
        json!({"path": to, "files": count.files, "bytes": count.bytes, "mismatched": mismatched, "sentence": sentence}),
    )
}

fn zip_folder(folder: &Path, out: &Path) -> io::Result<()> {
    let mut part = out.to_path_buf().into_os_string();
    part.push(".writing");
    let part = PathBuf::from(part);
    let mut z = ZipWriter::new(File::create(&part)?);
    let opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(true);
    let mut todo = vec![folder.to_path_buf()];
    while let Some(dir) = todo.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir)?.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let rel = e
                .path()
                .strip_prefix(folder)
                .map_err(io::Error::other)?
                .to_string_lossy()
                .replace('\\', "/");
            if e.file_type()?.is_dir() {
                z.add_directory(format!("{rel}/"), opts)
                    .map_err(io::Error::other)?;
                todo.push(e.path());
            } else {
                z.start_file(rel, opts).map_err(io::Error::other)?;
                io::copy(&mut File::open(e.path())?, &mut z)?;
            }
        }
    }
    z.finish().map_err(io::Error::other)?.sync_all()?;
    fs::rename(&part, out)
}

fn write_all(i: &Inner, out: &Path) -> Result<(Counter, Vec<String>), CoreError> {
    let mut count = Counter::default();
    let mut mismatched = Vec::new();
    let (clips, sequences, root) = {
        let store = i.store();
        (
            store.clips()?,
            store.sequences()?,
            store.root().to_path_buf(),
        )
    };
    let total = clips.len();
    let mut manifest = Vec::new();
    let mut songs = Vec::new();
    let mut films = Vec::new();
    for (n, c) in clips.iter().enumerate() {
        let src = root.join(&c.file);
        let name = Path::new(&c.file)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| c.id.clone());
        // Left-in-place files come along too, under their clip's id.
        let name = if Path::new(&c.file).is_absolute() {
            match Path::new(&c.file).extension() {
                Some(ext) => format!("{}.{}", c.id, ext.to_string_lossy()),
                None => c.id.clone(),
            }
        } else {
            name
        };
        let rel = format!("media/{name}");
        i.bus.emit(
            "export",
            json!({"done": n, "total": total, "sentence": format!("Exporting {} of {total}", n + 1)}),
        );
        let (sha256, bytes) = match copy_hashed(&src, &out.join(&rel)) {
            Ok(done) => done,
            Err(e) => {
                mismatched.push(format!("'{}' ({e})", c.title));
                continue;
            }
        };
        count.add(bytes);
        let verified = sha256 == c.sha256;
        if !verified {
            mismatched.push(format!("'{}'", c.title));
        }
        manifest.push(json!({
            "id": c.id, "path": rel, "kind": c.kind.as_str(), "title": c.title, "artist": c.artist,
            "sha256": sha256, "bytes": bytes, "verified": verified, "verdict": c.verdict,
        }));
        match c.kind {
            Kind::Wwav | Kind::Audio => songs.push(json!({
                "id": c.id, "title": c.title, "artist": c.artist, "file": rel, "key": c.key,
                "bpm": c.bpm, "duration": c.duration_ms as f64 / 1000.0,
                "stems": c.kind == Kind::Wwav && c.verdict == STEMS_VERDICT,
            })),
            Kind::Swav | Kind::Video => films.push(json!({
                "id": c.id, "title": c.title, "artist": c.artist, "file": rel,
            })),
            _ => {}
        }
    }
    count.write(
        &out.join("manifest.json"),
        &pretty(&json!({"files": manifest})),
    )?;
    fs::create_dir_all(out.join("sessions"))?;
    for s in &sequences {
        let src = root.join(&s.package);
        if src.is_dir() {
            copy_tree(&src, &out.join(&s.package), &mut count)?;
        }
    }

    // Every record, by kind, each with its Public switch (3.14). What isn't a
    // record is left out: the timer, the settings, and what other calendars'
    // feeds held. The calendars' addresses are in the Keychain, not here (3.11),
    // and the text of your mail stays in Gmail and on this Mac (3.10).
    let kinds =
        i.kv.query_strings("SELECT DISTINCT kind FROM docs ORDER BY kind")?;
    let mut records = Map::new();
    for kind in kinds.iter().filter(|k| {
        !["heatState", "heatSetting", "calendarEvent", "mailText", "syllabusDraft"].contains(&k.as_str())
    }) {
        let docs = i.store().docs(kind)?;
        let switch = [
            "task",
            "focusSession",
            "project",
            "milestone",
            "habit",
            "course",
            "grade",
            "dailyNote",
            "note",
        ]
        .contains(&kind.as_str());
        records.insert(
            kind.clone(),
            json!(docs
                .into_iter()
                .map(|d| {
                    let mut v = d.json;
                    if let (true, Some(m)) = (switch, v.as_object_mut()) {
                        m.entry("public").or_insert(json!(false));
                    }
                    v
                })
                .collect::<Vec<_>>()),
        );
    }
    let exported = jiff::Timestamp::now().to_string();
    count.write(
        &out.join("heat.json"),
        &pretty(&json!({"version": 1, "exported": exported, "records": records})),
    )?;

    write_notes(out, &clips, &records, &i.clock().zone, &mut count)?;
    count.write(&out.join("galaxy.json"), &pretty(&galaxy(i)))?;
    count.write(&out.join("index.html"), &page(&songs, &films, &exported))?;
    Ok((count, mismatched))
}

/// notes/: text clips, daily notes and captures as Markdown, and each space
/// and project as a note that links the other by [[wikilink]], as Obsidian
/// reads them.
fn write_notes(
    out: &Path,
    clips: &[wi_store::Clip],
    records: &Map<String, Value>,
    zone: &jiff::tz::TimeZone,
    count: &mut Counter,
) -> io::Result<()> {
    let dir = out.join("notes");
    fs::create_dir_all(&dir)?;
    let mut taken = BTreeSet::new();
    let of = |kind: &str| -> Vec<&Value> {
        records
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(kind))
            .flat_map(|(_, v)| v.as_array().into_iter().flatten())
            .collect()
    };
    for c in clips.iter().filter(|c| c.kind == Kind::Text) {
        let name = note_name(&c.title, &mut taken);
        let created = wwav_ids::ulid_ms(&c.id).map(day_of);
        let text = frontmatter(&[("id", json!(c.id)), ("created", json!(created))])
            + c.text.as_deref().unwrap_or("");
        count.write(&dir.join(format!("{name}.md")), &text)?;
    }
    for d in of("dailyNote") {
        let date = str_of(d, &["date"]);
        let name = note_name(date, &mut taken);
        let text = frontmatter(&[("date", json!(date))]) + str_of(d, &["markdown"]);
        count.write(&dir.join(format!("{name}.md")), &text)?;
    }
    for c in of("capture") {
        let name = note_name(&format!("Capture {}", str_of(c, &["id"])), &mut taken);
        let text = frontmatter(&[("id", c["id"].clone()), ("link", c["link"].clone())])
            + str_of(c, &["text"]);
        count.write(&dir.join(format!("{name}.md")), &text)?;
    }
    // Spaces and projects, linked both ways by name.
    let spaces = of("space");
    let projects = of("project");
    let milestones = of("milestone");
    let tasks = of("task");
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for s in &spaces {
        names.insert(
            str_of(s, &["id"]).to_string(),
            note_name(str_of(s, &["name"]), &mut taken),
        );
    }
    for p in &projects {
        names.insert(
            str_of(p, &["id"]).to_string(),
            note_name(str_of(p, &["title"]), &mut taken),
        );
    }
    for s in &spaces {
        let id = str_of(s, &["id"]);
        let mut text = frontmatter(&[("id", json!(id)), ("kind", json!("space"))]);
        for p in projects.iter().filter(|p| str_of(p, &["spaceId"]) == id) {
            text += &format!("- [[{}]]\n", names[str_of(p, &["id"])]);
        }
        count.write(&dir.join(format!("{}.md", names[id])), &text)?;
    }
    for p in &projects {
        let id = str_of(p, &["id"]);
        let space = names
            .get(str_of(p, &["spaceId"]))
            .map(|n| format!("[[{n}]]"));
        let mut text = frontmatter(&[
            ("id", json!(id)),
            ("status", p["status"].clone()),
            ("targetDate", p["targetDate"].clone()),
            ("space", json!(space)),
        ]);
        let mine: Vec<&&Value> = milestones
            .iter()
            .filter(|m| str_of(m, &["projectId"]) == id)
            .collect();
        if !mine.is_empty() {
            text += "## Milestones\n\n";
            for m in mine {
                let done = if m["done"] == json!(true) { "x" } else { " " };
                text += &format!(
                    "- [{done}] {} ({})\n",
                    str_of(m, &["title"]),
                    str_of(m, &["date"])
                );
            }
            text += "\n";
        }
        let open: Vec<&&Value> = tasks
            .iter()
            .filter(|t| str_of(t, &["projectId"]) == id)
            .collect();
        if !open.is_empty() {
            text += "## Tasks\n\n";
            for t in open {
                let done = if t["done"] == json!(true) { "x" } else { " " };
                text += &format!("- [{done}] {}\n", str_of(t, &["title"]));
            }
        }
        count.write(&dir.join(format!("{}.md", names[id])), &text)?;
    }

    // Notes, each linked to its project.
    for n in of("note") {
        let body = str_of(n, &["markdown"]);
        let title = match n["title"].as_str().map(str::trim).filter(|t| !t.is_empty()) {
            Some(t) => t.to_string(),
            None => body.lines().next().unwrap_or("").chars().take(40).collect(),
        };
        let name = note_name(&title, &mut taken);
        let project = n["projectId"]
            .as_str()
            .and_then(|p| names.get(p))
            .map(|p| format!("[[{p}]]"));
        let text = frontmatter(&[
            ("id", n["id"].clone()),
            ("project", json!(project)),
            ("public", n["public"].clone()),
        ]) + body;
        count.write(&dir.join(format!("{name}.md")), &text)?;
    }

    // Courses with their categories and grades, by the course's code.
    let terms: BTreeMap<&str, &str> = of("term")
        .into_iter()
        .map(|t| (str_of(t, &["id"]), str_of(t, &["name"])))
        .collect();
    let all_grades = of("grade");
    let mut course_names: BTreeMap<String, String> = BTreeMap::new();
    for c in of("course") {
        let id = str_of(c, &["id"]).to_string();
        course_names.insert(id, note_name(str_of(c, &["code"]), &mut taken));
    }
    for c in of("course") {
        let id = str_of(c, &["id"]);
        let term = terms.get(str_of(c, &["termId"])).map(|t| t.to_string());
        let mut text = frontmatter(&[
            ("id", json!(id)),
            ("name", c["name"].clone()),
            ("term", json!(term)),
            ("public", c["public"].clone()),
        ]);
        let cats = c["categories"].as_array().cloned().unwrap_or_default();
        if !cats.is_empty() {
            text += "## Categories\n\n";
            for k in &cats {
                text += &format!("- {}: {}%\n", str_of(k, &["name"]), k["weight"]);
            }
            text += "\n";
        }
        let mine: Vec<&&Value> = all_grades
            .iter()
            .filter(|g| str_of(g, &["courseId"]) == id)
            .collect();
        if !mine.is_empty() {
            text += "## Grades\n\n";
            for g in mine {
                let score = match g["score"].as_f64() {
                    Some(s) => format!("{s}/{}", g["outOf"]),
                    None => "pending".to_string(),
                };
                text += &format!("- {}: {score}\n", str_of(g, &["title"]));
            }
        }
        count.write(&dir.join(format!("{}.md", course_names[id])), &text)?;
    }

    // Habits: each day done is a link to that day's note.
    for h in of("habit") {
        let name = note_name(str_of(h, &["title"]), &mut taken);
        let mut text = frontmatter(&[
            ("id", h["id"].clone()),
            ("minutes", h["minutes"].clone()),
            ("public", h["public"].clone()),
        ]);
        let mut days: Vec<&str> = h["log"]
            .as_object()
            .map(|l| {
                l.iter()
                    .filter(|(_, v)| **v == json!(true))
                    .map(|(d, _)| d.as_str())
                    .collect()
            })
            .unwrap_or_default();
        days.sort_unstable();
        for d in days {
            text += &format!("- [[{d}]]\n");
        }
        count.write(&dir.join(format!("{name}.md")), &text)?;
    }

    // Every task, open and done, with links to its space, project and course.
    if !tasks.is_empty() {
        let day = |ms: f64| {
            jiff::Timestamp::from_millisecond(ms as i64)
                .map(|t| t.to_zoned(zone.clone()).strftime("%Y-%m-%d").to_string())
                .unwrap_or_default()
        };
        let line = |t: &Value| {
            let mut s = format!(
                "- [{}] {}",
                if t["done"] == json!(true) { "x" } else { " " },
                str_of(t, &["title"])
            );
            if let Some(due) = t["due"].as_f64() {
                s += &format!(", due {}", day(due));
            }
            for (field, map) in [
                ("spaceId", &names),
                ("projectId", &names),
                ("courseId", &course_names),
            ] {
                if let Some(n) = t[field].as_str().and_then(|id| map.get(id)) {
                    s += &format!(" [[{n}]]");
                }
            }
            s + "\n"
        };
        let mut text = String::from("## Open\n\n");
        text.extend(
            tasks
                .iter()
                .filter(|t| t["done"] != json!(true))
                .map(|t| line(t)),
        );
        text += "\n## Done\n\n";
        text.extend(
            tasks
                .iter()
                .filter(|t| t["done"] == json!(true))
                .map(|t| line(t)),
        );
        let name = note_name("Tasks", &mut taken);
        count.write(&dir.join(format!("{name}.md")), &text)?;
    }
    Ok(())
}
