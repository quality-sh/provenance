//! Whether a site is at the start of a statement, from the code tokens above
//! the site.
//!
//! In the languages with braces, a statement starts at the start of the file
//! and after `;`, `{`, or `}`. In Rust, the bracket that opens the body of a
//! macro call, as in `name!(`, also starts a statement, because the body can
//! hold items. In Go, JavaScript, and TypeScript, a line break outside
//! parentheses and square brackets also ends a statement when the two lines
//! fit, by the rule of `line_break_ends_statement`. In Python, a statement
//! starts at
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
            above.starts_statement(tokens.get(next).map(|token| token.text))
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
    /// The code token before the last one.
    before_last: Option<&'a str>,
    /// Whether the last code token opens the body of a Rust macro call.
    macro_body: bool,
    /// The brackets that are open, outside attributes.
    open: Vec<char>,
}

impl<'a> Above<'a> {
    const fn new(language: Language) -> Self {
        Self {
            language,
            attribute: Attribute::Outside,
            last: None,
            before_last: None,
            macro_body: false,
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
            (Attribute::Name, Some(character)) if is_word(self.language, character) => {
                Attribute::AfterName
            }
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
        self.macro_body = self.language == Language::Rust
            && matches!(token, "(" | "[" | "{")
            && self.last == Some("!")
            && self
                .before_last
                .is_some_and(|word| word.starts_with(|first| is_word(Language::Rust, first)));
        self.before_last = self.last;
        match token {
            "(" | "[" | "{" => self.open.extend(token.chars()),
            ")" | "]" | "}" => {
                self.open.pop();
            }
            _ => {}
        }
        self.last = Some(token);
    }

    /// Whether a site starts a statement, when `below` is the first code
    /// token below the site.
    fn starts_statement(&self, below: Option<&str>) -> bool {
        if !matches!(self.attribute, Attribute::Outside | Attribute::AfterName) {
            return false;
        }
        let Some(last) = self.last else {
            return true;
        };
        let in_block = matches!(self.open.last(), None | Some('{'));
        match self.language {
            Language::Python => self.open.is_empty() && last != "\\",
            Language::Rust => self.macro_body || matches!(last, ";" | "{" | "}"),
            Language::Java => matches!(last, ";" | "{" | "}"),
            Language::Go | Language::JavaScript | Language::TypeScript => {
                matches!(last, ";" | "{" | "}")
                    || (in_block && line_break_ends_statement(self.before_last, last, below))
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

/// Whether the line break between a line that ends with `before_last` and
/// `last`, and a line that starts with `below`, ends a statement in Go,
/// JavaScript, or TypeScript: the line above does not end with a token of
/// `NEEDS_RIGHT_SIDE`, and the line below does not start with a token of
/// `CONTINUES_EXPRESSION` or a template literal. `++` and `--` end an
/// expression. A `>` that is not the end of `=>` can end a JSX element or a
/// type argument list, or be an operator, so after it a statement starts
/// only when a word of `DECLARATION_KEYWORDS` starts the line below.
fn line_break_ends_statement(before_last: Option<&str>, last: &str, below: Option<&str>) -> bool {
    let continues =
        below.is_some_and(|below| CONTINUES_EXPRESSION.contains(&below) || below.starts_with('`'));
    match last {
        ">" if before_last != Some("=") => {
            below.is_some_and(|below| DECLARATION_KEYWORDS.contains(&below))
        }
        "+" | "-" if before_last == Some(last) => !continues,
        _ => !NEEDS_RIGHT_SIDE.contains(&last) && !continues,
    }
}

/// The tokens that need a right side, so a line that ends with one continues
/// on the next line: an operator, `,`, `.`, `?`, `:`, the `>` of `=>`, and
/// the words that an operand, a name, or a body follows. A line that ends with
/// `return`, `break`, `continue`, `throw`, or `yield` ends its statement in
/// these languages, so these words are not in the list.
const NEEDS_RIGHT_SIDE: [&str; 64] = [
    "=",
    "+",
    "-",
    "*",
    "/",
    "%",
    "<",
    ">",
    "&",
    "|",
    "^",
    "!",
    "~",
    "?",
    ":",
    ",",
    ".",
    "extends",
    "implements",
    "typeof",
    "new",
    "in",
    "of",
    "instanceof",
    "await",
    "as",
    "satisfies",
    "keyof",
    "void",
    "delete",
    "else",
    "do",
    "case",
    "default",
    "export",
    "import",
    "from",
    "declare",
    "abstract",
    "function",
    "class",
    "interface",
    "type",
    "enum",
    "namespace",
    "const",
    "let",
    "var",
    "if",
    "for",
    "while",
    "switch",
    "catch",
    "with",
    "func",
    "struct",
    "map",
    "chan",
    "go",
    "defer",
    "range",
    "select",
    "package",
    "goto",
];

/// The tokens that continue the expression of the line above when they start
/// a line: a binary operator, `.`, `?`, `:`, `,`, a closing bracket, an
/// operator word, and a `(` or `[` that calls or indexes the line above.
const CONTINUES_EXPRESSION: [&str; 23] = [
    "=",
    "+",
    "-",
    "*",
    "/",
    "%",
    "<",
    ">",
    "&",
    "|",
    "^",
    "?",
    ":",
    ",",
    ".",
    ")",
    "]",
    "(",
    "[",
    "in",
    "instanceof",
    "as",
    "satisfies",
];

/// The words that start a declaration below a line that ends with `>`.
const DECLARATION_KEYWORDS: [&str; 13] = [
    "export",
    "function",
    "class",
    "interface",
    "type",
    "enum",
    "namespace",
    "declare",
    "abstract",
    "const",
    "let",
    "var",
    "func",
];
