//! The windows stay on the app (docs/SPEC.md 9.8: "no remote scripts load").
//!
//! A page in a window can send the web view somewhere else, such as a link
//! in words someone else wrote or a script that sets `location`. The
//! capabilities would still refuse that page's calls to the core, but its
//! own scripts would run in a window that looks like the app's. So a
//! navigation to anything but the app's own pages is cancelled.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime, Url};

/// Whether a window may go to `url`: the app's own pages (`tauri://localhost`,
/// or `http(s)://tauri.localhost` where Windows serves them), a blank page
/// and, in a development build, the Vite server named in `tauri.conf.json`.
pub fn allowed(url: &Url, dev: Option<&Url>) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" => {
            url.host_str() == Some("tauri.localhost")
                || dev.is_some_and(|d| {
                    d.scheme() == url.scheme()
                        && d.host_str() == url.host_str()
                        && d.port_or_known_default() == url.port_or_known_default()
                })
        }
        "about" => url.as_str() == "about:blank",
        _ => false,
    }
}

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("fence")
        .on_navigation(|webview, url| {
            let dev = cfg!(dev)
                .then(|| webview.app_handle().config().build.dev_url.clone())
                .flatten();
            let ok = allowed(url, dev.as_ref());
            if !ok {
                eprintln!("wi-wwav: a window tried to go to {url}; it stays on the app.");
            }
            ok
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn the_apps_own_pages_load() {
        let dev = None;
        for page in [
            "tauri://localhost/index.html",
            "tauri://localhost/index.html?window=settings",
            "http://tauri.localhost/index.html",
            "https://tauri.localhost/",
            "about:blank",
        ] {
            assert!(allowed(&url(page), dev), "{page}");
        }
    }

    #[test]
    fn nothing_else_does() {
        let dev = None;
        for page in [
            "http://127.0.0.1:5000/page.html",
            "https://evil.example/",
            "https://tauri.localhost.evil.example/",
            "tauri://evil/index.html",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,<script>1</script>",
            "http://localhost:5173/",
        ] {
            assert!(!allowed(&url(page), dev), "{page}");
        }
    }

    #[test]
    fn a_development_build_also_loads_its_vite_server() {
        let dev = url("http://localhost:5173");
        assert!(allowed(
            &url("http://localhost:5173/index.html"),
            Some(&dev)
        ));
        assert!(!allowed(&url("http://localhost:5174/"), Some(&dev)));
        assert!(!allowed(&url("https://localhost:5173/"), Some(&dev)));
        assert!(!allowed(&url("http://evil.example:5173/"), Some(&dev)));
    }
}
