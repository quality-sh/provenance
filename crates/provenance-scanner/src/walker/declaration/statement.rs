//! Whether a site is at the start of a statement, from the code tokens above
//! the site.
//!
//! In the languages with braces, a statement starts at the start of the file
//! and after `;`, `{`, or `}`. In Go, JavaScript, and TypeScript, the end of a
//! line also ends a statement when the last token of the line can end one,
//! outside parentheses and square brackets. In Python, a statement starts at
//! the start of a logical line. Attributes, decorators, and annotations do not
//! change where a statement starts, so the sites and the attributes above one
//! declaration all start its statement.

use super::tokens::{is_word, Token};
use crate::walker::Language;

/// For each line of `site_lines`, in ascending order, whether a site on that
/// line starts a statement.
pub(in crate::walker) fn statement_starts(
    language: Language,
    tokens: &[Token<'_>],
    site_lines: &[usize],
) -> Vec<bool> {
    let mut above = Above::new(language);
    let mut next = 0;
    site_lines
        .iter()
        .map(|line| {
            while let Some(token) = tokens.get(next).filter(|token| token.line < *line) {
                above.push(token.text);
                next += 1;
            }
            above.starts_statement()
        })
        .collect()
}

/// The part of an attribute, a decorator, or an annotation that the last
/// token is in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Attribute {
    Outside,
    /// After `#` or `#!` in Rust.
    Hash,
    /// After `@` or after a `.` of an annotation name.
    Name,
    /// After a word of an annotation name.
    AfterName,
    /// In the brackets of an attribute or the arguments of an annotation.
    Group {
        open: char,
        close: char,
        depth: usize,
    },
}

/// The state of the code above a site.
struct Above<'a> {
    language: Language,
    attribute: Attribute,
    /// The last code token outside attributes.
    last: Option<&'a str>,
    /// The brackets that are open, outside attributes.
    open: Vec<char>,
}

impl<'a> Above<'a> {
    const fn new(language: Language) -> Self {
        Self {
            language,
            attribute: Attribute::Outside,
            last: None,
            open: Vec::new(),
        }
    }

    fn push(&mut self, token: &'a str) {
        let first = token.chars().next();
        self.attribute = match (self.attribute, first) {
            (Attribute::Outside | Attribute::AfterName, Some('#'))
                if self.language == Language::Rust =>
            {
                Attribute::Hash
            }
            (Attribute::Outside | Attribute::AfterName, Some('@')) if self.has_annotations() => {
                Attribute::Name
            }
            (Attribute::Hash, Some('!')) => Attribute::Hash,
            (Attribute::Hash, Some('[')) => group('[', ']'),
            (Attribute::Name, Some(character)) if is_word(character) => Attribute::AfterName,
            (Attribute::AfterName, Some('.')) => Attribute::Name,
            (Attribute::AfterName, Some('(')) => group('(', ')'),
            (Attribute::Group { open, close, depth }, Some(character)) => {
                let depth = match character {
                    _ if character == open => depth + 1,
                    _ if character == close => depth - 1,
                    _ => depth,
                };
                if depth == 0 {
                    Attribute::Outside
                } else {
                    Attribute::Group { open, close, depth }
                }
            }
            _ => {
                self.push_code(token);
                Attribute::Outside
            }
        };
    }

    const fn has_annotations(&self) -> bool {
        matches!(
            self.language,
            Language::Java | Language::JavaScript | Language::TypeScript
        )
    }

    fn push_code(&mut self, token: &'a str) {
        match token {
            "(" | "[" | "{" => self.open.extend(token.chars()),
            ")" | "]" | "}" => {
                self.open.pop();
            }
            _ => {}
        }
        self.last = Some(token);
    }

    fn starts_statement(&self) -> bool {
        if !matches!(self.attribute, Attribute::Outside | Attribute::AfterName) {
            return false;
        }
        let Some(last) = self.last else {
            return true;
        };
        let in_block = matches!(self.open.last(), None | Some('{'));
        match self.language {
            Language::Python => self.open.is_empty() && last != "\\",
            Language::Rust | Language::Java => matches!(last, ";" | "{" | "}"),
            Language::Go | Language::JavaScript | Language::TypeScript => {
                matches!(last, ";" | "{" | "}") || (in_block && ends_line_statement(last))
            }
        }
    }
}

const fn group(open: char, close: char) -> Attribute {
    Attribute::Group {
        open,
        close,
        depth: 1,
    }
}

/// Whether a line that ends with `token` ends its statement: a word, a
/// string, or a closing bracket.
fn ends_line_statement(token: &str) -> bool {
    token
        .chars()
        .next()
        .is_some_and(|first| is_word(first) || matches!(first, '"' | '\'' | '`' | ')' | ']'))
}
