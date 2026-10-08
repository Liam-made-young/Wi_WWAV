//! Home (docs/SPACE.md 1): the galaxy you start in. For the prototype it is
//! the founder's own, read from mi-wwav.com, which needs no account: a
//! galaxy, its solar systems and their worlds are public.
//!
//! `space.home {slug?, refresh?}` answers the galaxy as links: every solar
//! system and every world with the address of its page, and what the server
//! knows of where it sits. Laying them out in the sky is the UI's
//! (app/ui/src/space/flight/home.ts). The answer is kept, so home is there
//! at once the next time and with no connection.

use serde_json::{json, Value};

use crate::args::Args;
use crate::net::{encode, Body, Fail};
use crate::{CoreError, Inner};

/// Whose galaxy is home when none is named (docs/SPACE.md 12: fixed for the
/// prototype, each person's own later).
const HOME: &str = "liam-made-young";

/// Where mi-wwav.com serves its pages from, under the server's address.
const WEB: &str = "/summer_26";

/// The answer for a galaxy, from what the server said of it and of each of
/// its solar systems. `web` is where its pages are served.
pub fn home_from(galaxy: &Value, systems: &[Value], web: &str) -> Value {
    let slug = galaxy["galaxy"]["slug"].as_str().unwrap_or_default();
    let here = format!("{web}/g/{}", encode(slug));
    let systems: Vec<Value> = systems
        .iter()
        .map(|s| {
            let system = &s["system"];
            let system_slug = system["slug"].as_str().unwrap_or_default();
            let url = format!("{here}/s/{}", encode(system_slug));
            let worlds: Vec<Value> = s["planets"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| world(p, web, &url))
                .collect();
            json!({
                "url": url,
                "name": system["title"].as_str().unwrap_or(system_slug),
                "x": system["posX"],
                "y": system["posY"],
                "worlds": worlds,
            })
        })
        .collect();
    json!({
        "slug": slug,
        "url": here,
        "name": galaxy["galaxy"]["displayName"].as_str().unwrap_or(slug),
        "systems": systems,
    })
}

/// One world as a link. A song and a film have pages of their own; a
/// gallery is shown in its solar system's page, so its link is that page
/// with the gallery named.
fn world(p: &Value, web: &str, system: &str) -> Value {
    let kind = p["kind"].as_str().unwrap_or("song");
    let id = |key: &str| match &p[key] {
        Value::String(s) => Some(encode(s)),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    };
    let url = match kind {
        "film" => id("filmId").map(|f| format!("{web}/watch/{f}")),
        "gallery" => id("galleryId").map(|g| format!("{system}?gallery={g}")),
        _ => id("trackId").map(|t| format!("{web}/play/{t}")),
    };
    json!({
        "url": url.unwrap_or_else(|| format!("{system}?world={}", p["id"])),
        "name": p["title"].as_str().unwrap_or("Untitled"),
        "kind": kind,
        "order": p["orbitIndex"],
        "radius": p["orbitRadius"],
        "phase": p["phaseOffset"],
        "palette": p["appearance"]["palette"],
    })
}

fn data(i: &Inner, path: &str) -> Result<Value, Fail> {
    let reply = i.net.expect("GET", path, Body::None, None)?;
    Ok(reply.body["data"].clone())
}

fn read(i: &Inner, slug: &str) -> Result<Value, Fail> {
    let galaxy = data(i, &format!("/api/v2/galaxies/{}", encode(slug)))?;
    let mut systems = Vec::new();
    for s in galaxy["systems"].as_array().into_iter().flatten() {
        let Some(system) = s["slug"].as_str() else {
            continue;
        };
        let path = format!(
            "/api/v2/galaxies/{}/systems/{}",
            encode(slug),
            encode(system)
        );
        systems.push(data(i, &path)?);
    }
    let web = format!("{}{WEB}", i.net.url("").trim_end_matches('/'));
    Ok(home_from(&galaxy, &systems, &web))
}

