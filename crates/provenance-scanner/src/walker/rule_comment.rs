//! Where a Rule comment starts and where it ends.
//!
//! A comment line starts with the line-comment prefix or the block-comment
//! open of its language, or a block comment of its language is open at its
//! start. A Rule comment starts on a comment line that holds a marker and
//! continues on each comment line below. It ends at a line where code follows
//! the comment. The text of a block comment stops at its close.

use std::ops::{ControlFlow, Range};

use provenance_macros::rule;

use super::rust_lines::{rust_annotation_marker_position, rust_pieces, RustLexicalState};
use crate::binding_lexer::{lex_line, Piece};
use crate::language::CommentSyntax;
use crate::parser::annotation_marker_position;
use crate::Language;

/// The lexical state at the start of a line.
#[derive(Clone, Copy)]
pub(super) struct LineStart {
    /// The state of the Rust lexer; `Code` in the other languages.
    pub(super) rust_state: RustLexicalState,
    /// Whether a block comment is open, in a language other than Rust.
    pub(super) in_block_comment: bool,
    /// Whether a multiline string is open, in a language other than Rust.
    pub(super) in_multiline_string: bool,
}

/// The Rule comment that starts on line `start`, when the line holds a
/// marker that the comment-line gate admits.
pub(super) fn rule_comment_at(
    language: Language,
    lines: &[&str],
    start: usize,
    at_start: LineStart,
) -> Option<RuleComment> {
    let state = CommentState::new(language, at_start);
    let marker_position = annotation_marker_start(language, lines[start], at_start, state)?;
    rule_comment(lines, start, marker_position, state)
}

/// Finds the annotation marker on a line the comment-line gate admits.
///
/// The gate is `rule_prov_annot_014`: a directive binds only when, after
/// leading whitespace, the line starts with a comment of its language, or a
/// block comment is open at its start. A marker trailing code on the same
/// line never binds. Past the gate, markers sitting inside string literals
/// quoted within the comment are still skipped.
fn annotation_marker_start(
    language: Language,
    line: &str,
    at_start: LineStart,
    state: CommentState,
) -> Option<usize> {
    let in_string = at_start.in_multiline_string
        || matches!(
            at_start.rust_state,
            RustLexicalState::Quoted | RustLexicalState::Raw(_)
        );
    if in_string || !state.is_comment_line(line) {
        return None;
    }
    match language {
        Language::Rust => rust_annotation_marker_position(line, at_start.rust_state),
        Language::Python
        | Language::JavaScript
        | Language::TypeScript
        | Language::Java
        | Language::Go => annotation_marker_position(line),
    }
}

/// The comment state at the start of a line.
#[derive(Clone, Copy)]
enum CommentState {
    /// The state of the Rust lexer.
    Rust(RustLexicalState),
    /// Whether a block comment of `syntax` is open.
    Lexed { syntax: CommentSyntax, open: bool },
}

impl CommentState {
    /// The comment state at the start of a line of `language`.
    const fn new(language: Language, at_start: LineStart) -> Self {
        match language {
            Language::Rust => Self::Rust(at_start.rust_state),
            Language::Python
            | Language::JavaScript
            | Language::TypeScript
            | Language::Java
            | Language::Go => Self::Lexed {
                syntax: language.comment_syntax(),
                open: at_start.in_block_comment,
            },
        }
    }

    /// Whether `line`, read from this state, is a comment line: it starts
    /// with the line-comment prefix or the block-comment open of its
    /// language, or a block comment is open at its start.
    fn is_comment_line(self, line: &str) -> bool {
        let syntax = match self {
            Self::Rust(RustLexicalState::BlockComment(_)) | Self::Lexed { open: true, .. } => {
                return true;
            }
            Self::Rust(_) => Language::Rust.comment_syntax(),
            Self::Lexed { syntax, .. } => syntax,
        };
        syntax.starts_comment(line.trim_start().as_bytes())
    }

    /// Reads `line` from this state. With no code on the line, the state
    /// moves to the end of the line.
    fn read(&mut self, line: &str, from: usize) -> LineRead {
        let mut read = LineRead {
            code: None,
            close: None,
        };
        let mut visit = |range: Range<usize>, piece: Piece| {
            let part = &line[range.clone()];
            let blank = part.len() - part.trim_start().len();
            match piece {
                Piece::BlockClose if range.start >= from => {
                    read.close.get_or_insert(range.start);
                }
                Piece::Code | Piece::Quoted if blank < part.len() => {
                    read.code.get_or_insert(range.start + blank);
                }
                Piece::BlockClose | Piece::Code | Piece::Quoted => {}
            }
            read.code.is_some()
        };
        match self {
            Self::Rust(state) => rust_pieces(line, state, |range, piece| {
                visit(range, piece);
            }),
            Self::Lexed { syntax, open } => {
                *open = lex_line(line, *open, *syntax, |range, piece| {
                    if visit(range, piece) {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                });
            }
        }
        read
    }
}

/// What a line holds, read from the comment state at its start.
struct LineRead {
    /// The offset of the first code or string on the line.
    code: Option<usize>,
    /// The offset of the first close of a block comment at or after the
    /// offset where the read of the comment starts.
    close: Option<usize>,
}

/// The text and the extent of one Rule comment.
pub(super) struct RuleComment {
    /// The comment from its marker, one line of text for each line. The text
    /// of a line stops at the close of a block comment.
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
fn rule_comment(
    lines: &[&str],
    start: usize,
    marker_position: usize,
    mut state: CommentState,
) -> Option<RuleComment> {
    let mut text = String::new();
    let mut end = start;
    let mut from = marker_position;
    loop {
        let line = lines[end];
        let LineRead { code, close } = state.read(line, from);
        if code.is_some_and(|code| code < from) {
            return None;
        }
        let stop = close.or(code).unwrap_or(line.len());
        text.push_str(&line[from..stop]);
        if code.is_some() {
            return Some(RuleComment {
                text,
                end,
                below: end,
            });
        }
        let continues = lines
            .get(end + 1)
            .is_some_and(|next| state.is_comment_line(next));
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
