//! mi-wwav.com (docs/SPEC.md 9.7): one HTTP agent, the account's tokens in
//! the keychain and nowhere else, and the refresh rule. After a 401, or a 403
//! whose body is "Invalid Token" (docs/QUESTIONS.md #69), the client
//! refreshes once through `/api/auth/refresh` and asks again.

use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::bus::lock;
use crate::{CoreError, SecretStore};

/// The keychain item that holds the account's tokens.
const TOKENS: &str = "mi-wwav.com account";

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Tokens {
    pub access: String,
    pub refresh: String,
}

/// Why a request didn't get what it asked for.
#[derive(Debug)]
pub(crate) enum Fail {
    /// No connection at all.
    Offline,
    /// The server answered, with an error status.
    Status {
        status: u16,
        body: Value,
    },
    SignedOut,
}

impl Fail {
    /// The server's own sentence, when it gave one.
    pub fn sentence(&self) -> String {
        match self {
            Fail::Offline => "Needs a connection".to_string(),
            Fail::SignedOut => "Sign in to mi-wwav.com first.".to_string(),
            Fail::Status { status, body } => body
                .get("error")
                .and_then(|e| {
                    e.as_str()
                        .map(String::from)
                        .or_else(|| e.get("message")?.as_str().map(String::from))
                })
                .or_else(|| body.as_str().map(String::from))
                .unwrap_or_else(|| format!("mi-wwav.com answered {status}.")),
        }
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            Fail::Status { status, .. } => Some(*status),
            _ => None,
        }
    }

    pub fn code(&self) -> Option<&str> {
        match self {
            Fail::Status { body, .. } => body
                .get("code")
                .and_then(Value::as_str)
                .or_else(|| body.get("error")?.get("code")?.as_str()),
            _ => None,
        }
    }
}

impl From<Fail> for CoreError {
    fn from(f: Fail) -> CoreError {
        let code = match &f {
            Fail::Offline => "offline".to_string(),
            Fail::SignedOut => "signed_out".to_string(),
            Fail::Status { status, .. } => f.code().map_or(format!("http_{status}"), String::from),
        };
        CoreError::new(&code, f.sentence())
    }
}

