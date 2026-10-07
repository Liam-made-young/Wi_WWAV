//! Sign-in (docs/PLAN.md S1.11, the Linux part, against tools/mock-server;
//! docs/SPEC.md 2.4, 9.7, 9.8). What a fail looks like:
//! - the app sees a password (it never asks for one: the browser does);
//! - an answer on the loopback with another state is taken;
//! - the token lands anywhere but the secret store: the library folder,
//!   a result, an event;
//! - an expired token isn't refreshed, or signing out leaves it.

use std::sync::Arc;

use crate::common::*;
use serde_json::{json, Value};
use wi_core::{Core, SecretStore};

/// Every byte under `dir`, as text, to look for a token in.
fn everything_in(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(everything_in(&p));
        } else {
            out.push((p.display().to_string(), std::fs::read(&p).unwrap()));
        }
    }
    out
}

fn tokens(setup: &Setup) -> Value {
    let text = setup
        .secrets
        .get("mi-wwav.com account")
        .unwrap()
        .expect("the token is in the keychain");
    serde_json::from_str(&text).unwrap()
}

/// The page is opened on the sign-in address (www.wi-wwav.com), and the
/// code it hands back is redeemed on the server (mi-wwav.com). Fails if the
/// browser is sent to the server's address, or the token request to the
/// sign-in address, which has no token route.
#[test]
fn the_browser_opens_the_sign_in_address_and_the_code_goes_to_the_server() {
    let server = MockServer::start();
    let setup = Setup::new();
    let opened = Arc::new(std::sync::Mutex::new(Vec::new()));
    let browser = server.browser("lmy@mi-wwav.com", "WeWave-lmy1");
    let (seen, base) = (opened.clone(), server.url.clone());
    // One server answers the page on both addresses; nothing else lives on
    // the sign-in one.
    let opener: wi_core::Opener = Arc::new(move |url: &str| {
        seen.lock().unwrap().push(url.to_string());
        browser(&url.replace("https://www.wi-wwav.test", &base))
    });
    let mut config = setup.config(&server.url, opener);
    config.sign_in_url = "https://www.wi-wwav.test/".into();
    let core = Core::open(&setup.library(), config).unwrap();

    let r = ok(&core, "account.signIn", json!({}));
    assert_eq!(r, json!({"signedIn": true, "username": "LMY"}));
    let opened = opened.lock().unwrap();
    assert_eq!(opened.len(), 1);
    assert!(
        opened[0].starts_with(
            "https://www.wi-wwav.test/oauth/desktop/authorize?response_type=code&client_id=wi-wwav-desktop&"
        ),
        "{}",
        opened[0]
    );
    assert!(tokens(&setup)["access"].as_str().unwrap().len() > 20);
}

#[test]
fn sign_in_goes_through_the_browser_and_the_token_only_to_the_keychain() {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = Core::open(
        &setup.library(),
        setup.config(
            &server.url,
            server.browser("lmy@mi-wwav.com", "WeWave-lmy1"),
        ),
    )
    .unwrap();
    let events = core.events();
    assert_eq!(
        ok(&core, "account.status", json!({})),
        json!({"signedIn": false})
    );
    let r = ok(&core, "account.signIn", json!({}));
    assert_eq!(r, json!({"signedIn": true, "username": "LMY"}));
    let status = ok(&core, "account.status", json!({}));
    assert_eq!(
        status,
        json!({"signedIn": true, "username": "LMY", "galaxy": "lmy"})
    );
    assert_eq!(ok(&core, "app.hello", json!({}))["signedIn"], true);

    let t = tokens(&setup);
    let access = t["access"].as_str().unwrap();
    let refresh = t["refresh"].as_str().unwrap();
    assert!(access.len() > 20 && refresh.len() > 20);
    drop(core);
    for (path, bytes) in everything_in(&setup.library()) {
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            !text.contains(access) && !text.contains(refresh),
            "the token is in {path}"
        );
        assert!(!text.contains("WeWave-lmy1"), "a password is in {path}");
    }
    let seen: Vec<String> = events.try_iter().map(|e| e.payload.to_string()).collect();
    assert!(
        seen.iter().all(|e| !e.contains(access)),
        "an event carried the token"
    );
}

