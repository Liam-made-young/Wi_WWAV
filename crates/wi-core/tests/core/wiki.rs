//! The Wiki tab's commands through the core, against a stand-in for
//! Wikipedia (docs/ASK.md). What a fail looks like:
//! - a request goes out without the User-Agent Wikimedia asks for, or after
//!   Wikipedia said to wait;
//! - an article opened once is asked for again, or is gone with the network;
//! - a link out of the reader opens anywhere but the system browser;
//! - anything that could run or load reaches the page.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::common::*;
use serde_json::{json, Value};
use wi_core::Core;

const FOURIER: &str = r##"<!DOCTYPE html><html about="//en.wikipedia.org/wiki/Special:Redirect/revision/42"><head><title>Fourier transform</title></head><body>
<section data-mw-section-id="0"><div class="shortdescription" style="display:none">Mathematical transform</div>
<p>The <b>Fourier transform</b> is an <a rel="mw:WikiLink" href="./Integral_transform">integral transform</a>.
See <a rel="mw:WikiLink" href="./File:Plot.png">a plot</a> and <a rel="mw:ExtLink" href="https://example.org/x">a paper</a>.<script>alert(1)</script></p></section>
<section data-mw-section-id="1"><h2 id="History">History</h2><p>Joseph Fourier, 1822.</p></section>
</body></html>"##;

struct Request {
    path: String,
    agent: String,
}

/// Wikipedia, for a test: answers by path, remembers what it was asked, and
/// can go away or ask for a pause.
struct Wikipedia {
    url: String,
    seen: Arc<Mutex<Vec<Request>>>,
    down: Arc<AtomicBool>,
    slow: Arc<AtomicBool>,
}

impl Wikipedia {
    fn start() -> Wikipedia {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (down, slow) = (
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        );
        let (s, d, sl) = (seen.clone(), down.clone(), slow.clone());
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let Ok(mut conn) = conn else { continue };
                let (s, d, sl) = (s.clone(), d.clone(), sl.clone());
                std::thread::spawn(move || {
                    let mut seen = Vec::new();
                    let mut buf = [0u8; 2048];
                    while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                        match conn.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => seen.extend_from_slice(&buf[..n]),
                        }
                    }
                    // Gone: the connection closes with nothing said.
                    if d.load(Ordering::SeqCst) {
                        return;
                    }
                    let head = String::from_utf8_lossy(&seen).to_string();
                    let path = head.split(' ').nth(1).unwrap_or("").to_string();
                    let headers: HashMap<String, String> = head
                        .lines()
                        .skip(1)
                        .filter_map(|l| l.split_once(": "))
                        .map(|(k, v)| (k.to_lowercase(), v.to_string()))
                        .collect();
                    s.lock().unwrap().push(Request {
                        path: path.clone(),
                        agent: headers.get("user-agent").cloned().unwrap_or_default(),
                    });
                    let (status, kind, extra, body) = if sl.load(Ordering::SeqCst) {
                        (
                            429,
                            "text/plain",
                            "Retry-After: 120\r\n",
                            "slow down".to_string(),
                        )
                    } else {
                        let (status, kind, body) = answer(&path);
                        (status, kind, "", body)
                    };
                    let out = format!(
                        "HTTP/1.1 {status} X\r\nContent-Type: {kind}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = conn.write_all(out.as_bytes());
                });
            }
        });
        Wikipedia {
            url,
            seen,
            down,
            slow,
        }
    }

    fn paths(&self) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.path.clone())
            .collect()
    }

    fn core(&self, setup: &Setup) -> Core {
        let mut config = setup.config(NO_SERVER, no_browser());
        config.wiki_url = Some(self.url.clone());
        Core::open(&setup.library(), config).unwrap()
    }
}

fn answer(path: &str) -> (u16, &'static str, String) {
    let json = "application/json";
    if path.starts_with("/api/rest_v1/page/html/Fourier_transform") {
        return (200, "text/html", FOURIER.to_string());
    }
    // A redirect page answers with the article it points at.
    if path.starts_with("/api/rest_v1/page/html/FT") {
        return (200, "text/html", FOURIER.to_string());
    }
    if path.starts_with("/api/rest_v1/page/summary/Integral_transform") {
        return (
            200,
            json,
            json!({"type": "standard", "title": "Integral transform", "description": "Mapping between function spaces",
                   "extract": "In mathematics, an integral transform is a type of transform that maps a function from its original function space into another function space via integration.",
                   "thumbnail": {"source": "https://upload.wikimedia.org/x.png"}})
            .to_string(),
        );
    }
    if path.starts_with("/api/rest_v1/page/") {
        return (404, json, json!({"type": "not_found"}).to_string());
    }
    if path.contains("generator=search") && path.contains("morelike%3AFourier%20transform") {
        return (
            200,
            json,
            json!({"query": {"pages": [
                {"title": "Laplace transform", "index": 2, "description": "Integral transform useful in probability"},
                {"title": "Fourier series", "index": 1, "description": "Decomposition of periodic functions"},
                {"title": "Fourier transform", "index": 3},
                {"title": "Template:Fourier", "index": 4},
            ]}})
            .to_string(),
        );
    }
    if path.contains("generator=prefixsearch") {
        return (
            200,
            json,
            json!({"query": {"pages": [
                {"title": "Fourier transform", "index": 1, "description": "Mathematical transform"},
                {"title": "Fourier series", "index": 2},
            ]}})
            .to_string(),
        );
    }
    if path.contains("list=search") {
        return (
            200,
            json,
            json!({"query": {"search": [
                {"title": "Fourier transform", "snippet": "the <span class=\"searchmatch\">Fourier</span> <span class=\"searchmatch\">transform</span> (FT) is an integral transform"},
            ]}})
            .to_string(),
        );
    }
    (404, json, "{}".to_string())
}

