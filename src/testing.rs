//! Snapshot testing utilities and ANSI stripping for golden CLI tests.

/// Strips ANSI escape sequences (colors, styling, OSC 8 links) from a rendered string.
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_escape = false;
    let mut in_osc8 = false;

    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if let Some(&']') = chars.peek() {
                chars.next(); // consume ']'
                in_osc8 = true;
                continue;
            } else if let Some(&'[') = chars.peek() {
                chars.next(); // consume '['
                in_escape = true;
                continue;
            }
        }

        if in_escape {
            if ch.is_ascii_alphabetic() || ch == 'm' {
                in_escape = false;
            }
            continue;
        }

        if in_osc8 {
            if ch == '\x07' || ch == '\\' {
                in_osc8 = false;
            }
            continue;
        }

        out.push(ch);
    }

    out
}

/// Asserts that a rendered CLI component output matches the expected plain-text representation.
#[macro_export]
macro_rules! assert_plain_snapshot {
    ($actual:expr, $expected:expr) => {
        let plain = $crate::testing::strip_ansi(&$actual);
        assert_eq!(plain.trim(), $expected.trim());
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_colors() {
        let colored = "\x1b[31mError\x1b[0m occurred";
        assert_eq!(strip_ansi(colored), "Error occurred");
    }

    #[test]
    fn test_strip_ansi_osc8_hyperlinks() {
        let linked = "\x1b]8;;https://example.com\x1b\\\x1b[36mexample\x1b[0m\x1b]8;;\x1b\\";
        assert_eq!(strip_ansi(linked), "example");
    }
}
