//! From Wikipedia's HTML to blocks.
//!
//! An article is `{title, description, revision, modified, sections, blocks,
//! refs}`. A block is one of:
//!
//! ```text
//! {"t":"h", "level":2, "id":"Definition", "text":"Definition"}
//! {"t":"p", "c":[inline…]}
//! {"t":"note", "c":[inline…]}                       a hatnote, shown small
//! {"t":"ul"|"ol", "items":[{"c":[inline…], "sub":[block…]}]}
//! {"t":"dl", "items":[{"term":bool, "c":[inline…], "sub":[block…]}]}
//! {"t":"quote", "c":[block…]}
//! {"t":"pre", "text":"…"}
//! {"t":"math", "text":"…"}                          a formula on its own line
//! {"t":"table", "caption":[inline…], "rows":[[{"h":bool, "c":[inline…]}]]}
//! ```
//!
//! and inline content is a list of strings and marks:
//!
//! ```text
//! "plain text"
//! {"t":"b"|"i"|"sup"|"sub"|"code", "c":[inline…]}
//! {"t":"a", "title":"Fourier series", "frag":"History", "c":[…]}   another article
//! {"t":"j", "frag":"Definition", "c":[…]}                          a place in this one
//! {"t":"x", "href":"https://…", "c":[…]}                           the web
//! {"t":"math", "s":"f(x)"}
//! {"t":"ref", "n":"1", "id":"cite_note-1"}
//! {"t":"br"}
//! ```

use serde_json::{json, Map, Value};

use crate::html::{parse, Element, Node};
use crate::{display_title, is_article_title, percent_decode, tex};

/// Classes whose elements are left out whole.
const SKIP_CLASSES: &[&str] = &[
    "navbox",
    "navbox-styles",
    "vertical-navbox",
    "sidebar",
    "metadata",
    "ambox",
    "mbox-small",
    "mw-editsection",
    "noprint",
    "mw-empty-elt",
    "mw-cite-backlink",
    "Z3988",
    "sistersitebox",
    "side-box",
    "portalbox",
    "portal-bar",
    "authority-control",
    "catlinks",
    "printfooter",
    "gallery",
    "thumb",
    "tmulti",
    "mw-indicators",
    "toc",
    "infobox-image",
    "mw-kartographer-map",
    "mw-kartographer-container",
    "stub",
    "navigation-not-searchable-skip",
    "mw-jump-link",
    "shortdescription",
    "nomobile-skip",
    "locmap",
    "mw-file-element",
    "geo-nondefault",
    "coordinates",
    "spoken-wikipedia",
    "wikitable-skip",
    "mw-collapsible-toggle",
    "hlist-skip",
    "reference-accessdate-skip",
    "cs1-hidden-error",
    "cs1-maint",
    "citation-comment",
    "uncited-category",
    "mw-reflink-text-skip",
    "IPA-audio",
    "ext-phonos",
    "noexcerpt-skip",
    "bandeau-container",
    "dablink-skip",
    "reflist-skip",
    "refbegin-skip",
    "infobox-below-skip",
    "mw-graph",
    "timeline-wrapper",
    "listen",
    "haudio",
    "mw-tmh-player",
];

const SKIP_TAGS: &[&str] = &[
    "style",
    "script",
    "link",
    "meta",
    "figure",
    "img",
    "audio",
    "video",
    "map",
    "area",
    "object",
    "iframe",
    "input",
    "button",
    "form",
    "select",
    "textarea",
    "noscript",
    "head",
    "title",
    "base",
    "svg",
    "canvas",
    "figcaption",
    "picture",
    "source",
    "track",
];

const BLOCK_TAGS: &[&str] = &[
    "p",
    "div",
    "section",
    "ul",
    "ol",
    "dl",
    "table",
    "blockquote",
    "pre",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "center",
    "article",
    "main",
    "aside",
    "header",
    "footer",
    "details",
    "summary",
    "hr",
    "body",
    "html",
    "nav",
    "address",
    "fieldset",
];

