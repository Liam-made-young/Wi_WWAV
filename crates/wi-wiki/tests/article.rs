//! An article as the Wiki tab needs it, from HTML shaped like the REST
//! API's: links sorted into their three kinds, the clutter gone, references
//! gathered, and nothing that could run or load in what comes out.

use serde_json::{json, Value};
use wi_wiki::{parse_article, plain_text, section_text};

const PAGE: &str = r##"<!DOCTYPE html>
<html prefix="dc: http://purl.org/dc/terms/ mw: http://mediawiki.org/rdf/" about="//en.wikipedia.org/wiki/Special:Redirect/revision/1377137736">
<head><meta charset="utf-8"/><meta property="dc:modified" content="2026-09-28T01:44:15Z"/><title>Fourier transform</title>
<base href="//en.wikipedia.org/wiki/"/><link rel="stylesheet" href="/w/load.php?x"/></head>
<body class="mw-body-content">
<section data-mw-section-id="0" id="mwAQ">
<div class="shortdescription nomobile noexcerpt noprint searchaux" style="display:none">Mathematical transform</div>
<style data-mw-deduplicate="TemplateStyles:r1" typeof="mw:Extension/templatestyles">.hatnote{font-style:italic}</style>
<div role="note" class="hatnote navigation-not-searchable">Not to be confused with <a rel="mw:WikiLink" href="./Fourier_series" title="Fourier series">Fourier series</a>.</div>
<table class="infobox vcard"><tbody>
<tr><th colspan="2" class="infobox-above">Fourier transform</th></tr>
<tr><td colspan="2" class="infobox-image"><span typeof="mw:File"><a href="./File:Plot.png" class="mw-file-description"><img src="//upload.wikimedia.org/plot.png" class="mw-file-element"/></a></span><div class="infobox-caption">A plot</div></td></tr>
<tr><th class="infobox-label">Field</th><td class="infobox-data"><a rel="mw:WikiLink" href="./Harmonic_analysis">Harmonic analysis</a></td></tr>
</tbody></table>
<p>In <a rel="mw:WikiLink" href="./Mathematics" title="Mathematics">mathematics</a>, the <b>Fourier transform</b> (<b>FT</b>) is an
<a rel="mw:WikiLink" href="./Integral_transform#Definition" title="Integral transform">integral transform</a><sup about="#mwt9" class="mw-ref reference" id="cite_ref-1" rel="dc:references" typeof="mw:Extension/ref"><a href="./Fourier_transform#cite_note-1"><span class="mw-reflink-text"><span class="cite-bracket">[</span>1<span class="cite-bracket">]</span></span></a></sup>
written <span class="mwe-math-element mwe-math-element-inline"><span class="mwe-math-mathml-inline mwe-math-mathml-a11y" style="display: none;"><math xmlns="http://www.w3.org/1998/Math/MathML" alttext="{\displaystyle {\hat {f}}(\xi )}"><semantics><mrow><mi>f</mi></mrow><annotation encoding="application/x-tex">{\displaystyle {\hat {f}}(\xi )}</annotation></semantics></math></span><img src="https://wikimedia.org/api/rest_v1/media/math/render/svg/abc" class="mwe-math-fallback-image-inline" alt="{\displaystyle {\hat {f}}(\xi )}"/></span>.
See <a rel="mw:WikiLink" href="./File:Plot.png">the plot</a>, <a rel="mw:WikiLink" href="./Template:Cite_web">a template</a>,
<a rel="mw:WikiLink" href="./Talk:Fourier_transform">the talk page</a>, <a rel="mw:WikiLink" href="./Nonexistent_page?action=edit&amp;redlink=1" class="new">a page nobody wrote</a>,
<a rel="mw:WikiLink" href="./Fourier_transform#Definition" class="mw-selflink-fragment">the definition below</a>,
<a rel="mw:ExtLink nofollow" href="https://example.org/paper.pdf" class="external text">a paper</a>,
<a rel="mw:ExtLink" href="//example.org/relative">a protocol-relative link</a>,
<a href="javascript:alert(1)">a script</a> and
<a rel="mw:WikiLink" href="./Erd%C5%91s%E2%80%93R%C3%A9nyi_model">Erdős–Rényi</a>.<sup class="noprint Inline-Template Template-Fact">[<i>citation needed</i>]</sup></p>
<figure class="mw-default-size" typeof="mw:File/Thumb"><a href="./File:Wave.png"><img src="//upload.wikimedia.org/wave.png"/></a><figcaption>A wave</figcaption></figure>
</section>
<section data-mw-section-id="1"><h2 id="Definition">Definition<span class="mw-editsection">[<a href="/w/index.php?action=edit">edit</a>]</span></h2>
<p>The transform is</p>
<dl><dd><span class="mwe-math-element mwe-math-element-block"><math display="block"><semantics><mrow/><annotation encoding="application/x-tex">{\displaystyle x^{2}}</annotation></semantics></math></span></dd></dl>
<ul><li>One <i>item</i><ul><li>Nested</li></ul></li><li>Two</li></ul>
<table class="wikitable"><caption>Pairs</caption><tbody>
<tr><th>Function</th><th>Transform</th></tr>
<tr><td>rect</td><td>sinc <script>alert(1)</script></td></tr>
</tbody></table>
<section data-mw-section-id="2"><h3 id="History">History</h3><p>Joseph Fourier, 1822.</p></section>
</section>
<section data-mw-section-id="3"><h2 id="See_also">See also</h2>
<div class="navbox"><table><tbody><tr><td><a rel="mw:WikiLink" href="./Laplace_transform">Laplace transform</a></td></tr></tbody></table></div>
</section>
<section data-mw-section-id="4"><h2 id="References">References</h2>
<div class="mw-references-wrap"><ol class="mw-references references">
<li about="#cite_note-1" id="cite_note-1"><span class="mw-cite-backlink"><a href="./Fourier_transform#cite_ref-1"><span class="mw-linkback-text">↑ </span></a></span> <span id="mw-reference-text-cite_note-1" class="mw-reference-text reference-text">Stein &amp; Weiss, <a rel="mw:ExtLink" href="https://example.org/book">Introduction</a>, 1971.</span></li>
</ol></div></section>
<div role="navigation" class="navbox authority-control">Authority control</div>
</body></html>"##;

