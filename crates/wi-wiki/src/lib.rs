//! wi-wiki: a Wikipedia article as plain blocks of text (docs/ASK.md).
//!
//! The Wiki tab is a reader, not a browser: it shows an article's words in
//! Learn's own type and nothing else. [`parse_article`] takes the HTML that
//! Wikipedia's REST API serves for a page and gives back headings,
//! paragraphs, lists and tables as JSON, with every link sorted into one of
//! three kinds: another article (opens in the tab), a place in this article
//! (scrolls), or the web (opens in the system browser). Links to files,
//! templates, talk pages and pages that don't exist are plain text.
//!
//! Left out: images and their captions, infobox pictures, navigation boxes,
//! maintenance banners, edit links and the "citation needed" kind of note.
//! References are gathered into one list, for the collapsible section at the
//! bottom. Formulas become a line of text ([`tex::plain`]).
//!
//! No network and no clock here: `wi-core` fetches and caches, and this
//! crate only reads what it is handed, so it can be tested on saved pages.

mod article;
pub mod html;
pub mod tex;

pub use article::{parse_article, plain_text, section_text};

/// Namespaces whose pages aren't articles. A link into one is plain text.
const NOT_ARTICLES: &[&str] = &[
    "file",
    "image",
    "media",
    "template",
    "category",
    "help",
    "wikipedia",
    "wp",
    "special",
    "portal",
    "user",
    "draft",
    "module",
    "mediawiki",
    "timedtext",
    "book",
    "talk",
    "project",
    "gadget",
    "topic",
    "education program",
    "event",
];

/// Whether a page title names an article: not a file, a template, a talk
/// page or any other namespace.
pub fn is_article_title(title: &str) -> bool {
    let Some((space, rest)) = title.split_once(':') else {
        return !title.trim().is_empty();
    };
    if rest.starts_with(' ') || rest.is_empty() {
        // "Star Trek: The Next Generation": a colon in a title, not a namespace.
        return true;
    }
    let space = space.trim().to_lowercase().replace('_', " ");
    if space.ends_with(" talk") {
        return false;
    }
    !NOT_ARTICLES.contains(&space.as_str())
}

/// A title as Wikipedia shows it: underscores are spaces.
pub fn display_title(title: &str) -> String {
    title.replace('_', " ").trim().to_string()
}

/// A title as it goes in an address: spaces are underscores, and everything
/// that isn't plain is percent-encoded.
pub fn url_title(title: &str) -> String {
    let mut out = String::new();
    for b in title.trim().replace(' ', "_").bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'('
            | b')'
            | b','
            | b':'
            | b'!'
            | b'*'
            | b'\'' => out.push(b as char),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Undoes percent-encoding. Bytes that don't make text are dropped.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && s.is_char_boundary(i + 1) && s.is_char_boundary(i + 3)
        {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn articles_are_told_from_other_pages() {
        for yes in [
            "Fourier transform",
            "Star Trek: The Next Generation",
            "2001: A Space Odyssey",
            "C++",
            "Mission: Impossible",
        ] {
            assert!(is_article_title(yes), "{yes}");
        }
        for no in [
            "File:Example.png",
            "Template:Cite web",
            "Talk:Fourier transform",
            "User talk:Example",
            "Category:Mathematics",
            "Help:Contents",
            "Wikipedia:About",
            "Special:Search",
            "Portal:Science",
            "",
        ] {
            assert!(!is_article_title(no), "{no}");
        }
    }

    #[test]
    fn titles_go_into_addresses_and_back() {
        assert_eq!(url_title("Fourier transform"), "Fourier_transform");
        assert_eq!(
            url_title("Erdős–Rényi model"),
            "Erd%C5%91s%E2%80%93R%C3%A9nyi_model"
        );
        assert_eq!(url_title("AC/DC"), "AC%2FDC");
        assert_eq!(url_title("What?"), "What%3F");
        assert_eq!(
            percent_decode("Erd%C5%91s%E2%80%93R%C3%A9nyi_model"),
            "Erdős–Rényi_model"
        );
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%zz"), "%zz");
    }
}
