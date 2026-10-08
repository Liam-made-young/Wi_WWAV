//! Notes as markdown (docs/NOTES.md). What a fail looks like:
//! - a link, a tag or a checkbox inside code is read as one, or one outside
//!   code is missed;
//! - a note's file loses a line of front matter the person wrote, or reads
//!   back different from what was written;
//! - renaming a note leaves a link to its old name, or rewrites a link to
//!   another note;
//! - a search misses a word that is in a note, or finds a note that lacks
//!   one of the words asked for.

use std::collections::BTreeSet;

use serde_json::json;
use wi_heat::notes::*;

const NOTE: &str = "# Te-form\n\nSee [[Verb groups]] and [[JPN 101|the course]]. Also [[Particles#wa and ga]].\n\n\
- [ ] Do worksheet 4 by Friday\n- [x] Read chapter 6 ^t-01JABC\n* [ ] Ask about `[[not a link]]` #grammar\n\n\
```\n[[inside a fence]] #nottag\n- [ ] not a box\n```\n\n\
Tagged #JPN101 and #verbs/te-form, not #3 or a#b. ![[page.jpg]]\n";

#[test]
fn links_are_found_outside_code_and_nowhere_else() {
    let found: Vec<(String, Option<String>, usize)> = links(NOTE)
        .into_iter()
        .map(|l| (l.target, l.shown, l.line))
        .collect();
    assert_eq!(
        found,
        [
            ("Verb groups".to_string(), None, 2),
            ("JPN 101".to_string(), Some("the course".to_string()), 2),
            ("Particles".to_string(), None, 2),
        ]
    );
    assert!(links("[[]] [[ ]] [[|x]]").is_empty());
}

#[test]
fn tags_are_words_after_a_hash_and_a_heading_is_not_one() {
    assert_eq!(tags(NOTE), ["grammar", "jpn101", "verbs/te-form"]);
    assert_eq!(tags("#日本語 notes (#kanji)"), ["日本語", "kanji"]);
    assert!(tags("# Heading\n## Another\nissue #12").is_empty());
}

#[test]
fn checkboxes_know_their_line_their_text_and_their_task() {
    let boxes = checkboxes(NOTE);
    assert_eq!(boxes.len(), 3);
    assert_eq!(
        (boxes[0].line, boxes[0].text.as_str(), boxes[0].done),
        (4, "Do worksheet 4 by Friday", false)
    );
    assert_eq!(
        (
            boxes[1].done,
            boxes[1].task_id.as_deref(),
            boxes[1].text.as_str()
        ),
        (true, Some("01JABC"), "Read chapter 6")
    );
    assert_eq!(boxes[2].task_id, None);
    // Ticking one changes that line and no other.
    let ticked = set_checkbox(NOTE, 4, true).unwrap();
    assert_eq!(
        ticked.lines().nth(4),
        Some("- [x] Do worksheet 4 by Friday")
    );
    assert_eq!(
        ticked.replace("- [x] Do worksheet", "- [ ] Do worksheet"),
        NOTE
    );
    assert_eq!(set_checkbox(&ticked, 4, false).unwrap(), NOTE);
    assert_eq!(set_checkbox(NOTE, 0, true), None, "a heading has no box");
    assert_eq!(set_checkbox(NOTE, 99, true), None);
    // Turning one into a task ties the line to it, once.
    let linked = link_task(NOTE, 4, "01JXYZ").unwrap();
    assert_eq!(
        linked.lines().nth(4),
        Some("- [ ] Do worksheet 4 by Friday ^t-01JXYZ")
    );
    assert_eq!(checkboxes(&linked)[0].task_id.as_deref(), Some("01JXYZ"));
    assert_eq!(checkboxes(&linked)[0].text, "Do worksheet 4 by Friday");
    assert_eq!(link_task(&linked, 4, "again"), None);
    // Windows line endings stay as they are.
    assert_eq!(
        set_checkbox("a\r\n- [ ] b\r\nc", 1, true).unwrap(),
        "a\r\n- [x] b\r\nc"
    );
}

