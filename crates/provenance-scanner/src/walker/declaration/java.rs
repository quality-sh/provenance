//! The declaration forms of Java.
//!
//! Annotations and modifiers of `JAVA_MODIFIERS` come first, in any order.
//! Then one of these forms gives the name:
//!
//! - a row of `JAVA`: `class`, `interface`, `enum`, `record`, or
//!   `@interface`, then the name and a follower of the row;
//! - a method: type parameters, a return type, the name, and `(`;
//! - a constructor, or an enum constant with a body: type parameters, the
//!   name, a parameter list, and `{` or `throws`; or the name and `{`;
//! - a field, after at least one modifier of `FIELD_MODIFIERS`: a type, the
//!   name, and `=`, `;`, or `,`.
//!
//! A name is a word that is not a reserved word of Java. A type is a word,
//! more words after `.`, type arguments, and pairs of square brackets. The
//! forms that have no keyword start with a word, so a statement keyword of
//! `JAVA_KEYWORDS` in the place of that word gives no name.

use super::Follower::Token;
use super::{skip_annotation, skip_group, Grammar, Row, Tokens};

const JAVA_MODIFIERS: [&str; 13] = [
    "public",
    "protected",
    "private",
    "static",
    "final",
    "abstract",
    "synchronized",
    "native",
    "default",
    "strictfp",
    "sealed",
    "transient",
    "volatile",
];

/// The modifiers that a local variable cannot have.
const FIELD_MODIFIERS: [&str; 6] = [
    "public",
    "protected",
    "private",
    "static",
    "transient",
    "volatile",
];

const JAVA_KEYWORDS: [&str; 27] = [
    "return",
    "new",
    "throw",
    "throws",
    "if",
    "for",
    "while",
    "switch",
    "catch",
    "else",
    "do",
    "try",
    "finally",
    "assert",
    "case",
    "yield",
    "this",
    "super",
    "instanceof",
    "break",
    "continue",
    "import",
    "package",
    "goto",
    "const",
    "var",
    "extends",
];

pub(super) fn java(tokens: Tokens<'_, '_>) -> Option<String> {
    let mut index = 0;
    let mut field = false;
    loop {
        let token = tokens.get(index)?;
        let next = tokens.get(index + 1);
        match token {
            "@" if next == Some("interface") => return JAVA.row_name(tokens, index + 1, index + 2),
            "@" => index = skip_annotation(tokens, index + 1),
            _ if JAVA.has_row(token) => return JAVA.row_name(tokens, index, index + 1),
            "non" if next == Some("-") && tokens.get(index + 2) == Some("sealed") => index += 3,
            _ if JAVA_MODIFIERS.contains(&token) => {
                field |= FIELD_MODIFIERS.contains(&token);
                index += 1;
            }
            _ => return member(tokens, index, field),
        }
    }
}

/// A method, a constructor, an enum constant with a body, or a field, from
/// `start`.
fn member(tokens: Tokens<'_, '_>, start: usize, field: bool) -> Option<String> {
    let first = skip_group(tokens, start, "<", ">");
    if !is_type(tokens, tokens.get(first)?) {
        return None;
    }
    match tokens.get(first + 1)? {
        "(" => {
            let after = skip_group(tokens, first + 1, "(", ")");
            let body = matches!(tokens.get(after)?, "{" | "throws");
            return body.then(|| JAVA.name(tokens, first, &[Token("(")])).flatten();
        }
        "{" => return JAVA.name(tokens, first, &[Token("{")]),
        _ => {}
    }
    let name = type_end(tokens, first)?;
    let followers: &[_] = if field {
        &[Token("("), Token("="), Token(";"), Token(",")]
    } else {
        &[Token("(")]
    };
    JAVA.name(tokens, name, followers)
}

/// The index after the type that starts at `start`: a word, more words after
/// `.`, type arguments, and pairs of square brackets.
fn type_end(tokens: Tokens<'_, '_>, start: usize) -> Option<usize> {
    let mut index = start + 1;
    while tokens.get(index) == Some(".") && is_type(tokens, tokens.get(index + 1)?) {
        index += 2;
    }
    index = skip_group(tokens, index, "<", ">");
    while tokens.get(index) == Some("[") && tokens.get(index + 1) == Some("]") {
        index += 2;
    }
    Some(index)
}

/// A word that can be a type, or a part of a qualified type.
fn is_type(tokens: Tokens<'_, '_>, token: &str) -> bool {
    tokens.is_word(token) && !JAVA_KEYWORDS.contains(&token) && !JAVA_MODIFIERS.contains(&token)
}

const JAVA: Grammar = Grammar {
    rows: &[
        Row {
            keyword: "class",
            followers: &[
                Token("{"),
                Token("<"),
                Token("extends"),
                Token("implements"),
                Token("permits"),
            ],
        },
        Row {
            keyword: "interface",
            followers: &[Token("{"), Token("<"), Token("extends"), Token("permits")],
        },
        Row {
            keyword: "enum",
            followers: &[Token("{"), Token("implements")],
        },
        Row {
            keyword: "record",
            followers: &[Token("("), Token("<")],
        },
    ],
    reserved: &[
        "abstract",
        "assert",
        "boolean",
        "break",
        "byte",
        "case",
        "catch",
        "char",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "double",
        "else",
        "enum",
        "extends",
        "final",
        "finally",
        "float",
        "for",
        "goto",
        "if",
        "implements",
        "import",
        "instanceof",
        "int",
        "interface",
        "long",
        "native",
        "new",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "short",
        "static",
        "strictfp",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "try",
        "void",
        "volatile",
        "while",
        "true",
        "false",
        "null",
        "_",
    ],
};
