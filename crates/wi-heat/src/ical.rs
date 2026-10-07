//! A reader for the iCalendar files Brightspace publishes (RFC 5545), covering
//! what a calendar feed of due dates holds: folded lines, escaped text,
//! VEVENTs and the VTIMEZONEs their times refer to.
//!
//! Times come out as instants. A TZID is looked up in the bundled time zone
//! database first, because it knows a zone's whole history; a TZID it doesn't
//! know (D2L can write Windows names such as "Eastern Standard Time") is read
//! from the file's own VTIMEZONE. A time with no zone, or a zone neither
//! defines, is the school's local time.
//!
//! The reader is lenient, as a feed reader has to be: a line it can't read is
//! skipped, not fatal. The one hard failure is a body that holds no calendar
//! at all, such as the login page an expired private link returns, because
//! reading that as an empty feed would tag every task "No longer in
//! Brightspace".

use jiff::civil::{Date, DateTime};
use jiff::tz::{Offset, TimeZone};
use jiff::Timestamp;
use std::collections::HashMap;
use std::fmt;

/// When an event happens: a whole day, or an instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    Date(Date),
    At(Timestamp),
}

/// One VEVENT, with its text unescaped. Absent text is empty.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Event {
    pub uid: Option<String>,
    pub summary: String,
    pub description: String,
    pub location: String,
    pub url: Option<String>,
    /// Upper case, as RFC 5545 writes it: "CONFIRMED", "CANCELLED".
    pub status: Option<String>,
    pub start: Option<When>,
    pub end: Option<When>,
    pub due: Option<When>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotACalendar;

impl fmt::Display for NotACalendar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("The Brightspace link didn't return a calendar.")
    }
}

impl std::error::Error for NotACalendar {}

/// Every VEVENT in `bytes`, in file order. Times without a zone of their own
/// are read in `zone`, the school's.
pub fn parse(bytes: &[u8], zone: &TimeZone) -> Result<Vec<Event>, NotACalendar> {
    let text = unfold(bytes);
    let top = components(&text);
    let calendars: Vec<&Component> = top.iter().filter(|c| c.name == "VCALENDAR").collect();
    if calendars.is_empty() {
        return Err(NotACalendar);
    }
    let mut events = Vec::new();
    for cal in calendars {
        let zones: HashMap<String, TimeZone> = cal.children.iter().filter_map(vtimezone).collect();
        for c in cal.children.iter().filter(|c| c.name == "VEVENT") {
            events.push(event(c, &zones, zone));
        }
    }
    Ok(events)
}

/// Joins folded lines. Folding works on octets and may split a UTF-8
/// character in two (RFC 5545 3.1 warns of it), so it is undone before the
/// bytes are decoded.
fn unfold(bytes: &[u8]) -> String {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        if rest.starts_with(b"\r\n ") || rest.starts_with(b"\r\n\t") {
            i += 3;
        } else if rest.starts_with(b"\n ") || rest.starts_with(b"\n\t") {
            i += 2;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let text = String::from_utf8_lossy(&out).into_owned();
    match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => text,
    }
}

#[derive(Debug)]
struct Line {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Line {
    fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// `NAME;PARAM=VALUE;PARAM="QUOTED":value`. The value starts after the first
/// colon outside quotes, so a quoted TZID may hold colons.
fn content_line(s: &str) -> Option<Line> {
    let mut quoted = false;
    let mut cuts = Vec::new();
    let mut colon = None;
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => cuts.push(i),
            ':' if !quoted => {
                colon = Some(i);
                break;
            }
            _ => {}
        }
    }
    let colon = colon?;
    let head = &s[..colon];
    let name_end = cuts.first().copied().unwrap_or(colon);
    let name = head[..name_end].trim().to_ascii_uppercase();
    if name.is_empty() {
        return None;
    }
    let mut params = Vec::new();
    for (k, &start) in cuts.iter().enumerate() {
        let end = cuts.get(k + 1).copied().unwrap_or(colon);
        let p = &s[start + 1..end];
        if let Some((n, v)) = p.split_once('=') {
            params.push((
                n.trim().to_ascii_uppercase(),
                v.trim().trim_matches('"').to_string(),
            ));
        }
    }
    Some(Line {
        name,
        params,
        value: s[colon + 1..].to_string(),
    })
}