fn article() -> Value {
    parse_article(PAGE, "Fourier_transform")
}

/// Every inline mark of kind `t` anywhere in the article.
fn marks(v: &Value, t: &str, out: &mut Vec<Value>) {
    match v {
        Value::Array(items) => items.iter().for_each(|i| marks(i, t, out)),
        Value::Object(m) => {
            if m.get("t").and_then(Value::as_str) == Some(t) {
                out.push(v.clone());
            }
            m.values().for_each(|i| marks(i, t, out));
        }
        _ => {}
    }
}

fn all(t: &str) -> Vec<Value> {
    let mut out = Vec::new();
    marks(&article(), t, &mut out);
    out
}

#[test]
fn the_head_of_the_article() {
    let a = article();
    assert_eq!(a["title"], "Fourier transform");
    assert_eq!(a["description"], "Mathematical transform");
    assert_eq!(a["revision"], "1377137736");
    assert_eq!(a["modified"], "2026-09-28T01:44:15Z");
}

#[test]
fn it_opens_with_its_note_then_its_first_sentence_then_the_infobox() {
    let a = article();
    let kinds: Vec<&str> = a["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["t"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["note", "p", "table", "h", "p", "math", "ul", "table", "h", "p"]
    );
    assert_eq!(a["blocks"][2]["box"], true);
    // The picture's row is gone from the infobox; the facts stay.
    let rows = a["blocks"][2]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1][0], json!({"h": true, "c": ["Field"]}));
}

