//! A note as markdown (docs/NOTES.md), with no I/O: what a note links to,
//! its tags and checkboxes, the file it is on disk, search across notes, and
//! what Claude is asked about a captured page.
//!
//! A note is a plain `.md` file anyone can open. What Learn needs to find it
//! again (its id, its course, when a page was captured) sits in a few lines
//! of front matter; everything else in the file is the person's, and lines
//! of front matter Learn didn't write are kept as they are.
//!
//! - `[[Title]]` links a note, a course by its code, or a task by its title;
//!   `[[Title|shown]]` and `[[Title#heading]]` link the same thing.
//! - `#tag` is a tag, anywhere but in code.
//! - `- [ ] text` is a checkbox. One turned into a task keeps the task's id
//!   at the end of its line as `^t-<id>`, which is how the two stay linked.

use std::collections::BTreeSet;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;

/// The folder inside the notes folder that holds images and PDFs.
pub const ATTACHMENTS: &str = "attachments";

/// What comes before a linked task's id at the end of a checkbox line.
pub const TASK_MARK: &str = "^t-";

/// Vision's reading counts as poor below this confidence.
pub const POOR_CONFIDENCE: f64 = 0.5;

/// One `[[link]]` in a note.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WikiLink {
    /// What it points at: a note's title, a course's code, a task's title.
    pub target: String,
    /// The words shown instead, from `[[target|shown]]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shown: Option<String>,
    /// The line it is on, from 0.
    pub line: usize,
}

/// One `- [ ]` line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkbox {
    pub line: usize,
    pub text: String,
    pub done: bool,
    /// The task it was turned into.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

fn link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(!?)\[\[([^\[\]\n]+?)\]\]").expect("valid"))
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|[\s(])#([\p{L}\p{N}_][\p{L}\p{N}_/\-]*)").expect("valid"))
}

fn checkbox_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\s*(?:[-*+]|\d+[.)])\s+\[)([ xX])(\]\s+)(.*)$").expect("valid")
    })
}

fn task_mark_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s*\^t-([A-Za-z0-9_\-]+)\s*$").expect("valid"))
}

