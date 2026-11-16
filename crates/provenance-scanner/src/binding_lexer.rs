use std::ops::{ControlFlow, Range};

/// Finds a call outside strings and comments in one source line.
pub fn call_arguments<'a>(
    line: &'a str,
    starts_in_block_comment: bool,
    function: &str,
) -> Option<&'a str> {
    let marker = format!("{function}(");
    let bytes = line.as_bytes();
    let mut found = None;
    lex_line(
        line,
        starts_in_block_comment,
        CommentSyntax::CStyle,
        |range, piece| {
            if piece == Piece::Code {
                found = line[range.clone()]
                    .match_indices(&marker)
                    .map(|(offset, _)| range.start + offset)
                    .find(|start| is_name_start(bytes, *start));
            }
            if found.is_some() {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
    );
    found.map(|start| &line[start + marker.len()..])
}

/// Whether no word character comes before `start`, so a name starts there.
fn is_name_start(bytes: &[u8], start: usize) -> bool {
    let boundary = start.checked_sub(1).map(|previous| bytes[previous]);
    !boundary.is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// Carries C-style block-comment state between source lines.
pub fn block_comment_state(line: &str, starts_in_block_comment: bool) -> bool {
    lex_line(
        line,
        starts_in_block_comment,
        CommentSyntax::CStyle,
        |_, _| ControlFlow::Continue(()),
    )
}

/// The comments and quotes of a language for the line lexer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CommentSyntax {
    /// `//` and `/* */` comments; `'`, `"`, and `` ` `` quote a string.
    CStyle,
    /// `#` comments; `'`, `"`, and `` ` `` quote a string.
    Hash,
}

/// A part of a source line outside comments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Code,
    /// A string or a character literal, with its quotes.
    Quoted,
}

/// Gives each part of `line` outside comments to `visit`, in line order, and
/// returns whether a block comment is open at the end of the line. A quote
/// that does not close on the line quotes the rest of the line. `visit` can
/// stop the read; the returned state is then not known.
pub fn lex_line(
    line: &str,
    starts_in_block_comment: bool,
    syntax: CommentSyntax,
    mut visit: impl FnMut(Range<usize>, Piece) -> ControlFlow<()>,
) -> bool {
    let bytes = line.as_bytes();
    let slashes = syntax != CommentSyntax::Hash;
    let mut in_block_comment = starts_in_block_comment;
    let mut code_start = None;
    let mut quote: Option<(u8, usize)> = None;
    let mut escaped = false;
    let mut idx = 0;
    macro_rules! emit {
        ($range:expr, $piece:expr) => {
            if visit($range, $piece).is_break() {
                return in_block_comment;
            }
        };
    }
    macro_rules! end_code {
        () => {
            if let Some(start) = code_start.take() {
                emit!(start..idx, Piece::Code);
            }
        };
    }
    while idx < bytes.len() {
        if in_block_comment {
            if bytes[idx..].starts_with(b"*/") {
                in_block_comment = false;
                idx += 2;
            } else {
                idx += 1;
            }
            continue;
        }
        if let Some((active_quote, start)) = quote {
            if escaped {
                escaped = false;
            } else if bytes[idx] == b'\\' {
                escaped = true;
            } else if bytes[idx] == active_quote {
                quote = None;
                emit!(start..idx + 1, Piece::Quoted);
            }
            idx += 1;
            continue;
        }
        let comment = if slashes {
            bytes[idx..].starts_with(b"//")
        } else {
            bytes[idx] == b'#'
        };
        if comment {
            break;
        }
        if slashes && bytes[idx..].starts_with(b"/*") {
            end_code!();
            in_block_comment = true;
            idx += 2;
            continue;
        }
        if matches!(bytes[idx], b'\'' | b'"' | b'`') {
            end_code!();
            quote = Some((bytes[idx], idx));
            idx += 1;
            continue;
        }
        code_start.get_or_insert(idx);
        idx += 1;
    }
    end_code!();
    if let Some((_, start)) = quote {
        emit!(start..bytes.len(), Piece::Quoted);
    }
    in_block_comment
}

#[derive(Clone, Copy)]
pub enum MultilineStyle {
    Backtick,
    TripleBoth,
    TripleDouble,
}

