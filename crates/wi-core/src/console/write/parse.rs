use super::super::storage::{refused, Result};
use super::{Manuscript, Mode};
use fountain::data::Line;
use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag, TagEnd};
use serde::Serialize;
use serde_json::{json, Value};

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}
pub fn words(text: &str) -> usize {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '\u{2019}')
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count()
}

pub fn markdown_html(text: &str, lyrics: bool) -> String {
    let events = Parser::new_ext(text, options()).map(|event| match event {
        Event::Html(s) | Event::InlineHtml(s) => Event::Text(s),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let dest = if dest_url.starts_with("https://")
                || dest_url.starts_with("http://")
                || dest_url.starts_with("mailto:")
            {
                dest_url
            } else {
                CowStr::from("")
            };
            Event::Start(Tag::Link {
                link_type,
                dest_url: dest,
                title,
                id,
            })
        }
        // Research stays local: a preview never fetches embedded remote images.
        Event::Start(Tag::Image { .. }) => {
            Event::Html(CowStr::from("<span class=\"write-image-alt\">"))
        }
        Event::End(TagEnd::Image) => Event::Html(CowStr::from("</span>")),
        Event::SoftBreak if lyrics => Event::HardBreak,
        other => other,
    });
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}

#[derive(Clone, Debug, Serialize)]
pub struct Block {
    pub kind: String,
    pub text: String,
}

pub fn screenplay(text: &str) -> Result<Vec<Block>> {
    let input = format!("{}\n", text.replace("\r\n", "\n").replace('\r', "\n"));
    let (rest, mut doc) = fountain::parse_document::<nom::error::VerboseError<&str>>(&input)
        .map_err(|_| {
            refused("This Fountain text could not be parsed. Its source is still preserved.")
        })?;
    let mut rest = rest;
    // This parser stops at consecutive action/dialogue lines. Resume its grammar
    // without treating later colons as title-page fields; preserve raw source gaps.
    while !rest.trim().is_empty() {
        let start = input.len() - rest.len();
        let continuation = &input[start.saturating_sub(1)..];
        let (next, part) = fountain::parse_document::<nom::error::VerboseError<&str>>(continuation)
            .map_err(|_| {
                refused("This Fountain passage could not be parsed. Its source is still preserved.")
            })?;
        if next.len() >= rest.len() {
            return Err(refused(
                "This Fountain passage could not be parsed. Its source is still preserved.",
            ));
        }
        doc.lines.extend(part.lines);
        rest = next;
    }
    let mut blocks = Vec::new();
    if let Some(title) = doc.titlepage.title {
        blocks.push(Block {
            kind: "title".into(),
            text: title,
        });
    }
    if let Some(author) = doc.titlepage.author {
        blocks.push(Block {
            kind: "credit".into(),
            text: format!("Written by {author}"),
        });
    }
    for (key, value) in doc.titlepage.other {
        blocks.push(Block {
            kind: "credit".into(),
            text: format!("{key}: {value}"),
        });
    }
    let mut cursor = 0;
    let mut in_dialogue = false;
    for line in doc.lines {
        let (mut kind, mut value) = match line {
            Line::Scene(s) => ("scene", s),
            Line::Action(s) => ("action", s),
            Line::Dialogue(s) => ("dialogue", s),
            Line::Speaker { name, .. } => ("character", name),
            Line::Parenthetical(s) => ("parenthetical", format!("({s})")),
            Line::Transition(s) => ("transition", s),
            Line::Lyric(s) => ("lyric", s),
        };
        if let Some(at) = input[cursor..].find(&value) {
            let at = cursor + at;
            if input[cursor..at].contains("\n\n") {
                in_dialogue = false;
            }
            cursor = at + value.len();
        }
        if value.is_empty() {
            in_dialogue = false;
            continue;
        }
        // The established parser handles the grammar; these forcing markers
        // and continued dialogue extend its documented subset without altering source.
        if value.starts_with('.') && !value.starts_with("..") {
            kind = "scene";
            value = value[1..].into();
            in_dialogue = false;
        } else if value.starts_with('!') {
            kind = "action";
            value = value[1..].into();
            in_dialogue = false;
        } else if value.starts_with('@') {
            kind = "character";
            value = value[1..].into();
            in_dialogue = true;
        } else if kind == "character" {
            in_dialogue = true;
        } else if kind == "scene" || kind == "transition" {
            in_dialogue = false;
        } else if in_dialogue && kind == "action" {
            kind = if value.starts_with('(') && value.ends_with(')') {
                "parenthetical"
            } else {
                "dialogue"
            };
        }
        blocks.push(Block {
            kind: kind.into(),
            text: value,
        });
    }
    Ok(blocks)
}