/// A line with its inline code blanked out, so nothing inside backticks is
/// read as a link or a tag. The length is kept.
fn without_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut inside = false;
    for c in line.chars() {
        if c == '`' {
            inside = !inside;
            out.push(' ');
        } else if inside {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// The lines of a note that aren't inside a fenced code block, with their
/// numbers and their inline code blanked.
fn prose_lines(markdown: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fence: Option<&str> = None;
    for (n, line) in markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        let mark = if trimmed.starts_with("```") {
            Some("```")
        } else if trimmed.starts_with("~~~") {
            Some("~~~")
        } else {
            None
        };
        match (fence, mark) {
            (None, Some(m)) => fence = Some(m),
            (Some(open), Some(m)) if open == m => fence = None,
            (None, None) => out.push((n, without_code(line))),
            _ => {}
        }
    }
    out
}

/// Every `[[link]]` in a note, in order. An embedded file (`![[page.jpg]]`)
/// is not a link.
pub fn links(markdown: &str) -> Vec<WikiLink> {
    let mut out = Vec::new();
    for (line, text) in prose_lines(markdown) {
        for caps in link_re().captures_iter(&text) {
            if &caps[1] == "!" {
                continue;
            }
            let inner = &caps[2];
            let (target, shown) = match inner.split_once('|') {
                Some((t, s)) => (t, Some(s.trim().to_string()).filter(|s| !s.is_empty())),
                None => (inner, None),
            };
            // `[[Note#Heading]]` is a link to the note.
            let target = target.split('#').next().unwrap_or("").trim();
            if target.is_empty() {
                continue;
            }
            out.push(WikiLink {
                target: target.to_string(),
                shown,
                line,
            });
        }
    }
    out
}

/// The tags of a note, lower case, each once, in the order they first appear.
/// A heading (`# Title`) isn't one, and neither is a number alone (`#3`).
pub fn tags(markdown: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (_, text) in prose_lines(markdown) {
        for caps in tag_re().captures_iter(&text) {
            let tag = caps[1].trim_end_matches(['/', '-']).to_lowercase();
            if tag.chars().any(char::is_alphabetic) && seen.insert(tag.clone()) {
                out.push(tag);
            }
        }
    }
    out
}

/// Every checkbox line of a note.
pub fn checkboxes(markdown: &str) -> Vec<Checkbox> {
    let mut out = Vec::new();
    let mut fenced = false;
    for (line, text) in markdown.lines().enumerate() {
        let trimmed = text.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let Some(caps) = checkbox_re().captures(text) else {
            continue;
        };
        let rest = &caps[4];
        let task_id = task_mark_re().captures(rest).map(|c| c[1].to_string());
        out.push(Checkbox {
            line,
            text: task_mark_re().replace(rest, "").trim().to_string(),
            done: &caps[2] != " ",
            task_id,
        });
    }
    out
}

/// One line of a note changed by `f`, the rest as it was (its line endings
/// too). None when the note has no such line or `f` leaves it alone.
fn with_line(markdown: &str, line: usize, f: impl Fn(&str) -> Option<String>) -> Option<String> {
    let mut out = String::with_capacity(markdown.len() + 16);
    let mut changed = false;
    for (n, text) in markdown.split_inclusive('\n').enumerate() {
        if n == line {
            let body = text.trim_end_matches(['\n', '\r']);
            let ending = &text[body.len()..];
            out.push_str(&f(body)?);
            out.push_str(ending);
            changed = true;
            continue;
        }
        out.push_str(text);
    }
    changed.then_some(out)
}

/// The note with one checkbox ticked or cleared.
pub fn set_checkbox(markdown: &str, line: usize, done: bool) -> Option<String> {
    with_line(markdown, line, |text| {
        let caps = checkbox_re().captures(text)?;
        Some(format!(
            "{}{}{}{}",
            &caps[1],
            if done { "x" } else { " " },
            &caps[3],
            &caps[4]
        ))
    })
}

/// The note with a checkbox line tied to the task made from it.
pub fn link_task(markdown: &str, line: usize, task_id: &str) -> Option<String> {
    with_line(markdown, line, |text| {
        let caps = checkbox_re().captures(text)?;
        if task_mark_re().is_match(&caps[4]) {
            return None;
        }
        Some(format!("{} {TASK_MARK}{task_id}", text.trim_end()))
    })
}

/// The note with every link to `old` pointing at `new`, whatever case it was
/// written in. What a link shows, and the heading it names, are kept.
pub fn rename_links(markdown: &str, old: &str, new: &str) -> String {
    let old = old.trim().to_lowercase();
    link_re()
        .replace_all(markdown, |caps: &regex::Captures<'_>| {
            let inner = &caps[2];
            let (target, rest) = match inner.find(['|', '#']) {
                Some(at) => (&inner[..at], &inner[at..]),
                None => (inner, ""),
            };
            if target.trim().to_lowercase() == old {
                format!("{}[[{new}{rest}]]", &caps[1])
            } else {
                caps[0].to_string()
            }
        })
        .into_owned()
}

/// A note's markdown with a link added as a line of its own, unless it links
/// there already.
pub fn add_link(markdown: &str, target: &str) -> String {
    let want = target.trim().to_lowercase();
    if links(markdown)
        .iter()
        .any(|l| l.target.to_lowercase() == want)
    {
        return markdown.to_string();
    }
    let mut out = markdown.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&format!("[[{}]]\n", target.trim()));
    out
}

/// The first words of a note, for a list: no markup, no image, one line.
pub fn excerpt(markdown: &str, max: usize) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let image =
        RE.get_or_init(|| Regex::new(r"!\[[^\]]*\]\([^)]*\)|!\[\[[^\]]*\]\]").expect("valid"));
    for line in markdown.lines() {
        let line = image.replace_all(line, "");
        let line = task_mark_re().replace(&line, "");
        let text = line
            .trim()
            .trim_start_matches(['#', '>', '-', '*', '+'])
            .trim()
            .replace("[[", "")
            .replace("]]", "")
            .replace("**", "")
            .replace('`', "");
        let text = text
            .trim_start_matches("[ ] ")
            .trim_start_matches("[x] ")
            .trim();
        if !text.is_empty() {
            return text.chars().take(max).collect();
        }
    }
    String::new()
}

/// A title as a file's name: nothing a folder can't hold, and never empty.
pub fn file_stem(title: &str) -> String {
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
    let clean = clean.trim().trim_matches('.').trim();
    if clean.is_empty() {
        "Untitled".to_string()
    } else {
        clean.chars().take(120).collect()
    }
}

