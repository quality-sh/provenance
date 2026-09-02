//! The scanner half of the no-wildcard gate.
//!
//! Known limits, declared: the scanner reads source text, not the type
//! system. It does not see `matches!` invocations, `if let` patterns, or a
//! vocabulary type renamed through `use ... as`. Those stay covered by the
//! review convention that traversal code matches the vocabulary
//! exhaustively; the gate closes the front door, not every window.

/// One offending wildcard or catch-all arm: its line number in the scanned
/// text and the arm's pattern.
#[derive(Debug, PartialEq, Eq)]
pub struct Offender {
    pub line: usize,
    pub pattern: String,
}

/// Drops test modules so the gate reads only production lines.
///
/// Handles `#[cfg(test)]` followed by an inline `mod` block, whether the
/// opening brace sits on the `mod` line or a later line, and skips the
/// declaration line of a file-referenced test module.
pub fn production_lines(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        if !line.trim_start().starts_with("#[cfg(test)]") {
            output.push_str(line);
            output.push('\n');
            continue;
        }
        let Some(next) = lines.peek() else {
            continue;
        };
        if !next.trim_start().starts_with("mod ") {
            continue;
        }
        // Consume the mod declaration; a `mod name;` line references a file
        // the path rules already exclude.
        let declaration = lines.next().expect("peeked");
        if declaration.trim_end().ends_with(';') {
            continue;
        }
        // A one-line module, `mod tests {}`, opened and closed already.
        if declaration.contains('{') && brace_delta(declaration) == 0 {
            continue;
        }
        let mut depth = brace_delta(declaration);
        while depth == 0 {
            let Some(line) = lines.next() else {
                return output;
            };
            depth += brace_delta(line);
            if depth > 0 {
                break;
            }
        }
        while depth > 0 {
            let Some(line) = lines.next() else {
                return output;
            };
            depth += brace_delta(line);
        }
    }
    output
}

fn brace_delta(line: &str) -> i64 {
    let opens = i64::try_from(line.matches('{').count()).unwrap_or(i64::MAX);
    let closes = i64::try_from(line.matches('}').count()).unwrap_or(0);
    opens - closes
}

/// Finds wildcard and catch-all arms in every `match` over the closed
/// vocabularies. A match belongs to a vocabulary when its scrutinee names
/// the type, when any arm pattern names a variant, or when arm patterns use
/// `Self::` inside a file that implements the vocabulary type.
pub fn wildcard_arms(source: &str) -> Vec<Offender> {
    let self_is_vocabulary =
        source.contains("impl NodeType") || source.contains("impl RelationKind");
    let mut offenders = Vec::new();
    let mut search_from = 0usize;
    while let Some(position) = source[search_from..].find("match ") {
        let start = search_from + position;
        let Some(brace_offset) = source[start..].find('{') else {
            break;
        };
        let scrutinee = &source[start..start + brace_offset];
        let body_start = start + brace_offset + 1;
        let body_end = matching_brace(source, body_start);
        let body = &source[body_start..body_end];
        let arms = split_arms(body);
        let vocabulary = scrutinee.contains("NodeType")
            || scrutinee.contains("RelationKind")
            || arms.iter().any(|arm| {
                arm.pattern.contains("NodeType::")
                    || arm.pattern.contains("RelationKind::")
                    || (self_is_vocabulary && arm.pattern.contains("Self::"))
            });
        if vocabulary {
            for arm in &arms {
                if is_catch_all(&arm.pattern) {
                    offenders.push(Offender {
                        line: source[..body_start + arm.offset].lines().count(),
                        pattern: arm.pattern.trim().to_string(),
                    });
                }
            }
        }
        search_from = body_start;
    }
    offenders
}

const fn matching_brace(source: &str, body_start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut depth = 1i64;
    let mut cursor = body_start;
    while cursor < bytes.len() && depth > 0 {
        match bytes[cursor] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        cursor += 1;
    }
    cursor.saturating_sub(1)
}

struct Arm {
    pattern: String,
    offset: usize,
}

/// Splits a match body into arms, keeping only the pattern half of each.
/// Arm expressions — including nested matches — are skipped by bracket
/// depth, so an inner match never leaks arms into the outer one.
fn split_arms(body: &str) -> Vec<Arm> {
    let bytes = body.as_bytes();
    let mut arms = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let pattern_start = cursor;
        // Pattern runs to the arm arrow at depth zero.
        let mut depth = 0i64;
        let mut arrow = None;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'{' | b'(' | b'[' => depth += 1,
                b'}' | b')' | b']' => depth -= 1,
                b'=' if depth == 0
                    && cursor + 1 < bytes.len()
                    && bytes[cursor + 1] == b'>'
                    && cursor > 0
                    && bytes[cursor - 1] != b'>'
                    && bytes[cursor - 1] != b'<'
                    && bytes[cursor - 1] != b'=' =>
                {
                    arrow = Some(cursor);
                    break;
                }
                _ => {}
            }
            cursor += 1;
        }
        let Some(arrow) = arrow else {
            break;
        };
        arms.push(Arm {
            pattern: body[pattern_start..arrow].to_string(),
            offset: pattern_start,
        });
        // Skip the arm expression: a braced block, or text to the next
        // depth-zero comma.
        cursor = arrow + 2;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor < bytes.len() && bytes[cursor] == b'{' {
            cursor = matching_brace(body, cursor + 1) + 1;
            if cursor < bytes.len() && bytes[cursor] == b',' {
                cursor += 1;
            }
            continue;
        }
        let mut depth = 0i64;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'{' | b'(' | b'[' => depth += 1,
                b'}' | b')' | b']' => depth -= 1,
                b',' if depth == 0 => {
                    cursor += 1;
                    break;
                }
                _ => {}
            }
            cursor += 1;
        }
    }
    arms
}

/// Whether an arm pattern is a wildcard or a named catch-all, with or
/// without a guard.
fn is_catch_all(pattern: &str) -> bool {
    let pattern = pattern.trim();
    let head = pattern.split(" if ").next().unwrap_or(pattern).trim();
    if head == "_" {
        return true;
    }
    if head.split('|').any(|alternative| alternative.trim() == "_") {
        return true;
    }
    // A bare lowercase identifier binds everything: a catch-all by name.
    !head.is_empty()
        && head
            .chars()
            .all(|character| character.is_ascii_lowercase() || character == '_')
        && head != "true"
        && head != "false"
}
