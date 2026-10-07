//! Calendar arithmetic on day counts: days since 1970-01-01, as a civil date
//! in the person's own zone. No clock and no time zone here; the caller turns
//! an instant into a local day count before a formula sees it.

/// Days since 1970-01-01 for a civil date (proleptic Gregorian).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> f64 {
    // Months past 12 or before 1 roll into the years, as DATE(2026, 14, 1) does.
    let months = y * 12 + (m - 1);
    let (y, m) = (months.div_euclid(12), months.rem_euclid(12) + 1);
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468 + (d - 1)) as f64
}

/// The civil date of a day count: (year, month, day).
pub fn civil_from_days(days: f64) -> (i64, i64, i64) {
    let z = days.floor() as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn days_in_month(y: i64, m: i64) -> i64 {
    (days_from_civil(y, m + 1, 1) - days_from_civil(y, m, 1)) as i64
}

/// 1 for Sunday to 7 for Saturday, as WEEKDAY answers by default.
pub fn weekday_sunday_first(days: f64) -> i64 {
    // 1970-01-01 was a Thursday.
    (days.floor() as i64 + 4).rem_euclid(7) + 1
}

/// The time of day in a day count, as (hour, minute, second).
pub fn time_of(days: f64) -> (i64, i64, i64) {
    let secs = ((days - days.floor()) * 86_400.0).round() as i64;
    let secs = secs.clamp(0, 86_399);
    (secs / 3600, (secs % 3600) / 60, secs % 60)
}

/// A date as text: `2026-10-07`, or `2026-10-07 23:59` when it has a time.
pub fn format_date(days: f64) -> String {
    if !days.is_finite() {
        return String::new();
    }
    let (y, m, d) = civil_from_days(days);
    let (h, min, _) = time_of(days);
    if days.fract().abs() < 1e-9 {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        format!("{y:04}-{m:02}-{d:02} {h:02}:{min:02}")
    }
}

fn int(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn valid(y: i64, m: i64, d: i64) -> bool {
    (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// Reads a date a person typed: `2026-10-07`, `2026-10-07 23:59`,
/// `2026-10-07T23:59`, `10/7/2026`, `Oct 7, 2026`, `7 Oct 2026`. None for
/// anything else, so plain text is never taken for a date.
pub fn parse_date(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.len() < 6 || t.len() > 40 {
        return None;
    }
    // ISO, with an optional time.
    if t.len() >= 10 && t.as_bytes()[4] == b'-' && t.as_bytes()[7] == b'-' {
        let (y, m, d) = (int(&t[0..4])?, int(&t[5..7])?, int(&t[8..10])?);
        if !valid(y, m, d) {
            return None;
        }
        let day = days_from_civil(y, m, d);
        let rest = t[10..].trim_start_matches(['T', ' ']);
        if rest.is_empty() {
            return Some(day);
        }
        return Some(day + parse_time(rest)?);
    }
    // 10/7/2026, with an optional time after a space
    let (date_part, time_part) = match t.split_once(' ') {
        Some((d, rest)) if d.contains('/') => (d, Some(rest)),
        _ => (t, None),
    };
    let slash: Vec<&str> = date_part.split('/').collect();
    if slash.len() == 3 {
        let (m, d, y) = (int(slash[0])?, int(slash[1])?, int(slash[2])?);
        let y = if y < 100 { 2000 + y } else { y };
        if !valid(y, m, d) {
            return None;
        }
        let day = days_from_civil(y, m, d);
        return match time_part {
            Some(time) => Some(day + parse_time(time)?),
            None => Some(day),
        };
    }
    // Oct 7, 2026 and 7 Oct 2026, with an optional time after them
    let mut words: Vec<String> = t
        .split([' ', ','])
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_lowercase())
        .collect();
    let time = if words.len() > 3 {
        let rest = words.split_off(3).join(" ");
        Some(parse_time(rest.trim_start_matches("at ").trim())?)
    } else {
        None
    };
    if words.len() == 3 {
        let month = |w: &str| {
            MONTHS
                .iter()
                .position(|m| w.len() >= 3 && w.starts_with(m))
                .map(|i| i as i64 + 1)
        };
        let (m, d, y) = if let Some(m) = month(&words[0]) {
            (m, int(&words[1])?, int(&words[2])?)
        } else {
            (month(&words[1])?, int(&words[0])?, int(&words[2])?)
        };
        return valid(y, m, d).then(|| days_from_civil(y, m, d) + time.unwrap_or(0.0));
    }
    None
}

/// `23:59`, `23:59:30`, `11:59 PM` as a fraction of a day. A trailing zone
/// (`Z`, `-04:00`) is ignored: the caller has already made the time local.
fn parse_time(s: &str) -> Option<f64> {
    let lower = s.trim().to_ascii_lowercase();
    let (body, pm, am) = if let Some(b) = lower.strip_suffix("pm") {
        (b.trim().to_string(), true, false)
    } else if let Some(b) = lower.strip_suffix("am") {
        (b.trim().to_string(), false, true)
    } else {
        (lower.clone(), false, false)
    };
    let body = body.trim_end_matches('z');
    // Cut a zone offset: the last '+' or '-' after the minutes.
    let body = match body.rfind(['+', '-']) {
        Some(i) if i >= 4 => &body[..i],
        _ => body,
    };
    let body = body.split('.').next().unwrap_or(body);
    let parts: Vec<&str> = body.split(':').collect();
    // "5pm" has no minutes; a bare "5" is not a time.
    if parts.len() > 3 || (parts.len() < 2 && !pm && !am) {
        return None;
    }
    let mut h = int(parts[0].trim())?;
    let m = if parts.len() > 1 { int(parts[1])? } else { 0 };
    if (pm || am) && !(1..=12).contains(&h) {
        return None;
    }
    let sec = if parts.len() == 3 { int(parts[2])? } else { 0 };
    if pm && h < 12 {
        h += 12;
    }
    if am && h == 12 {
        h = 0;
    }
    if h > 23 || m > 59 || sec > 59 {
        return None;
    }
    Some((h * 3600 + m * 60 + sec) as f64 / 86_400.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_count_and_its_date_agree() {
        assert_eq!(days_from_civil(1970, 1, 1), 0.0);
        assert_eq!(days_from_civil(2026, 10, 7), 20_733.0);
        for days in [-800_000i64, -1, 0, 59, 60, 11_016, 20_733, 100_000] {
            let (y, m, d) = civil_from_days(days as f64);
            assert_eq!(days_from_civil(y, m, d), days as f64, "{y}-{m}-{d}");
        }
        assert_eq!(civil_from_days(20_733.99), (2026, 10, 7));
    }

    #[test]
    fn the_week_starts_on_sunday() {
        // 7 October 2026 is a Wednesday.
        assert_eq!(weekday_sunday_first(days_from_civil(2026, 10, 7)), 4);
        assert_eq!(weekday_sunday_first(days_from_civil(2026, 10, 4)), 1);
        assert_eq!(weekday_sunday_first(days_from_civil(1969, 12, 28)), 1);
    }

    #[test]
    fn dates_are_read_as_people_write_them() {
        let day = days_from_civil(2026, 10, 7);
        assert_eq!(parse_date("2026-10-07"), Some(day));
        assert_eq!(parse_date("10/7/2026"), Some(day));
        assert_eq!(parse_date("Oct 7, 2026"), Some(day));
        assert_eq!(parse_date("7 October 2026"), Some(day));
        assert_eq!(parse_date("2026-10-07T12:00:00-04:00"), Some(day + 0.5));
        assert_eq!(parse_date("2026-10-07 6:00 PM"), Some(day + 0.75));
        assert_eq!(parse_date("Oct 7, 2026 6:00 PM"), Some(day + 0.75));
        assert_eq!(parse_date("7 Oct 2026 at 18:00"), Some(day + 0.75));
        assert_eq!(parse_date("10/7/2026 6pm"), Some(day + 0.75));
        assert_eq!(parse_date("10/7/2026 12:00 am"), Some(day));
        assert_eq!(parse_date("Oct 7, 2026 sometime"), None);
        assert_eq!(parse_date("10/7/2026 25:00"), None);
        assert_eq!(parse_date("10/7/2026 13pm"), None);
        for not in [
            "",
            "JPN 101",
            "2026-13-01",
            "2026-02-30",
            "12",
            "1/2",
            "hello world now",
        ] {
            assert_eq!(parse_date(not), None, "{not}");
        }
    }

    #[test]
    fn a_date_reads_back_as_text() {
        let day = days_from_civil(2026, 10, 7);
        assert_eq!(format_date(day), "2026-10-07");
        assert_eq!(
            format_date(day + 23.0 / 24.0 + 59.0 / 1440.0),
            "2026-10-07 23:59"
        );
    }

    #[test]
    fn months_roll_over_into_years() {
        assert_eq!(days_from_civil(2026, 13, 1), days_from_civil(2027, 1, 1));
        assert_eq!(days_from_civil(2026, 0, 1), days_from_civil(2025, 12, 1));
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
    }
}
