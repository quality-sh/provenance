//! The declaration forms of JavaScript and TypeScript.
//!
//! Modifiers of `SCRIPT_MODIFIERS` come first; a modifier is followed by a
//! word or `*`, and `export` and `default` can also be followed by a
//! decorator. Then one of these forms gives the name:
//!
//! - `function`, an optional `*`, and the name;
//! - `class`, `interface`, `type`, `enum`, `const enum`, `namespace`,
//!   `const`, `let`, or `var`, and the name;
//! - a method: an optional `*`, an optional `get` or `set`, the name, type
//!   parameters, a parameter list, and a return type or a body on the line of
//!   the `)`;
//! - a property, after at least one modifier of `MEMBER_MODIFIERS`: the name,
//!   an optional `?` or `!`, and a type, a value, or `;`.
//!
//! The method and the property have no keyword, so a statement keyword of
//! `SCRIPT_KEYWORDS` in the place of their name gives no name. `async` in the
//! place of a method name gives no name, because `async (` and `async <`
//! start an arrow function.

use super::{is_identifier, named, skip_annotation, skip_group, Tokens};

const SCRIPT_MODIFIERS: [&str; 12] = [
    "export",
    "default",
    "declare",
    "abstract",
    "async",
    "public",
    "private",
    "protected",
    "static",
    "readonly",
    "override",
    "accessor",
];

/// The modifiers that only a member of a class can have.
const MEMBER_MODIFIERS: [&str; 9] = [
    "declare",
    "abstract",
    "public",
    "private",
    "protected",
    "static",
    "readonly",
    "override",
    "accessor",
];

const SCRIPT_KEYWORDS: [&str; 23] = [
    "if", "for", "while", "switch", "catch", "return", "throw", "new", "await", "yield", "typeof",
    "delete", "void", "do", "else", "try", "finally", "with", "super", "this", "import", "case",
    "async",
];

const NAMED_BY_KEYWORD: [&str; 8] = [
    "class",
    "interface",
    "type",
    "enum",
    "namespace",
    "const",
    "let",
    "var",
];

pub(super) fn script(tokens: Tokens<'_, '_>) -> Option<String> {
    let mut index = 0;
    let mut member = false;
    loop {
        let token = tokens.get(index)?;
        let next = tokens.get(index + 1);
        match token {
            "@" => index = skip_annotation(tokens, index + 1),
            "function" => {
                let generator = next == Some("*");
                return named(tokens, index + 1 + usize::from(generator));
            }
            "const" if next == Some("enum") => index += 1,
            _ if NAMED_BY_KEYWORD.contains(&token) => return named(tokens, index + 1),
            "*" => index += 1,
            "export" | "default" if next == Some("@") => index += 1,
            _ if is_modifier(token, next) => {
                member |= MEMBER_MODIFIERS.contains(&token);
                index += 1;
            }
            "get" | "set" if next.is_some_and(is_identifier) => index += 1,
            _ if is_identifier(token) && !SCRIPT_KEYWORDS.contains(&token) => {
                return method(tokens, index).or_else(|| property(tokens, index, member));
            }
            _ => return None,
        }
    }
}

fn is_modifier(token: &str, next: Option<&str>) -> bool {
    SCRIPT_MODIFIERS.contains(&token) && next.is_some_and(|next| is_identifier(next) || next == "*")
}

/// A method: the name, type parameters, and a parameter list that a return
/// type or a body on the line of the `)` follows. A `{` on a later line
/// follows a call statement as well, so it gives no name.
fn method(tokens: Tokens<'_, '_>, name: usize) -> Option<String> {
    let index = skip_group(tokens, name + 1, "<", ">");
    if tokens.get(index)? != "(" {
        return None;
    }
    let after = skip_group(tokens, index, "(", ")");
    let declared = match tokens.get(after)? {
        "{" => tokens.on_one_line(after - 1, after),
        ":" => true,
        _ => false,
    };
    declared
        .then(|| tokens.get(name).map(str::to_owned))
        .flatten()
}

/// A property after a modifier of a class member: the name, an optional `?`
/// or `!`, and `:`, `=`, or `;`.
fn property(tokens: Tokens<'_, '_>, name: usize, member: bool) -> Option<String> {
    if !member {
        return None;
    }
    let mut index = name + 1;
    if matches!(tokens.get(index)?, "?" | "!") {
        index += 1;
    }
    matches!(tokens.get(index)?, ":" | "=" | ";").then(|| tokens.get(name).map(str::to_owned))?
}
