//! The item below a binding: which declaration a binding comment or a binding
//! attribute marks, and the name of that declaration.
//!
//! The scanner does not parse a syntax tree. Each language has one function
//! that reads the tokens of the code below a binding, one at a time, and gives
//! the name of the function or type that they declare. The read stops where
//! the grammar decides.

mod tokens;

use provenance_macros::rule;

use super::Language;
pub(super) use tokens::LineStart;
use tokens::Tokens;

/// The name of the first declaration below a binding, when it is a function,
/// a method, or a type.
#[rule("rule_site_item_name_is_the_declaration_below")]
pub(super) fn item_name(
    language: Language,
    following: &[&str],
    start: LineStart<'_>,
) -> Option<String> {
    let mut tokens = Tokens::new(language, following, start);
    let first = attributes_end(language, &mut tokens);
    tokens.skip(first);
    declaration(language, &mut tokens)
}

/// The index after the attributes, decorators, or annotations at the start
/// of `tokens`, with their argument lists.
fn attributes_end(language: Language, tokens: &mut Tokens) -> usize {
    let mut index = 0;
    loop {
        let next = tokens.get(index + 1);
        index = match (language, tokens.get(index)) {
            (Language::Rust, Some("#")) => {
                let inner = usize::from(next == Some("!"));
                skip_group(tokens, index + 1 + inner, "[", "]")
            }
            (Language::Python | Language::JavaScript | Language::TypeScript, Some("@")) => {
                skip_annotation(tokens, index + 1)
            }
            (Language::Java, Some("@")) if next != Some("interface") => {
                skip_annotation(tokens, index + 1)
            }
            _ => return index,
        };
    }
}

/// The declaration at the start of `tokens`, in the grammar of `language`.
fn declaration(language: Language, tokens: &mut Tokens) -> Option<String> {
    match language {
        Language::Rust => rust(tokens),
        Language::Python => python(tokens),
        Language::Go => go(tokens),
        Language::JavaScript | Language::TypeScript => script(tokens),
        Language::Java => java(tokens),
    }
}

/// `fn`, `struct`, `enum`, `union`, `trait`, and `type` after visibility and
/// the function qualifiers.
fn rust(tokens: &mut Tokens) -> Option<String> {
    let mut index = 0;
    loop {
        let token = tokens.get(index)?;
        let next = tokens.get(index + 1);
        match token {
            "pub" => index = skip_group(tokens, index + 1, "(", ")"),
            "default" | "async" | "unsafe" => index += 1,
            "extern" => index += 1 + usize::from(next.is_some_and(|abi| abi.starts_with('"'))),
            "const" if matches!(next, Some("fn" | "async" | "unsafe" | "extern")) => index += 1,
            "fn" | "struct" | "enum" | "union" | "trait" | "type" => {
                return named(tokens, index + 1)
            }
            _ => return None,
        }
    }
}

/// `def` and `class`, after `async`.
fn python(tokens: &mut Tokens) -> Option<String> {
    let index = usize::from(tokens.get(0)? == "async");
    match tokens.get(index)? {
        "def" | "class" => named(tokens, index + 1),
        _ => None,
    }
}

/// `func`, with or without a receiver, and `type`.
fn go(tokens: &mut Tokens) -> Option<String> {
    match tokens.get(0)? {
        "func" => {
            let name = skip_group(tokens, 1, "(", ")");
            named(tokens, name)
        }
        "type" => named(tokens, 1),
        _ => None,
    }
}

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

const SCRIPT_KEYWORDS: [&str; 22] = [
    "if", "for", "while", "switch", "catch", "return", "throw", "new", "await", "yield", "typeof",
    "delete", "void", "do", "else", "try", "finally", "with", "super", "this", "import", "case",
];

