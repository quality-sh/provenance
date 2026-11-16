//! The declaration forms of Rust, Python, and Go. In these languages a
//! keyword starts each form that gives a name, so a statement that starts
//! with a different token gives no name.

use super::Follower::{Token, Word};
use super::{skip_group, Grammar, Row, Tokens};

/// The Rust forms. Modifiers, in any order: `pub` with its optional group,
/// `default`, `async`, `unsafe`, `extern` with its optional ABI string, and
/// `const` before `fn`, `async`, `unsafe`, or `extern`. Then a row of
/// `RUST`.
pub(super) fn rust(tokens: Tokens<'_, '_>) -> Option<String> {
    let mut index = 0;
    loop {
        let token = tokens.get(index)?;
        let next = tokens.get(index + 1);
        match token {
            "pub" => index = skip_group(tokens, index + 1, "(", ")"),
            "default" | "async" | "unsafe" => index += 1,
            "extern" => index += 1 + usize::from(next.is_some_and(|abi| abi.starts_with('"'))),
            "const" if matches!(next, Some("fn" | "async" | "unsafe" | "extern")) => index += 1,
            _ if RUST.has_row(token) => return RUST.row_name(tokens, index, index + 1),
            _ => return None,
        }
    }
}

const RUST: Grammar = Grammar {
    rows: &[
        Row {
            keyword: "fn",
            followers: &[Token("("), Token("<")],
        },
        Row {
            keyword: "struct",
            followers: &[
                Token("{"),
                Token("("),
                Token("<"),
                Token(";"),
                Token("where"),
            ],
        },
        Row {
            keyword: "enum",
            followers: &[Token("{"), Token("<"), Token("where")],
        },
        Row {
            keyword: "union",
            followers: &[Token("{"), Token("<"), Token("where")],
        },
        Row {
            keyword: "trait",
            followers: &[Token("{"), Token("<"), Token(":"), Token("where")],
        },
        Row {
            keyword: "type",
            followers: &[
                Token("="),
                Token("<"),
                Token(";"),
                Token(":"),
                Token("where"),
            ],
        },
    ],
    reserved: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true",
        "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final",
        "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
    ],
};

/// The Python forms: an optional `async`, and a row of `PYTHON`.
pub(super) fn python(tokens: Tokens<'_, '_>) -> Option<String> {
    let index = usize::from(tokens.get(0)? == "async");
    PYTHON.row_name(tokens, index, index + 1)
}

const PYTHON: Grammar = Grammar {
    rows: &[
        Row {
            keyword: "def",
            followers: &[Token("("), Token("[")],
        },
        Row {
            keyword: "class",
            followers: &[Token(":"), Token("("), Token("[")],
        },
    ],
    reserved: &[
        "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
        "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
        "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
        "try", "while", "with", "yield",
    ],
};

/// The Go forms: a row of `GO`, or `func`, a receiver in parentheses, the
/// name, and `(`. A function literal has no name after `func` or after its
/// parameters, so it gives no name.
pub(super) fn go(tokens: Tokens<'_, '_>) -> Option<String> {
    if tokens.get(0)? == "func" && tokens.get(1)? == "(" {
        let name = skip_group(tokens, 1, "(", ")");
        GO.name(tokens, name, &[Token("(")])
    } else {
        GO.row_name(tokens, 0, 1)
    }
}

const GO: Grammar = Grammar {
    rows: &[
        Row {
            keyword: "func",
            followers: &[Token("("), Token("[")],
        },
        Row {
            keyword: "type",
            followers: &[
                Word,
                Token("["),
                Token("*"),
                Token("("),
                Token("<"),
                Token("="),
            ],
        },
    ],
    reserved: &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "type",
        "var",
    ],
};