#[test]
fn an_answer_with_another_state_is_refused() {
    let server = MockServer::start();
    let setup = Setup::new();
    // Something that isn't the browser answers on the loopback first.
    let forged: wi_core::Opener = Arc::new(|url: &str| {
        let redirect = url
            .split("redirect_uri=")
            .nth(1)
            .and_then(|r| r.split('&').next())
            .unwrap()
            .replace("%3A", ":")
            .replace("%2F", "/");
        std::thread::spawn(move || {
            let _ = ureq::get(&format!("{redirect}?code=stolen&state=not-yours")).call();
        });
        Ok(())
    });
    let core = Core::open(&setup.library(), setup.config(&server.url, forged)).unwrap();
    let e = core.invoke("account.signIn", json!({})).unwrap_err();
    assert_eq!(e.code, "state_mismatch");
    assert!(setup.secrets.get("mi-wwav.com account").unwrap().is_none());
    assert_eq!(ok(&core, "account.status", json!({}))["signedIn"], false);
}

#[test]
fn a_wrong_password_stays_in_the_browser() {
    let server = MockServer::start();
    let setup = Setup::new();
    // The person types the wrong password, the page says so, and they
    // press Cancel. The app hears only the cancel.
    let base = server.url.clone();
    let (said_tx, said) = std::sync::mpsc::channel();
    let browser: wi_core::Opener = Arc::new(move |url: &str| {
        let (url, base, said_tx) = (url.to_string(), base.clone(), said_tx.clone());
        std::thread::spawn(move || {
            let agent = ureq::AgentBuilder::new().redirects(0).build();
            let page = agent.get(&url).call().unwrap().into_string().unwrap();
            let request = page
                .split("name=\"request\" value=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap()
                .to_string();
            let login = format!("{base}/oauth/desktop/login");
            let wrong = match agent.post(&login).send_form(&[
                ("request", &request),
                ("email", "lmy@mi-wwav.com"),
                ("password", "wrong"),
            ]) {
                Err(ureq::Error::Status(401, r)) => r.into_string().unwrap(),
                other => panic!("{other:?}"),
            };
            said_tx.send(wrong).unwrap();
            let cancel = match agent
                .post(&login)
                .send_form(&[("request", &request), ("action", "cancel")])
            {
                Ok(r) => r,
                Err(ureq::Error::Status(_, r)) => r,
                Err(e) => panic!("{e}"),
            };
            let back = cancel.header("location").unwrap().to_string();
            agent.get(&back).call().unwrap();
        });
        Ok(())
    });
    let core = Core::open(&setup.library(), setup.config(&server.url, browser)).unwrap();
    let e = core.invoke("account.signIn", json!({})).unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.message.as_str()),
        ("sign_in_cancelled", "Sign-in was cancelled.")
    );
    assert!(said.recv().unwrap().contains("That email and password don"));
    assert!(setup.secrets.get("mi-wwav.com account").unwrap().is_none());
}

#[test]
fn an_expired_token_refreshes_and_sign_out_clears_it() {
    let server = MockServer::start();
    let setup = Setup::new();
    let core = sign_in(&setup, &server);
    let before = tokens(&setup)["access"].as_str().unwrap().to_string();
    // A week and a day later, the access token has expired.
    let (status, _) = server.call(
        "POST",
        "/__mock/clock",
        Some(json!({"advanceMs": 8 * 24 * 3600 * 1000u64})),
    );
    assert_eq!(status, 200);
    core.sync_heat().unwrap();
    let after = tokens(&setup)["access"].as_str().unwrap().to_string();
    assert_ne!(after, before, "the token wasn't refreshed");

    assert_eq!(
        ok(&core, "account.signOut", json!({})),
        json!({"signedIn": false})
    );
    assert!(setup.secrets.get("mi-wwav.com account").unwrap().is_none());
    assert_eq!(
        ok(&core, "account.status", json!({})),
        json!({"signedIn": false})
    );
    assert_eq!(core.sync_heat().unwrap_err().code, "signed_out");
}
