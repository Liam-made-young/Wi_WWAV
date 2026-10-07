//! A small, forgiving HTML reader: enough for the HTML Wikipedia's REST API
//! serves (Parsoid's, which closes every element), and safe on anything
//! else. It never fails: a tag that doesn't close is closed at the end, and
//! a closing tag with nothing to close is dropped.

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Text(String),
    Element(Element),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn has_class(&self, class: &str) -> bool {
        self.attr("class")
            .is_some_and(|c| c.split_ascii_whitespace().any(|x| x == class))
    }

    /// Whether any of the element's classes starts with `prefix`.
    pub fn has_class_prefix(&self, prefix: &str) -> bool {
        self.attr("class")
            .is_some_and(|c| c.split_ascii_whitespace().any(|x| x.starts_with(prefix)))
    }

    /// Whether a space-separated attribute (`rel`, `typeof`) holds `word`.
    pub fn attr_has(&self, name: &str, word: &str) -> bool {
        self.attr(name)
            .is_some_and(|c| c.split_ascii_whitespace().any(|x| x == word))
    }

    /// All the text inside, in order.
    pub fn text(&self) -> String {
        let mut out = String::new();
        collect_text(&self.children, &mut out);
        out
    }

    /// The first descendant with this tag name, depth first.
    pub fn find(&self, name: &str) -> Option<&Element> {
        for c in &self.children {
            if let Node::Element(e) = c {
                if e.name == name {
                    return Some(e);
                }
                if let Some(found) = e.find(name) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// The first descendant carrying this class, depth first.
    pub fn find_class(&self, class: &str) -> Option<&Element> {
        for c in &self.children {
            if let Node::Element(e) = c {
                if e.has_class(class) {
                    return Some(e);
                }
                if let Some(found) = e.find_class(class) {
                    return Some(found);
                }
            }
        }
        None
    }
}

fn collect_text(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text(t) => out.push_str(t),
            Node::Element(e) => collect_text(&e.children, out),
        }
    }
}

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Elements whose content is not markup.
const RAW: &[&str] = &["script", "style"];

/// Reads `html` into a tree under one root element named `#root`.
pub fn parse(html: &str) -> Element {
    let b = html.as_bytes();
    let mut stack: Vec<Element> = vec![Element {
        name: "#root".to_string(),
        ..Element::default()
    }];
    let mut i = 0;
    let mut text_start = 0;

    fn flush(stack: &mut [Element], html: &str, from: usize, to: usize) {
        if to > from {
            let t = decode(&html[from..to]);
            if !t.is_empty() {
                if let Some(top) = stack.last_mut() {
                    top.children.push(Node::Text(t));
                }
            }
        }
    }

    fn close(stack: &mut Vec<Element>, name: &str) {
        // Only if something open has this name; else the stray tag is dropped.
        if let Some(at) = stack.iter().rposition(|e| e.name == name) {
            if at == 0 {
                return;
            }
            while stack.len() > at {
                let done = stack.pop().expect("at < len");
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(Node::Element(done));
                }
            }
        }
    }

    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &html[i..];
        if rest.starts_with("<!--") {
            flush(&mut stack, html, text_start, i);
            i = match rest.find("-->") {
                Some(end) => i + end + 3,
                None => b.len(),
            };
            text_start = i;
            continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            flush(&mut stack, html, text_start, i);
            i = match rest.find('>') {
                Some(end) => i + end + 1,
                None => b.len(),
            };
            text_start = i;
            continue;
        }
        if rest.starts_with("</") {
            let Some(end) = rest.find('>') else {
                break;
            };
            flush(&mut stack, html, text_start, i);
            let name = rest[2..end].trim().to_ascii_lowercase();
            close(&mut stack, &name);
            i += end + 1;
            text_start = i;
            continue;
        }
        // An opening tag needs a letter after '<'; anything else is text.
        if !b.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic()) {
            i += 1;
            continue;
        }
        flush(&mut stack, html, text_start, i);
        let (el, self_closed, next) = read_tag(html, i);
        i = next;
        text_start = i;
        if RAW.contains(&el.name.as_str()) && !self_closed {
            let closing = format!("</{}", el.name);
            let end = find_ignoring_case(b, i, closing.as_bytes()).unwrap_or(b.len());
            // The content is dropped: nothing reads a script or a style.
            i = match html[end..].find('>') {
                Some(gt) => end + gt + 1,
                None => b.len(),
            };
            text_start = i;
            if let Some(top) = stack.last_mut() {
                top.children.push(Node::Element(el));
            }
            continue;
        }
        if self_closed || VOID.contains(&el.name.as_str()) {
            if let Some(top) = stack.last_mut() {
                top.children.push(Node::Element(el));
            }
        } else {
            stack.push(el);
        }
    }
    flush(&mut stack, html, text_start, b.len().max(text_start));
    while stack.len() > 1 {
        let done = stack.pop().expect("len > 1");
        if let Some(parent) = stack.last_mut() {
            parent.children.push(Node::Element(done));
        }
    }
    stack.pop().unwrap_or_default()
}