struct Out {
    /// The page's own title as an address writes it, for telling a link to
    /// this article from a link to another.
    own: String,
    blocks: Vec<Value>,
    refs: Vec<Value>,
    description: Option<String>,
}

fn skipped(e: &Element) -> bool {
    if SKIP_TAGS.contains(&e.name.as_str()) {
        return true;
    }
    if e.attr("class").is_some_and(|c| {
        c.split_ascii_whitespace()
            .any(|x| SKIP_CLASSES.contains(&x))
    }) {
        return true;
    }
    // Styles and scripts that arrive as extensions, and hidden things.
    if e.attr_has("typeof", "mw:Extension/templatestyles") {
        return true;
    }
    if e.attr("style")
        .is_some_and(|s| s.replace(' ', "").contains("display:none"))
    {
        return true;
    }
    false
}

fn squash(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for ch in s.chars() {
        if ch.is_whitespace() && ch != '\u{a0}' {
            space = true;
        } else {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(ch);
        }
    }
    if space && !out.is_empty() {
        out.push(' ');
    }
    out
}

/// Adds text to inline content, joining it to a string already at the end
/// and collapsing runs of white space.
fn push_text(out: &mut Vec<Value>, text: &str) {
    if text.is_empty() {
        return;
    }
    let lead = text.starts_with(|c: char| c.is_whitespace() && c != '\u{a0}');
    let body = squash(text);
    let body = if lead && !body.starts_with(' ') {
        format!(" {body}")
    } else {
        body
    };
    if body.is_empty() {
        return;
    }
    if let Some(Value::String(last)) = out.last_mut() {
        if last.ends_with(' ') && body.starts_with(' ') {
            last.push_str(body.trim_start());
        } else {
            last.push_str(&body);
        }
        return;
    }
    out.push(Value::String(body));
}

/// Inline content with no white space at either end, or nothing at all.
fn trimmed(mut c: Vec<Value>) -> Vec<Value> {
    while let Some(Value::String(s)) = c.first() {
        let t = s.trim_start().to_string();
        if t.is_empty() {
            c.remove(0);
        } else {
            c[0] = Value::String(t);
            break;
        }
    }
    while let Some(Value::String(s)) = c.last() {
        let t = s.trim_end().to_string();
        let n = c.len() - 1;
        if t.is_empty() {
            c.pop();
        } else {
            c[n] = Value::String(t);
            break;
        }
    }
    while matches!(c.last(), Some(v) if v["t"] == "br") {
        c.pop();
    }
    while matches!(c.first(), Some(v) if v["t"] == "br") {
        c.remove(0);
    }
    c
}

fn is_empty_inline(c: &[Value]) -> bool {
    c.iter().all(|v| match v {
        Value::String(s) => s.trim().is_empty(),
        other => other["t"] == "br",
    })
}

/// A formula's text, from the TeX Wikipedia keeps beside the MathML.
fn math_text(e: &Element) -> String {
    let tex_src = e
        .find("annotation")
        .map(|a| a.text())
        .or_else(|| e.find("img").and_then(|i| i.attr("alt")).map(String::from))
        .unwrap_or_else(|| e.text());
    tex::plain(&tex_src)
}

impl Out {
    /// Where a wiki link goes: another article, a place in this one, or
    /// nowhere (plain text).
    fn wiki_link(&self, e: &Element, c: Vec<Value>) -> Vec<Value> {
        let href = e.attr("href").unwrap_or("");
        if e.has_class("new") || href.contains("redlink=1") {
            return c;
        }
        let path = href
            .strip_prefix("./")
            .or_else(|| href.strip_prefix("/wiki/"))
            .unwrap_or(href);
        let (page, frag) = match path.split_once('#') {
            Some((p, f)) => (p, Some(percent_decode(f))),
            None => (path, None),
        };
        // A query string would be an action on the page, not the page.
        let page = page.split('?').next().unwrap_or(page);
        let same = page.is_empty() || page == self.own;
        if same {
            return match frag {
                Some(frag) if !frag.is_empty() => vec![json!({"t": "j", "frag": frag, "c": c})],
                _ => c,
            };
        }
        let title = display_title(&percent_decode(page));
        if !is_article_title(&title) {
            return c;
        }
        let mut link = Map::new();
        link.insert("t".into(), json!("a"));
        link.insert("title".into(), json!(title));
        if let Some(frag) = frag.filter(|f| !f.is_empty()) {
            link.insert("frag".into(), json!(frag));
        }
        link.insert("c".into(), Value::Array(c));
        vec![Value::Object(link)]
    }