/// A function, a class, an interface, a type alias, an enum, a `const`,
/// `let`, or `var` declaration, or a class method, after decorators and
/// modifiers.
fn script(tokens: &mut Tokens) -> Option<String> {
    let mut index = 0;
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
            "class" | "interface" | "type" | "enum" | "const" | "let" | "var" => {
                return named(tokens, index + 1)
            }
            "*" => index += 1,
            "export" | "default" if next == Some("@") => index += 1,
            _ if is_modifier(&SCRIPT_MODIFIERS, token, next) => index += 1,
            "get" | "set" if next.is_some_and(is_identifier) => index += 1,
            _ if is_identifier(token) && !SCRIPT_KEYWORDS.contains(&token) => {
                return script_method(tokens, index)
            }
            _ => return None,
        }
    }
}

/// A class method: a name, type parameters, and a parameter list that a
/// return type or a body on the line of the `)` follows. A `{` on a later
/// line follows a call statement as well, so it gives no name.
fn script_method(tokens: &mut Tokens, name: usize) -> Option<String> {
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
    if declared {
        tokens.get(name).map(str::to_owned)
    } else {
        None
    }
}

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

/// A class, an interface, an enum, a record, an annotation type, a method, or
/// a constructor, after annotations and modifiers.
fn java(tokens: &mut Tokens) -> Option<String> {
    let mut index = 0;
    let mut modified = false;
    loop {
        let token = tokens.get(index)?;
        let next = tokens.get(index + 1);
        match token {
            "@" if next == Some("interface") => return named(tokens, index + 2),
            "@" => index = skip_annotation(tokens, index + 1),
            "class" | "interface" | "enum" | "record" => return named(tokens, index + 1),
            "non" if next == Some("-") && tokens.get(index + 2) == Some("sealed") => {
                index += 3;
                modified = true;
            }
            _ if JAVA_MODIFIERS.contains(&token) => {
                index += 1;
                modified = true;
            }
            _ => return java_method(tokens, index, modified),
        }
    }
}

/// A method or a constructor: type parameters, a return type, a name, and a
/// parameter list. Outside type arguments, only the words of the type and the
/// name and the brackets of an array type come before the `(`, so a call, a
/// qualified call, and a statement that starts with a keyword give no name. A
/// constructor has no return type, so it needs a modifier or a body.
fn java_method(tokens: &mut Tokens, start: usize, modified: bool) -> Option<String> {
    let mut index = skip_group(tokens, start, "<", ">");
    let mut depth = 0usize;
    let mut words = 0;
    loop {
        let token = tokens.get(index)?;
        match token {
            "<" => depth += 1,
            ">" if depth > 0 => depth -= 1,
            _ if depth > 0 => {}
            "(" => {
                let name = tokens.get(index.checked_sub(1)?)?;
                let after = skip_group(tokens, index, "(", ")");
                let body = matches!(tokens.get(after), Some("{" | "throws"));
                let declared = words > 0 && is_identifier(name) && (words == 2 || modified || body);
                return declared.then(|| name.to_owned());
            }
            "[" | "]" => {}
            _ if words < 2 && is_identifier(token) && !is_java_reserved(token) => words += 1,
            _ => return None,
        }
        index += 1;
    }
}

fn is_java_reserved(token: &str) -> bool {
    JAVA_KEYWORDS.contains(&token) || JAVA_MODIFIERS.contains(&token)
}

fn is_modifier(modifiers: &[&str], token: &str, next: Option<&str>) -> bool {
    modifiers.contains(&token) && next.is_some_and(|next| is_identifier(next) || next == "*")
}

/// The token at `index` as the name of a declaration. The name of a Rust raw
/// identifier `r#match` is `match`.
fn named(tokens: &mut Tokens, index: usize) -> Option<String> {
    let name = tokens.get(index)?;
    let name = name.strip_prefix("r#").unwrap_or(name);
    is_identifier(name).then(|| name.to_string())
}

/// The index after an annotation or a decorator name, such as
/// `pytest.mark.parametrize`, and its argument list.
fn skip_annotation(tokens: &mut Tokens, mut index: usize) -> usize {
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
/// opens there. A group that does not close ends at the end of the file.
fn skip_group(tokens: &mut Tokens, index: usize, open: &str, close: &str) -> usize {
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
