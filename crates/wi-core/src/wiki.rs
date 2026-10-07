//! The Wiki tab's commands, `wiki.*` (docs/ASK.md): Wikipedia read live
//! through its public REST and Action APIs, and kept on this Mac once read.
//!
//! The web view can reach nothing but the app (its content security policy),
//! so every request is made here. `wi-wiki` turns the HTML into blocks of
//! text; this module fetches, caches and waits its turn.
//!
//! - **Who is asking.** Every request carries a User-Agent that names the
//!   app and where to reach its maker, as Wikimedia asks of clients.
//! - **Rate.** One request at a time, at least 100 ms apart. A 429 or a 503
//!   with `Retry-After` is obeyed: nothing is sent until that time passes.
//! - **Cache.** An article opened once is in `wiki-cache.sqlite` in the
//!   library folder, with its summary and its related list. It reopens from
//!   there at once, is read again from Wikipedia when it is over a week old,
//!   and is what you get when there is no connection. The file is a cache:
//!   deleting it loses nothing but that.

use std::io::Read;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use wi_wiki::{
    display_title, is_article_title, parse_article, plain_text, section_text, url_title,
};

use crate::args::Args;
use crate::bus::lock;
use crate::net::encode;
use crate::{CoreError, Inner};

/// Wikimedia's User-Agent policy asks for the client's name and version and
/// a way to reach whoever runs it.
pub(crate) const USER_AGENT: &str = concat!(
    "Wi_WWAV/",
    env!("CARGO_PKG_VERSION"),
    " (https://www.wi-wwav.com; Learn's Wikipedia reader) ureq/2"
);

const WIKIPEDIA: &str = "https://en.wikipedia.org";
const GAP: Duration = Duration::from_millis(100);
/// An article older than this is read again when there is a connection.
const FRESH_MS: f64 = 7.0 * 86_400_000.0;
const HTML_PROFILE: &str =
    "text/html; charset=utf-8; profile=\"https://www.mediawiki.org/wiki/Specs/HTML/2.8.0\"";

/// When the next request may go, and when Wikipedia said to stay away until.
struct Gate {
    last: Option<Instant>,
    blocked_until: Option<Instant>,
}

pub(crate) struct Wiki {
    base: String,
    agent: ureq::Agent,
    cache: Mutex<Connection>,
    gate: Mutex<Gate>,
}

/// Why a request got nothing.
enum Fail {
    Offline,
    NotFound,
    /// Wikipedia asked for a pause of this many seconds.
    SlowDown(u64),
    Status(u16),
}

impl Fail {
    fn error(&self, what: &str) -> CoreError {
        match self {
            Fail::Offline => CoreError::new(
                "offline",
                format!(
                    "Wikipedia can't be reached right now, and {what} isn't saved on this Mac."
                ),
            ),
            Fail::NotFound => CoreError::new(
                "not_found",
                format!("Wikipedia has no article called {what}."),
            ),
            Fail::SlowDown(s) => CoreError::new(
                "slow_down",
                format!("Wikipedia asked Learn to slow down. Try again in {s} seconds."),
            ),
            Fail::Status(code) => CoreError::new(
                "wikipedia",
                format!("Wikipedia answered {code}. Try again in a moment."),
            ),
        }
    }
}