#[test]
fn renaming_a_note_moves_every_link_to_it_and_no_other() {
    let moved = rename_links(NOTE, "verb groups", "Verb classes");
    assert!(moved.contains("[[Verb classes]]"));
    assert!(moved.contains("[[JPN 101|the course]]"));
    let moved = rename_links(NOTE, "Particles", "Particles は・が");
    assert!(moved.contains("[[Particles は・が#wa and ga]]"));
    let moved = rename_links(NOTE, "JPN 101", "JPN 102");
    assert!(moved.contains("[[JPN 102|the course]]"));
    assert_eq!(rename_links(NOTE, "Nothing", "Something"), NOTE);
    // A link is added once, on a line of its own.
    assert_eq!(
        add_link("Some text", "JPN 101"),
        "Some text\n\n[[JPN 101]]\n"
    );
    assert_eq!(add_link("See [[jpn 101]].", "JPN 101"), "See [[jpn 101]].");
    assert_eq!(add_link("", "JPN 101"), "[[JPN 101]]\n");
}

#[test]
fn a_notes_file_reads_back_what_was_written() {
    let meta = Meta {
        id: Some("01JNOTE".into()),
        course: Some("JPN 101".into()),
        space: Some("School: Fall".into()),
        captured: Some("2026-10-07T10:22:00-04:00".into()),
        inbox: false,
        other: vec![
            "aliases:".into(),
            "  - te form".into(),
            "cssclass: wide".into(),
        ],
    };
    let text = to_file(&meta, NOTE);
    assert!(text.starts_with("---\nid: 01JNOTE\ncourse: JPN 101\nspace: \"School: Fall\"\ncaptured: 2026-10-07T10:22:00-04:00\naliases:\n  - te form\ncssclass: wide\n---\n\n# Te-form"), "{text}");
    let (back, body) = from_file(&text);
    assert_eq!(back, meta);
    assert_eq!(body, NOTE);
    // A file with no front matter is all note, and a note with no meta is all file.
    assert_eq!(
        from_file("Just words\n"),
        (Meta::default(), "Just words\n".to_string())
    );
    assert_eq!(to_file(&Meta::default(), "Just words\n"), "Just words\n");
    // Front matter that never closes is the note's own text.
    let open = "---\nid: x\nno end here";
    assert_eq!(from_file(open), (Meta::default(), open.to_string()));
    // A note that starts with a rule of its own keeps it.
    let ruled = to_file(
        &Meta {
            id: Some("a".into()),
            ..Meta::default()
        },
        "---\nunder a rule\n",
    );
    assert_eq!(from_file(&ruled).1, "---\nunder a rule\n");
    // The inbox flag, and Windows line endings.
    let (m, body) = from_file("---\r\nid: b\r\ninbox: true\r\n---\r\n\r\nText\r\n");
    assert_eq!(
        (m.id.as_deref(), m.inbox, body.as_str()),
        (Some("b"), true, "Text\r\n")
    );
}

#[test]
fn a_title_becomes_a_file_name_a_folder_can_hold() {
    assert_eq!(file_stem("JPN 101 · Oct 7"), "JPN 101 · Oct 7");
    assert_eq!(
        file_stem("What is 1/2: a \"half\"?"),
        "What is 1-2- a -half--"
    );
    assert_eq!(file_stem("  ..  "), "Untitled");
    assert_eq!(file_stem(""), "Untitled");
    let mut taken = BTreeSet::new();
    assert_eq!(free_name("JPN 101 · Oct 7", &taken), "JPN 101 · Oct 7");
    taken.insert("jpn 101 · oct 7".to_string());
    assert_eq!(free_name("JPN 101 · Oct 7", &taken), "JPN 101 · Oct 7 2");
    taken.insert("jpn 101 · oct 7 2".to_string());
    assert_eq!(free_name("JPN 101 · Oct 7", &taken), "JPN 101 · Oct 7 3");
}

fn library() -> Vec<(String, String, String)> {
    [
        ("a", "JPN 101 · Oct 7", "![](attachments/p1.jpg)\n\nTe-form: 食べる becomes 食べて. Group one verbs change their ending."),
        ("b", "Te-form drills", "Practice sheet for the te-form, twenty verbs."),
        ("c", "Circuits", "Kirchhoff's voltage law: the sum around a loop is zero."),
    ]
    .iter()
    .map(|(id, t, m)| (id.to_string(), t.to_string(), m.to_string()))
    .collect()
}

