//! The declaration forms of Rust, Python, and Go. In these languages a
//! keyword starts each form that gives a name, so a statement that starts
//! with a different token gives no name.

use super::{named, skip_group, Tokens};

/// The Rust forms. Modifiers, in any order: `pub` with its optional group,
/// `default`, `async`, `unsafe`, `extern` with its optional ABI string, and
/// `const` before `fn`, `async`, `unsafe`, or `extern`. Then a keyword of
/// `RUST_KEYWORDS`, and the name.
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
            _ if RUST_KEYWORDS.contains(&token) => return named(tokens, index + 1),
            _ => return None,
        }
    }
}

const RUST_KEYWORDS: [&str; 6] = ["fn", "struct", "enum", "union", "trait", "type"];

/// The Python forms: `def` after an optional `async`, and `class`. Then the
/// name.
pub(super) fn python(tokens: Tokens<'_, '_>) -> Option<String> {
    let index = usize::from(tokens.get(0)? == "async");
    match tokens.get(index)? {
        "def" | "class" => named(tokens, index + 1),
        _ => None,
    }
}

/// The Go forms: `type` and the name; `func`, the name, and `(` or `[`; and
/// `func`, a receiver in parentheses, the name, and `(`. A function literal
/// has no name after `func` or after its parameters, so it gives no name.
pub(super) fn go(tokens: Tokens<'_, '_>) -> Option<String> {
    match tokens.get(0)? {
        "type" => named(tokens, 1),
        "func" if tokens.get(1)? == "(" => {
            let name = skip_group(tokens, 1, "(", ")");
            go_function(tokens, name, &["("])
        }
        "func" => go_function(tokens, 1, &["(", "["]),
        _ => None,
    }
}

fn go_function(tokens: Tokens<'_, '_>, name: usize, after: &[&str]) -> Option<String> {
    let follows = tokens.get(name + 1)?;
    let keyword = GO_KEYWORDS.contains(&tokens.get(name)?);
    if after.contains(&follows) && !keyword {
        named(tokens, name)
    } else {
        None
    }
}

/// The Go keywords that can follow the parameters of a function literal as
/// the start of its result type.
const GO_KEYWORDS: [&str; 4] = ["func", "map", "chan", "interface"];