#[derive(Debug)]
struct Component {
    name: String,
    lines: Vec<Line>,
    children: Vec<Component>,
}

impl Component {
    fn new(name: &str) -> Self {
        Component {
            name: name.to_ascii_uppercase(),
            lines: Vec::new(),
            children: Vec::new(),
        }
    }

    fn prop(&self, name: &str) -> Option<&Line> {
        self.lines.iter().find(|l| l.name == name)
    }

    fn text(&self, name: &str) -> String {
        self.prop(name)
            .map(|l| unescape(&l.value))
            .unwrap_or_default()
    }
}

/// The BEGIN/END tree. A missing END closes at the next END that matches an
/// outer component, or at the end of the file.
fn components(text: &str) -> Vec<Component> {
    let mut stack = vec![Component::new("")];
    for raw in text.split('\n') {
        let Some(line) = content_line(raw.trim_end_matches('\r')) else {
            continue;
        };
        match line.name.as_str() {
            "BEGIN" => stack.push(Component::new(line.value.trim())),
            "END" => {
                let name = line.value.trim().to_ascii_uppercase();
                if stack.iter().skip(1).any(|c| c.name == name) {
                    loop {
                        let done = stack.pop().expect("the root is never popped here");
                        let matched = done.name == name;
                        stack.last_mut().expect("root").children.push(done);
                        if matched {
                            break;
                        }
                    }
                }
            }
            _ => stack.last_mut().expect("root").lines.push(line),
        }
    }
    while stack.len() > 1 {
        let done = stack.pop().expect("checked");
        stack.last_mut().expect("root").children.push(done);
    }
    stack.pop().expect("root").children
}

/// TEXT values escape `\\`, `\;`, `\,` and newlines as `\n` or `\N`.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn event(c: &Component, zones: &HashMap<String, TimeZone>, zone: &TimeZone) -> Event {
    let uid = c.text("UID").trim().to_string();
    let when = |name| c.prop(name).and_then(|l| when(l, zones, zone));
    Event {
        uid: (!uid.is_empty()).then_some(uid),
        summary: c.text("SUMMARY"),
        description: c.text("DESCRIPTION"),
        location: c.text("LOCATION"),
        url: c
            .prop("URL")
            .map(|l| l.value.trim().to_string())
            .filter(|u| !u.is_empty()),
        status: c
            .prop("STATUS")
            .map(|l| l.value.trim().to_ascii_uppercase()),
        start: when("DTSTART"),
        end: when("DTEND"),
        due: when("DUE"),
    }
}

fn when(line: &Line, zones: &HashMap<String, TimeZone>, zone: &TimeZone) -> Option<When> {
    let v = line.value.trim();
    let date_only = line
        .param("VALUE")
        .is_some_and(|p| p.eq_ignore_ascii_case("DATE"))
        || v.len() == 8;
    if date_only {
        return civil_date(v).map(When::Date);
    }
    if let Some(utc) = v.strip_suffix('Z').or_else(|| v.strip_suffix('z')) {
        return TimeZone::UTC
            .to_timestamp(civil_datetime(utc)?)
            .ok()
            .map(When::At);
    }
    let dt = civil_datetime(v)?;
    let tz = line
        .param("TZID")
        .and_then(|id| TimeZone::get(id).ok().or_else(|| zones.get(id).cloned()));
    // RFC 5545 3.3.5 reads a time in a gap with the offset before it, and a
    // repeated time as its first occurrence: jiff's "compatible".
    tz.as_ref()
        .unwrap_or(zone)
        .to_ambiguous_timestamp(dt)
        .compatible()
        .ok()
        .map(When::At)
}

fn digits(s: &str) -> Option<i32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// `YYYYMMDD`.
fn civil_date(s: &str) -> Option<Date> {
    if s.len() != 8 {
        return None;
    }
    let y = digits(s.get(0..4)?)?;
    let m = digits(s.get(4..6)?)?;
    let d = digits(s.get(6..8)?)?;
    Date::new(y as i16, m as i8, d as i8).ok()
}

/// `YYYYMMDDTHHMMSS`.
fn civil_datetime(s: &str) -> Option<DateTime> {
    if s.len() != 15 || s.as_bytes()[8] != b'T' {
        return None;
    }
    let date = civil_date(&s[..8])?;
    let h = digits(s.get(9..11)?)?;
    let m = digits(s.get(11..13)?)?;
    let sec = digits(s.get(13..15)?)?;
    // A leap second (60) is read as the last second of the minute.
    DateTime::new(
        date.year(),
        date.month(),
        date.day(),
        h as i8,
        m as i8,
        sec.min(59) as i8,
        0,
    )
    .ok()
}