    fn inline(&mut self, nodes: &[Node], out: &mut Vec<Value>) {
        for node in nodes {
            match node {
                Node::Text(t) => push_text(out, t),
                Node::Element(e) => self.inline_element(e, out),
            }
        }
    }

    fn inline_children(&mut self, e: &Element) -> Vec<Value> {
        let mut c = Vec::new();
        self.inline(&e.children, &mut c);
        c
    }

    fn inline_element(&mut self, e: &Element, out: &mut Vec<Value>) {
        if e.has_class("mwe-math-element") || e.name == "math" {
            let s = math_text(e);
            if !s.is_empty() {
                out.push(json!({"t": "math", "s": s}));
            }
            return;
        }
        // A reference marker: [1].
        if e.name == "sup" && (e.has_class("mw-ref") || e.has_class("reference")) {
            let id = e
                .find("a")
                .and_then(|a| a.attr("href"))
                .and_then(|h| h.split_once('#'))
                .map(|(_, f)| percent_decode(f))
                .unwrap_or_default();
            let n = e
                .text()
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .to_string();
            if !id.is_empty() && !n.is_empty() {
                out.push(json!({"t": "ref", "n": n, "id": id}));
            }
            return;
        }
        if skipped(e) {
            return;
        }
        match e.name.as_str() {
            "br" => out.push(json!({"t": "br"})),
            "wbr" => {}
            "a" => {
                let c = self.inline_children(e);
                if is_empty_inline(&c) {
                    return;
                }
                let rel = e.attr("rel").unwrap_or("");
                let href = e.attr("href").unwrap_or("");
                let wiki = rel.split_ascii_whitespace().any(|r| r == "mw:WikiLink")
                    || href.starts_with("./");
                if wiki {
                    out.extend(self.wiki_link(e, c));
                    return;
                }
                let href = if let Some(rest) = href.strip_prefix("//") {
                    format!("https://{rest}")
                } else {
                    href.to_string()
                };
                if href.starts_with("https://") || href.starts_with("http://") {
                    out.push(json!({"t": "x", "href": href, "c": c}));
                } else if let Some(frag) = href.strip_prefix('#') {
                    out.push(json!({"t": "j", "frag": percent_decode(frag), "c": c}));
                } else {
                    out.extend(c);
                }
            }
            "b" | "strong" => self.mark("b", e, out),
            "i" | "em" | "cite" | "var" | "dfn" => self.mark("i", e, out),
            "sup" => self.mark("sup", e, out),
            "sub" => self.mark("sub", e, out),
            "code" | "kbd" | "samp" | "tt" => self.mark("code", e, out),
            "q" => {
                push_text(out, "“");
                self.inline(&e.children, out);
                push_text(out, "”");
            }
            "li" | "dd" | "dt" | "td" | "th" | "tr" | "caption" => {
                // A list or a table met inside running text: its words, spaced.
                push_text(out, " ");
                self.inline(&e.children, out);
                push_text(out, " ");
            }
            name if BLOCK_TAGS.contains(&name) => {
                if !out.is_empty() && !matches!(out.last(), Some(v) if v["t"] == "br") {
                    out.push(json!({"t": "br"}));
                }
                self.inline(&e.children, out);
                out.push(json!({"t": "br"}));
            }
            _ => self.inline(&e.children, out),
        }
    }

