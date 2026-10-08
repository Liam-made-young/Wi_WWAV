//! The capture inbox (docs/NOTES.md, "The capture inbox"), against stand-ins
//! for the text reader and for the command line. What a fail looks like:
//! - a page photographed in class isn't filed to that class, or isn't
//!   titled for it, or its image isn't on top with its text below;
//! - a word from the page can't be found;
//! - the original is left in the inbox, or is lost;
//! - undoing the filing loses the page;
//! - Claude is sent a page without being allowed to, or what it suggests is
//!   applied without a click.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

const DAY: &str = "2026-10-07";
const NOW: &str = "2026-10-07 10:40";
const T: Duration = Duration::from_secs(30);

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// A stand-in for `wi-ocr`: it copies the file to each page's image and
/// answers `pages` (each `(text, confidence)`), with `taken` when given.
fn fake_reader(dir: &Path, taken: Option<&str>, pages: &[(&str, f64)]) -> PathBuf {
    let mut copies = String::new();
    let mut list = Vec::new();
    for (n, (text, confidence)) in pages.iter().enumerate() {
        copies.push_str(&format!("cp \"$1\" \"$2/page-{}.jpg\"\n", n + 1));
        list.push(format!(
            "{{\"text\":{},\"confidence\":{confidence},\"image\":\"'\"$2\"'/page-{}.jpg\"}}",
            json!(text),
            n + 1
        ));
    }
    let taken = taken.map_or(String::new(), |t| {
        format!("\"taken\":\"{t}\",\"offset\":\"-04:00\",")
    });
    script(
        dir,
        "wi-ocr",
        &format!(
            "{copies}printf '%s\\n' '{{{taken}\"pages\":[{}]}}'",
            list.join(",")
        ),
    )
}

fn fake_claude(dir: &Path, answer: &Value) -> PathBuf {
    std::fs::write(
        dir.join("answer.json"),
        json!({"is_error": false, "result": "", "structured_output": answer}).to_string(),
    )
    .unwrap();
    script(
        dir,
        "claude",
        &format!(
            "printf '%s\\n' \"$@\" > '{d}/args'\nls > '{d}/files'\ncat > '{d}/prompt'\ncat '{d}/answer.json'",
            d = dir.display()
        ),
    )
}

struct Desk {
    core: Core,
    notes: PathBuf,
    inbox: PathBuf,
}

fn desk(setup: &Setup, reader: Option<PathBuf>, claude: Option<PathBuf>) -> Desk {
    let notes = setup.dir.path().join("Notes");
    let inbox = setup.dir.path().join("Wi-WWAV Inbox");
    std::fs::create_dir_all(&inbox).unwrap();
    let mut config = setup.config(NO_SERVER, no_browser());
    config.now = Some(ny(NOW));
    config.notes_dir = Some(notes.clone());
    config.capture_inboxes = vec![inbox.clone()];
    config.ocr = Some(reader.unwrap_or_else(|| setup.dir.path().join("no-reader")));
    config.capture_claude = claude.is_some();
    config.claude = Some(claude.unwrap_or_else(|| setup.dir.path().join("no-claude")));
    let core = Core::open(&setup.library(), config).unwrap();
    ok(
        &core,
        "app.settings.set",
        json!({"patch": {"heat": {"timeZone": "America/New_York"}}}),
    );
    ok(
        &core,
        "heat.school.set",
        json!({"name": "URI", "host": "brightspace.uri.edu", "codePattern": "([A-Z]{2,4})\\s?(\\d{3})", "termStart": "2026-09-09", "termEnd": "2026-12-11"}),
    );
    ok(
        &core,
        "heat.commitment.create",
        json!({"course": "JPN 101", "days": ["MO", "WE", "FR"], "start": "10:00", "end": "10:50"}),
    );
    Desk { core, notes, inbox }
}