#[test]
fn an_article_is_read_once_then_kept_and_reads_with_the_network_off() {
    let wikipedia = Wikipedia::start();
    let setup = Setup::new();
    let core = wikipedia.core(&setup);
    let first = ok(&core, "wiki.article", json!({"title": "fourier_transform"}));
    assert_eq!(first["cached"], false);
    let a = &first["article"];
    assert_eq!(a["title"], "Fourier transform");
    assert_eq!(a["description"], "Mathematical transform");
    assert_eq!(a["url"], "https://en.wikipedia.org/wiki/Fourier_transform");
    assert_eq!(a["sections"][0]["title"], "History");
    // The link to an article is a link; the one to a file is plain text; the script is gone.
    let text = a.to_string();
    assert!(
        text.contains(r#"{"t":"a","title":"Integral transform","c":["integral transform"]}"#),
        "{text}"
    );
    assert!(text.contains(r#"". See a plot and ""#), "{text}");
    assert!(
        text.contains("a plot") && !text.contains("Plot.png") && !text.contains("alert"),
        "{text}"
    );

    // Who asked: the app, by name, with where to reach its maker.
    let seen = wikipedia.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert!(
        seen[0].agent.starts_with("Wi_WWAV/") && seen[0].agent.contains("https://www.wi-wwav.com"),
        "{}",
        seen[0].agent
    );
    assert_eq!(
        seen[0].path,
        "/api/rest_v1/page/html/Fourier_transform?redirect=true"
    );
    drop(seen);

    // Again, by another spelling of the title: from this Mac, with nothing asked.
    let again = ok(&core, "wiki.article", json!({"title": "Fourier transform"}));
    assert_eq!(again["cached"], true);
    assert_eq!(again["article"], first["article"]);
    assert_eq!(wikipedia.paths().len(), 1);

    // The network goes; the app is opened again; the article is still there.
    wikipedia.down.store(true, Ordering::SeqCst);
    drop(core);
    let core = wikipedia.core(&setup);
    let offline = ok(&core, "wiki.article", json!({"title": "Fourier transform"}));
    assert_eq!(offline["article"]["title"], "Fourier transform");
    assert_eq!(offline["cached"], true);
    let text = ok(
        &core,
        "wiki.text",
        json!({"title": "Fourier transform", "section": "History"}),
    );
    assert_eq!(text["text"], "## History\n\nJoseph Fourier, 1822.");
    assert_eq!(
        ok(&core, "wiki.saved", json!({}))["articles"][0]["title"],
        "Fourier transform"
    );
    // The search box still finds what is saved here.
    let found = ok(&core, "wiki.suggest", json!({"q": "four"}));
    assert_eq!(found["offline"], true);
    assert_eq!(found["results"][0]["title"], "Fourier transform");
    // One never opened says so, in a sentence.
    let (code, said) = refused(&core, "wiki.article", json!({"title": "Laplace transform"}));
    assert_eq!(code, "offline");
    assert_eq!(
        said,
        "Wikipedia can't be reached right now, and 'Laplace transform' isn't saved on this Mac."
    );
    // A refresh that can't reach Wikipedia answers with what is saved.
    let stale = ok(
        &core,
        "wiki.article",
        json!({"title": "Fourier transform", "refresh": true}),
    );
    assert_eq!(
        (stale["cached"].clone(), stale["offline"].clone()),
        (json!(true), json!(true))
    );
}

#[test]
fn a_redirect_is_kept_under_both_titles() {
    let wikipedia = Wikipedia::start();
    let setup = Setup::new();
    let core = wikipedia.core(&setup);
    assert_eq!(
        ok(&core, "wiki.article", json!({"title": "FT"}))["article"]["title"],
        "Fourier transform"
    );
    assert_eq!(
        ok(&core, "wiki.article", json!({"title": "FT"}))["cached"],
        true
    );
    assert_eq!(
        ok(&core, "wiki.article", json!({"title": "Fourier transform"}))["cached"],
        true
    );
    assert_eq!(wikipedia.paths().len(), 1);
}

#[test]
fn previews_related_articles_suggestions_and_search() {
    let wikipedia = Wikipedia::start();
    let setup = Setup::new();
    let core = wikipedia.core(&setup);
    // A link's preview: title and first paragraph, text only.
    let s = ok(
        &core,
        "wiki.summary",
        json!({"title": "Integral transform"}),
    );
    assert_eq!(s["title"], "Integral transform");
    assert!(s["extract"]
        .as_str()
        .unwrap()
        .starts_with("In mathematics, an integral transform"));
    assert!(!s.to_string().contains("upload.wikimedia.org"));
    ok(
        &core,
        "wiki.summary",
        json!({"title": "integral_transform"}),
    );
    assert_eq!(wikipedia.paths().len(), 1, "a preview is asked for once");

    // Related: in Wikipedia's order, without the article itself or non-articles.
    let r = ok(&core, "wiki.related", json!({"title": "Fourier transform"}));
    let titles: Vec<&str> = r["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["Fourier series", "Laplace transform"]);
    assert_eq!(
        r["results"][0]["description"],
        "Decomposition of periodic functions"
    );
    ok(&core, "wiki.related", json!({"title": "Fourier transform"}));
    assert_eq!(wikipedia.paths().len(), 2);

    let sug = ok(&core, "wiki.suggest", json!({"q": "fourier t"}));
    assert_eq!(
        sug["results"][0],
        json!({"title": "Fourier transform", "description": "Mathematical transform"})
    );
    assert!(wikipedia.paths()[2].contains("gpssearch=fourier%20t"));
    assert_eq!(
        ok(&core, "wiki.suggest", json!({"q": "  "}))["results"],
        json!([])
    );

    let found = ok(
        &core,
        "wiki.search",
        json!({"q": "what is a fourier transform", "limit": 3}),
    );
    assert_eq!(
        found["results"][0]["snippet"],
        "the Fourier transform (FT) is an integral transform"
    );
    assert!(wikipedia.paths()[3].contains("srlimit=3"));
}

#[test]
fn what_isnt_an_article_isnt_asked_for() {
    let wikipedia = Wikipedia::start();
    let setup = Setup::new();
    let core = wikipedia.core(&setup);
    let (code, said) = refused(&core, "wiki.article", json!({"title": "No such page"}));
    assert_eq!(
        (code.as_str(), said.as_str()),
        (
            "not_found",
            "Wikipedia has no article called 'No such page'."
        )
    );
    let (_, said) = refused(&core, "wiki.article", json!({"title": "File:Plot.png"}));
    assert_eq!(
        said,
        "'File:Plot.png' is not an article. The Wiki tab reads articles only."
    );
    let (_, said) = refused(
        &core,
        "wiki.text",
        json!({"title": "Fourier transform", "section": "Nowhere"}),
    );
    assert_eq!(said, "'Fourier transform' has no section called 'Nowhere'.");
    assert_eq!(wikipedia.paths().len(), 2, "the file was never requested");
}

#[test]
fn when_wikipedia_says_wait_nothing_more_is_sent() {
    let wikipedia = Wikipedia::start();
    let setup = Setup::new();
    let core = wikipedia.core(&setup);
    wikipedia.slow.store(true, Ordering::SeqCst);
    let (code, said) = refused(&core, "wiki.article", json!({"title": "Fourier transform"}));
    assert_eq!(code, "slow_down");
    assert_eq!(
        said,
        "Wikipedia asked Learn to slow down. Try again in 120 seconds."
    );
    // Wikipedia would answer now, but the wait it asked for isn't over.
    wikipedia.slow.store(false, Ordering::SeqCst);
    for _ in 0..3 {
        let (code, _) = refused(
            &core,
            "wiki.summary",
            json!({"title": "Integral transform"}),
        );
        assert_eq!(code, "slow_down");
    }
    assert_eq!(wikipedia.paths().len(), 1);
}

#[test]
fn a_link_out_opens_in_the_system_browser_and_only_a_web_address_does() {
    let wikipedia = Wikipedia::start();
    let setup = Setup::new();
    let opened: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = opened.clone();
    let mut config = setup.config(
        NO_SERVER,
        Arc::new(move |url: &str| {
            seen.lock().unwrap().push(url.to_string());
            Ok(())
        }),
    );
    config.wiki_url = Some(wikipedia.url.clone());
    let core = Core::open(&setup.library(), config).unwrap();
    ok(
        &core,
        "wiki.open",
        json!({"url": "https://example.org/paper.pdf"}),
    );
    assert_eq!(*opened.lock().unwrap(), ["https://example.org/paper.pdf"]);
    for bad in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "wi-wwav://x",
        "https://a b",
        "",
    ] {
        assert!(
            core.invoke("wiki.open", json!({"url": bad})).is_err(),
            "{bad}"
        );
    }
    assert_eq!(opened.lock().unwrap().len(), 1);
    let _: Value = json!(null);
}