    fn mark(&mut self, t: &str, e: &Element, out: &mut Vec<Value>) {
        let c = self.inline_children(e);
        if !is_empty_inline(&c) {
            out.push(json!({"t": t, "c": c}));
        }
    }

    /// The blocks of a run of nodes. Loose text and inline elements between
    /// blocks are gathered into paragraphs.
    fn blocks(&mut self, nodes: &[Node]) -> Vec<Value> {
        let mut out = Vec::new();
        let mut run: Vec<Value> = Vec::new();
        fn flush(run: &mut Vec<Value>, out: &mut Vec<Value>) {
            let c = trimmed(std::mem::take(run));
            if !is_empty_inline(&c) {
                out.push(json!({"t": "p", "c": c}));
            }
        }
        for node in nodes {
            match node {
                Node::Text(t) => push_text(&mut run, t),
                Node::Element(e) => {
                    let block_math = e.has_class("mwe-math-element-block")
                        || (e.name == "math" && e.attr("display") == Some("block"));
                    let is_block = BLOCK_TAGS.contains(&e.name.as_str()) || block_math;
                    if !is_block {
                        self.inline_element(e, &mut run);
                        continue;
                    }
                    flush(&mut run, &mut out);
                    if block_math {
                        let text = math_text(e);
                        if !text.is_empty() {
                            out.push(json!({"t": "math", "text": text}));
                        }
                        continue;
                    }
                    self.block(e, &mut out);
                }
            }
        }
        flush(&mut run, &mut out);
        out
    }

