//! The tokens of the code below a binding, read one line at a time when the
//! grammar asks for the next token.
//!
//! The walker's lexers decide which text is code: `rust_pieces` for Rust,
//! and `multiline_parts` with `lex_line` for the other languages. A comment
//! is a span of text, so a line can hold a comment and code. A string is one
//! token, and each other part of the code is a word or one punctuation
//! character.

use std::ops::ControlFlow;

use crate::binding_lexer::{lex_line, multiline_parts, CommentSyntax, Piece};
use crate::walker::rust_lines::{rust_pieces, RustLexicalState};
use crate::walker::{multiline_style, Language};

/// The lexical state of the walker at the first line below a binding.
pub(in crate::walker) enum LineStart<'a> {
    /// The state at the start of each line below the binding, for Rust.
    Rust(&'a [RustLexicalState]),
    /// The block-comment state and the delimiter of an open multiline string,
    /// for the other languages.
    Carried {
        in_block_comment: bool,
        multiline_delimiter: Option<String>,
    },
}

/// The tokens below a binding, each with the index of its line.
pub(super) struct Tokens<'a> {
    language: Language,
    lines: &'a [&'a str],
    start: LineStart<'a>,
    next_line: usize,
    read: Vec<(&'a str, usize)>,
    first: usize,
}

impl<'a> Tokens<'a> {
    pub(super) const fn new(language: Language, lines: &'a [&'a str], start: LineStart<'a>) -> Self {
        Self {
            language,
            lines,
            start,
            next_line: 0,
            read: Vec::new(),
            first: 0,
        }
    }

    /// The token at `index`, counted from the first token. The lines are read
    /// up to that token and no further.
    pub(super) fn get(&mut self, index: usize) -> Option<&'a str> {
        while self.first + index >= self.read.len() {
            if !self.read_line() {
                return None;
            }
        }
        Some(self.read[self.first + index].0)
    }

    /// Whether the tokens at `left` and `right`, which `get` gave, are on one
    /// line.
    pub(super) fn on_one_line(&self, left: usize, right: usize) -> bool {
        self.read[self.first + left].1 == self.read[self.first + right].1
    }

    /// Makes the token at `index` the first token.
    pub(super) const fn skip(&mut self, index: usize) {
        self.first += index;
    }

    /// Adds the tokens of the next line. False at the end of the file.
    fn read_line(&mut self) -> bool {
        let Some(line) = self.lines.get(self.next_line).copied() else {
            return false;
        };
        let number = self.next_line;
        self.next_line += 1;
        let mut pieces = Vec::new();
        match &mut self.start {
            LineStart::Rust(states) => {
                let mut state = states[number];
                rust_pieces(line, &mut state, |range, piece| pieces.push((range, piece)));
            }
            LineStart::Carried {
                in_block_comment,
                multiline_delimiter,
            } => {
                let style = multiline_style(self.language);
                let parts = multiline_parts(line, style, multiline_delimiter, *in_block_comment);
                if let Some(parts) = parts {
                    let offset = parts.code.start;
                    *in_block_comment = lex_line(
                        &line[parts.code],
                        *in_block_comment,
                        comment_syntax(self.language),
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
                Piece::Quoted => self.read.push((&line[range], number)),
                Piece::Code => self.push_words(&line[range], number),
            }
        }
        true
    }

    /// Adds the words and the punctuation characters of one code part. In
    /// Rust, `r#` with a word is one raw identifier.
    fn push_words(&mut self, code: &'a str, number: usize) {
        let mut characters = code.char_indices().peekable();
        while let Some((start, character)) = characters.next() {
            if character.is_whitespace() {
                continue;
            }
            let mut end = start + character.len_utf8();
            if is_word(character) {
                let raw = self.language == Language::Rust && character == 'r';
                if raw && is_raw_prefix(&code[end..]) {
                    characters.next();
                }
                while let Some((index, next)) = characters.next_if(|(_, next)| is_word(*next)) {
                    end = index + next.len_utf8();
                }
            }
            self.read.push((&code[start..end], number));
        }
    }
}

const fn comment_syntax(language: Language) -> CommentSyntax {
    match language {
        Language::Python => CommentSyntax::Hash,
        Language::Rust => CommentSyntax::Rust,
        Language::JavaScript | Language::TypeScript | Language::Java | Language::Go => {
            CommentSyntax::CStyle
        }
    }
}

fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == '$'
}

/// Whether `rest`, the text after an `r`, continues a Rust raw identifier.
fn is_raw_prefix(rest: &str) -> bool {
    let mut characters = rest.chars();
    characters.next() == Some('#') && characters.next().is_some_and(is_word)
}