#[test]
fn links_to_articles_are_links_and_the_rest_are_not() {
    let titles: Vec<String> = all("a")
        .iter()
        .map(|l| l["title"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        titles,
        [
            "Fourier series",
            "Mathematics",
            "Integral transform",
            "Erdős–Rényi model",
            "Harmonic analysis"
        ]
    );
    let frag = all("a")
        .into_iter()
        .find(|l| l["title"] == "Integral transform")
        .unwrap();
    assert_eq!(frag["frag"], "Definition");
    // A file, a template, a talk page and a page nobody wrote are plain text.
    let text = plain_text(&article(), 100_000);
    for words in [
        "the plot",
        "a template",
        "the talk page",
        "a page nobody wrote",
    ] {
        assert!(text.contains(words), "{words}");
    }
    // The navigation box's link is gone with the box.
    assert!(!titles.contains(&"Laplace transform".to_string()));
}

#[test]
fn a_link_into_this_article_jumps_and_a_web_link_leaves() {
    assert_eq!(
        all("j"),
        [json!({"t": "j", "frag": "Definition", "c": ["the definition below"]})]
    );
    let hrefs: Vec<String> = all("x")
        .iter()
        .map(|l| l["href"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        hrefs,
        [
            "https://example.org/paper.pdf",
            "https://example.org/relative",
            "https://example.org/book"
        ]
    );
}

#[test]
fn nothing_that_runs_or_loads_comes_through() {
    let json = article().to_string();
    for bad in [
        "javascript:",
        "<script",
        "alert(1)",
        "<img",
        "upload.wikimedia.org",
        "<style",
        "font-style",
        "A wave",
        "A plot",
        "citation needed",
        "edit",
        "Authority control",
    ] {
        assert!(!json.contains(bad), "{bad} is in the article");
    }
    // The script link's words stay, as plain text.
    assert!(plain_text(&article(), 100_000).contains("a script"));
}

#[test]
fn formulas_are_text() {
    assert_eq!(all("math")[0], json!({"t": "math", "s": "f̂(ξ)"}));
    let a = article();
    assert_eq!(a["blocks"][5], json!({"t": "math", "text": "x²"}));
}

#[test]
fn references_are_gathered_and_their_section_is_dropped() {
    let a = article();
    assert_eq!(
        all("ref"),
        [json!({"t": "ref", "n": "1", "id": "cite_note-1"})]
    );
    assert_eq!(a["refs"].as_array().unwrap().len(), 1);
    assert_eq!(a["refs"][0]["id"], "cite_note-1");
    assert_eq!(
        a["refs"][0]["c"],
        json!(["Stein & Weiss, ", {"t": "x", "href": "https://example.org/book", "c": ["Introduction"]}, ", 1971."])
    );
    // "See also" held only a navigation box and "References" only the list.
    let titles: Vec<&str> = a["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["Definition", "History"]);
    assert_eq!(a["sections"][0]["number"], "1");
    assert_eq!(a["sections"][1]["number"], "1.1");
    assert_eq!(a["sections"][1]["level"], 2);
}

#[test]
fn lists_nest_and_tables_keep_their_cells() {
    let a = article();
    let list = &a["blocks"][6];
    assert_eq!(
        list["items"][0]["c"],
        json!(["One ", {"t": "i", "c": ["item"]}])
    );
    assert_eq!(
        list["items"][0]["sub"][0]["items"][0]["c"],
        json!(["Nested"])
    );
    let table = &a["blocks"][7];
    assert_eq!(table["caption"], json!(["Pairs"]));
    assert_eq!(table["rows"][0][1], json!({"h": true, "c": ["Transform"]}));
    assert_eq!(table["rows"][1][1], json!({"c": ["sinc"]}));
}

#[test]
fn claude_reads_it_as_plain_text() {
    let a = article();
    let text = plain_text(&a, 100_000);
    assert!(text.starts_with("Not to be confused with Fourier series."));
    assert!(text.contains("## Definition"));
    assert!(text.contains("| rect | sinc |"));
    assert!(text.contains("- One item\n  - Nested"));
    let short = plain_text(&a, 40);
    assert_eq!(short.chars().count(), 41);
    assert!(short.ends_with('…'));

    let history = section_text(&a, "history", 10_000).unwrap();
    assert_eq!(history, "### History\n\nJoseph Fourier, 1822.");
    let definition = section_text(&a, "Definition", 10_000).unwrap();
    assert!(definition.contains("The transform is") && definition.contains("History"));
    let lead = section_text(&a, "", 10_000).unwrap();
    assert!(lead.contains("integral transform") && !lead.contains("Definition\n"));
    assert_eq!(section_text(&a, "Nowhere", 10_000), None);
}

#[test]
fn anything_at_all_reads_without_failing() {
    for html in [
        "",
        "<",
        "not html",
        "<table><tr><td>",
        "<ol class='references'><li>",
        "<h2></h2>",
        "<p><a rel='mw:WikiLink'>x</a></p>",
    ] {
        let a = parse_article(html, "Some title");
        assert_eq!(a["title"], "Some title");
        assert!(a["blocks"].is_array());
    }
}
