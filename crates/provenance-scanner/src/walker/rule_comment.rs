//! Where a Rule comment starts and where it ends.
//!
//! A comment line starts with a comment of its language, or a block comment
//! of its language is open at its start. A Rule comment starts on a comment
//! line that holds a marker and continues on each line below that starts
//! with a line comment or is inside an open block comment. It ends at a line
//! where code follows the comment.

use std::ops::{ControlFlow, Range};

use provenance_macros::rule;

use super::rust_lines::{rust_annotation_marker_position, rust_pieces, RustLexicalState};
use super::Language;
use crate::binding_lexer::lex_line;
use crate::language::CommentSyntax;
use crate::parser::annotation_marker_position;

/// Finds the annotation marker on a line the comment-line gate admits.
///
/// The gate is `rule_prov_annot_014`: a directive binds only when, after
/// leading whitespace, the line starts with a comment of its language, or a
/// block comment is open at its start. A marker trailing code on the same
/// line never binds. Past the gate, markers sitting inside string literals
/// quoted within the comment are still skipped.
pub(super) fn annotation_marker_start(
    language: Language,
    line: &str,
    rust_state: RustLexicalState,
    started_in_block_comment: bool,
    started_in_multiline_string: bool,
) -> Option<usize> {
    let starts_comment = language
        .comment_syntax()
        .starts_comment(line.trim_start().as_bytes());
    if language == Language::Rust {
        let in_string = matches!(
            rust_state,
            RustLexicalState::Quoted | RustLexicalState::Raw(_)
        );
        let in_comment = matches!(rust_state, RustLexicalState::BlockComment(_));
        return (!in_string && (in_comment || starts_comment))
            .then(|| rust_annotation_marker_position(line, rust_state))
            .flatten();
    }
    (!started_in_multiline_string && (started_in_block_comment || starts_comment))
        .then(|| annotation_marker_position(line))
        .flatten()
}

/// The comment state at the start of a line.
#[derive(Clone, Copy)]
pub(super) enum CommentState {
    /// The state of the Rust lexer.
    Rust(RustLexicalState),
    /// Whether a block comment of `syntax` is open.
    Lexed { syntax: CommentSyntax, open: bool },
}

impl CommentState {
    /// The comment state at the start of a line of `language`.
    pub(super) const fn new(
        language: Language,
        rust_state: RustLexicalState,
        in_block_comment: bool,
    ) -> Self {
        match language {
            Language::Rust => Self::Rust(rust_state),
            Language::Python
            | Language::JavaScript
            | Language::TypeScript
            | Language::Java
            | Language::Go => Self::Lexed {
                syntax: language.comment_syntax(),
                open: in_block_comment,
            },
        }
    }

    const fn in_block_comment(self) -> bool {
        match self {
            Self::Rust(state) => matches!(state, RustLexicalState::BlockComment(_)),
            Self::Lexed { open, .. } => open,
        }
    }

    /// Reads `line` from this state: the offset of the first code or string
    /// on the line. With no code on the line, the state moves to the end of
    /// the line.
    fn first_code(&mut self, line: &str) -> Option<usize> {
        let mut first = None;
        let mut find = |range: Range<usize>| {
            let part = &line[range.clone()];
            let blank = part.len() - part.trim_start().len();
            if first.is_none() && blank < part.len() {
                first = Some(range.start + blank);
            }
            first.is_some()
        };
        match self {
            Self::Rust(state) => rust_pieces(line, state, |range, _| {
                find(range);
            }),
            Self::Lexed { syntax, open } => {
                *open = lex_line(line, *open, *syntax, |range, _| {
                    if find(range) {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                });
            }
        }
        first
    }
}

/// The text and the extent of one Rule comment.
pub(super) struct RuleComment {
    /// The comment from its marker, one line of text for each line, without
    /// the close of a block comment.
    pub(super) text: String,
    /// The index of the last line of the comment.
    pub(super) end: usize,
    /// The index of the first line that can hold the item of the comment:
    /// the line of the comment when code follows the comment on that line.
    pub(super) below: usize,
}

/// The Rule comment whose marker is at `marker_position` on line `start`,
/// read from the comment state `state` at the start of that line. `None`
/// when code comes before the marker on its line.
#[rule("rule_comment_ends_above_the_first_code_line")]
pub(super) fn rule_comment(
    language: Language,
    lines: &[&str],
    start: usize,
    marker_position: usize,
    mut state: CommentState,
) -> Option<RuleComment> {
    let syntax = language.comment_syntax();
    let mut text = String::new();
    let mut end = start;
    let mut from = marker_position;
    loop {
        let line = lines[end];
        let in_block = state.in_block_comment()
            || syntax.block_open_at(line.trim_start().as_bytes()).is_some();
        let code = state.first_code(line);
        if code.is_some_and(|code| code < from) {
            return None;
        }
        let part = &line[from..code.unwrap_or(line.len())];
        let closes_block = code.is_some() || (in_block && !state.in_block_comment());
        text.push_str(if closes_block {
            without_block_close(part, syntax)
        } else {
            part
        });
        if code.is_some() {
            let below = if end == start { start } else { start + 1 };
            return Some(RuleComment { text, end, below });
        }
        let continues = lines.get(end + 1).is_some_and(|next| {
            state.in_block_comment() || syntax.starts_line_comment(next.trim_start().as_bytes())
        });
        if !continues {
            return Some(RuleComment {
                text,
                end,
                below: start + 1,
            });
        }
        text.push('\n');
        end += 1;
        from = 0;
    }
}

/// `part`, the comment part of a line where a block comment closes, without
/// the close.
fn without_block_close(part: &str, syntax: CommentSyntax) -> &str {
    syntax
        .block
        .and_then(|block| part.trim_end().strip_suffix(block.close))
        .unwrap_or(part)
}
