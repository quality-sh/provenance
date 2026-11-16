//! The code tokens of one file, each with the index of its line.
//!
//! The walker's lexers decide which text is code: `rust_pieces` for Rust,
//! and `multiline_parts` with `lex_line` for the other languages. A comment
//! is a span of text, so a line can hold a comment and code. A string is one
//! token, and each other part of the code is a word or one punctuation
//! character.

use std::ops::ControlFlow;

use crate::binding_lexer::{lex_line, multiline_parts, Piece};
use crate::walker::multiline_style;
use crate::walker::rust_lines::{rust_pieces, RustLexicalState};
use crate::Language;

/// One code token and the index of its line.
#[derive(Clone, Copy)]
pub(in crate::walker) struct Token<'a> {
    pub(in crate::walker) text: &'a str,
    pub(in crate::walker) line: usize,
}

/// The code tokens of `lines`. `rust_states` holds the state at the start of
/// each line for Rust, and is empty for the other languages.
pub(in crate::walker) fn file_tokens<'a>(
    language: Language,
    lines: &[&'a str],
    rust_states: &[RustLexicalState],
) -> Vec<Token<'a>> {
    let mut tokens = Vec::new();
    let mut in_block_comment = false;
    let mut multiline_delimiter = None;
    for (number, line) in lines.iter().copied().enumerate() {
        let mut pieces = Vec::new();
        match multiline_style(language) {
            None => {
                let mut state = rust_states[number];
                rust_pieces(line, &mut state, |range, piece| pieces.push((range, piece)));
            }
            Some(style) => {
                let parts = multiline_parts(
                    line,
                    style,
                    language.comment_syntax(),
                    &mut multiline_delimiter,
                    in_block_comment,
                );
                if let Some(parts) = parts {
                    let offset = parts.code.start;
                    in_block_comment = lex_line(
                        &line[parts.code],
                        in_block_comment,
                        language.comment_syntax(),
                        |range, piece| {
                            pieces.push((range.start + offset..range.end + offset, piece));
                            ControlFlow::Continue(())
                        },
                    );
                    pieces.extend(parts.opened.map(|opened| (opened, Piece::Quoted)));
                }
            }
        }
        for (range, piece) in pieces {
            match piece {
                Piece::Quoted => tokens.push(Token {
                    text: &line[range],
                    line: number,
                }),
                Piece::Code => push_words(language, &line[range], number, &mut tokens),
                Piece::BlockClose => {}
            }
        }
    }
    tokens
}

/// Adds the words and the punctuation characters of one code part. In Rust,
/// `r#` with a word is one raw identifier.
fn push_words<'a>(language: Language, code: &'a str, line: usize, tokens: &mut Vec<Token<'a>>) {
    let mut characters = code.char_indices().peekable();
    while let Some((start, character)) = characters.next() {
        if character.is_whitespace() {
            continue;
        }
        let mut end = start + character.len_utf8();
        if is_word(language, character) {
            let raw = language == Language::Rust && character == 'r';
            if raw && is_raw_prefix(&code[end..]) {
                characters.next();
            }
            while let Some((index, next)) = characters.next_if(|(_, next)| is_word(language, *next))
            {
                end = index + next.len_utf8();
            }
        }
        tokens.push(Token {
            text: &code[start..end],
            line,
        });
    }
}

/// Whether `character` can be in a word of `language`. Only JavaScript,
/// TypeScript, and Java permit `$` in an identifier, so in the other
/// languages `$` is a punctuation token, as in a Rust `$name` metavariable.
pub(in crate::walker) fn is_word(language: Language, character: char) -> bool {
    character.is_alphanumeric()
        || character == '_'
        || (character == '$'
            && matches!(
                language,
                Language::JavaScript | Language::TypeScript | Language::Java
            ))
}

/// Whether `rest`, the text after an `r`, continues a Rust raw identifier.
fn is_raw_prefix(rest: &str) -> bool {
    let mut characters = rest.chars();
    characters.next() == Some('#')
        && characters
            .next()
            .is_some_and(|character| is_word(Language::Rust, character))
}
