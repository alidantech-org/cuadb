//! Multi-statement scripts for the trusted local instance: splitting on top-level
//! semicolons (quotes, comments and dollar-quoted bodies respected) and recognising
//! transaction control so a script can bring its own `BEGIN` / `COMMIT` / `ROLLBACK`.

/// Split a script into statements. Empty statements are dropped.
pub fn split(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == quote {
                        if bytes.get(i + 1) == Some(&quote) {
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'$' => {
                // $tag$ ... $tag$ (PostgreSQL); a lone `$1` is a parameter, not a quote.
                let mut j = i + 1;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                if bytes.get(j) == Some(&b'$')
                    && !bytes[i + 1..j].first().is_some_and(u8::is_ascii_digit)
                {
                    let tag = &text[i..=j];
                    match text[j + 1..].find(tag) {
                        Some(end) => i = j + 1 + end + tag.len(),
                        None => i = bytes.len(),
                    }
                } else {
                    i += 1;
                }
            }
            b';' => {
                push(&mut out, &text[start..i]);
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    if start < text.len() {
        push(&mut out, &text[start..]);
    }
    out
}

fn push(out: &mut Vec<String>, piece: &str) {
    let trimmed = piece.trim();
    // A piece that is only comments is not a statement.
    let meaningful = strip_comments(trimmed);
    if !meaningful.trim().is_empty() {
        out.push(trimmed.to_string());
    }
}

fn strip_comments(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '-' && chars.peek() == Some(&'-') {
            for n in chars.by_ref() {
                if n == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = ' ';
            for n in chars.by_ref() {
                if prev == '*' && n == '/' {
                    break;
                }
                prev = n;
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn first_words(sql: &str) -> Vec<String> {
    strip_comments(sql)
        .split_whitespace()
        .take(3)
        .map(|w| w.trim_end_matches(';').to_ascii_lowercase())
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Begin,
    Commit,
    Rollback,
}

/// Plain transaction control (`SAVEPOINT`, `RELEASE` and `ROLLBACK TO` pass through).
pub fn control(sql: &str) -> Option<Control> {
    let words = first_words(sql);
    match words.first().map(String::as_str)? {
        "begin" | "start" => Some(Control::Begin),
        "commit" | "end" => Some(Control::Commit),
        "rollback" | "abort" => {
            if words.get(1).map(String::as_str) == Some("to")
                || words.get(2).map(String::as_str) == Some("to")
            {
                None
            } else {
                Some(Control::Rollback)
            }
        }
        _ => None,
    }
}

/// Does the statement produce a result set?
pub fn returns_rows(sql: &str) -> bool {
    let words = first_words(sql);
    let first = words.first().map(String::as_str).unwrap_or_default();
    if matches!(
        first,
        "select" | "show" | "explain" | "values" | "table" | "pragma"
    ) {
        return true;
    }
    // `WITH … SELECT` returns rows; `WITH … INSERT/UPDATE/DELETE` is run as a write.
    first == "with" && {
        let lower = strip_comments(sql).to_ascii_lowercase();
        !(lower.contains("insert into")
            || lower.contains("update ")
            || lower.contains("delete from"))
    }
}

/// Does the statement change data or schema (anything that is not a plain read or
/// transaction control)?
pub fn is_write(sql: &str) -> bool {
    control(sql).is_none() && !returns_rows(sql)
}

/// The statement with string literals and numbers replaced by `?`, bounded, for display
/// and audit text that must not carry values.
pub fn mask_literals(sql: &str) -> String {
    let mut out = String::new();
    let flat = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = flat.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            while let Some(n) = chars.next() {
                if n == '\'' {
                    if chars.peek() == Some(&'\'') {
                        chars.next();
                        continue;
                    }
                    break;
                }
            }
            out.push('?');
        } else if c.is_ascii_digit() && !out.ends_with(|p: char| p.is_alphanumeric() || p == '_') {
            while chars
                .peek()
                .is_some_and(|n| n.is_ascii_digit() || *n == '.')
            {
                chars.next();
            }
            out.push('?');
        } else {
            out.push(c);
        }
        if out.chars().count() >= 160 {
            out.push('…');
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_top_level_semicolons_only() {
        let parts = split(
            "select 1; -- note; ignored\n insert into t values ('a;b', \"c;d\"); /* x; */ select $$a;b$$; ;",
        );
        assert_eq!(parts.len(), 3, "{parts:?}");
        assert!(parts[1].contains("'a;b'"));
        assert!(parts[2].contains("$$a;b$$"));
    }

    #[test]
    fn keeps_a_final_statement_without_semicolon_and_skips_comment_only_pieces() {
        assert_eq!(split("select 1; select 2"), ["select 1", "select 2"]);
        assert!(split("-- nothing here\n;").is_empty());
    }

    #[test]
    fn recognises_transaction_control_but_not_savepoints() {
        assert_eq!(control("BEGIN"), Some(Control::Begin));
        assert_eq!(control("start transaction"), Some(Control::Begin));
        assert_eq!(control("-- c\ncommit;"), Some(Control::Commit));
        assert_eq!(control("ROLLBACK"), Some(Control::Rollback));
        assert_eq!(control("rollback to savepoint a"), None);
        assert_eq!(control("rollback transaction to a"), None);
        assert_eq!(control("select 1"), None);
    }

    #[test]
    fn writes_are_everything_that_is_not_a_read_or_control() {
        assert!(is_write("insert into t values (1)"));
        assert!(is_write("DROP TABLE t"));
        assert!(is_write("with a as (select 1) delete from t"));
        assert!(!is_write("select 1"));
        assert!(!is_write("begin"));
    }

    #[test]
    fn masking_hides_literals() {
        assert_eq!(
            mask_literals("update t set name = 'Bob''s', n = 42 where id = 7"),
            "update t set name = ?, n = ? where id = ?"
        );
    }

    #[test]
    fn row_returning_detection() {
        assert!(returns_rows("SELECT 1"));
        assert!(returns_rows("with a as (select 1) select * from a"));
        assert!(!returns_rows(
            "with a as (select 1) insert into t select * from a"
        ));
        assert!(!returns_rows("create table t(x int)"));
    }
}