/// A VTIMEZONE as a jiff time zone. Its latest STANDARD and DAYLIGHT rules
/// become a POSIX TZ string, which holds exactly what a yearly
/// "Nth weekday of a month" rule says. A rule that doesn't fit (a one-off
/// observance, BYMONTHDAY lists) leaves the TZID to the school's zone.
fn vtimezone(c: &Component) -> Option<(String, TimeZone)> {
    if c.name != "VTIMEZONE" {
        return None;
    }
    let tzid = c.prop("TZID")?.value.trim().to_string();
    let latest = |kind: &str| {
        c.children
            .iter()
            .filter(|o| o.name == kind)
            .max_by_key(|o| o.prop("DTSTART").map(|l| l.value.trim().to_string()))
    };
    let tz = match (latest("STANDARD"), latest("DAYLIGHT")) {
        (Some(std), None) => TimeZone::fixed(utc_offset(std)?),
        (Some(std), Some(dst)) => {
            let (s, d) = (utc_offset(std)?.seconds(), utc_offset(dst)?.seconds());
            let posix = format!(
                "{}{}{}{},{},{}",
                posix_name(s),
                posix_offset(s),
                posix_name(d),
                posix_offset(d),
                posix_rule(dst)?,
                posix_rule(std)?
            );
            TimeZone::posix(&posix).ok()?
        }
        _ => return None,
    };
    Some((tzid, tz))
}

/// TZOFFSETTO: `-0500`, `+0530`, `-045000`.
fn utc_offset(observance: &Component) -> Option<Offset> {
    let v = observance.prop("TZOFFSETTO")?.value.trim().to_string();
    let (sign, rest) = match v.as_bytes().first()? {
        b'+' => (1, &v[1..]),
        b'-' => (-1, &v[1..]),
        _ => return None,
    };
    if !rest.is_ascii() || (rest.len() != 4 && rest.len() != 6) {
        return None;
    }
    let h = digits(&rest[0..2])?;
    let m = digits(&rest[2..4])?;
    let s = if rest.len() == 6 {
        digits(&rest[4..6])?
    } else {
        0
    };
    Offset::from_seconds(sign * (h * 3600 + m * 60 + s)).ok()
}

/// POSIX names a zone with three or more characters; `<-0500>` always works.
fn posix_name(secs: i32) -> String {
    let a = secs.abs();
    format!(
        "<{}{:02}{:02}>",
        if secs < 0 { '-' } else { '+' },
        a / 3600,
        a % 3600 / 60
    )
}

/// POSIX offsets count west of Greenwich as positive.
fn posix_offset(secs: i32) -> String {
    let w = -secs;
    let a = w.abs();
    format!(
        "{}{}:{:02}:{:02}",
        if w < 0 { "-" } else { "" },
        a / 3600,
        a % 3600 / 60,
        a % 60
    )
}

