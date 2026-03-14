//! Pure source-text analysis functions — no AST.

pub(super) fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Detect trailing dot pattern in source text before cursor.
/// Returns (receiver_expr, member_prefix).
pub(super) fn detect_trailing_dot_in_text(before_cursor: &str) -> Option<(String, String)> {
    let s = before_cursor.trim_end();
    if s.is_empty() {
        return None;
    }
    let last_line = s.rsplit('\n').next().unwrap_or(s);
    let bytes = last_line.as_bytes();
    let len = bytes.len();

    let mut i = len;
    while i > 0 && is_ident_char(bytes[i - 1]) {
        i -= 1;
    }
    let member_prefix = last_line[i..].to_string();

    if i == 0 || bytes[i - 1] != b'.' {
        return None;
    }
    let dot_pos = i - 1;

    if !member_prefix
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }

    let before_dot = &last_line[..dot_pos];
    let receiver_expr = extract_last_expr_from_text(before_dot)?;

    if receiver_expr.is_empty() {
        return None;
    }

    Some((receiver_expr, member_prefix))
}

/// Extract the last complete expression from the end of a string.
/// `"class A { void f() { this"` → `"this"`
/// `"{ RealMain.getInstance()"` → `"RealMain.getInstance()"`
pub(super) fn extract_last_expr_from_text(s: &str) -> Option<String> {
    let s = s.trim_end();
    if s.is_empty() {
        return None;
    }
    let bytes = s.as_bytes();
    let end = bytes.len();
    let mut paren_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut i = end;

    while i > 0 {
        i -= 1;
        match bytes[i] {
            b')' => paren_depth += 1,
            b'(' => {
                if paren_depth > 0 {
                    paren_depth -= 1;
                } else {
                    return non_empty(s[i + 1..end].trim());
                }
            }
            b'}' => brace_depth += 1,
            b'{' => {
                if brace_depth > 0 {
                    brace_depth -= 1;
                } else {
                    return non_empty(s[i + 1..end].trim());
                }
            }
            b';' if paren_depth == 0 && brace_depth == 0 => {
                return non_empty(s[i + 1..end].trim());
            }
            b'\n' if paren_depth == 0 && brace_depth == 0 => {
                return non_empty(s[i + 1..end].trim());
            }
            // lambda arrow `->`
            b'>' if paren_depth == 0 && brace_depth == 0 && i > 0 && bytes[i - 1] == b'-' => {
                return non_empty(s[i + 1..end].trim());
            }
            // assignment `=` (not `==`, `!=`, `<=`, `>=`)
            b'=' if paren_depth == 0 && brace_depth == 0 => {
                let prev = if i > 0 { bytes[i - 1] } else { 0 };
                let next = if i + 1 < end { bytes[i + 1] } else { 0 };
                if !matches!(prev, b'=' | b'!' | b'<' | b'>') && next != b'=' {
                    return non_empty(s[i + 1..end].trim());
                }
            }
            _ => {}
        }
    }

    non_empty(s[..end].trim())
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Detect `new ClassName` pattern before cursor.
/// Returns (class_prefix, expected_type_hint).
pub(super) fn detect_new_keyword_before_cursor(
    before_cursor: &str,
) -> Option<(String, Option<String>)> {
    let s = before_cursor.trim_end();
    if s.is_empty() {
        return None;
    }

    let last_line = s.rsplit('\n').next().unwrap_or(s).trim_start();

    let new_start = find_new_token_pos(last_line)?;
    let after_new = last_line[new_start + 3..].trim_start();

    if !after_new.is_empty() {
        let first = after_new.chars().next().unwrap();
        if !first.is_ascii_alphabetic() && first != '_' && first != '$' {
            return None;
        }
    }

    let class_prefix: String = after_new
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$' || *c == '.')
        .collect();

    Some((class_prefix, None))
}

/// Find position of `new` token in string (as standalone keyword).
fn find_new_token_pos(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        if &bytes[i..i + 3] == b"new" {
            let before_ok = i == 0 || !is_ident_char(bytes[i - 1]);
            let after_ok = i + 3 >= bytes.len() || !is_ident_char(bytes[i + 3]);
            if before_ok && after_ok {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_last_expr_simple_identifier() {
        assert_eq!(
            extract_last_expr_from_text("class A { void f() { this"),
            Some("this".to_string())
        );
    }

    #[test]
    fn test_extract_last_expr_method_call() {
        assert_eq!(
            extract_last_expr_from_text("{ RealMain.getInstance()"),
            Some("RealMain.getInstance()".to_string())
        );
    }

    #[test]
    fn test_extract_last_expr_after_semicolon() {
        assert_eq!(
            extract_last_expr_from_text("int x = 1; obj"),
            Some("obj".to_string())
        );
    }

    #[test]
    fn test_detect_trailing_dot_simple() {
        assert_eq!(
            detect_trailing_dot_in_text("class A { void f() { cl."),
            Some(("cl".to_string(), String::new()))
        );
    }

    #[test]
    fn test_detect_trailing_dot_with_prefix() {
        assert_eq!(
            detect_trailing_dot_in_text("class A { void f() { a.p"),
            Some(("a".to_string(), "p".to_string()))
        );
    }

    #[test]
    fn test_detect_trailing_dot_this() {
        assert_eq!(
            detect_trailing_dot_in_text("class A { void f() { this."),
            Some(("this".to_string(), String::new()))
        );
    }

    #[test]
    fn test_detect_trailing_dot_chained() {
        assert_eq!(
            detect_trailing_dot_in_text("class A { void f() { RealMain.getInstance()."),
            Some(("RealMain.getInstance()".to_string(), String::new()))
        );
    }

    #[test]
    fn test_detect_new_simple() {
        assert_eq!(
            detect_new_keyword_before_cursor("class A { void f() { new RandomCla"),
            Some(("RandomCla".to_string(), None))
        );
    }

    #[test]
    fn test_detect_new_empty() {
        assert_eq!(
            detect_new_keyword_before_cursor("class A { void f() { new "),
            Some((String::new(), None))
        );
    }

    #[test]
    fn test_detect_new_with_newline_rejects() {
        // `new\nFoo` should not be treated as constructor
        assert_eq!(
            detect_new_keyword_before_cursor("class A { void f() { new\nFoo"),
            None
        );
    }
}
