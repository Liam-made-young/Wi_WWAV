//! One account (docs/SPEC.md 2.4, 9.7, 9.8). Sign-in happens in the system
//! browser: the app opens mi-wwav.com's desktop authorization page with a
//! PKCE challenge and listens on a loopback address for the answer (RFC
//! 8252). The password never reaches the app. The answer must carry the
//! state the app sent; then the code and the verifier go to the token
//! endpoint, and the tokens go to the keychain and nowhere else.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::net::{decode, encode, Body, Fail, Tokens};
use crate::{CoreError, Inner};

pub const CLIENT_ID: &str = "wi-wwav-desktop";
/// How long the browser has to hand the sign-in back.
const WAIT: Duration = Duration::from_secs(300);
const ACCOUNT: &str = "account";

fn random(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// RFC 7636: a 43-character verifier and its S256 challenge.
pub fn pkce_pair() -> (String, String) {
    let verifier = random(32);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

pub(crate) fn status(i: &Inner) -> Result<Value, CoreError> {
    let signed_in = i.net.signed_in();
    let known = i.kv.get(ACCOUNT)?.unwrap_or(Value::Null);
    if !signed_in {
        return Ok(json!({"signedIn": false}));
    }
    Ok(json!({"signedIn": true, "username": known["username"], "galaxy": known["galaxy"]}))
}

/// The browser's answer on the loopback: the query of the one request.
fn answer(stream: &mut TcpStream) -> std::io::Result<Vec<(String, String)>> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8_lossy(&buf);
    let target = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("");
    let query = target.split_once('?').map_or("", |(_, q)| q);
    Ok(query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect())
}

fn reply(stream: &mut TcpStream, sentence: &str) {
    let page =
        format!("<!doctype html><meta charset=\"utf-8\"><title>Wi_WWAV</title><p>{sentence}</p>");
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{page}",
        page.len()
    );
}

/// Waits for the browser to come back to the loopback address.
fn wait_for_answer(
    i: &Inner,
    listener: &TcpListener,
) -> Result<(TcpStream, Vec<(String, String)>), CoreError> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + WAIT;
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false)?;
                let query = answer(&mut stream)?;
                // A browser asking for a favicon isn't the answer.
                if query.is_empty() {
                    reply(&mut stream, "Waiting for the sign-in…");
                    continue;
                }
                return Ok((stream, query));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline || i.closing() {
                    return Err(CoreError::new(
                        "sign_in_timed_out",
                        "The browser didn't finish signing in. Try again.",
                    ));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

pub(crate) fn sign_in(i: &Inner) -> Result<Value, CoreError> {
    let Ok(_one) = i.signing_in.try_lock() else {
        return Err(CoreError::new(
            "signing_in",
            "A sign-in is already open in your browser.",
        ));
    };
    let (verifier, challenge) = pkce_pair();
    let state = random(16);
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let redirect = format!(
        "http://127.0.0.1:{}/callback",
        listener.local_addr()?.port()
    );
    let url = format!(
        "{}/oauth/desktop/authorize?response_type=code&client_id={CLIENT_ID}&redirect_uri={}&code_challenge={challenge}&code_challenge_method=S256&state={state}",
        i.net.base(),
        encode(&redirect),
    );
    (i.opener)(&url).map_err(|e| {
        CoreError::new(
            "no_browser",
            format!("Wi_WWAV couldn't open your browser: {e}"),
        )
    })?;
    let (mut stream, query) = wait_for_answer(i, &listener)?;
    let get = |k: &str| {
        query
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    };
    if get("state") != Some(state.as_str()) {
        reply(
            &mut stream,
            "This sign-in didn't come from Wi_WWAV, so it was ignored.",
        );
        return Err(CoreError::new(
            "state_mismatch",
            "That sign-in didn't come from this window, so Wi_WWAV ignored it. Try again.",
        ));
    }
    if get("error").is_some() {
        reply(&mut stream, "Sign-in cancelled. You can close this tab.");
        return Err(CoreError::new(
            "sign_in_cancelled",
            "Sign-in was cancelled.",
        ));
    }
    let Some(code) = get("code").map(String::from) else {
        reply(
            &mut stream,
            "Something went wrong. Try signing in again from Wi_WWAV.",
        );
        return Err(CoreError::new(
            "sign_in_failed",
            "The browser came back without a sign-in.",
        ));
    };
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("code_verifier", verifier.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("client_id", CLIENT_ID),
        ("state", state.as_str()),
    ];
    let tokens = match i
        .net
        .expect("POST", "/oauth/desktop/token", Body::Form(&form), None)
    {
        Ok(r) => r.body,
        Err(e) => {
            reply(
                &mut stream,
                "Signing in didn't work. Try again from Wi_WWAV.",
            );
            return Err(e.into());
        }
    };
    let (Some(access), Some(refresh)) = (
        tokens.get("access_token").and_then(Value::as_str),
        tokens.get("refresh_token").and_then(Value::as_str),
    ) else {
        reply(
            &mut stream,
            "Signing in didn't work. Try again from Wi_WWAV.",
        );
        return Err(CoreError::new(
            "sign_in_failed",
            "mi-wwav.com answered without a token.",
        ));
    };
    i.net.save_tokens(&Tokens {
        access: access.to_string(),
        refresh: refresh.to_string(),
    })?;
    reply(
        &mut stream,
        "Signed in. You can close this tab and go back to Wi_WWAV.",
    );
    drop(stream);
    let username = refresh_profile(i)?;
    i.poke();
    Ok(json!({"signedIn": true, "username": username}))
}

/// Who the account is, and its galaxy, kept beside the library (never the
/// token).
pub(crate) fn refresh_profile(i: &Inner) -> Result<Value, CoreError> {
    let me = i.net.api("GET", "/api/auth/me", None)?;
    let galaxy = match i.net.api("GET", "/api/v2/galaxies/mine", None) {
        Ok(v) => v["data"]["galaxy"]["slug"].clone(),
        Err(Fail::Status { status: 404, .. }) => Value::Null,
        Err(e) => return Err(e.into()),
    };
    let username = me["username"].clone();
    i.kv.set(ACCOUNT, &json!({"username": username, "galaxy": galaxy}))?;
    i.bus.emit(
        "account",
        json!({"signedIn": true, "username": username, "galaxy": galaxy}),
    );
    Ok(username)
}

pub(crate) fn sign_out(i: &Inner) -> Result<Value, CoreError> {
    i.net.forget()?;
    i.kv.delete(ACCOUNT)?;
    i.bus.emit("account", json!({"signedIn": false}));
    Ok(json!({"signedIn": false}))
}