    fn block(&mut self, e: &Element, out: &mut Vec<Value>) {
        if e.has_class("shortdescription") {
            let text = squash(&e.text()).trim().to_string();
            if !text.is_empty() && self.description.is_none() {
                self.description = Some(text);
            }
            return;
        }
        // The reference list is gathered, not shown where it stands.
        if e.name == "ol" && (e.has_class("references") || e.has_class("mw-references")) {
            self.references(e);
            return;
        }
        if skipped(e) {
            return;
        }
        match e.name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = e.name[1..].parse::<u8>().unwrap_or(2);
                let text = squash(&heading_text(e)).trim().to_string();
                if text.is_empty() {
                    return;
                }
                let id = e
                    .attr("id")
                    .map(String::from)
                    .unwrap_or_else(|| text.replace(' ', "_"));
                out.push(json!({"t": "h", "level": level, "id": id, "text": text}));
            }
            "p" => {
                // A formula alone in a paragraph stands on its own line.
                let c = trimmed(self.inline_children(e));
                if is_empty_inline(&c) {
                    return;
                }
                if c.len() == 1
                    && c[0]["t"] == "math"
                    && e.find_class("mwe-math-element-block").is_some()
                {
                    out.push(json!({"t": "math", "text": c[0]["s"]}));
                    return;
                }
                out.push(json!({"t": "p", "c": c}));
            }
            "ul" | "ol" => {
                let mut items = Vec::new();
                for child in &e.children {
                    if let Node::Element(li) = child {
                        if li.name == "li" && !skipped(li) {
                            let (c, sub) = self.item(li);
                            if !is_empty_inline(&c) || !sub.is_empty() {
                                items.push(json!({"c": c, "sub": sub}));
                            }
                        }
                    }
                }
                if !items.is_empty() {
                    out.push(json!({"t": e.name, "items": items}));
                }
            }
            "dl" => {
                let mut items = Vec::new();
                for child in &e.children {
                    if let Node::Element(d) = child {
                        if (d.name == "dt" || d.name == "dd") && !skipped(d) {
                            let (c, sub) = self.item(d);
                            if !is_empty_inline(&c) || !sub.is_empty() {
                                items.push(json!({"term": d.name == "dt", "c": c, "sub": sub}));
                            }
                        }
                    }
                }
                // An indented formula is a dl of one dd: show the formula.
                if items.len() == 1 && items[0]["term"] == false {
                    let c = items[0]["c"].as_array().cloned().unwrap_or_default();
                    if c.len() == 1 && c[0]["t"] == "math" {
                        out.push(json!({"t": "math", "text": c[0]["s"]}));
                        return;
                    }
                }
                if !items.is_empty() {
                    out.push(json!({"t": "dl", "items": items}));
                }
            }
            "blockquote" => {
                let c = self.blocks(&e.children);
                if !c.is_empty() {
                    out.push(json!({"t": "quote", "c": c}));
                }
            }
            "pre" => {
                let text = e.text();
                if !text.trim().is_empty() {
                    out.push(json!({"t": "pre", "text": text.trim_end()}));
                }
            }
            "table" => self.table(e, out),
            "hr" => {}
            _ => {
                if e.has_class("hatnote") || e.has_class("dablink") || e.has_class("rellink") {
                    let c = trimmed(self.inline_children(e));
                    if !is_empty_inline(&c) {
                        out.push(json!({"t": "note", "c": c}));
                    }
                    return;
                }
                // A div, a section and their kind hold blocks of their own.
                let inner = self.blocks(&e.children);
                out.extend(inner);
            }
        }
    }

    /// A list item: its own line, and the lists and blocks nested in it.
    fn item(&mut self, li: &Element) -> (Vec<Value>, Vec<Value>) {
        let mut c = Vec::new();
        let mut sub = Vec::new();
        for node in &li.children {
            match node {
                Node::Text(t) => push_text(&mut c, t),
                Node::Element(e) => {
                    if matches!(
                        e.name.as_str(),
                        "ul" | "ol" | "dl" | "table" | "blockquote" | "pre"
                    ) {
                        self.block(e, &mut sub);
                    } else if e.name == "p" || e.name == "div" {
                        if skipped(e) {
                            continue;
                        }
                        // The first paragraph is the item's line; later ones go below it.
                        if is_empty_inline(&c) && sub.is_empty() {
                            self.inline(&e.children, &mut c);
                        } else {
                            let more = self.blocks(std::slice::from_ref(node));
                            sub.extend(more);
                        }
                    } else {
                        self.inline_element(e, &mut c);
                    }
                }
            }
        }
        (trimmed(c), sub)
    }

    fn table(&mut self, table: &Element, out: &mut Vec<Value>) {
        let mut caption = Vec::new();
        let mut rows: Vec<Value> = Vec::new();
        let mut widest = 0usize;
        fn each_row<'a>(
            e: &'a Element,
            rows: &mut Vec<&'a Element>,
            caption: &mut Option<&'a Element>,
        ) {
            for child in &e.children {
                if let Node::Element(c) = child {
                    match c.name.as_str() {
                        "tr" => rows.push(c),
                        "thead" | "tbody" | "tfoot" => each_row(c, rows, caption),
                        "caption" if caption.is_none() => *caption = Some(c),
                        _ => {}
                    }
                }
            }
        }
        let mut trs = Vec::new();
        let mut cap = None;
        each_row(table, &mut trs, &mut cap);
        if let Some(cap) = cap {
            caption = trimmed(self.inline_children(cap));
        }
        let infobox = table.has_class("infobox") || table.has_class_prefix("infobox");
        // A numbered equation is laid out as a table: the formula, then its label.
        if table.has_class("numblk") {
            let c = trimmed(self.inline_children(table));
            let formula = c.iter().find(|v| v["t"] == "math").map(|v| v["s"].clone());
            if let Some(formula) = formula {
                let mut label = String::new();
                inline_text(
                    &Value::Array(c.iter().filter(|v| v["t"] != "math").cloned().collect()),
                    &mut label,
                );
                let label = squash(&label).trim().to_string();
                let text = if label.is_empty() {
                    formula.as_str().unwrap_or("").to_string()
                } else {
                    format!("{}    ({label})", formula.as_str().unwrap_or(""))
                };
                out.push(json!({"t": "math", "text": text}));
                return;
            }
        }
        for tr in trs {
            if skipped(tr) {
                continue;
            }
            // In an infobox, a row that held a picture or a map is its caption: left out.
            if infobox && holds_picture(tr) {
                continue;
            }
            let mut cells = Vec::new();
            for child in &tr.children {
                if let Node::Element(cell) = child {
                    if (cell.name == "td" || cell.name == "th") && !skipped(cell) {
                        let c = trimmed(self.inline_children(cell));
                        let span = cell
                            .attr("colspan")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                            .unwrap_or(1)
                            .clamp(1, 40);
                        let mut one = Map::new();
                        if cell.name == "th" {
                            one.insert("h".into(), json!(true));
                        }
                        if span > 1 {
                            one.insert("span".into(), json!(span));
                        }
                        one.insert("c".into(), Value::Array(c));
                        cells.push(Value::Object(one));
                    }
                }
            }
            let has_text = cells
                .iter()
                .any(|c| !is_empty_inline(c["c"].as_array().map_or(&[][..], |a| a.as_slice())));
            if has_text {
                widest = widest.max(cells.len());
                rows.push(Value::Array(cells));
            }
        }
        if rows.is_empty() {
            return;
        }
        // A one-cell table is a box around some text (an equation, a note).
        if widest == 1 && rows.len() == 1 {
            let c = rows[0][0]["c"].as_array().cloned().unwrap_or_default();
            if c.len() == 1 && c[0]["t"] == "math" {
                out.push(json!({"t": "math", "text": c[0]["s"]}));
            } else {
                out.push(json!({"t": "p", "c": c}));
            }
            return;
        }
        let mut t = Map::new();
        t.insert("t".into(), json!("table"));
        if infobox {
            t.insert("box".into(), json!(true));
        }
        if !is_empty_inline(&caption) {
            t.insert("caption".into(), Value::Array(caption));
        }
        t.insert("rows".into(), Value::Array(rows));
        out.push(Value::Object(t));
    }

    fn references(&mut self, ol: &Element) {
        for child in &ol.children {
            let Node::Element(li) = child else { continue };
            if li.name != "li" {
                continue;
            }
            let id = li.attr("id").unwrap_or("").to_string();
            let body = li
                .find_class("mw-reference-text")
                .or_else(|| li.find_class("reference-text"))
                .unwrap_or(li);
            let c = trimmed(self.inline_children(body));
            if is_empty_inline(&c) {
                continue;
            }
            let n = self.refs.len() + 1;
            self.refs
                .push(json!({"id": id, "n": n.to_string(), "c": c}));
        }
    }
}

