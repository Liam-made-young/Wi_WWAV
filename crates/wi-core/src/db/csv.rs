//! CSV in and out (RFC 4180): fields in quotes when they hold a comma, a
//! quote or a line break, a quote doubled inside them. Reading is forgiving:
//! `\n` or `\r\n`, a byte-order mark, and a last line with no line break.

/// Rows of fields from CSV text.
pub(crate) fn parse(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut was_quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.is_empty() && !was_quoted => {
                quoted = true;
                was_quoted = true;
            }
            ',' => {
                row.push(std::mem::take(&mut field));
                was_quoted = false;
            }
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                was_quoted = false;
                rows.push(std::mem::take(&mut row));
            }
            other => field.push(other),
        }
    }
    if !field.is_empty() || !row.is_empty() || was_quoted {
        row.push(field);
        rows.push(row);
    }
    // A blank line is no row.
    rows.retain(|r| !(r.len() == 1 && r[0].is_empty()));
    rows
}

fn field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// CSV text from rows of fields, each line ended with `\r\n`.
pub(crate) fn write(rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for row in rows {
        let line: Vec<String> = row.iter().map(|f| field(f)).collect();
        out.push_str(&line.join(","));
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_with_commas_quotes_and_line_breaks_round_trip() {
        let rows = vec![
            vec![
                "Name".to_string(),
                "Note".to_string(),
                "Minutes".to_string(),
            ],
            vec![
                "Kanji, quiz".to_string(),
                "He said \"hi\"".to_string(),
                "45".to_string(),
            ],
            vec![
                "Two\nlines".to_string(),
                String::new(),
                " padded ".to_string(),
            ],
            vec!["日本語".to_string(), "é".to_string(), "-1.5".to_string()],
        ];
        let text = write(&rows);
        assert!(
            text.starts_with("Name,Note,Minutes\r\n\"Kanji, quiz\",\"He said \"\"hi\"\"\",45\r\n")
        );
        assert_eq!(parse(&text), rows);
    }

    #[test]
    fn reading_forgives_what_other_programs_write() {
        assert_eq!(
            parse("\u{feff}a,b\nc,d"),
            vec![vec!["a", "b"], vec!["c", "d"]]
        );
        assert_eq!(
            parse("a,b\r\n\r\nc,\r\n"),
            vec![vec!["a", "b"], vec!["c", ""]]
        );
        assert_eq!(parse("\"\",x\n"), vec![vec!["", "x"]]);
        assert_eq!(
            parse("a \"quoted\" word,b"),
            vec![vec!["a \"quoted\" word", "b"]]
        );
        assert_eq!(
            parse("\"never closed, still read"),
            vec![vec!["never closed, still read"]]
        );
        assert!(parse("").is_empty());
        assert!(parse("\n\n").is_empty());
    }
}