/// `RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU` with `DTSTART:16010311T020000` is
/// `M3.2.0/2:00:00`. Both read the time on the clock before the change.
fn posix_rule(observance: &Component) -> Option<String> {
    let rule = &observance.prop("RRULE")?.value;
    let parts: HashMap<String, String> = rule
        .split(';')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.trim().to_ascii_uppercase(), v.trim().to_ascii_uppercase()))
        .collect();
    if parts.get("FREQ").map(String::as_str) != Some("YEARLY") {
        return None;
    }
    let month = digits(parts.get("BYMONTH")?)?;
    let byday = parts.get("BYDAY")?;
    let cut = byday.len().checked_sub(2)?;
    let (nth, day) = (byday.get(..cut)?, byday.get(cut..)?);
    let week = match nth.trim_start_matches('+') {
        "-1" => 5,
        n => digits(n).filter(|w| (1..=4).contains(w))?,
    };
    let weekday = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"]
        .iter()
        .position(|d| *d == day)?;
    let start = civil_datetime(observance.prop("DTSTART")?.value.trim())?;
    Some(format!(
        "M{month}.{week}.{weekday}/{}:{:02}:{:02}",
        start.hour(),
        start.minute(),
        start.second()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ny() -> TimeZone {
        TimeZone::get("America/New_York").unwrap()
    }

    fn wrap(body: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{body}END:VCALENDAR\r\n")
    }

    fn one(body: &str) -> Event {
        let mut events = parse(wrap(body).as_bytes(), &ny()).unwrap();
        assert_eq!(events.len(), 1);
        events.remove(0)
    }

    fn at(s: &str) -> Option<When> {
        Some(When::At(s.parse().unwrap()))
    }

    #[test]
    fn folded_lines_join_even_inside_a_character() {
        // "漢" is e6 bc a2, and the fold falls after its first byte.
        let bytes = b"BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:a\r\nSUMMARY:Kanji \xe6\r\n \xbc\xa2 quiz\r\n\tthree\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert_eq!(
            parse(bytes, &ny()).unwrap()[0].summary,
            "Kanji 漢 quizthree"
        );
    }

    #[test]
    fn bare_newlines_and_a_byte_order_mark_are_read() {
        let ics = "\u{feff}BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:b\nSUMMARY:Lab 2 -\n  Due\nEND:VEVENT\nEND:VCALENDAR\n";
        assert_eq!(
            parse(ics.as_bytes(), &ny()).unwrap()[0].summary,
            "Lab 2 - Due"
        );
    }

    #[test]
    fn text_is_unescaped() {
        let e = one(
            "BEGIN:VEVENT\r\nUID:c\r\nDESCRIPTION:a\\, b\\; c\\\\d\\nline\\Nx\r\nEND:VEVENT\r\n",
        );
        assert_eq!(e.description, "a, b; c\\d\nline\nx");
    }

    #[test]
    fn params_may_be_quoted_and_hold_colons() {
        let l =
            content_line(r#"DTSTART;TZID="Custom: Zone";VALUE=DATE-TIME:20261012T090000"#).unwrap();
        assert_eq!(l.name, "DTSTART");
        assert_eq!(l.param("TZID"), Some("Custom: Zone"));
        assert_eq!(l.param("VALUE"), Some("DATE-TIME"));
        assert_eq!(l.value, "20261012T090000");
        let l = content_line("summary:Quiz: part 2").unwrap();
        assert_eq!(
            (l.name.as_str(), l.value.as_str()),
            ("SUMMARY", "Quiz: part 2")
        );
        assert!(content_line("no colon here").is_none());
    }

    #[test]
    fn dates_utc_zoned_and_floating_times() {
        let e = one(
            "BEGIN:VEVENT\r\nUID:d\r\nDTSTART;VALUE=DATE:20261020\r\nDTEND:20261021T035900Z\r\n\
             DUE;TZID=America/Los_Angeles:20261020T235900\r\nEND:VEVENT\r\n",
        );
        assert_eq!(e.start, Some(When::Date(jiff::civil::date(2026, 10, 20))));
        assert_eq!(e.end, at("2026-10-21T03:59:00Z"));
        assert_eq!(e.due, at("2026-10-21T06:59:00Z"));
        let f = one("BEGIN:VEVENT\r\nUID:e\r\nDTSTART:20261209T235900\r\nEND:VEVENT\r\n");
        assert_eq!(
            f.start,
            at("2026-12-10T04:59:00Z"),
            "a floating time is the school's, here EST"
        );
        let g = one("BEGIN:VEVENT\r\nUID:f\r\nDTSTART:2026-10-09\r\nDTEND:20261332T000000Z\r\nEND:VEVENT\r\n");
        assert_eq!(
            (g.start, g.end),
            (None, None),
            "a malformed time is left out, not guessed"
        );
    }

    const WINDOWS_ZONES: &str = "BEGIN:VTIMEZONE\r\nTZID:Eastern Standard Time\r\n\
        BEGIN:STANDARD\r\nDTSTART:16011104T020000\r\nRRULE:FREQ=YEARLY;BYDAY=1SU;BYMONTH=11\r\n\
        TZOFFSETFROM:-0400\r\nTZOFFSETTO:-0500\r\nEND:STANDARD\r\n\
        BEGIN:DAYLIGHT\r\nDTSTART:16010311T020000\r\nRRULE:FREQ=YEARLY;BYDAY=2SU;BYMONTH=3\r\n\
        TZOFFSETFROM:-0500\r\nTZOFFSETTO:-0400\r\nEND:DAYLIGHT\r\nEND:VTIMEZONE\r\n\
        BEGIN:VTIMEZONE\r\nTZID:AUS Eastern Standard Time\r\n\
        BEGIN:STANDARD\r\nDTSTART:16010401T030000\r\nRRULE:FREQ=YEARLY;BYDAY=1SU;BYMONTH=4\r\n\
        TZOFFSETFROM:+1100\r\nTZOFFSETTO:+1000\r\nEND:STANDARD\r\n\
        BEGIN:DAYLIGHT\r\nDTSTART:16011007T020000\r\nRRULE:FREQ=YEARLY;BYDAY=1SU;BYMONTH=10\r\n\
        TZOFFSETFROM:+1000\r\nTZOFFSETTO:+1100\r\nEND:DAYLIGHT\r\nEND:VTIMEZONE\r\n\
        BEGIN:VTIMEZONE\r\nTZID:India Standard Time\r\n\
        BEGIN:STANDARD\r\nDTSTART:16010101T000000\r\nTZOFFSETFROM:+0530\r\nTZOFFSETTO:+0530\r\n\
        END:STANDARD\r\nEND:VTIMEZONE\r\n";

    #[test]
    fn a_vtimezone_defines_a_tzid_the_database_lacks() {
        let starts = [
            (
                "\"Eastern Standard Time\"",
                "20261012T090000",
                "2026-10-12T13:00:00Z",
            ),
            (
                "Eastern Standard Time",
                "20261203T090000",
                "2026-12-03T14:00:00Z",
            ),
            // 2:30 AM doesn't exist on March 14, 2027; the offset before the gap reads it.
            (
                "Eastern Standard Time",
                "20270314T023000",
                "2027-03-14T07:30:00Z",
            ),
            // The southern rule wraps the new year.
            (
                "AUS Eastern Standard Time",
                "20270115T120000",
                "2027-01-15T01:00:00Z",
            ),
            (
                "AUS Eastern Standard Time",
                "20270715T120000",
                "2027-07-15T02:00:00Z",
            ),
            (
                "India Standard Time",
                "20261012T090000",
                "2026-10-12T03:30:00Z",
            ),
        ];
        let mut body = WINDOWS_ZONES.to_string();
        for (n, (tzid, local, _)) in starts.iter().enumerate() {
            body += &format!(
                "BEGIN:VEVENT\r\nUID:{n}\r\nDTSTART;TZID={tzid}:{local}\r\nEND:VEVENT\r\n"
            );
        }
        let events = parse(wrap(&body).as_bytes(), &TimeZone::UTC).unwrap();
        for (e, (tzid, _, utc)) in events.iter().zip(starts) {
            assert_eq!(e.start, at(utc), "{tzid}");
        }
    }

    #[test]
    fn a_windows_vtimezone_agrees_with_the_database_hour_by_hour() {
        let top = components(&unfold(wrap(WINDOWS_ZONES).as_bytes()));
        let zones: HashMap<String, TimeZone> =
            top[0].children.iter().filter_map(vtimezone).collect();
        for (windows, iana) in [
            ("Eastern Standard Time", "America/New_York"),
            ("AUS Eastern Standard Time", "Australia/Sydney"),
        ] {
            let (ours, theirs) = (&zones[windows], TimeZone::get(iana).unwrap());
            let mut dt = jiff::civil::date(2026, 1, 1).at(0, 30, 0, 0);
            while dt.year() < 2029 {
                let a = ours.to_ambiguous_timestamp(dt).compatible().unwrap();
                let b = theirs.to_ambiguous_timestamp(dt).compatible().unwrap();
                assert_eq!(a, b, "{windows} at {dt}");
                dt = dt.checked_add(jiff::Span::new().hours(1)).unwrap();
            }
        }
    }

    #[test]
    fn an_unknown_tzid_is_the_school_zone() {
        let e = one("BEGIN:VEVENT\r\nUID:f\r\nDTSTART;TZID=Nowhere/Special:20261012T090000\r\nEND:VEVENT\r\n");
        assert_eq!(e.start, at("2026-10-12T13:00:00Z"));
    }

    #[test]
    fn a_missing_end_or_a_junk_line_loses_no_event() {
        let ics = "BEGIN:VCALENDAR\r\nthis line has no colon\r\nBEGIN:VEVENT\r\nUID:g\r\nSUMMARY:Quiz 1 - Due\r\n\
                   BEGIN:VALARM\r\nACTION:DISPLAY\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:h\r\n";
        let events = parse(ics.as_bytes(), &ny()).unwrap();
        let uids: Vec<&str> = events.iter().map(|e| e.uid.as_deref().unwrap()).collect();
        assert_eq!(uids, ["g", "h"]);
        assert_eq!(events[0].summary, "Quiz 1 - Due");
    }

    #[test]
    fn status_and_url_are_read() {
        let e = one(
            "BEGIN:VEVENT\r\nUID: i \r\nSTATUS:cancelled\r\nURL;VALUE=URI:https://brightspace.uri.edu/d2l/home\r\nEND:VEVENT\r\n",
        );
        assert_eq!(e.uid.as_deref(), Some("i"));
        assert_eq!(e.status.as_deref(), Some("CANCELLED"));
        assert_eq!(
            e.url.as_deref(),
            Some("https://brightspace.uri.edu/d2l/home")
        );
        let bare = one("BEGIN:VEVENT\r\nSUMMARY:No uid\r\nEND:VEVENT\r\n");
        assert_eq!((bare.uid, bare.url, bare.status), (None, None, None));
    }

    // Feeds come from outside: no line, however malformed, may panic.
    fn feedish_line() -> impl proptest::strategy::Strategy<Value = String> {
        use proptest::prelude::*;
        let junk = "[-+0-9A-Za-zé漢;:=,\" ]{0,9}";
        prop_oneof![
            Just("BEGIN:VCALENDAR".to_string()),
            Just("BEGIN:VTIMEZONE".to_string()),
            Just("TZID:Z".to_string()),
            prop::sample::select(vec![
                "BEGIN:STANDARD",
                "BEGIN:DAYLIGHT",
                "END:STANDARD",
                "END:DAYLIGHT"
            ])
            .prop_map(str::to_string),
            junk.prop_map(|v| format!("TZOFFSETTO:{v}")),
            (junk, junk).prop_map(|(m, d)| format!("RRULE:FREQ=YEARLY;BYMONTH={m};BYDAY={d}")),
            junk.prop_map(|v| format!("DTSTART:{v}")),
            Just("END:VTIMEZONE".to_string()),
            Just("BEGIN:VEVENT".to_string()),
            (junk, junk).prop_map(|(p, v)| format!("DTSTART;TZID=Z{p}:{v}")),
            junk.prop_map(|v| format!("SUMMARY:{v}")),
            Just("END:VEVENT".to_string()),
            junk.prop_map(|v| v.to_string()),
        ]
    }

    proptest::proptest! {
        #[test]
        fn malformed_feeds_never_panic(lines in proptest::collection::vec(feedish_line(), 0..40)) {
            let _ = parse(lines.join("\r\n").as_bytes(), &ny());
        }

        #[test]
        fn malformed_zone_values_never_panic(
            v in proptest::collection::vec("[-+0-9A-Zé漢 ]{0,7}", 8),
        ) {
            let ics = format!(
                "BEGIN:VCALENDAR\r\nBEGIN:VTIMEZONE\r\nTZID:Z\r\n\
                 BEGIN:STANDARD\r\nDTSTART:{}\r\nRRULE:FREQ=YEARLY;BYMONTH={};BYDAY={}\r\nTZOFFSETTO:{}\r\nEND:STANDARD\r\n\
                 BEGIN:DAYLIGHT\r\nDTSTART:16010311T020000\r\nRRULE:FREQ=YEARLY;BYMONTH={};BYDAY={}\r\nTZOFFSETTO:{}\r\nEND:DAYLIGHT\r\n\
                 END:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:x\r\nDTSTART;TZID=Z:{}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
                v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]
            );
            let _ = parse(ics.as_bytes(), &ny());
        }

        #[test]
        fn random_bytes_never_panic(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..300)) {
            let _ = parse(&bytes, &ny());
        }
    }

    #[test]
    fn a_body_with_no_calendar_is_an_error() {
        assert_eq!(parse(b"", &ny()), Err(NotACalendar));
        assert_eq!(
            parse(b"<html><body>Login</body></html>", &ny()),
            Err(NotACalendar)
        );
        assert_eq!(
            parse(b"BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n", &ny()),
            Ok(vec![])
        );
    }
}