/// Whether an element holds a picture, a map or a player anywhere inside.
fn holds_picture(e: &Element) -> bool {
    e.children.iter().any(|c| match c {
        Node::Element(el) => {
            matches!(
                el.name.as_str(),
                "img" | "figure" | "audio" | "video" | "svg"
            ) || el.has_class("mw-file-element")
                || el.has_class("mw-kartographer-map")
                || el.has_class("mw-kartographer-container")
                || el.has_class("locmap")
                || el.has_class("geo-inline")
                || el.has_class("geo-default")
                || holds_picture(el)
        }
        Node::Text(_) => false,
    })
}

/// An infobox that stands before the first paragraph goes after it, so an
/// article opens with its first sentence.
fn lead_first(mut blocks: Vec<Value>) -> Vec<Value> {
    let first_p = blocks.iter().position(|b| b["t"] == "p");
    let first_h = blocks
        .iter()
        .position(|b| b["t"] == "h")
        .unwrap_or(blocks.len());
    let Some(p) = first_p.filter(|p| *p < first_h) else {
        return blocks;
    };
    let mut moved = Vec::new();
    let mut i = 0;
    while i < p - moved.len() {
        if blocks[i]["box"] == true {
            moved.push(blocks.remove(i));
        } else {
            i += 1;
        }
    }
    let at = p - moved.len() + 1;
    for (n, b) in moved.into_iter().enumerate() {
        blocks.insert(at + n, b);
    }
    blocks
}