/// A name no note has yet: "Title", then "Title 2", "Title 3". `taken`
/// holds the names in use, lower case.
pub fn free_name(base: &str, taken: &BTreeSet<String>) -> String {
    let mut name = base.to_string();
    let mut n = 1;
    while taken.contains(&name.to_lowercase()) {
        n += 1;
        name = format!("{base} {n}");
    }
    name
}

/// What Learn keeps at the top of a note's file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meta {
    pub id: Option<String>,
    /// The course's code.
    pub course: Option<String>,
    /// The space's name.
    pub space: Option<String>,
    /// When a page was captured, ISO 8601 with its offset.
    pub captured: Option<String>,
    /// True while a captured note waits in the Notes inbox to be filed.
    pub inbox: bool,
    /// Lines of front matter Learn didn't write, kept word for word.
    pub other: Vec<String>,
}

const OURS: [&str; 5] = ["id", "course", "space", "captured", "inbox"];

fn yaml_value(v: &str) -> String {
    // A value YAML would read as something else, or cut short, is quoted.
    let plain = !v.is_empty()
        && !v.starts_with([
            ' ', '"', '\'', '[', '{', '#', '&', '*', '!', '|', '>', '%', '@', '`', '-',
        ])
        && !v.ends_with(' ')
        && !v.contains(": ")
        && !v.contains(" #")
        && !v.contains('\n')
        && !matches!(v, "true" | "false" | "null" | "yes" | "no" | "~");
    if plain {
        v.to_string()
    } else {
        Value::String(v.to_string()).to_string()
    }
}

fn yaml_read(v: &str) -> String {
    let v = v.trim();
    if v.starts_with('"') {
        if let Ok(Value::String(s)) = serde_json::from_str::<Value>(v) {
            return s;
        }
    }
    if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        return v[1..v.len() - 1].replace("''", "'");
    }
    v.to_string()
}

/// A note as the text of its file: front matter, then the markdown.
pub fn to_file(meta: &Meta, markdown: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut put = |key: &str, value: &Option<String>| {
        if let Some(v) = value.as_deref().filter(|v| !v.is_empty()) {
            lines.push(format!("{key}: {}", yaml_value(v)));
        }
    };
    put("id", &meta.id);
    put("course", &meta.course);
    put("space", &meta.space);
    put("captured", &meta.captured);
    if meta.inbox {
        lines.push("inbox: true".to_string());
    }
    lines.extend(meta.other.iter().cloned());
    if lines.is_empty() {
        return markdown.to_string();
    }
    format!("---\n{}\n---\n\n{}", lines.join("\n"), markdown)
}

/// The text of a file as a note: what its front matter says, and the
/// markdown below it. A file with no front matter is all markdown.
pub fn from_file(text: &str) -> (Meta, String) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut meta = Meta::default();
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return (meta, text.to_string());
    };
    // The closing line is `---` (or `...`) alone on a line.
    let mut at = 0;
    let mut end: Option<(usize, usize)> = None;
    for line in rest.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if bare == "---" || bare == "..." {
            end = Some((at, at + line.len()));
            break;
        }
        at += line.len();
    }
    let Some((front_end, body_start)) = end else {
        return (meta, text.to_string());
    };
    for line in rest[..front_end].lines() {
        let own = line
            .split_once(':')
            .filter(|(k, _)| !line.starts_with([' ', '\t', '-']) && OURS.contains(&k.trim()));
        match own {
            Some((key, value)) => {
                let value = yaml_read(value);
                match key.trim() {
                    "id" => meta.id = Some(value).filter(|v| !v.is_empty()),
                    "course" => meta.course = Some(value).filter(|v| !v.is_empty()),
                    "space" => meta.space = Some(value).filter(|v| !v.is_empty()),
                    "captured" => meta.captured = Some(value).filter(|v| !v.is_empty()),
                    _ => meta.inbox = value == "true",
                }
            }
            None => meta.other.push(line.to_string()),
        }
    }
    let body = &rest[body_start..];
    // The one blank line `to_file` puts under the front matter isn't the note's.
    let body = body
        .strip_prefix("\r\n")
        .or_else(|| body.strip_prefix('\n'))
        .unwrap_or(body);
    (meta, body.to_string())
}

