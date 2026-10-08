//! Space (docs/SPACE.md): the rules about links that need no window to
//! check. A link in Space is a body; what its page loads, and what a page
//! says back to the app, are decided here and tested here.

use serde_json::Value;
use url::Url;

/// A YouTube video's id, if `link` is one of the addresses a video has.
pub fn youtube_id(link: &Url) -> Option<String> {
    let host = link.host_str()?.trim_start_matches("www.");
    let id = match host {
        "youtu.be" => link.path_segments()?.next()?.to_string(),
        "youtube.com" | "m.youtube.com" | "music.youtube.com" => {
            let mut parts = link.path_segments()?;
            match parts.next()? {
                "watch" => link.query_pairs().find(|(k, _)| k == "v")?.1.into_owned(),
                "shorts" | "embed" | "live" => parts.next()?.to_string(),
                _ => return None,
            }
        }
        _ => return None,
    };
    let plain = id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    (plain && (6..=20).contains(&id.len())).then_some(id)
}

/// What Space loads for a link: the platform's own embed where it has one
/// ("everything that can be fitted in through an embed is"), the page itself
/// otherwise. `holder` is the port of the page on this Mac that holds
/// YouTube's player, which refuses a page with no web address of its own.
pub fn fit(link: &Url, holder: Option<u16>) -> Url {
    let host = link.host_str().unwrap_or_default();
    if let (Some(id), Some(port)) = (youtube_id(link), holder) {
        let held = format!("http://127.0.0.1:{port}/youtube?v={id}");
        return Url::parse(&held).expect("an id of letters and digits");
    }
    if host == "open.spotify.com" {
        let mut parts = link.path_segments().into_iter().flatten();
        // Spotify puts a language before the kind in some links.
        let mut kind = parts.next().unwrap_or_default();
        if kind.starts_with("intl-") {
            kind = parts.next().unwrap_or_default();
        }
        let id = parts.next().unwrap_or_default();
        let kinds = ["track", "album", "playlist", "artist", "episode", "show"];
        let plain = !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric());
        if kinds.contains(&kind) && plain {
            let embed = format!("https://open.spotify.com/embed/{kind}/{id}");
            return Url::parse(&embed).expect("a kind and an id");
        }
    }
    if host == "music.apple.com" && link.path().len() > 1 {
        let mut embed = link.clone();
        if embed.set_host(Some("embed.music.apple.com")).is_ok() {
            return embed;
        }
    }
    link.clone()
}

/// `%41` and the like, back to bytes.
fn unescape(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%')
            .then(|| s.get(i + 1..i + 3))
            .flatten()
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What a page said. A page speaks by asking to go to
/// `wwavspace://say/?<json>`, which is never a place; this reads the request
/// as a list of objects. Anything else a page could write there is nothing.
pub fn said(url: &Url) -> Vec<Value> {
    let text = unescape(url.query().unwrap_or_default());
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Array(items)) => items.into_iter().filter(Value::is_object).collect(),
        Ok(one) if one.is_object() => vec![one],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn a_youtube_video_is_held_in_the_page_on_this_mac_whatever_its_address() {
        for link in [
            "https://www.youtube.com/watch?v=jNQXAC9IVRw",
            "https://youtube.com/watch?list=PL1&v=jNQXAC9IVRw&t=4s",
            "https://youtu.be/jNQXAC9IVRw?si=abc",
            "https://m.youtube.com/shorts/jNQXAC9IVRw",
            "https://www.youtube.com/embed/jNQXAC9IVRw",
        ] {
            assert_eq!(
                fit(&url(link), Some(4100)).as_str(),
                "http://127.0.0.1:4100/youtube?v=jNQXAC9IVRw",
                "{link}"
            );
        }
    }

    #[test]
    fn youtube_without_the_holder_or_without_a_video_is_the_page_itself() {
        let video = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
        assert_eq!(fit(&url(video), None).as_str(), video);
        for link in [
            "https://www.youtube.com/",
            "https://www.youtube.com/@jawed",
            "https://www.youtube.com/watch?v=%3Cscript%3E",
            "https://www.youtube.com/results?search_query=saturn",
        ] {
            assert_eq!(fit(&url(link), Some(4100)).as_str(), link, "{link}");
        }
    }

    #[test]
    fn spotify_and_apple_music_become_their_embeds() {
        let cases = [
            (
                "https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT?si=x",
                "https://open.spotify.com/embed/track/4cOdK2wGLETKBW3PvgPWqT",
            ),
            (
                "https://open.spotify.com/intl-fr/album/6XhjNHCyCDyyGJRM5mg40G",
                "https://open.spotify.com/embed/album/6XhjNHCyCDyyGJRM5mg40G",
            ),
            (
                "https://music.apple.com/us/album/x/1612648318?i=1612648440",
                "https://embed.music.apple.com/us/album/x/1612648318?i=1612648440",
            ),
        ];
        for (link, embed) in cases {
            assert_eq!(fit(&url(link), None).as_str(), embed, "{link}");
        }
    }

    #[test]
    fn every_other_link_is_the_page_itself() {
        for link in [
            "https://en.wikipedia.org/wiki/Saturn",
            "https://x.com/NASA",
            "https://open.spotify.com/",
            "https://open.spotify.com/search/saturn",
            "https://music.apple.com/",
            "https://duckduckgo.com/?q=saturn",
        ] {
            assert_eq!(fit(&url(link), Some(4100)).as_str(), link, "{link}");
        }
    }

    #[test]
    fn what_a_page_says_is_read_as_a_list_of_objects_and_nothing_else() {
        let one = url("wwavspace://say/?%7B%22leave%22%3A1%7D");
        assert_eq!(said(&one), vec![json!({ "leave": 1 })]);
        let two = url("wwavspace://say/?%5B%7B%22wheel%22%3A%5B0%2C-12%5D%7D%2C%7B%22title%22%3A%22Saturn%20%E2%80%93%20Wikipedia%22%7D%2C7%5D");
        assert_eq!(
            said(&two),
            vec![
                json!({ "wheel": [0, -12] }),
                json!({ "title": "Saturn – Wikipedia" })
            ]
        );
        for nothing in [
            "wwavspace://say/",
            "wwavspace://say/?not%20json",
            "wwavspace://say/?%22text%22",
            "wwavspace://say/?%ZZ",
        ] {
            assert!(said(&url(nothing)).is_empty(), "{nothing}");
        }
    }
}
