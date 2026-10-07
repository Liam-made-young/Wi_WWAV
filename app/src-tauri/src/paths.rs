//! Where things are: the engine beside the app, the library, the server.
//!
//! Each has an environment variable for development and the tests, which
//! run the app from `target/` with `mock-engine`, a throwaway library and
//! `tools/mock-server`.

use std::path::{Path, PathBuf};

/// The engine for an app binary at `exe` on `os` (`std::env::consts::OS`).
/// On macOS it is the helper app in the bundle's `Contents/Helpers`, so it
/// has no Dock icon and its plugin windows belong to an application of their
/// own (9.1); elsewhere it sits beside the app's binary.
pub fn engine_beside(exe: &Path, os: &str) -> PathBuf {
    let dir = exe.parent().unwrap_or(Path::new("."));
    match os {
        "macos" => dir
            .parent()
            .unwrap_or(dir)
            .join("Helpers/wwav-engine.app/Contents/MacOS/wwav-engine"),
        "windows" => dir.join("wwav-engine.exe"),
        _ => dir.join("wwav-engine"),
    }
}

/// The engine the core starts: `WI_WWAV_ENGINE`, or the one shipped beside
/// the app.
pub fn engine() -> PathBuf {
    if let Some(path) = std::env::var_os("WI_WWAV_ENGINE") {
        return path.into();
    }
    let exe = std::env::current_exe().unwrap_or_default();
    engine_beside(&exe, std::env::consts::OS)
}

/// The library folder (2.5): `WI_WWAV_LIBRARY`, or `Wi_WWAV` in the user's
/// Music folder.
pub fn library(music: Option<PathBuf>) -> PathBuf {
    if let Some(path) = std::env::var_os("WI_WWAV_LIBRARY") {
        return path.into();
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    music.unwrap_or_else(|| home.join("Music")).join("Wi_WWAV")
}

/// The server (9.7): `WI_WWAV_SERVER`, or mi-wwav.com.
pub fn server() -> String {
    std::env::var("WI_WWAV_SERVER").unwrap_or_else(|_| "https://www.mi-wwav.com".into())
}

/// Where the browser signs in (9.7): `WI_WWAV_SIGN_IN`, or www.wi-wwav.com.
/// A server set by `WI_WWAV_SERVER` serves its own sign-in page.
pub fn sign_in() -> String {
    sign_in_from(
        std::env::var("WI_WWAV_SIGN_IN").ok(),
        std::env::var("WI_WWAV_SERVER").ok(),
    )
}

fn sign_in_from(sign_in: Option<String>, server: Option<String>) -> String {
    sign_in
        .or(server)
        .unwrap_or_else(|| "https://www.wi-wwav.com".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_macos_the_engine_is_the_helper_app_in_the_bundle() {
        let exe = Path::new("/Applications/Wi_WWAV.app/Contents/MacOS/wi-wwav");
        assert_eq!(
            engine_beside(exe, "macos"),
            Path::new("/Applications/Wi_WWAV.app/Contents/Helpers/wwav-engine.app/Contents/MacOS/wwav-engine")
        );
    }

    #[test]
    fn sign_in_opens_on_wi_wwav_unless_a_server_or_an_address_is_set() {
        assert_eq!(sign_in_from(None, None), "https://www.wi-wwav.com");
        let mock = Some("http://127.0.0.1:4010".to_string());
        assert_eq!(sign_in_from(None, mock.clone()), "http://127.0.0.1:4010");
        assert_eq!(
            sign_in_from(Some("https://www.mi-wwav.com".into()), mock),
            "https://www.mi-wwav.com"
        );
    }

    #[test]
    fn elsewhere_the_engine_sits_beside_the_app() {
        assert_eq!(
            engine_beside(Path::new("/usr/bin/wi-wwav"), "linux"),
            Path::new("/usr/bin/wwav-engine")
        );
        assert_eq!(
            engine_beside(Path::new("C:/Program Files/Wi_WWAV/wi-wwav.exe"), "windows"),
            Path::new("C:/Program Files/Wi_WWAV/wwav-engine.exe")
        );
    }
}
