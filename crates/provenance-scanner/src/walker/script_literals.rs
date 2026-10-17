//! Removes regex contents before the shared comment and binding lexers run.
use crate::parser::ParseWarning;
use provenance_macros::rule;

/// Keeps regex contents out of string and comment scans without moving source anchors.
#[rule("rule_prov_annot_regex")]
pub fn mask_regex_literals(source: &str) -> (String, Vec<ParseWarning>) {
    let bytes = source.as_bytes();
    let mut masked = bytes.to_vec();
    let mut warnings = Vec::new();
    let mut expression_start = true;
    let mut after_dot = false;
    let mut line = 1;
    let mut index = 0;
    if bytes.starts_with(b"#!") {
        index = line_end(bytes, 0);
        masked[..index].fill(b' ');
    }
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                line += 1;
                index += 1;
            }
            byte if byte.is_ascii_whitespace() => index += 1,
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = line_end(bytes, index);
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index < bytes.len() {
                    if bytes[index..].starts_with(b"*/") {
                        index += 2;
                        break;
                    }
                    line += usize::from(bytes[index] == b'\n');
                    index += 1;
                }
            }
            b'\'' | b'"' | b'`' => {
                let opening_line = line;
                let quote = bytes[index];
                let (end, closed) = quoted_end(bytes, index, &mut line);
                if !closed && quote == b'`' {
                    warnings.push(unterminated(opening_line, "template literal"));
                }
                index = end;
                expression_start = false;
                after_dot = false;
                // A template can continue across lines. An unclosed template
                // consumes the file, but the warning identifies its opening.
                if !closed && quote == b'`' {
                    break;
                }
            }
            b'/' if expression_start && (index == 0 || bytes[index - 1] != b'<') => {
                let (end, closed) = regex_end(bytes, index);
                if closed {
                    masked[index..end].fill(b' ');
                    masked[index] = b'0';
                    index = end;
                } else {
                    // An unclosed candidate can be division or JSX. Keep its bytes.
                    index += 1;
                }
                expression_start = false;
                after_dot = false;
            }
            byte if identifier_byte(byte) => {
                let start = index;
                index += 1;
                while index < bytes.len() && identifier_byte(bytes[index]) {
                    index += 1;
                }
                expression_start = !after_dot
                    && matches!(
                        &source[start..index],
                        "return"
                            | "throw"
                            | "case"
                            | "delete"
                            | "void"
                            | "typeof"
                            | "new"
                            | "in"
                            | "instanceof"
                            | "yield"
                            | "await"
                    );
                after_dot = false;
            }
            b'+' | b'-' if bytes.get(index + 1) == Some(&bytes[index]) => {
                // Prefix operators still need an operand; postfix operators
                // retain the preceding value as the left side of division.
                index += 2;
                after_dot = false;
            }
            byte => {
                expression_start = !matches!(byte, b')' | b']' | b'}' | b'.');
                after_dot = byte == b'.';
                index += 1;
            }
        }
    }
    // Masked spans have ASCII boundaries and are fully replaced, so UTF-8 stays valid.
    (
        String::from_utf8(masked).expect("masked source is UTF-8"),
        warnings,
    )
}

const fn identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte >= 128
}

fn unterminated(line: usize, kind: &str) -> ParseWarning {
    ParseWarning {
        line,
        message: format!("unterminated {kind}; coverage scan can be incomplete"),
    }
}

const fn line_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && !matches!(bytes[index], b'\n' | b'\r') {
        index += 1;
    }
    index
}

const fn regex_end(bytes: &[u8], start: usize) -> (usize, bool) {
    let mut index = start + 1;
    let mut in_class = false;
    while index < bytes.len() {
        match bytes[index] {
            b'\n' | b'\r' => return (index, false),
            b'\\' => {
                index += 1;
                if index < bytes.len() && !matches!(bytes[index], b'\n' | b'\r') {
                    index += 1;
                }
            }
            b'[' => {
                in_class = true;
                index += 1;
            }
            b']' => {
                in_class = false;
                index += 1;
            }
            b'/' if !in_class => {
                index += 1;
                while index < bytes.len() && identifier_byte(bytes[index]) {
                    index += 1;
                }
                return (index, true);
            }
            _ => index += 1,
        }
    }
    (index, false)
}

fn quoted_end(bytes: &[u8], start: usize, line: &mut usize) -> (usize, bool) {
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            byte if byte == quote => return (index + 1, true),
            b'\n' | b'\r' if quote != b'`' => return (index, false),
            b'\\' => {
                index += 1;
                if index < bytes.len() {
                    *line += usize::from(bytes[index] == b'\n');
                    index += 1;
                }
            }
            byte => {
                *line += usize::from(byte == b'\n');
                index += 1;
            }
        }
    }
    (index, false)
}