#[test]
fn search_finds_every_word_and_puts_a_title_first() {
    let notes = library();
    let ids =
        |q: &str| -> Vec<String> { search(&notes, q, 10).into_iter().map(|h| h.id).collect() };
    assert_eq!(
        ids("te-form"),
        ["b", "a"],
        "the note named for it comes first"
    );
    assert_eq!(ids("verbs te-form"), ["b", "a"]);
    assert_eq!(ids("kirchhoff"), ["c"]);
    assert_eq!(
        ids("食べて"),
        ["a"],
        "a word from a handwritten page, in Japanese"
    );
    assert_eq!(
        ids("te-form kirchhoff"),
        Vec::<String>::new(),
        "every word, not any"
    );
    assert_eq!(ids("   "), Vec::<String>::new());
    // An image's file name isn't the note's words, and never shows in a snippet.
    assert_eq!(ids("attachments"), Vec::<String>::new());
    assert!(!search(&notes, "食べて", 10)[0].snippet.contains("p1.jpg"));
    assert_eq!(search(&notes, "te-form", 1).len(), 1);
    let hit = &search(&notes, "loop", 10)[0];
    assert!(
        hit.snippet.contains("sum around a loop is zero"),
        "{}",
        hit.snippet
    );
    assert_eq!(
        excerpt(&notes[0].2, 31),
        "Te-form: 食べる becomes 食べて. Group"
    );
    assert_eq!(excerpt("# Heading\ntext", 80), "Heading");
    assert_eq!(excerpt("- [ ] Do the thing ^t-01J\n", 80), "Do the thing");
}

#[test]
fn a_captured_page_is_its_image_and_then_its_text() {
    let one = capture_markdown(
        &[Page {
            image: "attachments/2026-10-07-1022.jpg".into(),
            text: "Te-form\n食べて".into(),
        }],
        None,
    );
    assert_eq!(
        one,
        "![](attachments/2026-10-07-1022.jpg)\n\nTe-form\n食べて\n"
    );
    let two = capture_markdown(
        &[
            Page {
                image: "attachments/scan p1.jpg".into(),
                text: "First".into(),
            },
            Page {
                image: "attachments/scan p2.jpg".into(),
                text: String::new(),
            },
        ],
        Some("attachments/scan.pdf"),
    );
    assert_eq!(two, "![Page 1](<attachments/scan p1.jpg>)\n\nFirst\n\n![Page 2](<attachments/scan p2.jpg>)\n\n[The PDF](attachments/scan.pdf)\n");
    // What Vision read badly, or in Japanese, goes to Claude.
    assert!(!poorly_read(
        "Kirchhoff's voltage law says the sum is zero",
        0.9
    ));
    assert!(poorly_read(
        "Kirchhoff's voltage law says the sum is zero",
        0.3
    ));
    assert!(poorly_read("K v l", 0.9));
    assert!(poorly_read(
        "今日は te-form を勉強しました and more words",
        0.9
    ));
}

#[test]
fn claudes_answer_about_a_page_is_checked() {
    let answer = json!({
        "text": "  Quiz Friday on te-form  ",
        "place": {"kind": "course", "name": "JPN 101"},
        "tasks": [{"title": "Study for te-form quiz", "due": "2026-10-09"}, {"title": " ", "due": null}, {"title": "Worksheet 4", "due": "Friday"}],
        "terms": ["te-form", "Te-Form", "", "group one verbs"]
    });
    let f = parse_filing(&answer);
    assert_eq!(f.text.as_deref(), Some("Quiz Friday on te-form"));
    assert_eq!(f.place, Place::Course("JPN 101".into()));
    assert_eq!(
        f.tasks,
        [
            SuggestedTask {
                title: "Study for te-form quiz".into(),
                due: Some("2026-10-09".into())
            },
            SuggestedTask {
                title: "Worksheet 4".into(),
                due: None
            },
        ]
    );
    assert_eq!(f.terms, ["te-form", "group one verbs"]);
    assert_eq!(
        parse_filing(&json!({"place": {"kind": "course", "name": null}, "tasks": [], "terms": []}))
            .place,
        Place::None
    );
    assert_eq!(parse_filing(&json!({})), Filing::default());
    // Asked to read the image, it must give the text; asked only to file, it needn't.
    assert_eq!(
        filing_schema(true)["required"],
        json!(["text", "place", "tasks", "terms"])
    );
    assert_eq!(
        filing_schema(false)["required"],
        json!(["place", "tasks", "terms"])
    );
    let prompt = filing_prompt(
        Some("Quiz Friday"),
        &[],
        &["JPN 101".into()],
        &["School".into()],
        &[],
        "2026-10-07",
        true,
    );
    assert!(
        prompt.contains("Courses: JPN 101")
            && prompt.contains("Quiz Friday")
            && prompt.contains("Today is 2026-10-07.")
    );
    let filed = filing_prompt(
        None,
        &["page-1.jpg".into()],
        &[],
        &[],
        &[],
        "2026-10-07",
        false,
    );
    assert!(filed.contains("Open page-1.jpg") && filed.contains("It is filed already"));
}
