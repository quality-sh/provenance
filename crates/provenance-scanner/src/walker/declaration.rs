//! The item below a site: which declaration a Rule comment, attribute, or
//! decorator marks, and the name of that declaration.
//!
//! The scanner does not parse a syntax tree. A name comes only from a row of
//! the table of declaration forms of the language: the rows of Rust, Python,
//! and Go are in `rows`, and those of JavaScript, TypeScript, and Java are in
//! `script` and `java`. Tokens at the start of the statement below the site
//! decide the row, and a statement that starts with no row gives no name. A
//! row gives a name only when the complete head of the declaration fits: the
//! keyword, a name that is not a reserved word of the language, and a token
//! that the language permits after that name in that form.

mod java;
mod rows;
mod script;
mod statement;
mod tokens;

use provenance_macros::rule;

use crate::Language;
pub(super) use statement::statement_starts;
use tokens::is_word;
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
    let first = attributes_end(Tokens {
        tokens: below,
        language,
    });
    if first >= below.len() {
        return Read::NextSite;
    }
    let tokens = Tokens {
        tokens: &below[first..],
        language,
    };
    Read::Declaration(match language {
        Language::Rust => rows::rust(tokens),
        Language::Python => rows::python(tokens),
        Language::Go => rows::go(tokens),
        Language::JavaScript | Language::TypeScript => script::script(tokens),
        Language::Java => java::java(tokens),
    })
}

/// The code tokens of a read, in one language.
#[derive(Clone, Copy)]
struct Tokens<'t, 'a> {
    tokens: &'t [Token<'a>],
    language: Language,
}

impl<'a> Tokens<'_, 'a> {
    fn get(self, index: usize) -> Option<&'a str> {
        self.tokens.get(index).map(|token| token.text)
    }

    const fn on_one_line(self, left: usize, right: usize) -> bool {
        self.tokens[left].line == self.tokens[right].line
    }

    /// Whether `token` is an identifier or a keyword: a word of the language
    /// that does not start with a digit. A Rust raw identifier `r#match` is
    /// one word.
    fn is_word(self, token: &str) -> bool {
        let raw = token
            .strip_prefix("r#")
            .filter(|_| self.language == Language::Rust);
        raw.unwrap_or(token)
            .chars()
            .next()
            .is_some_and(|first| is_word(self.language, first) && !first.is_numeric())
    }
}

/// A declaration form that starts with a keyword: the keyword, the name, and
/// one of the followers.
struct Row {
    keyword: &'static str,
    followers: &'static [Follower],
}

/// A token that the language permits after the name of a declaration.
enum Follower {
    Token(&'static str),
    /// A word, such as the name of a type.
    Word,
}

/// The keyword rows and the reserved words of one language.
struct Grammar {
    rows: &'static [Row],
    reserved: &'static [&'static str],
}

impl Grammar {
    fn has_row(&self, keyword: &str) -> bool {
        self.rows.iter().any(|row| row.keyword == keyword)
    }

    /// The name at `name` of the row whose keyword is at `keyword`, when the
    /// name and a follower of the row fit.
    fn row_name(&self, tokens: Tokens<'_, '_>, keyword: usize, name: usize) -> Option<String> {
        let keyword = tokens.get(keyword)?;
        let row = self.rows.iter().find(|row| row.keyword == keyword)?;
        self.name(tokens, name, row.followers)
    }

    /// The token at `index` as a name: a word that is not a reserved word of
    /// the language, with one of `followers` after it. The name of a Rust raw
    /// identifier `r#match` is `match`.
    fn name(&self, tokens: Tokens<'_, '_>, index: usize, followers: &[Follower]) -> Option<String> {
        let token = tokens.get(index)?;
        let after = tokens.get(index + 1)?;
        let follows = followers.iter().any(|follower| match follower {
            Follower::Token(text) => *text == after,
            Follower::Word => tokens.is_word(after),
        });
        let raw = token
            .strip_prefix("r#")
            .filter(|_| tokens.language == Language::Rust);
        let reserved = raw.is_none() && self.reserved.contains(&token);
        (follows && tokens.is_word(token) && !reserved).then(|| raw.unwrap_or(token).to_owned())
    }
}

/// The index after the attributes, decorators, or annotations at the start
/// of `tokens`, with their arguments.
fn attributes_end(tokens: Tokens<'_, '_>) -> usize {
    let mut index = 0;
    loop {
        let next = tokens.get(index + 1);
        index = match (tokens.language, tokens.get(index)) {
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

/// The index after an annotation or a decorator name, such as
/// `pytest.mark.parametrize`, and its argument list.
fn skip_annotation(tokens: Tokens<'_, '_>, mut index: usize) -> usize {
    while tokens.get(index).is_some_and(|token| tokens.is_word(token)) {
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