/// A title as the cache keys it: spaces, and the first letter a capital, as
/// Wikipedia treats `fourier transform` and `Fourier transform` as one page.
fn key_of(title: &str) -> String {
    let shown = display_title(title);
    let mut chars = shown.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

impl Wiki {
    pub fn open(library: &Path, base: Option<&str>) -> Result<Wiki, CoreError> {
        let conn = Connection::open(library.join("wiki-cache.sqlite"))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS article (key TEXT PRIMARY KEY, title TEXT NOT NULL, json TEXT NOT NULL, fetched_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS alias   (key TEXT PRIMARY KEY, target TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS summary (key TEXT PRIMARY KEY, json TEXT NOT NULL, fetched_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS related (key TEXT PRIMARY KEY, json TEXT NOT NULL, fetched_at INTEGER NOT NULL);",
        )?;
        Ok(Wiki {
            base: base.unwrap_or(WIKIPEDIA).trim_end_matches('/').to_string(),
            agent: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(6))
                .timeout_read(Duration::from_secs(20))
                .redirects(5)
                .user_agent(USER_AGENT)
                .build(),
            cache: Mutex::new(conn),
            gate: Mutex::new(Gate {
                last: None,
                blocked_until: None,
            }),
        })
    }

    /// One GET, in its turn. The gate is held while the request runs, so
    /// requests go one at a time.
    fn get(&self, path: &str, accept: &str) -> Result<String, Fail> {
        let mut gate = lock(&self.gate);
        if let Some(until) = gate.blocked_until {
            let left = until.saturating_duration_since(Instant::now());
            if !left.is_zero() {
                return Err(Fail::SlowDown(left.as_secs().max(1)));
            }
            gate.blocked_until = None;
        }
        if let Some(last) = gate.last {
            let since = last.elapsed();
            if since < GAP {
                std::thread::sleep(GAP - since);
            }
        }
        gate.last = Some(Instant::now());
        let url = format!("{}{path}", self.base);
        match self.agent.get(&url).set("Accept", accept).call() {
            Ok(resp) => {
                let mut text = String::new();
                resp.into_reader()
                    .take(48 << 20)
                    .read_to_string(&mut text)
                    .map_err(|_| Fail::Offline)?;
                Ok(text)
            }
            Err(ureq::Error::Status(404, _)) => Err(Fail::NotFound),
            Err(ureq::Error::Status(code @ (429 | 503), resp)) => {
                let secs = resp
                    .header("retry-after")
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .unwrap_or(if code == 429 { 30 } else { 5 })
                    .clamp(1, 600);
                gate.blocked_until = Some(Instant::now() + Duration::from_secs(secs));
                Err(Fail::SlowDown(secs))
            }
            Err(ureq::Error::Status(code, _)) => Err(Fail::Status(code)),
            Err(ureq::Error::Transport(_)) => Err(Fail::Offline),
        }
    }

    fn api(&self, query: &str) -> Result<Value, Fail> {
        let text = self.get(
            &format!("/w/api.php?format=json&formatversion=2&{query}"),
            "application/json",
        )?;
        serde_json::from_str(&text).map_err(|_| Fail::Status(502))
    }

    fn cached(&self, table: &str, key: &str) -> Option<(Value, f64)> {
        let conn = lock(&self.cache);
        // An article may be kept under the title it redirects to.
        let key = if table == "article" {
            conn.query_row("SELECT target FROM alias WHERE key = ?1", [key], |r| {
                r.get::<_, String>(0)
            })
            .optional()
            .ok()
            .flatten()
            .unwrap_or_else(|| key.to_string())
        } else {
            key.to_string()
        };
        conn.query_row(
            &format!("SELECT json, fetched_at FROM {table} WHERE key = ?1"),
            [&key],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()
        .ok()
        .flatten()
        .and_then(|(text, at)| Some((serde_json::from_str(&text).ok()?, at as f64)))
    }

    fn keep(&self, table: &str, key: &str, value: &Value, now: f64) {
        let conn = lock(&self.cache);
        let _ = conn.execute(
            &format!("INSERT OR REPLACE INTO {table} (key, json, fetched_at) VALUES (?1, ?2, ?3)"),
            params![key, value.to_string(), now as i64],
        );
    }

    fn keep_article(&self, asked: &str, article: &Value, now: f64) {
        let title = article["title"].as_str().unwrap_or(asked);
        let key = key_of(title);
        let conn = lock(&self.cache);
        let _ = conn.execute(
            "INSERT OR REPLACE INTO article (key, title, json, fetched_at) VALUES (?1, ?2, ?3, ?4)",
            params![key, title, article.to_string(), now as i64],
        );
        if key_of(asked) != key {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO alias (key, target) VALUES (?1, ?2)",
                params![key_of(asked), key],
            );
        }
    }

    /// The article called `title`, from the cache when it is there and
    /// fresh, else from Wikipedia, else from the cache however old.
    fn article(&self, title: &str, refresh: bool, now: f64) -> Result<Value, CoreError> {
        let key = key_of(title);
        if key.is_empty() {
            return Err(CoreError::new(
                "bad_args",
                "wiki.article needs title, the article's name.",
            ));
        }
        if !is_article_title(&key) {
            return Err(CoreError::new(
                "refused",
                format!("'{key}' is not an article. The Wiki tab reads articles only."),
            ));
        }
        let have = self.cached("article", &key);
        if let Some((article, at)) = &have {
            if !refresh && now - at < FRESH_MS {
                return Ok(json!({"article": article, "fetchedAt": at, "cached": true}));
            }
        }
        let path = format!("/api/rest_v1/page/html/{}?redirect=true", url_title(&key));
        match self.get(&path, HTML_PROFILE) {
            Ok(html) => {
                let mut article = parse_article(&html, &key);
                let shown = article["title"].as_str().unwrap_or(&key).to_string();
                article["url"] = json!(format!(
                    "https://en.wikipedia.org/wiki/{}",
                    url_title(&shown)
                ));
                self.keep_article(&key, &article, now);
                Ok(json!({"article": article, "fetchedAt": now, "cached": false}))
            }
            Err(fail) => match have {
                // No connection, or Wikipedia is busy: what was read before still reads.
                Some((article, at)) if !matches!(fail, Fail::NotFound) => Ok(json!({
                    "article": article, "fetchedAt": at, "cached": true,
                    "offline": matches!(fail, Fail::Offline),
                })),
                _ => Err(fail.error(&format!("'{key}'"))),
            },
        }
    }

    fn summary(&self, title: &str, now: f64) -> Result<Value, CoreError> {
        let key = key_of(title);
        let have = self.cached("summary", &key);
        if let Some((s, at)) = &have {
            if now - at < FRESH_MS {
                return Ok(s.clone());
            }
        }
        let path = format!(
            "/api/rest_v1/page/summary/{}?redirect=true",
            url_title(&key)
        );
        match self.get(&path, "application/json") {
            Ok(text) => {
                let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                let shown = v["title"].as_str().unwrap_or(&key).to_string();
                let s = json!({
                    "title": shown,
                    "description": v["description"].as_str().unwrap_or(""),
                    "extract": v["extract"].as_str().unwrap_or(""),
                    "disambiguation": v["type"] == "disambiguation",
                });
                self.keep("summary", &key, &s, now);
                Ok(s)
            }
            Err(fail) => {
                if let Some((s, _)) = have {
                    return Ok(s);
                }
                // An article saved here can still say its first paragraph.
                if let Some((article, _)) = self.cached("article", &key) {
                    let extract = section_text(&article, "", 600).unwrap_or_default();
                    let extract = extract
                        .split("\n\n")
                        .find(|p| p.len() > 80)
                        .unwrap_or(&extract)
                        .to_string();
                    return Ok(json!({
                        "title": article["title"], "description": article["description"],
                        "extract": extract, "disambiguation": false,
                    }));
                }
                Err(fail.error(&format!("'{key}'")))
            }
        }
    }

    /// Titles with their one-line descriptions, from a generator query.
    fn pages(&self, query: &str) -> Result<Vec<Value>, Fail> {
        let v = self.api(query)?;
        let mut pages: Vec<&Value> = v["query"]["pages"]
            .as_array()
            .into_iter()
            .flatten()
            .collect();
        pages.sort_by_key(|p| p["index"].as_i64().unwrap_or(i64::MAX));
        Ok(pages
            .into_iter()
            .filter_map(|p| {
                let title = p["title"].as_str()?;
                is_article_title(title).then(|| {
                    json!({"title": title, "description": p["description"].as_str().unwrap_or("")})
                })
            })
            .collect())
    }

    fn related(&self, title: &str, now: f64) -> Result<Value, CoreError> {
        let key = key_of(title);
        let have = self.cached("related", &key);
        if let Some((r, at)) = &have {
            if now - at < FRESH_MS {
                return Ok(json!({"results": r}));
            }
        }
        let query = format!(
            "action=query&generator=search&gsrsearch={}&gsrlimit=8&gsrnamespace=0&prop=description",
            encode(&format!("morelike:{key}"))
        );
        match self.pages(&query) {
            Ok(mut list) => {
                list.retain(|p| p["title"].as_str().is_some_and(|t| key_of(t) != key));
                list.truncate(8);
                let list = Value::Array(list);
                self.keep("related", &key, &list, now);
                Ok(json!({"results": list}))
            }
            Err(fail) => match have {
                Some((r, _)) => Ok(json!({"results": r, "offline": true})),
                None if matches!(fail, Fail::Offline) => {
                    Ok(json!({"results": [], "offline": true}))
                }
                None => Err(fail.error("the related list")),
            },
        }
    }

    /// Articles saved here whose titles start with, or hold, what was typed.
    fn saved_titles(&self, q: &str, limit: usize) -> Vec<Value> {
        let conn = lock(&self.cache);
        let like = format!("%{}%", q.replace(['%', '_'], ""));
        let Ok(mut stmt) = conn.prepare(
            "SELECT title, json_extract(json, '$.description') FROM article WHERE title LIKE ?1
             ORDER BY (title LIKE ?2) DESC, fetched_at DESC LIMIT ?3",
        ) else {
            return Vec::new();
        };
        let prefix = format!("{}%", q.replace(['%', '_'], ""));
        stmt.query_map(params![like, prefix, limit as i64], |r| {
            Ok(json!({
                "title": r.get::<_, String>(0)?,
                "description": r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                "saved": true,
            }))
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default()
    }

    fn suggest(&self, q: &str) -> Result<Value, CoreError> {
        let q = q.trim();
        if q.is_empty() {
            return Ok(json!({"results": []}));
        }
        let query = format!(
            "action=query&generator=prefixsearch&gpssearch={}&gpslimit=8&gpsnamespace=0&prop=description&redirects=1",
            encode(q)
        );
        match self.pages(&query) {
            Ok(list) => Ok(json!({"results": list})),
            // Offline, the box still finds what is saved on this Mac.
            Err(Fail::Offline) => Ok(json!({"results": self.saved_titles(q, 8), "offline": true})),
            Err(fail) => Err(fail.error("that search")),
        }
    }

    fn search(&self, q: &str, limit: usize) -> Result<Value, CoreError> {
        let q = q.trim();
        if q.is_empty() {
            return Ok(json!({"results": []}));
        }
        let query = format!(
            "action=query&list=search&srsearch={}&srlimit={}&srnamespace=0&srprop=snippet",
            encode(q),
            limit.clamp(1, 20)
        );
        match self.api(&query) {
            Ok(v) => {
                let results: Vec<Value> = v["query"]["search"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|r| {
                        let title = r["title"].as_str()?;
                        // The snippet marks matches with spans: only its words are kept.
                        let snippet = wi_wiki::html::parse(r["snippet"].as_str().unwrap_or("")).text();
                        Some(json!({"title": title, "snippet": snippet.split_whitespace().collect::<Vec<_>>().join(" ")}))
                    })
                    .collect();
                Ok(json!({"results": results}))
            }
            Err(Fail::Offline) => {
                Ok(json!({"results": self.saved_titles(q, limit), "offline": true}))
            }
            Err(fail) => Err(fail.error("that search")),
        }
    }

    fn saved(&self) -> Value {
        let conn = lock(&self.cache);
        let list: Vec<Value> = conn
            .prepare("SELECT title, fetched_at FROM article ORDER BY fetched_at DESC LIMIT 500")
            .and_then(|mut stmt| {
                stmt.query_map([], |r| {
                    Ok(json!({"title": r.get::<_, String>(0)?, "fetchedAt": r.get::<_, i64>(1)?}))
                })
                .map(|rows| rows.filter_map(Result::ok).collect())
            })
            .unwrap_or_default();
        json!({"articles": list})
    }
}

fn now_ms(i: &Inner) -> f64 {
    i.clock().now_ms
}

pub(crate) fn invoke(i: &Inner, cmd: &str, a: &Args) -> Result<Value, CoreError> {
    let w = &i.wiki;
    match cmd {
        "wiki.suggest" => w.suggest(a.opt_str("q").unwrap_or("")),
        "wiki.search" => w.search(
            a.opt_str("q").unwrap_or(""),
            a.opt_usize("limit")?.unwrap_or(8),
        ),
        "wiki.article" => w.article(
            a.str("title")?,
            a.opt_bool("refresh")?.unwrap_or(false),
            now_ms(i),
        ),
        "wiki.summary" => w.summary(a.str("title")?, now_ms(i)),
        "wiki.related" => w.related(a.str("title")?, now_ms(i)),
        "wiki.saved" => Ok(w.saved()),
        // An article as plain text, whole or one section: what Claude reads.
        "wiki.text" => {
            let got = w.article(a.str("title")?, false, now_ms(i))?;
            let article = &got["article"];
            let max = a
                .opt_usize("maxChars")?
                .unwrap_or(12_000)
                .clamp(200, 60_000);
            let text = match a.opt_str("section") {
                Some(section) => section_text(article, section, max).ok_or_else(|| {
                    CoreError::new(
                        "refused",
                        format!(
                            "'{}' has no section called '{section}'.",
                            article["title"].as_str().unwrap_or("")
                        ),
                    )
                })?,
                None => plain_text(article, max),
            };
            Ok(json!({
                "title": article["title"],
                "description": article["description"],
                "url": article["url"],
                "sections": article["sections"].as_array().map(|s| s.iter().map(|x| x["title"].clone()).collect::<Vec<_>>()),
                "text": text,
                "license": "Text from Wikipedia, CC BY-SA 4.0.",
            }))
        }
        // A link out of the reader: the system browser, never a window of the app.
        "wiki.open" => {
            let url = a.str("url")?;
            if !(url.starts_with("https://") || url.starts_with("http://"))
                || url.contains(char::is_whitespace)
            {
                return Err(CoreError::new(
                    "refused",
                    "Only a web address opens in the browser.",
                ));
            }
            (i.opener)(url)
                .map_err(|e| CoreError::new("io", format!("The browser didn't open: {e}")))?;
            Ok(json!({}))
        }
        _ => Err(CoreError::new(
            "unknown_command",
            format!("There is no command called '{cmd}'."),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_titles_first_letter_and_underscores_dont_make_two_pages() {
        assert_eq!(key_of("fourier_transform"), "Fourier transform");
        assert_eq!(key_of(" Fourier transform "), "Fourier transform");
        assert_eq!(key_of("iPhone"), "IPhone");
        assert_eq!(key_of("éclair"), "Éclair");
        assert_eq!(key_of(""), "");
    }

    #[test]
    fn the_user_agent_names_the_app_and_where_to_reach_it() {
        assert!(USER_AGENT.starts_with("Wi_WWAV/"));
        assert!(USER_AGENT.contains("https://www.wi-wwav.com"));
    }
}