// ----- search -----

/// One note that holds every word asked for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub id: String,
    pub title: String,
    /// The words around the first match, on one line.
    pub snippet: String,
    pub score: f64,
}

fn snippet(markdown: &str, word: &str) -> String {
    let lower = markdown.to_lowercase();
    let Some(at) = lower.find(word) else {
        return excerpt(markdown, 140);
    };
    // Back to a character boundary of the original: lower-casing can change lengths.
    let chars: Vec<(usize, char)> = markdown.char_indices().collect();
    let hit = lower[..at]
        .chars()
        .count()
        .min(chars.len().saturating_sub(1));
    let from = hit.saturating_sub(50);
    let to = (hit + 90).min(chars.len());
    let mut text: String = chars[from..to].iter().map(|(_, c)| *c).collect();
    text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if from > 0 {
        text.insert(0, '…');
    }
    if to < chars.len() {
        text.push('…');
    }
    text
}

/// Notes that hold every word of `query` in their title or text, best first:
/// a word in the title counts for more than one in the text. `notes` are
/// `(id, title, markdown)`.
pub fn search(notes: &[(String, String, String)], query: &str, limit: usize) -> Vec<Hit> {
    let words: Vec<String> = query
        .split_whitespace()
        .map(|w| w.trim_start_matches('#').to_lowercase())
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Hit> = Vec::new();
    for (id, title, markdown) in notes {
        let (t, body) = (title.to_lowercase(), markdown.to_lowercase());
        if !words.iter().all(|w| t.contains(w) || body.contains(w)) {
            continue;
        }
        let mut score = 0.0;
        for w in &words {
            if t.contains(w) {
                score += 10.0;
            }
            score += (body.matches(w.as_str()).count() as f64).min(8.0);
        }
        let first = words.iter().find(|w| body.contains(w.as_str()));
        out.push(Hit {
            id: id.clone(),
            title: title.clone(),
            snippet: first.map_or_else(|| excerpt(markdown, 140), |w| snippet(markdown, w)),
            score,
        });
    }
    out.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    out.truncate(limit);
    out
}

// ----- a captured page -----

/// One page of a capture: where its image is kept, relative to the notes
/// folder, and the text read from it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub image: String,
    pub text: String,
}

/// A path as a markdown link target: in angle brackets when it holds a
/// space, as CommonMark asks.
fn target(path: &str) -> String {
    if path.contains([' ', '(', ')']) {
        format!("<{path}>")
    } else {
        path.to_string()
    }
}

/// A captured note's markdown: each page's image, with its text below it.
/// The image is the page; the text is what makes it searchable.
pub fn capture_markdown(pages: &[Page], original: Option<&str>) -> String {
    let mut out = String::new();
    let many = pages.len() > 1;
    for (n, page) in pages.iter().enumerate() {
        if !out.is_empty() {
            out.push('\n');
        }
        let alt = if many {
            format!("Page {}", n + 1)
        } else {
            String::new()
        };
        out.push_str(&format!("![{alt}]({})\n", target(&page.image)));
        let text = page.text.trim();
        if !text.is_empty() {
            out.push('\n');
            out.push_str(text);
            out.push('\n');
        }
    }
    if let Some(file) = original {
        out.push_str(&format!("\n[The PDF]({})\n", target(file)));
    }
    out
}

/// Whether text holds kana or kanji.
pub fn has_japanese(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c as u32, 0x3040..=0x30FF | 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xFF66..=0xFF9F)
    })
}

/// Whether what Vision read is poor enough to ask Claude to read the page
/// instead: little confidence, next to nothing found, or Japanese.
pub fn poorly_read(text: &str, confidence: f64) -> bool {
    confidence < POOR_CONFIDENCE || text.split_whitespace().count() < 4 || has_japanese(text)
}

/// Where Claude thinks a captured page belongs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "name", rename_all = "lowercase")]
pub enum Place {
    Course(String),
    Space(String),
    /// An existing note to link it to.
    Note(String),
    #[default]
    None,
}

/// A task a page seems to ask for. Only ever suggested.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestedTask {
    pub title: String,
    /// `YYYY-MM-DD`, when the page gives a day.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
}

/// Claude's one answer about a captured page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filing {
    /// The page's text, when Claude was asked to read the image.
    pub text: Option<String>,
    pub place: Place,
    pub tasks: Vec<SuggestedTask>,
    pub terms: Vec<String>,
}