pub fn markdown_blocks(text: &str, lyrics: bool) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut kind = "paragraph".to_string();
    let mut code = false;
    let flush = |blocks: &mut Vec<Block>, current: &mut String, kind: &str| {
        if !current.trim().is_empty() {
            blocks.push(Block {
                kind: kind.into(),
                text: std::mem::take(current),
            });
        }
    };
    for event in Parser::new_ext(text, options()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush(&mut blocks, &mut current, &kind);
                kind = format!("h{}", level as usize);
            }
            Event::Start(Tag::Paragraph) => {
                if current.is_empty() {
                    kind = "paragraph".into();
                }
            }
            Event::Start(Tag::Item) => {
                flush(&mut blocks, &mut current, &kind);
                kind = "item".into();
                current.push_str("- ");
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush(&mut blocks, &mut current, &kind);
                kind = "code".into();
                code = true;
            }
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::CodeBlock
                | TagEnd::TableRow
                | TagEnd::TableHead,
            ) => {
                flush(&mut blocks, &mut current, &kind);
                kind = "paragraph".into();
                code = false;
            }
            Event::End(TagEnd::TableCell) => current.push_str(" | "),
            Event::Text(s) | Event::Code(s) | Event::Html(s) | Event::InlineHtml(s) => {
                current.push_str(&s)
            }
            Event::SoftBreak => current.push(if lyrics || code { '\n' } else { ' ' }),
            Event::HardBreak => current.push('\n'),
            Event::TaskListMarker(done) => current.push_str(if done { "[x] " } else { "[ ] " }),
            Event::Rule => {
                flush(&mut blocks, &mut current, &kind);
                blocks.push(Block {
                    kind: "rule".into(),
                    text: "---".into(),
                });
            }
            _ => {}
        }
    }
    flush(&mut blocks, &mut current, &kind);
    blocks
}

pub fn blocks(text: &str, mode: Mode) -> Result<Vec<Block>> {
    if mode == Mode::Screenplay {
        screenplay(text)
    } else {
        Ok(markdown_blocks(text, mode == Mode::Lyrics))
    }
}
pub fn plain(text: &str, mode: Mode) -> Result<String> {
    Ok(blocks(text, mode)?
        .into_iter()
        .map(|b| b.text)
        .collect::<Vec<_>>()
        .join("\n\n"))
}

fn syllables(word: &str) -> usize {
    let word = word.to_ascii_lowercase();
    let letters: Vec<_> = word.chars().filter(char::is_ascii_alphabetic).collect();
    if letters.is_empty() {
        return 0;
    }
    let vowel = |c: char| "aeiouy".contains(c);
    let mut groups = letters
        .iter()
        .enumerate()
        .filter(|(i, c)| vowel(**c) && (*i == 0 || !vowel(letters[i - 1])))
        .count();
    if word.ends_with('e') && !word.ends_with("le") && groups > 1 {
        groups -= 1;
    }
    if word.ends_with("ed") && !word.ends_with("ted") && !word.ends_with("ded") && groups > 1 {
        groups -= 1;
    }
    groups.max(1)
}

pub fn stats(text: &str, mode: Mode) -> Value {
    let body = plain(text, mode).unwrap_or_else(|_| text.into());
    let mut outline = Vec::new();
    let mut heading: Option<(usize, usize, String)> = None;
    for (event, span) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some((
                    level as usize,
                    text[..span.start].bytes().filter(|b| *b == b'\n').count() + 1,
                    String::new(),
                ))
            }
            Event::Text(s) | Event::Code(s) => {
                if let Some((_, _, t)) = &mut heading {
                    t.push_str(&s);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, line, title)) = heading.take() {
                    outline.push(json!({"level":level,"line":line,"title":title}));
                }
            }
            _ => {}
        }
    }
    let lines: Vec<_> = text
        .lines()
        .enumerate()
        .filter(|(_, s)| !s.trim().is_empty())
        .map(|(i, s)| {
            let count = s
                .split(|c: char| !c.is_alphabetic() && c != '\'')
                .filter(|w| !w.is_empty())
                .map(syllables)
                .sum::<usize>();
            json!({"line":i+1,"text":s,"syllables":count,"words":words(s)})
        })
        .collect();
    json!({"words":words(&body),"characters":text.chars().count(),"lineCount":lines.len(),"lines":lines,"outline":outline,"syllablesEstimated":true})
}

pub fn manuscript_stats(m: &Manuscript) -> Value {
    let sections: Vec<_> = m
        .sections
        .iter()
        .map(|s| json!({"id":s.id,"stats":stats(&s.text,m.mode)}))
        .collect();
    let total = sections
        .iter()
        .map(|s| s["stats"]["words"].as_u64().unwrap_or(0))
        .sum::<u64>();
    json!({"words":total,"sections":sections})
}

pub fn render(text: &str, mode: Mode) -> Result<Value> {
    let html = if mode == Mode::Screenplay {
        screenplay(text)?
            .iter()
            .map(|b| {
                format!(
                    "<div class=\"write-script-{}\">{}</div>",
                    b.kind,
                    markdown_html(&b.text, true)
                )
            })
            .collect::<String>()
    } else {
        markdown_html(text, mode == Mode::Lyrics)
    };
    Ok(json!({"html":html,"stats":stats(text,mode)}))
}
