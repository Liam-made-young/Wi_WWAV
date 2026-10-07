//! The python string and path rules the tools' output rests on: how
//! song.txt and film.txt are read, `str.strip()`, `float()`, the device's
//! folder title, `os.path`'s normpath, splitext and join, and today's date.

use std::collections::HashMap;
use std::path::Path;

/// python's `str.isspace()`: Rust's whitespace and the four information
/// separators U+001C to U+001F.
pub fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `str.strip()`.
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(is_py_space)
}

/// The first code point of each run of ten Unicode decimal digits (Nd),
/// as python 3.13 knows them (Unicode 15.1): what `\d`, `int()` and
/// `float()` read as digits.
const DECIMAL_ZEROS: [u32; 68] = [
    0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66,
    0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90,
    0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0, 0xa9f0, 0xaa50, 0xabf0, 0xff10,
    0x104a0, 0x10d30, 0x11066, 0x110f0, 0x11136, 0x111d0, 0x112f0, 0x11450, 0x114d0, 0x11650,
    0x116c0, 0x11730, 0x118e0, 0x11950, 0x11c50, 0x11d50, 0x11da0, 0x11f50, 0x16a60, 0x16ac0,
    0x16b50, 0x1d7ce, 0x1d7d8, 0x1d7e2, 0x1d7ec, 0x1d7f6, 0x1e140, 0x1e2f0, 0x1e4f0, 0x1e950,
    0x1fbf0,
];

/// A decimal digit's value, in any script (python's `\d`).
pub fn decimal(c: char) -> Option<u32> {
    let n = c as u32;
    DECIMAL_ZEROS
        .iter()
        .find(|&&z| (z..z + 10).contains(&n))
        .map(|z| n - z)
}

/// python's `float(s)`: any script's digits, surrounding whitespace, and
/// "_" between two digits, then the usual decimal, inf and nan forms.
pub fn py_float(s: &str) -> Option<f64> {
    // python first turns every decimal digit to ASCII and every space to
    // " " (anything else non-ASCII to "?", which no number has)
    let ascii: String = s
        .chars()
        .map(|c| match decimal(c) {
            Some(d) => char::from_digit(d, 10).unwrap_or('?'),
            None if is_py_space(c) => ' ',
            None if c.is_ascii() => c,
            None => '?',
        })
        .collect();
    let t = ascii.trim_matches(|c: char| c.is_ascii_whitespace() || c == '\u{b}');
    let b = t.as_bytes();
    let mut plain = String::with_capacity(t.len());
    for (i, &c) in b.iter().enumerate() {
        if c == b'_' {
            let digit = |j: Option<&u8>| j.is_some_and(u8::is_ascii_digit);
            if !(i > 0 && digit(b.get(i - 1)) && digit(b.get(i + 1))) {
                return None;
            }
        } else {
            plain.push(c as char);
        }
    }
    if plain.bytes().any(|c| c.is_ascii_whitespace()) {
        return None;
    }
    plain.parse().ok()
}

/// A song.txt or film.txt: `key = value` lines, keys lowercased, both
/// trimmed, `#` lines and lines without "=" skipped, later lines winning.
/// Read as python opens it: UTF-8 with U+FFFD for bad bytes, and "\r\n",
/// "\r" and "\n" all ending a line. No file reads as no lines.
pub fn key_values(path: &Path) -> HashMap<String, String> {
    let mut info = HashMap::new();
    let Ok(bytes) = std::fs::read(path) else {
        return info;
    };
    let text = String::from_utf8_lossy(&bytes)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    for line in text.split('\n') {
        let line = py_strip(line);
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            info.insert(py_strip(k).to_lowercase(), py_strip(v).to_string());
        }
    }
    info
}

/// `os.path.normpath` on POSIX: "a//b/./c/../" is "a/b".
pub fn normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".into();
    }
    let slashes = if path.starts_with("//") && !path.starts_with("///") {
        2
    } else {
        usize::from(path.starts_with('/'))
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if (slashes == 0 && parts.is_empty()) || parts.last() == Some(&"..") => {
                parts.push(part)
            }
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let out = "/".repeat(slashes) + &parts.join("/");
    if out.is_empty() {
        ".".into()
    } else {
        out
    }
}

/// `os.path.basename`.
pub fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// `os.path.splitext`: the extension starts at the last dot of the last
/// part, unless that part is only dots before it (".hidden" has none).
pub fn splitext(path: &str) -> (&str, &str) {
    let name = path.rfind('/').map_or(0, |i| i + 1);
    if let Some(dot) = path.rfind('.').filter(|&d| d > name) {
        if path[name..dot].bytes().any(|b| b != b'.') {
            return path.split_at(dot);
        }
    }
    (path, "")
}

/// `os.path.join(a, b)` on POSIX.
pub fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') || a.is_empty() {
        b.into()
    } else if a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

/// "01 Test Song" -> "Test Song", as the device names a folder: up to four
/// leading digits (any script's), then spaces, "-", "_" or ".", then the
/// rest (`^\d{1,4}[ \-_.]+(.+)$` in `wwav_pack.py title_of`). Anything
/// else is the folder's own name.
pub fn title_of(folder: &str) -> String {
    let path = normpath(folder);
    let name = basename(&path);
    let digits = name.chars().take_while(|&c| decimal(c).is_some()).count();
    if digits == 0 || digits > 4 {
        return name.into();
    }
    let rest: String = name.chars().skip(digits).collect();
    // `.` doesn't match a line break, and `$` matches before a final one
    let body = rest.strip_suffix('\n').unwrap_or(&rest);
    if body.contains('\n') {
        return name.into();
    }
    let after = body.trim_start_matches([' ', '-', '_', '.']);
    match (body.len() - after.len(), after) {
        (0, _) | (1, "") => name.into(),
        // the separators ran to the end: `.+` takes the last one back
        (seps, "") => body[seps - 1..].into(),
        (_, title) => title.into(),
    }
}

/// Today as YYYY-MM-DD, by the local clock: `datetime.date.today()`.
#[cfg(unix)]
pub fn today() -> String {
    // SAFETY: time(NULL) only returns the time; localtime_r writes the
    // struct tm it is given, which lives on this stack frame.
    let tm = unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm
    };
    format!(
        "{:04}-{:02}-{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday
    )
}

/// Today as YYYY-MM-DD. Off Unix there is no local zone to hand here
/// without another crate, so this is UTC's date.
#[cfg(not(unix))]
pub fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    // days to the civil date (proleptic Gregorian)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_split_as_os_path_splits_them() {
        assert_eq!(normpath("a//b/./c/../"), "a/b");
        assert_eq!(normpath("../x/.."), "..");
        assert_eq!(normpath("//x"), "//x");
        assert_eq!(normpath("///x/"), "/x");
        assert_eq!(splitext("a/film.mp4"), ("a/film", ".mp4"));
        assert_eq!(splitext("a.b/.hidden"), ("a.b/.hidden", ""));
        assert_eq!(splitext("a/..x.mp4"), ("a/..x", ".mp4"));
        assert_eq!(splitext("film"), ("film", ""));
        assert_eq!(join("a/", "b"), "a/b");
        assert_eq!(join("a", "/b"), "/b");
    }

    #[test]
    fn titles_come_from_folder_names() {
        assert_eq!(title_of("01 Test Song"), "Test Song");
        assert_eq!(title_of("x/05-Song/"), "Song");
        assert_eq!(title_of("12345 Song"), "12345 Song");
        assert_eq!(title_of("01  "), " ");
        assert_eq!(title_of("01 "), "01 ");
    }
}