/// `space.home {slug?, refresh?}`.
pub(crate) fn home(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let slug = a.opt_str("slug").unwrap_or(HOME);
    let key = format!("space.home.{slug}");
    let kept = i.kv.get(&key)?;
    if !a.opt_bool("refresh")?.unwrap_or(false) {
        if let Some(kept) = kept {
            return Ok(json!({ "home": kept, "fresh": false }));
        }
    }
    match read(i, slug) {
        Ok(home) => {
            i.kv.set(&key, &home)?;
            Ok(json!({ "home": home, "fresh": true }))
        }
        // No connection, or the server is unwell: home as it was last seen.
        Err(fail) => match kept {
            Some(kept) => Ok(json!({ "home": kept, "fresh": false })),
            None => Err(fail.into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn galaxy() -> Value {
        json!({
            "galaxy": { "id": 1, "slug": "liam-made-young", "displayName": "liam_made_young" },
            "systems": [{ "id": 10, "slug": "miscellaneous" }],
        })
    }

    fn system() -> Value {
        json!({
            "system": { "id": 10, "slug": "miscellaneous", "title": "Miscellaneous", "posX": 2932.08, "posY": 3018.32 },
            "planets": [
                { "id": 41, "kind": "film", "orbitIndex": 0, "orbitRadius": 1120.1, "phaseOffset": 2.23,
                  "appearance": { "v": 1, "palette": { "base": "#b51a00", "glow": "#ffffff", "accent": "#ffffff" } },
                  "filmId": 67, "title": "2026" },
                { "id": 46, "kind": "song", "orbitIndex": 2, "orbitRadius": 1164.9, "phaseOffset": 5.21,
                  "appearance": null, "trackId": "track_1786796723313_8z505vzdb", "title": "2-young picasso" },
                { "id": 48, "kind": "gallery", "orbitIndex": 3, "orbitRadius": 1133.7, "phaseOffset": 0.79,
                  "appearance": null, "galleryId": 1, "title": "Buttocks" },
                { "id": 49, "kind": "song", "orbitIndex": 4, "orbitRadius": null, "phaseOffset": null,
                  "trackId": "fork_1786922087009_1azgx7t6b", "title": "REGULAR GUY (fork)" },
            ],
        })
    }

    #[test]
    fn every_system_and_world_is_a_link_to_its_own_page() {
        let home = home_from(&galaxy(), &[system()], "https://www.mi-wwav.com/summer_26");
        assert_eq!(home["name"], "liam_made_young");
        assert_eq!(
            home["url"],
            "https://www.mi-wwav.com/summer_26/g/liam-made-young"
        );
        let s = &home["systems"][0];
        assert_eq!(
            s["url"],
            "https://www.mi-wwav.com/summer_26/g/liam-made-young/s/miscellaneous"
        );
        assert_eq!(s["name"], "Miscellaneous");
        assert_eq!(s["x"], 2932.08);
        let urls: Vec<&str> = s["worlds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["url"].as_str().unwrap())
            .collect();
        assert_eq!(
            urls,
            [
                "https://www.mi-wwav.com/summer_26/watch/67",
                "https://www.mi-wwav.com/summer_26/play/track_1786796723313_8z505vzdb",
                "https://www.mi-wwav.com/summer_26/g/liam-made-young/s/miscellaneous?gallery=1",
                "https://www.mi-wwav.com/summer_26/play/fork_1786922087009_1azgx7t6b",
            ]
        );
    }

    #[test]
    fn a_world_keeps_what_the_server_knows_of_its_orbit_and_colour_and_says_so_when_it_knows_nothing(
    ) {
        let home = home_from(&galaxy(), &[system()], "https://x.example");
        let worlds = home["systems"][0]["worlds"].as_array().unwrap();
        assert_eq!(worlds[0]["kind"], "film");
        assert_eq!(worlds[0]["radius"], 1120.1);
        assert_eq!(worlds[0]["palette"]["base"], "#b51a00");
        assert_eq!(worlds[1]["palette"], Value::Null);
        assert_eq!(worlds[3]["radius"], Value::Null);
        assert_eq!(worlds[3]["phase"], Value::Null);
    }

    #[test]
    fn a_galaxy_with_no_systems_is_still_a_place() {
        let home = home_from(&galaxy(), &[], "https://x.example");
        assert_eq!(home["systems"], json!([]));
        assert_eq!(home["url"], "https://x.example/g/liam-made-young");
    }
}