/// A request's body.
pub(crate) enum Body<'a> {
    None,
    Json(&'a Value),
    Form(&'a [(&'a str, &'a str)]),
    Bytes(&'a [u8], &'a str),
}

pub(crate) struct Reply {
    pub status: u16,
    pub body: Value,
    pub etag: Option<String>,
}

pub(crate) struct Net {
    base: String,
    agent: ureq::Agent,
    secrets: Arc<dyn SecretStore>,
    refreshing: Mutex<()>,
}

/// Every answer read as JSON when it is, and as its text when it isn't.
fn read(resp: ureq::Response) -> Reply {
    let status = resp.status();
    let etag = resp.header("etag").map(String::from);
    let mut text = String::new();
    let _ = resp.into_reader().take(16 << 20).read_to_string(&mut text);
    let body = serde_json::from_str(&text).unwrap_or(Value::String(text));
    Reply { status, body, etag }
}

impl Net {
    pub fn new(base: &str, secrets: Arc<dyn SecretStore>) -> Net {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(120))
            .timeout_write(Duration::from_secs(120))
            .redirects(0)
            .build();
        Net {
            base: base.to_string(),
            agent,
            secrets,
            refreshing: Mutex::new(()),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else {
            format!("{}{path}", self.base)
        }
    }

    pub fn tokens(&self) -> Option<Tokens> {
        let text = self.secrets.get(TOKENS).ok().flatten()?;
        serde_json::from_str(&text).ok()
    }

    pub fn signed_in(&self) -> bool {
        self.tokens().is_some()
    }

    pub fn save_tokens(&self, t: &Tokens) -> Result<(), CoreError> {
        let text =
            serde_json::to_string(t).map_err(|e| CoreError::new("keychain", e.to_string()))?;
        self.secrets.set(TOKENS, &text).map_err(|e| {
            CoreError::new("keychain", format!("The keychain refused the sign-in: {e}"))
        })
    }

    pub fn forget(&self) -> Result<(), CoreError> {
        self.secrets
            .delete(TOKENS)
            .map_err(|e| CoreError::new("keychain", e))
    }

    /// One request, with `token` if given. Error statuses come back as
    /// replies; only a failed connection is an error here.
    pub fn send(
        &self,
        method: &str,
        path: &str,
        body: Body,
        token: Option<&str>,
    ) -> Result<Reply, Fail> {
        let mut req = self.agent.request(method, &self.url(path));
        if let Some(t) = token {
            req = req.set("Authorization", &format!("Bearer {t}"));
        }
        let sent = match body {
            Body::None => req.call(),
            Body::Json(v) => req.send_json(v),
            Body::Form(pairs) => req.send_form(pairs),
            Body::Bytes(bytes, kind) => {
                let req = if kind.is_empty() {
                    req
                } else {
                    req.set("Content-Type", kind)
                };
                req.send_bytes(bytes)
            }
        };
        match sent {
            Ok(resp) => Ok(read(resp)),
            Err(ureq::Error::Status(_, resp)) => Ok(read(resp)),
            Err(ureq::Error::Transport(_)) => Err(Fail::Offline),
        }
    }

    /// One calendar feed, read as bytes. No account, no token: the address is
    /// the only secret, and it is never in what fails. Redirects are followed
    /// (a feed link may move), up to five.
    pub fn fetch_feed(&self, address: &str) -> Result<Vec<u8>, Fail> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(30))
            .redirects(5)
            .user_agent("Wi_WWAV")
            .build();
        match agent.get(address).call() {
            Ok(resp) => {
                let mut bytes = Vec::new();
                resp.into_reader()
                    .take(16 << 20)
                    .read_to_end(&mut bytes)
                    .map_err(|_| Fail::Offline)?;
                Ok(bytes)
            }
            Err(ureq::Error::Status(status, _)) => Err(Fail::Status { status, body: Value::Null }),
            Err(ureq::Error::Transport(_)) => Err(Fail::Offline),
        }
    }

    /// A request that must succeed (2xx): any other answer is a [`Fail`].
    pub fn expect(
        &self,
        method: &str,
        path: &str,
        body: Body,
        token: Option<&str>,
    ) -> Result<Reply, Fail> {
        let r = self.send(method, path, body, token)?;
        if (200..300).contains(&r.status) {
            Ok(r)
        } else {
            Err(Fail::Status {
                status: r.status,
                body: r.body,
            })
        }
    }

    /// A request as the signed-in account, refreshing its token once if the
    /// server says it has expired.
    pub fn api(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, Fail> {
        let tokens = self.tokens().ok_or(Fail::SignedOut)?;
        let body_of = || body.map_or(Body::None, Body::Json);
        let r = self.send(method, path, body_of(), Some(&tokens.access))?;
        let r = if stale(r.status, &r.body) {
            let fresh = self.refresh(&tokens)?;
            self.send(method, path, body_of(), Some(&fresh.access))?
        } else {
            r
        };
        if (200..300).contains(&r.status) {
            Ok(r.body)
        } else {
            Err(Fail::Status {
                status: r.status,
                body: r.body,
            })
        }
    }

    /// A new pair from the refresh token. One refresh at a time: a caller
    /// that waited finds the pair another just fetched.
    fn refresh(&self, used: &Tokens) -> Result<Tokens, Fail> {
        let _one = lock(&self.refreshing);
        if let Some(now) = self.tokens() {
            if now.access != used.access {
                return Ok(now);
            }
        }
        let body = json!({"refreshToken": used.refresh});
        let r = self.send("POST", "/api/auth/refresh", Body::Json(&body), None)?;
        let pair = (
            r.body.get("token").and_then(Value::as_str),
            r.body.get("refreshToken").and_then(Value::as_str),
        );
        match (r.status, pair) {
            (200, (Some(access), Some(refresh))) => {
                let t = Tokens {
                    access: access.to_string(),
                    refresh: refresh.to_string(),
                };
                self.save_tokens(&t).map_err(|_| Fail::SignedOut)?;
                Ok(t)
            }
            // The refresh token is refused (missing, invalid or expired): sign in
            // again. Any other answer, a 429 included, says nothing about the
            // token: the server limits /api/auth/refresh per address, shared
            // with sign-in, so a busy school or home network can be refused
            // while every token is good. Keep the account and try again later.
            (400 | 401 | 403, _) => {
                let _ = self.forget();
                Err(Fail::SignedOut)
            }
            _ => Err(Fail::Status {
                status: r.status,
                body: r.body,
            }),
        }
    }
}

/// An expired token: a 401, or the server's 403 "Invalid Token" (#69).
fn stale(status: u16, body: &Value) -> bool {
    status == 401
        || (status == 403 && body.get("error").and_then(Value::as_str) == Some("Invalid Token"))
}

/// A query string's value, percent-encoded (RFC 3986 unreserved kept).
pub(crate) fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A query string's value, decoded; `+` is a space, as forms send it.
pub(crate) fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => match (hex(b[i + 1]), hex(b[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h * 16 + l);
                    i += 3;
                    continue;
                }
                _ => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_values_round_trip() {
        for s in [
            "http://127.0.0.1:5000/callback",
            "a b+c",
            "ü/?&=#",
            "",
            "%",
            "%4",
        ] {
            assert_eq!(decode(&encode(s)), s, "{s}");
        }
        assert_eq!(decode("a+b%20c"), "a b c");
        assert_eq!(decode("100%"), "100%");
    }

    #[test]
    fn an_expired_token_is_a_401_or_the_servers_403() {
        assert!(stale(401, &json!({"error": "Invalid Token"})));
        assert!(stale(403, &json!({"error": "Invalid Token"})));
        assert!(!stale(403, &json!({"error": "Not authorized"})));
        assert!(!stale(200, &json!({})));
    }
}
