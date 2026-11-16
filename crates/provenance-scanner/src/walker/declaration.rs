//! The item below a site: which declaration a Rule comment, attribute, or
//! decorator marks, and the name of that declaration.
//!
//! The scanner does not parse a syntax tree. A name comes only from a row of
//! the table of declaration forms of the language: the rows of Rust, Python,
//! and Go are in `rows`, and those of JavaScript, TypeScript, and Java are in
//! `script` and `java`. Tokens at the start of the statement below the site
//! decide the row, and a statement that starts with no row gives no name.

mod java;
mod rows;
mod script;
mod statement;
mod tokens;

use provenance_macros::rule;

use super::Language;
pub(super) use statement::statement_starts;
pub(super) use tokens::{file_tokens, Token};

/// The result of the read of the code between a site and the next site.
pub(super) enum Read {
    /// The code starts a declaration with this name, or a statement with no
    /// name.
    Declaration(Option<String>),
    /// The code holds only attributes, decorators, or annotations, so the
    /// declaration is below the next site.
    NextSite,
}

/// The name of the declaration that starts in `below`, the code tokens
/// between a site and the next site, when a row of the table of declaration
/// forms of `language` gives a name.
#[rule("rule_site_item_name_is_the_declaration_below")]
pub(super) fn item_name(language: Language, below: &[Token<'_>]) -> Read {
    let tokens = Tokens(below);
    let first = attributes_end(language, tokens);
    if first >= below.len() {
        return Read::NextSite;
    }
    let tokens = Tokens(&below[first..]);
    Read::Declaration(match language {
        Language::Rust => rows::rust(tokens),
        Language::Python => rows::python(tokens),
        Language::Go => rows::go(tokens),
        Language::JavaScript | Language::TypeScript => script::script(tokens),
        Language::Java => java::java(tokens),
    })
}

/// The code tokens of a read.
#[derive(Clone, Copy)]
struct Tokens<'t, 'a>(&'t [Token<'a>]);

impl<'a> Tokens<'_, 'a> {
    fn get(self, index: usize) -> Option<&'a str> {
        self.0.get(index).map(|token| token.text)
    }

    fn on_one_line(self, left: usize, right: usize) -> bool {
        self.0[left].line == self.0[right].line
    }
}

/// The index after the attributes, decorators, or annotations at the start
/// of `tokens`, with their arguments.
fn attributes_end(language: Language, tokens: Tokens<'_, '_>) -> usize {
    let mut index = 0;
    loop {
        let next = tokens.get(index + 1);
        index = match (language, tokens.get(index)) {
            (Language::Rust, Some("#")) => {
                let inner = usize::from(next == Some("!"));
                skip_group(tokens, index + 1 + inner, "[", "]")
            }
            (Language::Python, Some("@")) => logical_line_end(tokens, index),
            (Language::JavaScript | Language::TypeScript, Some("@")) => {
                skip_annotation(tokens, index + 1)
            }
            (Language::Java, Some("@")) if next != Some("interface") => {
                skip_annotation(tokens, index + 1)
            }
            _ => return index,
        };
    }
}

/// The index of the first token of the next logical Python line after the
/// token at `start`: a line ends outside brackets, where no `\` continues it.
fn logical_line_end(tokens: Tokens<'_, '_>, start: usize) -> usize {
    let mut depth = 0usize;
    let mut index = start;
    while let Some(token) = tokens.get(index) {
        let continued = index > start && (depth > 0 || tokens.get(index - 1) == Some("\\"));
        if index > start && !continued && !tokens.on_one_line(index - 1, index) {
            return index;
        }
        match token {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
        index += 1;
    }
    index
}

/// The token at `index` as the name of a declaration. The name of a Rust raw
/// identifier `r#match` is `match`.
fn named(tokens: Tokens<'_, '_>, index: usize) -> Option<String> {
    let name = tokens.get(index)?;
    let name = name.strip_prefix("r#").unwrap_or(name);
    is_identifier(name).then(|| name.to_string())
}

/// The index after an annotation or a decorator name, such as
/// `pytest.mark.parametrize`, and its argument list.
fn skip_annotation(tokens: Tokens<'_, '_>, mut index: usize) -> usize {
    while tokens.get(index).is_some_and(is_identifier) {
        index += 1;
        if tokens.get(index) != Some(".") {
            break;
        }
        index += 1;
    }
    skip_group(tokens, index, "(", ")")
}

/// The index after the group that opens at `index`, or `index` when no group
/// opens there. A group that does not close ends at the end of the tokens.
fn skip_group(tokens: Tokens<'_, '_>, index: usize, open: &str, close: &str) -> usize {
    if tokens.get(index) != Some(open) {
        return index;
    }
    let mut depth = 0usize;
    let mut position = index;
    while let Some(token) = tokens.get(position) {
        if token == open {
            depth += 1;
        } else if token == close {
            depth -= 1;
            if depth == 0 {
                return position + 1;
            }
        }
        position += 1;
    }
    position
}

fn is_identifier(token: &str) -> bool {
    token
        .chars()
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_' || first == '$')
}
