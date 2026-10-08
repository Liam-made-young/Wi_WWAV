mod assist;
mod parse;
mod pdf;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

use super::storage::{self, Actor, Asset, Document, Library, Result, Tool, Version, MAX_TEXT};
use crate::{Core, CoreError};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Prose,
    Lyrics,
    Screenplay,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Research {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub note_id: Option<String>,
    pub excerpt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Section {
    pub id: String,
    pub parent: Option<String>,
    pub title: String,
    pub kind: String,
    pub synopsis: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manuscript {
    pub format: String,
    pub mode: Mode,
    pub sections: Vec<Section>,
    pub notes: String,
    pub research: Vec<Research>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionRecord {
    pub id: String,
    pub parent: Option<String>,
    pub title: String,
    pub kind: String,
    pub synopsis: String,
    pub asset: Asset,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub format: String,
    pub mode: Mode,
    pub sections: Vec<SectionRecord>,
    pub notes: Asset,
    pub research: Vec<Research>,
}

pub fn seed(id: &str, name: &str, bytes: &[u8]) -> Result<Manuscript> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| storage::refused("Write needs UTF-8 text."))?;
    Ok(Manuscript {
        format: "wi-write/1".into(),
        mode: if name.to_ascii_lowercase().ends_with(".fountain") {
            Mode::Screenplay
        } else {
            Mode::Prose
        },
        sections: vec![Section {
            id: id.into(),
            parent: None,
            title: "Draft".into(),
            kind: "section".into(),
            synopsis: String::new(),
            text: text.into(),
        }],
        notes: String::new(),
        research: Vec::new(),
    })
}

pub fn load(l: &Library, doc: &Document, v: &Version) -> Result<Manuscript> {
    if doc.tool != Tool::Write {
        return Err(storage::refused(
            "This document belongs to another Console tool.",
        ));
    }
    let text = |asset: &Asset| -> Result<String> {
        String::from_utf8(l.bytes(&doc.id, asset)?)
            .map_err(|_| storage::refused("A Write section is not UTF-8."))
    };
    let manuscript = if let Some(record) = &v.write {
        Manuscript {
            format: record.format.clone(),
            mode: record.mode,
            sections: record
                .sections
                .iter()
                .map(|s| {
                    Ok(Section {
                        id: s.id.clone(),
                        parent: s.parent.clone(),
                        title: s.title.clone(),
                        kind: s.kind.clone(),
                        synopsis: s.synopsis.clone(),
                        text: text(&s.asset)?,
                    })
                })
                .collect::<Result<_>>()?,
            notes: text(&record.notes)?,
            research: record.research.clone(),
        }
    } else {
        if !v.asset.mime.starts_with("text/") {
            return Err(storage::refused(
                "Import Markdown, plain text or Fountain to edit it in Write.",
            ));
        }
        seed(&doc.id, &v.asset.name, &l.bytes(&doc.id, &v.asset)?)?
    };
    validate(&manuscript)?;
    Ok(manuscript)
}

pub fn validate(m: &Manuscript) -> Result<()> {
    if m.format != "wi-write/1" {
        return Err(storage::refused("Unsupported Write manuscript format."));
    }
    if m.sections.is_empty() || m.sections.len() > 256 {
        return Err(storage::refused("A manuscript needs 1 to 256 sections."));
    }
    let mut ids = BTreeSet::new();
    let mut ancestors: Vec<&str> = Vec::new();
    for s in &m.sections {
        storage::check_id(&s.id)?;
        if !ids.insert(&s.id) {
            return Err(storage::refused("A section ID occurs more than once."));
        }
        storage::title_checked(&s.title)?;
        if s.synopsis.len() > 4000 {
            return Err(storage::refused(
                "An outline summary is limited to 4,000 bytes.",
            ));
        }
        if ![
            "section", "verse", "hook", "bridge", "intro", "outro", "scene",
        ]
        .contains(&s.kind.as_str())
        {
            return Err(storage::refused("Unknown section type."));
        }
        if let Some(parent) = &s.parent {
            while ancestors.last().is_some_and(|id| *id != parent) {
                ancestors.pop();
            }
            if ancestors.is_empty() {
                return Err(storage::refused("Parents must precede their children, and a section's children must stay together."));
            }
        } else {
            ancestors.clear();
        }
        if ancestors.len() >= 8 {
            return Err(storage::refused(
                "Sections can be nested up to eight levels.",
            ));
        }
        ancestors.push(&s.id);
    }
    if m.research.len() > 100 {
        return Err(storage::refused(
            "A document can link up to 100 research sources.",
        ));
    }
    let mut sources = BTreeSet::new();
    for r in &m.research {
        storage::check_id(&r.id)?;
        if !sources.insert(&r.id) {
            return Err(storage::refused("A research ID occurs more than once."));
        }
        storage::title_checked(&r.title)?;
        if let Some(id) = &r.note_id {
            storage::check_id(id)?;
        }
        if let Some(url) = &r.url {
            if !(url.starts_with("https://") || url.starts_with("http://"))
                || url.len() > 2000
                || url.chars().any(char::is_whitespace)
            {
                return Err(storage::refused(
                    "A source URL must be an HTTP or HTTPS address.",
                ));
            }
        }
        if r.excerpt.len() > 16000 {
            return Err(storage::refused(
                "A research excerpt is limited to 16,000 bytes.",
            ));
        }
    }
    if serde_json::to_vec(m)
        .map_err(|e| storage::refused(e.to_string()))?
        .len()
        > MAX_TEXT
    {
        return Err(storage::refused(
            "Write manuscripts are limited to 1 MiB, including notes and research.",
        ));
    }
    Ok(())
}

pub fn store(l: &Library, id: &str, m: &Manuscript) -> Result<(Record, Asset)> {
    validate(m)?;
    let sections = m
        .sections
        .iter()
        .map(|s| {
            Ok(SectionRecord {
                id: s.id.clone(),
                parent: s.parent.clone(),
                title: s.title.clone(),
                kind: s.kind.clone(),
                synopsis: s.synopsis.clone(),
                asset: l.asset(id, &format!("{}.md", s.id), s.text.as_bytes())?,
            })
        })
        .collect::<Result<_>>()?;
    let notes = l.asset(id, "notes.md", m.notes.as_bytes())?;
    let asset = l.asset(
        id,
        if m.mode == Mode::Screenplay {
            "script.fountain"
        } else {
            "manuscript.md"
        },
        joined(m, false).as_bytes(),
    )?;
    Ok((
        Record {
            format: m.format.clone(),
            mode: m.mode,
            sections,
            notes,
            research: m.research.clone(),
        },
        asset,
    ))
}

pub fn joined(m: &Manuscript, headings: bool) -> String {
    let mut text = String::new();
    let mut depth = std::collections::BTreeMap::new();
    for s in &m.sections {
        let level = s
            .parent
            .as_ref()
            .and_then(|p| depth.get(p))
            .copied()
            .unwrap_or(0)
            + 1;
        depth.insert(s.id.clone(), level);
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        if headings && m.mode != Mode::Screenplay {
            text.push_str(&format!("{} {}\n\n", "#".repeat(level.min(6)), s.title));
        }
        text.push_str(&s.text);
    }
    text
}

pub fn changes(old: &Manuscript, new: &Manuscript) -> Vec<String> {
    let mut changes = Vec::new();
    if old.mode != new.mode {
        changes.push(format!("Mode: {:?} -> {:?}", old.mode, new.mode));
    }
    if old
        .sections
        .iter()
        .map(|s| (&s.id, &s.parent))
        .collect::<Vec<_>>()
        != new
            .sections
            .iter()
            .map(|s| (&s.id, &s.parent))
            .collect::<Vec<_>>()
    {
        changes.push("Binder order or nesting changed".into());
    }
    for s in &new.sections {
        if let Some(before) = old.sections.iter().find(|o| o.id == s.id) {
            if before.title != s.title {
                changes.push(format!("Section title: {} -> {}", before.title, s.title));
            }
            if before.text != s.text {
                changes.push(format!(
                    "{}: text edited ({} -> {} words)",
                    s.title,
                    parse::words(&before.text),
                    parse::words(&s.text)
                ));
            }
            if before.kind != s.kind || before.synopsis != s.synopsis {
                changes.push(format!("{}: outline or type edited", s.title));
            }
        } else {
            changes.push(format!("Section added: {}", s.title));
        }
    }
    for s in &old.sections {
        if !new.sections.iter().any(|n| n.id == s.id) {
            changes.push(format!("Section removed: {}", s.title));
        }
    }
    if old.notes != new.notes {
        changes.push("Document notes edited".into());
    }
    if old.research != new.research {
        changes.push("Research links edited".into());
    }
    changes
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum Edit {
    Add {
        title: String,
        parent: Option<String>,
        kind: Option<String>,
    },
    Update {
        section: String,
        title: Option<String>,
        text: Option<String>,
        kind: Option<String>,
        synopsis: Option<String>,
    },
    Move {
        section: String,
        parent: Option<String>,
        before: Option<String>,
    },
    Delete {
        section: String,
    },
    Reorder {
        order: Vec<String>,
    },
    Mode {
        mode: Mode,
    },
    Notes {
        text: String,
    },
    ResearchAdd {
        title: String,
        url: Option<String>,
        excerpt: Option<String>,
    },
    ResearchRemove {
        source: String,
    },
    LinkNote {
        note: String,
    },
    Replace {
        section: String,
        from: usize,
        to: usize,
        text: String,
    },
    Format {
        section: String,
        from: usize,
        to: usize,
        style: String,
    },
}

fn index(m: &Manuscript, id: &str) -> Result<usize> {
    m.sections
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| storage::refused("That section is no longer in the manuscript."))
}
fn subtree_end(m: &Manuscript, at: usize) -> usize {
    let mut descendants = BTreeSet::from([m.sections[at].id.as_str()]);
    let mut end = at + 1;
    while end < m.sections.len()
        && m.sections[end]
            .parent
            .as_deref()
            .is_some_and(|p| descendants.contains(p))
    {
        descendants.insert(&m.sections[end].id);
        end += 1;
    }
    end
}

/// Editor and Claude offsets are UTF-16, matching DOM and CodeMirror selections.
pub fn range(text: &str, from: usize, to: usize) -> Result<std::ops::Range<usize>> {
    let byte = |offset| {
        let mut n = 0;
        for (at, c) in text.char_indices() {
            if n == offset {
                return Ok(at);
            }
            n += c.len_utf16();
        }
        if n == offset {
            Ok(text.len())
        } else {
            Err(storage::refused(
                "The selection is outside the section or splits a Unicode character.",
            ))
        }
    };
    if from > to {
        return Err(storage::refused("The selection starts after it ends."));
    }
    Ok(byte(from)?..byte(to)?)
}

pub fn transform(text: &str, from: usize, to: usize, style: &str) -> Result<String> {
    let selected = range(text, from, to)?;
    let (span, replacement) = match style {
        "bold" | "italic" | "code" => {
            let marker = match style {
                "bold" => "**",
                "italic" => "*",
                _ => "`",
            };
            (
                selected.clone(),
                format!("{marker}{}{marker}", &text[selected]),
            )
        }
        "heading" | "quote" | "bullet" | "scene" | "action" | "character" | "dialogue"
        | "parenthetical" | "transition" => {
            let start = text[..selected.start].rfind('\n').map_or(0, |n| n + 1);
            let end = text[selected.end..]
                .find('\n')
                .map_or(text.len(), |n| selected.end + n);
            let line = text[start..end].trim();
            let clean = line
                .trim_start_matches(['#', '>', '!', '@', '.'])
                .trim()
                .trim_start_matches("- ");
            let value = match style {
                "heading" => format!("# {clean}"),
                "quote" => format!("> {clean}"),
                "bullet" => format!("- {clean}"),
                "scene" => format!(".{}", clean.to_uppercase()),
                "action" => format!("!{clean}"),
                "character" => format!("@{}", clean.to_uppercase()),
                "dialogue" => clean.into(),
                "parenthetical" => format!("({})", clean.trim_matches(['(', ')'])),
                "transition" => format!("> {}", clean.to_uppercase()),
                _ => unreachable!(),
            };
            (start..end, value)
        }
        _ => return Err(storage::refused("Unknown writing format.")),
    };
    let mut result = text.to_string();
    result.replace_range(span, &replacement);
    Ok(result)
}

pub fn edit(core: &Core, m: &mut Manuscript, action: Edit) -> Result<Option<String>> {
    let mut selected = None;
    match action {
        Edit::Add {
            title,
            parent,
            kind,
        } => {
            let at = if let Some(p) = &parent {
                subtree_end(m, index(m, p)?)
            } else {
                m.sections.len()
            };
            let id = wwav_ids::ulid();
            selected = Some(id.clone());
            m.sections.insert(
                at,
                Section {
                    id,
                    parent,
                    title: storage::title_checked(&title)?,
                    kind: kind.unwrap_or_else(|| "section".into()),
                    synopsis: String::new(),
                    text: String::new(),
                },
            );
        }
        Edit::Update {
            section,
            title,
            text,
            kind,
            synopsis,
        } => {
            let at = index(m, &section)?;
            let s = &mut m.sections[at];
            if let Some(v) = title {
                s.title = storage::title_checked(&v)?;
            }
            if let Some(v) = text {
                s.text = v;
            }
            if let Some(v) = kind {
                s.kind = v;
            }
            if let Some(v) = synopsis {
                s.synopsis = v;
            }
        }
        Edit::Move {
            section,
            parent,
            before,
        } => {
            let at = index(m, &section)?;
            let end = subtree_end(m, at);
            if parent
                .as_ref()
                .is_some_and(|p| m.sections[at..end].iter().any(|s| &s.id == p))
            {
                return Err(storage::refused(
                    "A section cannot be nested inside itself.",
                ));
            }
            let mut moved: Vec<_> = m.sections.drain(at..end).collect();
            moved[0].parent = parent.clone();
            let dest = if let Some(before) = before {
                let i = index(m, &before)?;
                if m.sections[i].parent != parent {
                    return Err(storage::refused(
                        "Place a section before another section at the same level.",
                    ));
                }
                i
            } else if let Some(p) = parent {
                subtree_end(m, index(m, &p)?)
            } else {
                m.sections.len()
            };
            m.sections.splice(dest..dest, moved);
        }
        Edit::Delete { section } => {
            let at = index(m, &section)?;
            let end = subtree_end(m, at);
            if end - at == m.sections.len() {
                return Err(storage::refused(
                    "Keep at least one section in the manuscript.",
                ));
            }
            m.sections.drain(at..end);
        }
        Edit::Reorder { order } => {
            if order.len() != m.sections.len()
                || order.iter().collect::<BTreeSet<_>>().len() != order.len()
            {
                return Err(storage::refused(
                    "A reorder must include every section exactly once.",
                ));
            }
            m.sections = order
                .iter()
                .map(|id| Ok(m.sections[index(m, id)?].clone()))
                .collect::<Result<_>>()?;
        }
        Edit::Mode { mode } => m.mode = mode,
        Edit::Notes { text } => m.notes = text,
        Edit::ResearchAdd {
            title,
            url,
            excerpt,
        } => m.research.push(Research {
            id: wwav_ids::ulid(),
            title,
            url,
            note_id: None,
            excerpt: excerpt.unwrap_or_default(),
        }),
        Edit::ResearchRemove { source } => {
            let before = m.research.len();
            m.research.retain(|r| r.id != source);
            if m.research.len() == before {
                return Err(storage::refused("That research source is missing."));
            }
        }
        Edit::LinkNote { note } => {
            let n = wi_heat_store::notes::find(&core.inner.store(), &note)
                .map_err(|e| storage::refused(e.to_string()))?;
            let id = n["id"]
                .as_str()
                .ok_or_else(|| storage::refused("This Learn note has no ID."))?;
            if !m.research.iter().any(|r| r.note_id.as_deref() == Some(id)) {
                m.research.push(Research {
                    id: wwav_ids::ulid(),
                    title: n["title"].as_str().unwrap_or("Learn note").into(),
                    url: None,
                    note_id: Some(id.into()),
                    excerpt: n["markdown"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(3000)
                        .collect(),
                });
            }
        }
        Edit::Replace {
            section,
            from,
            to,
            text,
        } => {
            let at = index(m, &section)?;
            let s = &mut m.sections[at];
            let span = range(&s.text, from, to)?;
            s.text.replace_range(span, &text);
        }
        Edit::Format {
            section,
            from,
            to,
            style,
        } => {
            let at = index(m, &section)?;
            let s = &mut m.sections[at];
            s.text = transform(&s.text, from, to, &style)?;
        }
    }
    validate(m)?;
    Ok(selected)
}

pub fn invoke(core: &Core, l: &Library, cmd: &str, a: &Value, actor: Actor) -> Result<Value> {
    use super::{opt_string, string};
    if cmd == "console.write.render" {
        let text = string(a, "text")?;
        if text.len() > MAX_TEXT {
            return Err(storage::refused("This preview is too large."));
        }
        let mode: Mode = serde_json::from_value(a["mode"].clone())
            .map_err(|_| storage::refused("Choose a writing mode."))?;
        return parse::render(text, mode);
    }
    if cmd == "console.write.transform" {
        let text = string(a, "text")?;
        if text.len() > MAX_TEXT {
            return Err(storage::refused("This text is too large."));
        }
        return Ok(
            json!({"text":transform(text,usize_arg(a,"from")?,usize_arg(a,"to")?,string(a,"style")?)?}),
        );
    }
    if cmd == "console.write.research.search" {
        return core.invoke(
            "heat.note.search",
            json!({"q":opt_string(a,"query").unwrap_or(""),"limit":12}),
        );
    }
    if cmd == "console.write.research.read" {
        return wi_heat_store::notes::find(&core.inner.store(), string(a, "note")?)
            .map_err(|e| storage::refused(e.to_string()));
    }
    let d = l.load(string(a, "id")?)?;
    let v = if let Some(version) = opt_string(a, "version") {
        d.versions
            .iter()
            .find(|v| v.id == version)
            .ok_or_else(|| storage::refused("That manuscript version is missing."))?
    } else {
        d.current()?
    };
    let mut m = load(l, &d, v)?;
    match cmd {
        "console.write.read" => {
            let w = super::workspace(l)?;
            let view = w
                .tools
                .get("write")
                .map(|t| t.views.get(&d.id).cloned().unwrap_or_else(|| json!({})))
                .unwrap_or_else(|| json!({}));
            Ok(
                json!({"document":d.summary()?,"base":v.id,"manuscript":m,"stats":parse::manuscript_stats(&m),"view":view}),
            )
        }
        "console.write.save" => {
            m = serde_json::from_value(a["manuscript"].clone())
                .map_err(|e| storage::refused(format!("Invalid manuscript: {e}")))?;
            super::reply(&l.save_write(
                &d.id,
                string(a, "base")?,
                opt_string(a, "title"),
                &m,
                actor,
                "save manuscript",
            )?)
        }
        "console.write.edit" => {
            d.check_base(string(a, "base")?)?;
            let action: Edit = serde_json::from_value(a["action"].clone())
                .map_err(|e| storage::refused(format!("Invalid Write action: {e}")))?;
            let selected = edit(core, &mut m, action)?;
            let saved = l.save_write(
                &d.id,
                string(a, "base")?,
                None,
                &m,
                actor,
                "edit manuscript",
            )?;
            Ok(json!({"document":saved.summary()?,"sectionId":selected}))
        }
        "console.write.view" => {
            let view = a["view"]
                .as_object()
                .ok_or_else(|| storage::refused("View settings need an object."))?;
            for (key, value) in view {
                let good = match key.as_str() {
                    "section" => {
                        value.is_null()
                            || value
                                .as_str()
                                .is_some_and(|id| m.sections.iter().any(|s| s.id == id))
                    }
                    "focus" | "typewriter" | "preview" | "binder" | "research" | "outline" => {
                        value.is_boolean()
                    }
                    _ => false,
                };
                if !good {
                    return Err(storage::refused("Unknown or invalid writing view setting."));
                }
            }
            let mut w = super::workspace(l)?;
            let t = w.tools.entry("write".into()).or_default();
            let settings = t.views.entry(d.id).or_insert_with(|| json!({}));
            for (k, v) in view {
                settings[k] = v.clone();
            }
            super::write_workspace(l, &w)?;
            Ok(json!({"workspace":w}))
        }
        "console.write.export" => export(l, &d, v, &m, string(a, "format")?),
        _ => Err(CoreError::new(
            "unknown_command",
            format!("There is no Write command called '{cmd}'."),
        )),
    }
}

pub fn usize_arg(a: &Value, k: &str) -> Result<usize> {
    a[k].as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| storage::refused(format!("{k} must be a nonnegative whole number.")))
}

fn export(l: &Library, d: &Document, v: &Version, m: &Manuscript, format: &str) -> Result<Value> {
    let (extension, mime, bytes) = match format {
        "markdown" => ("md", "text/markdown", joined(m, true).into_bytes()),
        "text" => (
            "txt",
            "text/plain",
            parse::plain(&joined(m, true), m.mode)?.into_bytes(),
        ),
        "fountain" => {
            if m.mode != Mode::Screenplay {
                return Err(storage::refused(
                    "Choose Screenplay mode before exporting Fountain.",
                ));
            }
            ("fountain", "text/plain", joined(m, false).into_bytes())
        }
        "pdf" => ("pdf", "application/pdf", pdf::export(&v.title, m)?),
        _ => {
            return Err(storage::refused(
                "Choose Markdown, PDF, Fountain or plain text.",
            ))
        }
    };
    let name = format!("{}-{}.{}", d.id, v.id, extension);
    let path = l.root.join("exports").join(&name);
    storage::atomic_bytes(&path, &bytes)?;
    Ok(
        json!({"path":path,"name":format!("{}.{}",v.title.chars().map(|c|if c.is_alphanumeric()||" -_".contains(c){c}else{'_'}).collect::<String>(),extension),"mime":mime,"base64":STANDARD.encode(bytes),"version":v.id}),
    )
}

pub use assist::ask;