pub fn unclosed_multiline_delimiter(line: &str, style: MultilineStyle) -> Option<(String, usize)> {
    let delimiters = match style {
        MultilineStyle::Backtick => &["`"][..],
        MultilineStyle::TripleBoth => &["'''", "\"\"\""][..],
        MultilineStyle::TripleDouble => &["\"\"\""][..],
    };
    delimiters
        .iter()
        .filter_map(|delimiter| {
            unpaired_delimiter_start(line, delimiter, matches!(style, MultilineStyle::TripleBoth))
                .map(|start| ((*delimiter).to_string(), start))
        })
        .min_by_key(|(_, start)| *start)
}

pub fn code_outside_multiline_string<'a>(
    line: &'a str,
    style: MultilineStyle,
    active_delimiter: &mut Option<String>,
    starts_in_block_comment: bool,
) -> Option<&'a str> {
    multiline_parts(line, style, active_delimiter, starts_in_block_comment)
        .map(|parts| &line[parts.code])
}

/// The parts of one line around multiline strings.
pub struct MultilineParts {
    /// The text outside multiline strings.
    pub code: Range<usize>,
    /// The text of a multiline string that opens on the line and does not
    /// close on it, with its delimiter.
    pub opened: Option<Range<usize>>,
}

/// The parts of `line` around multiline strings, with the delimiter of the
/// string that is open before the line. `None` when the line is all inside
/// a string.
pub fn multiline_parts(
    line: &str,
    style: MultilineStyle,
    active_delimiter: &mut Option<String>,
    starts_in_block_comment: bool,
) -> Option<MultilineParts> {
    let mut code = 0..line.len();
    if let Some(delimiter) = active_delimiter.as_deref() {
        code.start = line.find(delimiter)? + delimiter.len();
        *active_delimiter = None;
    }
    if starts_in_block_comment {
        return Some(MultilineParts { code, opened: None });
    }
    let opened =
        unclosed_multiline_delimiter(&line[code.clone()], style).map(|(delimiter, opening)| {
            *active_delimiter = Some(delimiter);
            code.start + opening..line.len()
        });
    if let Some(opened) = &opened {
        code.end = opened.start;
    }
    Some(MultilineParts { code, opened })
}

/// Everything between a delimiter pair is string content: quotes, comment
/// openers, and hash marks inside it must not derail the scan for the
/// closing delimiter (a Go `` `"` `` literal is closed, not an open quote).
fn unpaired_delimiter_start(
    line: &str,
    delimiter: &str,
    hash_starts_comment: bool,
) -> Option<usize> {
    let bytes = line.as_bytes();
    let marker = delimiter.as_bytes();
    let mut positions = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut idx = 0;
    while idx < bytes.len() {
        if positions.len() % 2 == 1 {
            if bytes[idx..].starts_with(marker) {
                positions.push(idx);
                idx += marker.len();
            } else {
                idx += 1;
            }
            continue;
        }
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if bytes[idx] == b'\\' {
                escaped = true;
            } else if bytes[idx] == active_quote {
                quote = None;
            }
            idx += 1;
            continue;
        }
        if bytes[idx..].starts_with(b"//") {
            break;
        }
        if bytes[idx..].starts_with(b"/*") {
            let Some(end) = line[idx + 2..].find("*/") else {
                break;
            };
            idx = idx + 2 + end + 2;
            continue;
        }
        if hash_starts_comment && bytes[idx] == b'#' {
            break;
        }
        if bytes[idx..].starts_with(marker) {
            positions.push(idx);
            idx += marker.len();
            continue;
        }
        if matches!(bytes[idx], b'\'' | b'"') {
            quote = Some(bytes[idx]);
        }
        idx += 1;
    }
    (positions.len() % 2 == 1).then(|| positions[positions.len() - 1])
}

pub fn rust_character_literal_end(line: &str, index: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let content_end = if bytes.get(index) == Some(&b'\\') {
        match bytes.get(index + 1) {
            Some(b'x') => index + 4,
            Some(b'u') if bytes.get(index + 2) == Some(&b'{') => {
                index + 3 + bytes[index + 3..].iter().position(|byte| *byte == b'}')? + 1
            }
            Some(_) => index + 2,
            None => return None,
        }
    } else {
        index + line.get(index..)?.chars().next()?.len_utf8()
    };
    (bytes.get(content_end) == Some(&b'\'')).then_some(content_end)
}
