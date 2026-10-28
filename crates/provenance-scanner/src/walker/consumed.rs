//! Lexer state carried across the source lines that a wrapped binding or an
//! annotation comment consumed.

use super::{multiline_style, Language};
use crate::binding_lexer::{block_comment_state, code_outside_multiline_string};

/// Carries the lexer state across the lines a wrapped binding consumed.
pub(super) fn consume_binding_lines(
    language: Language,
    consumed: &[&str],
    multiline_delimiter: &mut Option<String>,
    in_block_comment: &mut bool,
) {
    for line in consumed {
        let Some(code) = code_outside_multiline_string(
            line,
            multiline_style(language),
            multiline_delimiter,
            *in_block_comment,
        ) else {
            break;
        };
        if language != Language::Python {
            *in_block_comment =
                block_comment_state(code, *in_block_comment, language == Language::Rust);
        }
    }
}

/// Carries the block-comment state across the lines an annotation comment
/// consumed.
pub(super) fn consume_comment_lines(
    language: Language,
    consumed: &[&str],
    in_block_comment: &mut bool,
) {
    if language == Language::Python {
        return;
    }
    for line in consumed {
        *in_block_comment =
            block_comment_state(line, *in_block_comment, language == Language::Rust);
    }
}
