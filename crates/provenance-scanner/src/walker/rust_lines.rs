//! Per-line lexical states for Rust source.
//!
//! `rust_line_states` records, for each line, whether the line starts in
//! code, a string, a raw string, or a (possibly nested) block comment. The
//! walker gates both channels on these states so directives and attributes
//! inside strings never bind.

use super::Language;
use std::ops::Range;

use crate::binding_lexer::{rust_character_literal_end, Piece};
use crate::language::CommentSyntax;
use crate::parser::annotation_marker_positions;
use crate::string_context::marker_is_inside_quoted_region;

pub(super) fn rust_line_states(language: Language, lines: &[&str]) -> Vec<RustLexicalState> {
    let mut state = RustLexicalState::Code;
    lines
        .iter()
        .map(|line| {
            let line_state = state;
            if language == Language::Rust {
                advance_rust_state(line, &mut state);
            }
            line_state
        })
        .collect()
}

pub(super) fn rust_annotation_marker_position(
    line: &str,
    initial_state: RustLexicalState,
) -> Option<usize> {
    annotation_marker_positions(line)
        .filter(|position| {
            let mut state = initial_state;
            advance_rust_state(&line[..*position], &mut state);
            let closed_multiline_string = matches!(
                initial_state,
                RustLexicalState::Quoted | RustLexicalState::Raw(_)
            ) && state == RustLexicalState::Code;
            !matches!(state, RustLexicalState::Quoted | RustLexicalState::Raw(_))
                && (closed_multiline_string || !marker_is_inside_quoted_region(line, *position))
        })
        .min()
}

/// The comments of Rust. Its block comments nest, so a state counts the depth.
const RUST_COMMENTS: CommentSyntax = Language::Rust.comment_syntax();

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RustLexicalState {
    Code,
    Quoted,
    Raw(usize),
    BlockComment(usize),
}

fn advance_rust_state(line: &str, state: &mut RustLexicalState) {
    rust_pieces(line, state, |_, _| {});
}

/// Advances `state` over `line` and gives each part of the line outside
/// comments to `visit`, in line order. A string is one quoted part, on the
/// line where it opens.
pub(super) fn rust_pieces(
    line: &str,
    state: &mut RustLexicalState,
    mut visit: impl FnMut(Range<usize>, Piece),
) {
    let bytes = line.as_bytes();
    let mut code_start = None;
    let mut quote_start = None;
    let mut index = 0;
    while index < bytes.len() {
        match *state {
            RustLexicalState::Code => {
                let rest = &bytes[index..];
                if RUST_COMMENTS.starts_line_comment(rest) {
                    break;
                }
                if let Some(open) = RUST_COMMENTS.block_open_at(rest) {
                    end_code(index, &mut code_start, &mut visit);
                    *state = RustLexicalState::BlockComment(1);
                    index += open;
                    continue;
                }
                match bytes[index] {
                    b'\'' => {
                        if let Some(end) = rust_character_literal_end(line, index + 1) {
                            end_code(index, &mut code_start, &mut visit);
                            visit(index..end + 1, Piece::Quoted);
                            index = end + 1;
                        } else {
                            code_start.get_or_insert(index);
                            index += 1;
                        }
                    }
                    b'"' => {
                        end_code(index, &mut code_start, &mut visit);
                        quote_start = Some(index);
                        *state = RustLexicalState::Quoted;
                        index += 1;
                    }
                    b'r' => {
                        if let Some(hashes) = raw_string_hashes(&bytes[index + 1..]) {
                            end_code(index, &mut code_start, &mut visit);
                            quote_start = Some(index);
                            *state = RustLexicalState::Raw(hashes);
                            index += hashes + 2;
                        } else {
                            code_start.get_or_insert(index);
                            index += 1;
                        }
                    }
                    _ => {
                        code_start.get_or_insert(index);
                        index += 1;
                    }
                }
            }
            RustLexicalState::Quoted => match bytes[index] {
                b'\\' => index = (index + 2).min(bytes.len()),
                b'"' => {
                    *state = RustLexicalState::Code;
                    index += 1;
                    if let Some(start) = quote_start.take() {
                        visit(start..index, Piece::Quoted);
                    }
                }
                _ => index += 1,
            },
            RustLexicalState::Raw(hashes) => {
                let closes = bytes[index] == b'"'
                    && bytes[index + 1..]
                        .iter()
                        .take(hashes)
                        .all(|byte| *byte == b'#')
                    && bytes.len().saturating_sub(index + 1) >= hashes;
                if closes {
                    *state = RustLexicalState::Code;
                    index += hashes + 1;
                    if let Some(start) = quote_start.take() {
                        visit(start..index, Piece::Quoted);
                    }
                } else {
                    index += 1;
                }
            }
            RustLexicalState::BlockComment(depth) => {
                let (next, length) = block_comment_step(&bytes[index..], depth);
                *state = next;
                index += length;
            }
        }
    }
    end_code(index.min(bytes.len()), &mut code_start, &mut visit);
    if let Some(start) = quote_start {
        visit(start..bytes.len(), Piece::Quoted);
    }
}

/// The state after the text at the start of `rest`, in a block comment at
/// depth `depth`, and the length of that text.
fn block_comment_step(rest: &[u8], depth: usize) -> (RustLexicalState, usize) {
    match (
        RUST_COMMENTS.block_open_at(rest),
        RUST_COMMENTS.block_close_at(rest),
    ) {
        (Some(open), _) => (RustLexicalState::BlockComment(depth + 1), open),
        (None, Some(close)) if depth == 1 => (RustLexicalState::Code, close),
        (None, Some(close)) => (RustLexicalState::BlockComment(depth - 1), close),
        (None, None) => (RustLexicalState::BlockComment(depth), 1),
    }
}

/// Gives the code part that starts at `code_start` and ends at `index`.
fn end_code(
    index: usize,
    code_start: &mut Option<usize>,
    visit: &mut impl FnMut(Range<usize>, Piece),
) {
    if let Some(start) = code_start.take() {
        visit(start..index, Piece::Code);
    }
}

/// The number of `#` after an `r` when a raw string opens there.
fn raw_string_hashes(after_r: &[u8]) -> Option<usize> {
    let hashes = after_r.iter().take_while(|byte| **byte == b'#').count();
    (after_r.get(hashes) == Some(&b'"')).then_some(hashes)
}