/// A heading's words, without the edit link beside them.
fn heading_text(e: &Element) -> String {
    let mut out = String::new();
    fn walk(nodes: &[Node], out: &mut String) {
        for n in nodes {
            match n {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) => {
                    if e.has_class("mw-editsection") || skipped(e) {
                        continue;
                    }
                    if e.has_class("mwe-math-element") {
                        out.push_str(&math_text(e));
                        continue;
                    }
                    walk(&e.children, out);
                }
            }
        }
    }
    walk(&e.children, &mut out);
    out
}

/// Headings that hold nothing once references and navigation are taken out.
fn drop_empty_sections(blocks: Vec<Value>) -> Vec<Value> {
    let level = |b: &Value| b["level"].as_u64().unwrap_or(0);
    let mut out: Vec<Value> = Vec::new();
    for b in blocks {
        if b["t"] == "h" {
            // A heading straight after one as deep or deeper means that one was empty.
            while let Some(prev) = out.last() {
                if prev["t"] == "h" && level(prev) >= level(&b) {
                    out.pop();
                } else {
                    break;
                }
            }
        }
        out.push(b);
    }
    while matches!(out.last(), Some(b) if b["t"] == "h") {
        out.pop();
    }
    out
}

/// The numbered contents: every heading that survived.
fn contents(blocks: &[Value]) -> Vec<Value> {
    let mut counters = [0u32; 7];
    let mut out = Vec::new();
    let top = blocks
        .iter()
        .filter(|b| b["t"] == "h")
        .filter_map(|b| b["level"].as_u64())
        .min()
        .unwrap_or(2) as usize;
    for b in blocks {
        if b["t"] != "h" {
            continue;
        }
        let level = (b["level"].as_u64().unwrap_or(2) as usize).clamp(top, 6);
        counters[level] += 1;
        for c in counters.iter_mut().skip(level + 1) {
            *c = 0;
        }
        let number = (top..=level)
            .map(|l| counters[l].max(1).to_string())
            .collect::<Vec<_>>()
            .join(".");
        out.push(json!({
            "id": b["id"], "title": b["text"], "level": level - top + 1, "number": number,
        }));
    }
    out
}

/// Reads the HTML Wikipedia's REST API serves for `title` into an article.
pub fn parse_article(html: &str, title: &str) -> Value {
    let root = parse(html);
    let head_title = root
        .find("title")
        .map(|t| t.text())
        .filter(|t| !t.trim().is_empty());
    let shown = head_title.unwrap_or_else(|| display_title(title));
    let meta = |property: &str| -> Option<String> {
        fn walk<'a>(e: &'a Element, property: &str) -> Option<&'a str> {
            for c in &e.children {
                if let Node::Element(el) = c {
                    if el.name == "meta" && el.attr("property") == Some(property) {
                        return el.attr("content");
                    }
                    if el.name == "head" || el.name == "html" {
                        if let Some(found) = walk(el, property) {
                            return Some(found);
                        }
                    }
                }
            }
            None
        }
        walk(&root, property).map(String::from)
    };
    let revision = root
        .find("html")
        .and_then(|h| h.attr("about"))
        .and_then(|a| a.rsplit('/').next())
        .unwrap_or("")
        .to_string();
    let mut out = Out {
        own: display_title(&shown).replace(' ', "_"),
        blocks: Vec::new(),
        refs: Vec::new(),
        description: None,
    };
    let body = root.find("body").unwrap_or(&root);
    let blocks = out.blocks(&body.children);
    out.blocks = lead_first(drop_empty_sections(blocks));
    let sections = contents(&out.blocks);
    json!({
        "title": display_title(&shown),
        "description": out.description,
        "revision": revision,
        "modified": meta("dc:modified"),
        "sections": sections,
        "blocks": out.blocks,
        "refs": out.refs,
    })
}

fn inline_text(c: &Value, out: &mut String) {
    let Some(items) = c.as_array() else { return };
    for v in items {
        match v {
            Value::String(s) => out.push_str(s),
            other => match other["t"].as_str() {
                Some("br") => out.push(' '),
                Some("ref") => {}
                Some("math") => out.push_str(other["s"].as_str().unwrap_or("")),
                _ => inline_text(&other["c"], out),
            },
        }
    }
}