fn files(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn note_titled(core: &Core, title: &str) -> Value {
    records(&snap(core, DAY), "note")
        .into_iter()
        .find(|n| n["title"] == title)
        .unwrap_or(Value::Null)
}

#[test]
fn a_page_photographed_in_class_is_filed_to_the_class_image_on_top_text_below() {
    let setup = Setup::new();
    let reader = fake_reader(
        setup.dir.path(),
        Some("2026-10-07T10:22:31"),
        &[("Te-form: taberu becomes tabete\nQuiz on Friday", 0.93)],
    );
    let d = desk(&setup, Some(reader), None);
    std::fs::write(d.inbox.join("IMG_2211.jpg"), b"a photo of a notebook page").unwrap();
    std::fs::write(d.inbox.join(".IMG_2212.jpg.icloud"), b"not here yet").unwrap();
    std::fs::write(d.inbox.join("notes.txt"), b"not a capture").unwrap();
    assert_eq!(snap(&d.core, DAY)["notes"]["capture"]["waiting"], 1);

    let done = ok(&d.core, "heat.capture.process", json!({"wait": true}));
    assert_eq!(
        done["notes"],
        json!([{"id": done["notes"][0]["id"], "title": "JPN 101 · Oct 7", "line": "Filed to JPN 101 · Oct 7"}])
    );
    let note = note_titled(&d.core, "JPN 101 · Oct 7");
    let course = records(&snap(&d.core, DAY), "course")[0].clone();
    assert_eq!(note["courseId"], course["id"]);
    assert!(note.get("inbox").is_none());
    assert_eq!(
        note["capturedAt"].as_f64(),
        Some(ny("2026-10-07 10:22") + 31_000.0)
    );
    // The image on top, the text below.
    assert_eq!(note["markdown"], "![](attachments/2026-10-07-1022-jpn101.jpg)\n\nTe-form: taberu becomes tabete\nQuiz on Friday\n");
    assert_eq!(
        note["attachments"],
        json!([
            "attachments/2026-10-07-1022-jpn101.jpg",
            "attachments/2026-10-07-1022-jpn101-original.jpg"
        ])
    );
    // The original is out of the inbox and in the attachments; what isn't a capture is left alone.
    assert_eq!(files(&d.inbox), [".IMG_2212.jpg.icloud", "notes.txt"]);
    assert_eq!(
        files(&d.notes.join("attachments")),
        [
            "2026-10-07-1022-jpn101-original.jpg",
            "2026-10-07-1022-jpn101.jpg"
        ]
    );
    assert_eq!(
        std::fs::read(
            d.notes
                .join("attachments/2026-10-07-1022-jpn101-original.jpg")
        )
        .unwrap(),
        b"a photo of a notebook page"
    );
    // The note is a file, saying where it is filed and when it was taken.
    let text = std::fs::read_to_string(d.notes.join("JPN 101 · Oct 7.md")).unwrap();
    assert!(
        text.contains(
            "course: JPN 101\ncaptured: 2026-10-07T10:22:31-04:00\n---\n\n![](attachments/"
        ),
        "{text}"
    );
    // A word from the page finds it, here and in the search ⌘K reads.
    assert_eq!(
        ok(&d.core, "heat.note.search", json!({"q": "tabete"}))["hits"][0]["title"],
        "JPN 101 · Oct 7"
    );
    let index = wi_store::Store::open(&setup.library()).unwrap();
    let found = index.search_docs("tabete", 10).unwrap();
    assert!(
        found
            .iter()
            .any(|doc| doc.kind == "note" && doc.key == note["id"].as_str().unwrap()),
        "the local index doesn't hold the page's text"
    );
    drop(index);
    // The view can show the page.
    let image = ok(
        &d.core,
        "heat.note.attachment",
        json!({"path": "attachments/2026-10-07-1022-jpn101.jpg"}),
    );
    assert!(image["dataUrl"]
        .as_str()
        .unwrap()
        .starts_with("data:image/jpeg;base64,"));

    // One quiet notice, with an undo that takes back the filing and keeps the page.
    let s = snap(&d.core, DAY);
    let notice = &s["notices"][0];
    assert_eq!(
        (
            notice["kind"].as_str(),
            notice["text"].as_str(),
            notice["noteId"].clone()
        ),
        (
            Some("filed"),
            Some("Filed to JPN 101 · Oct 7"),
            note["id"].clone()
        )
    );
    assert_eq!(
        ok(&d.core, "history.get", json!({"room": "heat"}))["undo"],
        "Undo file note"
    );
    ok(
        &d.core,
        "history.undoEntry",
        json!({"txnId": notice["undo"]["txnId"]}),
    );
    let back = note_titled(&d.core, "Capture · Oct 7, 10:22 AM");
    assert_eq!(
        (back["id"].clone(), back["inbox"].clone()),
        (note["id"].clone(), json!(true))
    );
    assert!(back.get("courseId").is_none());
    assert_eq!(snap(&d.core, DAY)["notes"]["inbox"], json!([note["id"]]));
    // Filed by hand with one click, it leaves the inbox.
    let filed = ok(
        &d.core,
        "heat.note.file",
        json!({"id": note["id"], "course": "JPN 101"}),
    );
    assert_eq!(filed["line"], "Filed to JPN 101");
    assert!(snap(&d.core, DAY)["notes"]["inbox"]
        .as_array()
        .unwrap()
        .is_empty());
    ok(&d.core, "heat.notice.dismiss", json!({"id": notice["id"]}));
    assert!(snap(&d.core, DAY)["notices"].as_array().unwrap().is_empty());
    // Nothing was sent anywhere: this core may not ask Claude.
    assert!(!setup.dir.path().join("prompt").exists());
}

#[test]
fn the_folder_is_watched_and_a_dropped_file_is_read_by_itself() {
    let setup = Setup::new();
    // No date in the file: the Shortcut's name carries the time it was shared, in class.
    let reader = fake_reader(
        setup.dir.path(),
        None,
        &[("Particles wa and ga, with examples from the board", 0.9)],
    );
    let d = desk(&setup, Some(reader), None);
    std::fs::write(d.inbox.join("2026-10-07 10.31.05 Photo.png"), b"png").unwrap();
    assert!(
        eventually(T, || !note_titled(&d.core, "JPN 101 · Oct 7").is_null()),
        "the inbox was never read"
    );
    let note = note_titled(&d.core, "JPN 101 · Oct 7");
    assert_eq!(note["markdown"], "![](attachments/2026-10-07-1031-jpn101.jpg)\n\nParticles wa and ga, with examples from the board\n");
    assert!(files(&d.inbox).is_empty());
    // A second page from the same class gets a name of its own.
    std::fs::write(d.inbox.join("2026-10-07 10.45.00 Photo.png"), b"png2").unwrap();
    assert!(eventually(T, || !note_titled(&d.core, "JPN 101 · Oct 7 2").is_null()));
    // Dropped on the Notes tab, or pasted: into the notes folder's own inbox, and read.
    let pasted = ok(
        &d.core,
        "heat.capture.inbox.add",
        json!({"name": "2026-10-06 15.00.00 pasted.png", "base64": "cG5n"}),
    );
    assert_eq!(pasted["added"], 1);
    assert!(
        eventually(T, || !note_titled(&d.core, "Capture · Oct 6, 3:00 PM")
            .is_null()),
        "a pasted image was never read"
    );
    // Tuesday afternoon is no class: it waits in the Notes inbox.
    assert_eq!(
        note_titled(&d.core, "Capture · Oct 6, 3:00 PM")["inbox"],
        true
    );
    assert_eq!(
        refused(
            &d.core,
            "heat.capture.inbox.add",
            json!({"name": "essay.docx", "base64": "eA=="})
        )
        .1,
        "A capture is a photo, a screenshot or a PDF."
    );
}

#[test]
fn outside_class_claude_picks_the_place_and_only_suggests_the_rest() {
    let setup = Setup::new();
    let reader = fake_reader(
        setup.dir.path(),
        Some("2026-10-07T15:05:00"),
        &[(
            "The voltage law: the sum around a loop is zero. Lab report due Friday.",
            0.95,
        )],
    );
    let claude = fake_claude(
        setup.dir.path(),
        &json!({"place": {"kind": "course", "name": "EGR 101 (Foundations of Engineering)"},
                "tasks": [{"title": "Write the lab report", "due": "2026-10-09"}], "terms": ["voltage law", "loop"]}),
    );
    let d = desk(&setup, Some(reader), Some(claude));
    ok(
        &d.core,
        "heat.commitment.create",
        json!({"course": "EGR 101", "days": ["TU", "TH"], "start": "11:00", "end": "12:15"}),
    );
    let egr = records(&snap(&d.core, DAY), "course")
        .into_iter()
        .find(|c| c["code"] == "EGR 101")
        .unwrap();
    ok(
        &d.core,
        "heat.course.update",
        json!({"id": egr["id"], "set": {"name": "Foundations of Engineering"}}),
    );
    let photo = setup.dir.path().join("board.jpg");
    std::fs::write(&photo, b"jpg").unwrap();
    // A file named by its path is read where it is, and left there.
    let done = ok(&d.core, "heat.capture.process", json!({"path": photo}));
    assert_eq!(done["notes"][0]["line"], "Filed to EGR 101 · Oct 7");
    assert!(photo.exists());
    let note = note_titled(&d.core, "EGR 101 · Oct 7");
    assert_eq!(note["courseId"], egr["id"]);
    // One run, text only: no tools, the small model, the person's own lists to pick from.
    let args = std::fs::read_to_string(setup.dir.path().join("args")).unwrap();
    assert!(
        args.contains("--tools\n\n") && args.contains("--model\nhaiku"),
        "{args}"
    );
    let prompt = std::fs::read_to_string(setup.dir.path().join("prompt")).unwrap();
    assert!(
        prompt.contains("Courses: JPN 101; EGR 101 (Foundations of Engineering)")
            && prompt.contains("The voltage law")
            && prompt.contains("Spaces: "),
        "{prompt}"
    );
    // What it suggested waits beside the note. Nothing is a task until it is clicked.
    let s = snap(&d.core, DAY);
    let sug = &s["notes"]["suggestions"][note["id"].as_str().unwrap()];
    assert_eq!(
        sug["tasks"],
        json!([{"title": "Write the lab report", "due": "2026-10-09"}])
    );
    assert_eq!(sug["terms"], json!(["voltage law", "loop"]));
    assert!(records(&s, "task").is_empty());
    let added = ok(
        &d.core,
        "heat.note.suggestion.accept",
        json!({"noteId": note["id"], "index": 0}),
    );
    assert_eq!(added["undo"], "Undo add task");
    let task = &added["task"];
    assert_eq!(
        (
            task["title"].as_str(),
            task["courseId"].clone(),
            task["noteId"].clone()
        ),
        (
            Some("Write the lab report"),
            egr["id"].clone(),
            note["id"].clone()
        )
    );
    assert_eq!(task["due"].as_f64(), Some(ny("2026-10-09 23:59")));
    assert_eq!(
        refused(
            &d.core,
            "heat.note.suggestion.accept",
            json!({"noteId": note["id"], "index": 0})
        )
        .1,
        "That one is a task already."
    );
    ok(
        &d.core,
        "heat.note.suggestion.dismiss",
        json!({"noteId": note["id"]}),
    );
    assert!(snap(&d.core, DAY)["notes"]["suggestions"]
        .as_object()
        .unwrap()
        .is_empty());

    // Switched off by the person, a page goes nowhere but the Notes inbox.
    ok(
        &d.core,
        "heat.capture.settings.set",
        json!({"claude": false}),
    );
    assert_eq!(snap(&d.core, DAY)["notes"]["capture"]["claude"], false);
    std::fs::remove_file(setup.dir.path().join("prompt")).unwrap();
    let quiet = ok(&d.core, "heat.capture.process", json!({"path": photo}));
    assert_eq!(quiet["notes"][0]["line"], "Captured to the Notes inbox");
    assert!(
        !setup.dir.path().join("prompt").exists(),
        "Claude was asked with the switch off"
    );
}

#[test]
fn a_page_this_mac_reads_poorly_is_read_by_claude_from_the_image_alone() {
    let setup = Setup::new();
    // Low confidence, and Japanese: both go to Claude.
    let reader = fake_reader(
        setup.dir.path(),
        Some("2026-10-07T10:30:00"),
        &[("te fom tabe7", 0.21)],
    );
    let claude = fake_claude(
        setup.dir.path(),
        &json!({"text": "て形: 食べる → 食べて\nQuiz Friday", "place": {"kind": "none", "name": null}, "tasks": [], "terms": ["て形"]}),
    );
    let d = desk(&setup, Some(reader), Some(claude));
    std::fs::write(d.inbox.join("IMG_1.jpg"), b"jpg").unwrap();
    ok(&d.core, "heat.capture.process", json!({"wait": true}));
    let note = note_titled(&d.core, "JPN 101 · Oct 7");
    assert_eq!(
        note["markdown"],
        "![](attachments/2026-10-07-1030-jpn101.jpg)\n\nて形: 食べる → 食べて\nQuiz Friday\n"
    );
    assert_eq!(
        ok(&d.core, "heat.note.search", json!({"q": "食べて"}))["hits"][0]["id"],
        note["id"]
    );
    // The run could read, and only read, standing in a folder with the page alone.
    let args = std::fs::read_to_string(setup.dir.path().join("args")).unwrap();
    assert!(
        args.contains("--tools\nRead\n") && args.contains("--model\nsonnet"),
        "{args}"
    );
    assert_eq!(
        std::fs::read_to_string(setup.dir.path().join("files"))
            .unwrap()
            .trim(),
        "page-1.jpg"
    );
    let prompt = std::fs::read_to_string(setup.dir.path().join("prompt")).unwrap();
    assert!(
        prompt.contains("Open page-1.jpg") && prompt.contains("It is filed already"),
        "{prompt}"
    );
    assert!(
        !prompt.contains("te fom"),
        "what was misread isn't passed on"
    );
    assert_eq!(
        snap(&d.core, DAY)["notes"]["suggestions"][note["id"].as_str().unwrap()]["terms"],
        json!(["て形"])
    );
}

#[test]
fn a_scanned_pdf_is_one_note_with_every_page() {
    let setup = Setup::new();
    let reader = fake_reader(
        setup.dir.path(),
        None,
        &[
            ("First page of the handout, about loops", 0.9),
            ("Second page of the handout, about nodes", 0.9),
        ],
    );
    let d = desk(&setup, Some(reader), None);
    std::fs::write(d.inbox.join("2026-10-08 20.00.00 Scan.pdf"), b"%PDF-1.4").unwrap();
    let done = ok(&d.core, "heat.capture.process", json!({"wait": true}));
    assert_eq!(done["notes"][0]["line"], "Captured to the Notes inbox");
    let note = note_titled(&d.core, "Capture · Oct 8, 8:00 PM");
    assert_eq!(
        note["markdown"],
        "![Page 1](attachments/2026-10-08-2000-capture-p1.jpg)\n\nFirst page of the handout, about loops\n\n\
         ![Page 2](attachments/2026-10-08-2000-capture-p2.jpg)\n\nSecond page of the handout, about nodes\n\n\
         [The PDF](attachments/2026-10-08-2000-capture.pdf)\n"
    );
    assert_eq!(files(&d.notes.join("attachments")).len(), 3);
    assert!(files(&d.inbox).is_empty());
    assert_eq!(
        ok(&d.core, "heat.note.search", json!({"q": "nodes"}))["hits"][0]["id"],
        note["id"]
    );
}

#[test]
fn with_no_reader_the_page_is_still_kept_image_on_top() {
    let setup = Setup::new();
    let d = desk(&setup, None, None);
    assert_eq!(snap(&d.core, DAY)["notes"]["capture"]["reader"], "missing");
    std::fs::write(d.inbox.join("2026-10-07 10.05.00 Page.png"), b"png").unwrap();
    ok(&d.core, "heat.capture.process", json!({"wait": true}));
    let note = note_titled(&d.core, "JPN 101 · Oct 7");
    assert_eq!(
        note["markdown"],
        "![](attachments/2026-10-07-1005-jpn101.png)\n"
    );
    assert_eq!(
        files(&d.notes.join("attachments")),
        ["2026-10-07-1005-jpn101.png"]
    );
    // A file the reader refuses is said once, and left where it is.
    let broken = script(
        setup.dir.path(),
        "broken-ocr",
        "echo \"That file isn't an image Learn can read.\" >&2; exit 1",
    );
    let again = Setup::new();
    let d2 = desk(&again, Some(broken), None);
    std::fs::write(d2.inbox.join("x.jpg"), b"").unwrap();
    // No reader's answer is the same as no reader: the photo is kept as it is.
    ok(&d2.core, "heat.capture.process", json!({"wait": true}));
    assert_eq!(records(&snap(&d2.core, DAY), "note").len(), 1);
}

/// The real reader, built from this crate's Swift source and run on a real
/// image and a real two-page PDF. It needs a Mac with Xcode's command-line
/// tools and takes a few seconds to build, so it is run by name:
/// `cargo test -p wi-core --test core -- --ignored the_real_reader`.
#[test]
#[ignore = "builds wi-ocr with swiftc; macOS only"]
fn the_real_reader_reads_a_page_and_a_scanned_pdf_on_this_mac() {
    let setup = Setup::new();
    let notes = setup.dir.path().join("Notes");
    let mut config = setup.config(NO_SERVER, no_browser());
    config.now = Some(ny(NOW));
    config.notes_dir = Some(notes.clone());
    config.ocr_build = true;
    let core = Core::open(&setup.library(), config).unwrap();
    ok(
        &core,
        "app.settings.set",
        json!({"patch": {"heat": {"timeZone": "America/New_York"}}}),
    );
    ok(
        &core,
        "heat.commitment.create",
        json!({"title": "JPN 101", "kind": "class", "days": ["MO", "WE", "FR"], "start": "10:00", "end": "10:50"}),
    );
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/core/fixtures");
    let page = setup.dir.path().join("2026-10-07 10.22.31 page.png");
    std::fs::copy(fixtures.join("page.png"), &page).unwrap();
    let done = ok(&core, "heat.capture.process", json!({"path": page}));
    let id = done["notes"][0]["id"].clone();
    let note = records(&snap(&core, DAY), "note")
        .into_iter()
        .find(|n| n["id"] == id)
        .unwrap();
    let markdown = note["markdown"].as_str().unwrap();
    assert!(
        markdown.starts_with("![](attachments/2026-10-07-1022-capture.jpg)\n\n"),
        "{markdown}"
    );
    assert!(
        markdown.contains("Group one verbs change their ending")
            && markdown.contains("Quiz on Friday"),
        "{markdown}"
    );
    assert_eq!(
        ok(&core, "heat.note.search", json!({"q": "verbs ending"}))["hits"][0]["id"],
        id
    );
    assert_eq!(snap(&core, DAY)["notes"]["capture"]["reader"], "ready");
    let image = std::fs::read(notes.join("attachments/2026-10-07-1022-capture.jpg")).unwrap();
    assert_eq!(
        &image[..3],
        [0xFF, 0xD8, 0xFF],
        "the page is kept as a JPEG"
    );
    // A scanned PDF: both pages drawn, both read.
    let scan = setup.dir.path().join("2026-10-08 20.00.00 scan.pdf");
    std::fs::copy(fixtures.join("scan.pdf"), &scan).unwrap();
    let done = ok(&core, "heat.capture.process", json!({"path": scan}));
    let id = done["notes"][0]["id"].clone();
    let note = records(&snap(&core, DAY), "note")
        .into_iter()
        .find(|n| n["id"] == id)
        .unwrap();
    let markdown = note["markdown"].as_str().unwrap();
    assert!(
        markdown.contains("![Page 1]")
            && markdown.contains("![Page 2]")
            && markdown.contains("[The PDF]"),
        "{markdown}"
    );
    assert_eq!(
        markdown.matches("The sum around a loop is zero").count(),
        2,
        "{markdown}"
    );
}
