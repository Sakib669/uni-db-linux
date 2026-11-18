//! Splits SQL scripts into statements without a full parser, respecting quoted
//! strings, quoted identifiers, line comments, and block comments.

/// Splits a SQL script into trimmed, non empty statements.
pub fn split_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut chars = sql.chars().peekable();
    let mut single = false;
    let mut double = false;
    let mut backtick = false;
    let mut line_comment = false;
    let mut block_comment = false;

    while let Some(character) = chars.next() {
        if line_comment {
            current.push(character);
            if character == '\n' {
                line_comment = false;
            }
            continue;
        }
        if block_comment {
            current.push(character);
            if character == '*' && chars.peek() == Some(&'/') {
                current.push(chars.next().unwrap_or('/'));
                block_comment = false;
            }
            continue;
        }
        if !single && !double && !backtick {
            if character == '-' && chars.peek() == Some(&'-') {
                current.push(character);
                block_comment = false;
                line_comment = true;
                continue;
            }
            if character == '/' && chars.peek() == Some(&'*') {
                current.push(character);
                current.push(chars.next().unwrap_or('*'));
                block_comment = true;
                continue;
            }
        }
        match character {
            '\'' if !double && !backtick => {
                single = !single;
                current.push(character);
            }
            '"' if !single && !backtick => {
                double = !double;
                current.push(character);
            }
            '`' if !single && !double => {
                backtick = !backtick;
                current.push(character);
            }
            ';' if !single && !double && !backtick => {
                let statement = current.trim().to_string();
                if !statement.is_empty() {
                    statements.push(statement);
                }
                current.clear();
            }
            _ => current.push(character),
        }
    }
    let tail = current.trim().to_string();
    if !tail.is_empty()
        && !tail
            .lines()
            .all(|line| line.trim().is_empty() || line.trim().starts_with("--"))
    {
        statements.push(tail);
    }
    statements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_plain_statements() {
        let statements = split_statements("SELECT 1; SELECT 2;\n;\n");
        assert_eq!(statements, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn keeps_semicolons_inside_quotes_and_comments() {
        let sql = "INSERT INTO t VALUES ('a;b'); -- a comment; with semicolon\nSELECT \"x;y\" FROM t; /* block; comment */ SELECT `a;b`;";
        let statements = split_statements(sql);
        assert_eq!(statements.len(), 3);
        assert!(statements[0].contains("'a;b'"));
        assert!(statements[1].contains("\"x;y\""));
        assert!(statements[2].contains("`a;b`"));
    }

    #[test]
    fn ignores_comment_only_tails() {
        assert!(split_statements("-- nothing here\n").is_empty());
        assert!(split_statements("").is_empty());
    }
}
