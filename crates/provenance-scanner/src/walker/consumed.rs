//! Lexer state carried across the source lines that a wrapped binding or an
//! annotation comment consumed.

use super::multiline_style;
use crate::binding_lexer::{block_comment_state, code_outside_multiline_string};
use crate::Language;

/// Carries the multiline-string and block-comment state across the lines
/// that a site consumed. The Rust line states hold the state of Rust.
pub(super) fn consume_lines(
    language: Language,
    consumed: &[&str],
    multiline_delimiter: &mut Option<String>,
    in_block_comment: &mut bool,
) {
    let Some(style) = multiline_style(language) else {
        return;
    };
    let syntax = language.comment_syntax();
    for line in consumed {
        let Some(code) = code_outside_multiline_string(
            line,
            style,
            syntax,
            multiline_delimiter,
            *in_block_comment,
        ) else {
            break;
        };
        *in_block_comment = block_comment_state(code, syntax, *in_block_comment);
    }
}