fn find_ignoring_case(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&at| {
        hay[at..at + needle.len()]
            .iter()
            .zip(needle)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

/// Reads one opening tag at `start` (a '<'): the element, whether it closed
/// itself with `/>`, and where the tag ends.
fn read_tag(html: &str, start: usize) -> (Element, bool, usize) {
    let b = html.as_bytes();
    let mut i = start + 1;
    let name_start = i;
    while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' && b[i] != b'/' {
        i += 1;
    }
    let mut el = Element {
        name: html[name_start..i].to_ascii_lowercase(),
        ..Element::default()
    };
    let mut self_closed = false;
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        if b[i] == b'>' {
            i += 1;
            break;
        }
        if b[i] == b'/' {
            self_closed = b.get(i + 1) == Some(&b'>');
            i += 1;
            continue;
        }
        let key_start = i;
        while i < b.len()
            && !b[i].is_ascii_whitespace()
            && b[i] != b'='
            && b[i] != b'>'
            && b[i] != b'/'
        {
            i += 1;
        }
        let key = html[key_start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let quote = b[i];
                i += 1;
                let v_start = i;
                while i < b.len() && b[i] != quote {
                    i += 1;
                }
                value = decode(&html[v_start..i]);
                i = (i + 1).min(b.len());
            } else {
                let v_start = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                value = decode(&html[v_start..i]);
            }
        }
        if !key.is_empty() {
            el.attrs.push((key, value));
        }
    }
    (el, self_closed, i)
}

const ENTITIES: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", "\u{a0}"),
    ("ndash", "–"),
    ("mdash", "—"),
    ("hellip", "…"),
    ("minus", "−"),
    ("times", "×"),
    ("divide", "÷"),
    ("thinsp", "\u{2009}"),
    ("ensp", "\u{2002}"),
    ("emsp", "\u{2003}"),
    ("hairsp", "\u{200a}"),
    ("lrm", ""),
    ("rlm", ""),
    ("zwj", ""),
    ("zwnj", ""),
    ("shy", ""),
    ("larr", "←"),
    ("rarr", "→"),
    ("uarr", "↑"),
    ("darr", "↓"),
    ("harr", "↔"),
    ("rArr", "⇒"),
    ("prime", "′"),
    ("Prime", "″"),
    ("deg", "°"),
    ("plusmn", "±"),
    ("middot", "·"),
    ("bull", "•"),
    ("copy", "©"),
    ("reg", "®"),
    ("trade", "™"),
    ("lsquo", "‘"),
    ("rsquo", "’"),
    ("ldquo", "“"),
    ("rdquo", "”"),
    ("laquo", "«"),
    ("raquo", "»"),
    ("sect", "§"),
    ("para", "¶"),
    ("micro", "µ"),
    ("frac12", "½"),
    ("frac14", "¼"),
    ("frac34", "¾"),
    ("sup2", "²"),
    ("sup3", "³"),
    ("le", "≤"),
    ("ge", "≥"),
    ("ne", "≠"),
    ("asymp", "≈"),
    ("infin", "∞"),
    ("pi", "π"),
    ("alpha", "α"),
    ("beta", "β"),
    ("gamma", "γ"),
    ("delta", "δ"),
    ("theta", "θ"),
    ("lambda", "λ"),
    ("mu", "μ"),
    ("sigma", "σ"),
    ("omega", "ω"),
    ("Delta", "Δ"),
    ("Omega", "Ω"),
    ("euro", "€"),
    ("pound", "£"),
    ("yen", "¥"),
    ("cent", "¢"),
    ("dagger", "†"),
    ("Dagger", "‡"),
    ("sdot", "⋅"),
    ("radic", "√"),
    ("part", "∂"),
    ("sum", "∑"),
    ("int", "∫"),
];