pub fn filing_schema(reads_image: bool) -> Value {
    let mut properties = json!({
        "place": {
            "type": "object",
            "properties": {
                "kind": {"type": "string", "enum": ["course", "space", "note", "none"]},
                "name": {"type": ["string", "null"], "description": "The course's code, the space's name or the note's title, exactly as listed. Null for none."}
            },
            "required": ["kind", "name"],
            "additionalProperties": false
        },
        "tasks": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "due": {"type": ["string", "null"], "description": "YYYY-MM-DD, only if the page gives a day."}
                },
                "required": ["title", "due"],
                "additionalProperties": false
            }
        },
        "terms": {"type": "array", "items": {"type": "string"}}
    });
    let mut required = vec!["place", "tasks", "terms"];
    if reads_image {
        properties["text"] = json!({"type": "string", "description": "Everything written on the page or pages, in reading order, as plain text."});
        required.insert(0, "text");
    }
    json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
}

/// What Claude is told about a captured page. `text` is what Vision read;
/// `images` are the files to open when Claude is to read the page itself.
pub fn filing_prompt(
    text: Option<&str>,
    images: &[String],
    courses: &[String],
    spaces: &[String],
    notes: &[String],
    today: &str,
    needs_place: bool,
) -> String {
    let mut p = String::new();
    if images.is_empty() {
        p.push_str("Below, between the lines of dashes, is the text of a notebook page a student photographed. It was read by a machine and may have mistakes.");
    } else {
        p.push_str(&format!(
            "A student photographed a notebook page. Open {} in this folder and read {}: give all of the handwriting or print as plain text in `text`, in reading order, in the language it is written in (Japanese stays Japanese). Write nothing that isn't on the page.",
            images.join(", "),
            if images.len() == 1 { "it" } else { "them in order" }
        ));
    }
    p.push_str(&format!("\n\nToday is {today}."));
    if needs_place {
        p.push_str("\n\nSay where the page belongs in `place`: one of the student's courses, else one of their spaces, else an existing note it plainly continues, else none. Pick only from the lists, and only when the page is clearly about it; otherwise none.");
        let list = |name: &str, items: &[String]| {
            if items.is_empty() {
                String::new()
            } else {
                format!("\n{name}: {}", items.join("; "))
            }
        };
        p.push_str(&list("Courses", courses));
        p.push_str(&list("Spaces", spaces));
        p.push_str(&list("Notes", notes));
    } else {
        p.push_str("\n\nIt is filed already: answer `place` with kind \"none\".");
    }
    p.push_str("\n\nIn `tasks`, list anything on the page that sounds like something due: homework, a quiz, a reading, a deadline. Give a title as the student would write a to-do, and `due` only if the page gives a day. An empty list if there is nothing.\nIn `terms`, list up to twelve key terms from the page that would make good flashcards: the term itself, as written. An empty list if there are none.");
    if let Some(text) = text {
        p.push_str(&format!("\n\n----------\n{text}\n----------"));
    }
    p
}

/// Claude's answer, checked: a place that isn't on the lists is none.
pub fn parse_filing(answer: &Value) -> Filing {
    let name = answer["place"]["name"]
        .as_str()
        .map(str::trim)
        .unwrap_or("");
    let place = match (answer["place"]["kind"].as_str(), name.is_empty()) {
        (Some("course"), false) => Place::Course(name.to_string()),
        (Some("space"), false) => Place::Space(name.to_string()),
        (Some("note"), false) => Place::Note(name.to_string()),
        _ => Place::None,
    };
    let tasks = answer["tasks"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|t| {
            let title = t["title"].as_str()?.trim();
            (!title.is_empty()).then(|| SuggestedTask {
                title: title.chars().take(200).collect(),
                due: t["due"]
                    .as_str()
                    .filter(|d| crate::commitments::is_day(d))
                    .map(String::from),
            })
        })
        .take(12)
        .collect();
    let mut seen = BTreeSet::new();
    let terms = answer["terms"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty() && seen.insert(t.to_lowercase()))
        .map(|t| t.chars().take(80).collect())
        .take(12)
        .collect();
    Filing {
        text: answer["text"]
            .as_str()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(String::from),
        place,
        tasks,
        terms,
    }
}
