//! The declaration forms of JavaScript and TypeScript.
//!
//! Modifiers of `SCRIPT_MODIFIERS` come first; a modifier is followed by a
//! word or `*`, and `export` and `default` can also be followed by a
//! decorator. Then one of these forms gives the name:
//!
//! - a row of `SCRIPT`: `function` with an optional `*`, `class`,
//!   `interface`, `type`, `enum` or `const enum`, `namespace`, `const`,
//!   `let`, or `var`, then the name and a follower of the row;
//! - a method: an optional `*`, an optional `get` or `set`, the name, type
//!   parameters, a parameter list, and a return type or a body on the line of
//!   the `)`;
//! - a property, after at least one modifier of `MEMBER_MODIFIERS`: the name,
//!   an optional `?` or `!`, and a type, a value, or `;`.
//!
//! The method and the property have no keyword, and their name can be a
//! reserved word, such as `delete`. A statement that starts with a word of
//! `REFUSED`, a parenthesized part, and `{` or `:` has the tokens of a method,
//! so these words in the place of a method name or a property name give no
//! name. `async (x): T => {}` is an arrow function, so `async` is in the list.

use super::Follower::Token;
use super::{skip_annotation, skip_group, Follower, Grammar, Row, Tokens};

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

const REFUSED: [&str; 8] = [
    "if", "for", "while", "switch", "catch", "with", "case", "async",
];

const SCRIPT: Grammar = Grammar {
    rows: &[
        Row {
            keyword: "function",
            followers: &[Token("("), Token("<")],
        },
        Row {
            keyword: "class",
            followers: &[
                Token("{"),
                Token("<"),
                Token("extends"),
                Token("implements"),
            ],
        },
        Row {
            keyword: "interface",
            followers: &[Token("{"), Token("<"), Token("extends")],
        },
        Row {
            keyword: "type",
            followers: &[Token("="), Token("<")],
        },
        Row {
            keyword: "enum",
            followers: &[Token("{")],
        },
        Row {
            keyword: "namespace",
            followers: &[Token("{"), Token(".")],
        },
        Row {
            keyword: "const",
            followers: VARIABLE,
        },
        Row {
            keyword: "let",
            followers: VARIABLE,
        },
        Row {
            keyword: "var",
            followers: VARIABLE,
        },
    ],
    reserved: &[
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "function",
        "if",
        "implements",
        "import",
        "in",
        "instanceof",
        "interface",
        "let",
        "new",
        "null",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "var",
        "void",
        "while",
        "with",
        "yield",
    ],
};

/// The followers of the name of a variable: a value, a type, `!` before a
/// type, `,` before the next variable, or `;`.
const VARIABLE: &[Follower] = &[
    Token("="),
    Token(":"),
    Token("!"),
    Token(","),
    Token(";"),
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
                let name = index + 1 + usize::from(next == Some("*"));
                return SCRIPT.row_name(tokens, index, name);
            }
            "const" if next == Some("enum") => index += 1,
            _ if SCRIPT.has_row(token) => return SCRIPT.row_name(tokens, index, index + 1),
            "*" => index += 1,
            "export" | "default" if next == Some("@") => index += 1,
            _ if is_modifier(tokens, token, next) => {
                member |= MEMBER_MODIFIERS.contains(&token);
                index += 1;
            }
            "get" | "set" if next.is_some_and(|next| tokens.is_word(next)) => index += 1,
            _ if tokens.is_word(token) && !REFUSED.contains(&token) => {
                return method(tokens, index).or_else(|| property(tokens, index, member));
            }
            _ => return None,
        }
    }
}

fn is_modifier(tokens: Tokens<'_, '_>, token: &str, next: Option<&str>) -> bool {
    SCRIPT_MODIFIERS.contains(&token)
        && next.is_some_and(|next| tokens.is_word(next) || next == "*")
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