fn block_text(b: &Value, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match b["t"].as_str() {
        Some("h") => {
            let level = b["level"].as_u64().unwrap_or(2) as usize;
            out.push_str(&format!(
                "\n{} {}\n\n",
                "#".repeat(level),
                b["text"].as_str().unwrap_or("")
            ));
        }
        Some("p") | Some("note") => {
            out.push_str(&pad);
            inline_text(&b["c"], out);
            out.push_str("\n\n");
        }
        Some("math") => {
            out.push_str(&format!(
                "{pad}    {}\n\n",
                b["text"].as_str().unwrap_or("")
            ));
        }
        Some("pre") => {
            out.push_str(b["text"].as_str().unwrap_or(""));
            out.push_str("\n\n");
        }
        Some("quote") => {
            for inner in b["c"].as_array().into_iter().flatten() {
                block_text(inner, depth + 1, out);
            }
        }
        Some(list @ ("ul" | "ol" | "dl")) => {
            for (i, item) in b["items"].as_array().into_iter().flatten().enumerate() {
                let bullet = match list {
                    "ol" => format!("{}. ", i + 1),
                    "dl" => String::new(),
                    _ => "- ".to_string(),
                };
                out.push_str(&format!("{pad}{bullet}"));
                inline_text(&item["c"], out);
                out.push('\n');
                for inner in item["sub"].as_array().into_iter().flatten() {
                    block_text(inner, depth + 1, out);
                }
            }
            out.push('\n');
        }
        Some("table") => {
            if b["caption"].is_array() {
                out.push_str(&pad);
                inline_text(&b["caption"], out);
                out.push('\n');
            }
            for row in b["rows"].as_array().into_iter().flatten() {
                let cells: Vec<String> = row
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|cell| {
                        let mut s = String::new();
                        inline_text(&cell["c"], &mut s);
                        s.trim().to_string()
                    })
                    .collect();
                out.push_str(&format!("{pad}| {} |\n", cells.join(" | ")));
            }
            out.push('\n');
        }
        _ => {}
    }
}

fn tidy(s: String, max_chars: usize) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank = 0;
    for line in s.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    let out = out.trim().to_string();
    if out.chars().count() <= max_chars {
        return out;
    }
    let cut: String = out.chars().take(max_chars).collect();
    format!("{cut}…")
}

/// An article as plain text with `#` headings, for Claude to read. Cut to
/// `max_chars` characters, ending in "…" when it was longer.
pub fn plain_text(article: &Value, max_chars: usize) -> String {
    let mut out = String::new();
    for b in article["blocks"].as_array().into_iter().flatten() {
        block_text(b, 0, &mut out);
    }
    tidy(out, max_chars)
}

/// One section's text: from its heading up to the next heading as deep. The
/// section is named by its id or its title (any case); None when there is no
/// such section. The lead, before the first heading, is the section `""`.
pub fn section_text(article: &Value, section: &str, max_chars: usize) -> Option<String> {
    let blocks = article["blocks"].as_array()?;
    let want = section.trim().to_lowercase().replace('_', " ");
    let mut out = String::new();
    if want.is_empty() {
        for b in blocks {
            if b["t"] == "h" {
                break;
            }
            block_text(b, 0, &mut out);
        }
        return Some(tidy(out, max_chars));
    }
    let name = |b: &Value, key: &str| {
        b[key]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .replace('_', " ")
    };
    let start = blocks
        .iter()
        .position(|b| b["t"] == "h" && (name(b, "id") == want || name(b, "text") == want))?;
    let level = blocks[start]["level"].as_u64().unwrap_or(2);
    block_text(&blocks[start], 0, &mut out);
    for b in &blocks[start + 1..] {
        if b["t"] == "h" && b["level"].as_u64().unwrap_or(2) <= level {
            break;
        }
        block_text(b, 0, &mut out);
    }
    Some(tidy(out, max_chars))
}