/// Text with its character references resolved. An unknown one stays as it
/// was written.
pub fn decode(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let semi = after.find(';').filter(|n| *n <= 10 && *n > 0);
        let resolved = semi.and_then(|n| {
            let name = &after[..n];
            if let Some(num) = name.strip_prefix('#') {
                let code = match num.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => num.parse::<u32>().ok(),
                };
                code.and_then(char::from_u32).map(|c| c.to_string())
            } else {
                ENTITIES
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| v.to_string())
            }
        });
        match (resolved, semi) {
            (Some(text), Some(n)) => {
                out.push_str(&text);
                rest = &after[n + 1..];
            }
            _ => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(n: &Node) -> &Element {
        match n {
            Node::Element(e) => e,
            Node::Text(t) => panic!("text: {t}"),
        }
    }

    #[test]
    fn a_tree_comes_out_of_well_formed_html() {
        let root = parse(r#"<p id="a" class='x y'>One <b>two</b> &amp; three<br/>four</p>"#);
        let p = el(&root.children[0]);
        assert_eq!(p.name, "p");
        assert_eq!(p.attr("id"), Some("a"));
        assert!(p.has_class("y"));
        assert!(!p.has_class("z"));
        assert_eq!(p.text(), "One two & threefour");
        assert_eq!(p.children.len(), 5);
    }

    #[test]
    fn broken_html_never_fails() {
        for html in [
            "<p>unclosed <b>bold",
            "</p>stray</div>",
            "<a href=x>y</a",
            "<",
            "a < b and c > d",
            "<!-- never closed",
            "<style>p { color: red } </p>",
            "<p attr='unterminated>text</p>",
            "",
        ] {
            let root = parse(html);
            assert_eq!(root.name, "#root", "{html}");
        }
        assert_eq!(parse("a < b and c > d").text(), "a < b and c > d");
        assert_eq!(parse("<p>unclosed <b>bold").text(), "unclosed bold");
    }

    #[test]
    fn scripts_styles_and_comments_hold_no_text() {
        let root = parse("<style>.a{}</style>x<!-- c -->y<script>if (a<b) {}</script>z");
        assert_eq!(root.text(), "xyz");
    }

    #[test]
    fn character_references_resolve() {
        assert_eq!(
            decode("a &amp; b &lt; c &#233; &#x2014; &nbsp;|"),
            "a & b < c é — \u{a0}|"
        );
        assert_eq!(decode("AT&T &unknown; &"), "AT&T &unknown; &");
    }

    #[test]
    fn an_attribute_in_single_quotes_may_hold_double_ones() {
        let root = parse(r#"<span data-mw='{"a":"b > c"}' id="x">t</span>"#);
        let s = el(&root.children[0]);
        assert_eq!(s.attr("data-mw"), Some(r#"{"a":"b > c"}"#));
        assert_eq!(s.attr("id"), Some("x"));
    }
}
