//! The item below a binding: which declaration a binding comment or a binding
//! attribute marks, and the name of that declaration.
//!
//! The scanner does not parse a syntax tree. Each language has one function
//! that reads the tokens of the code below a binding and gives the name of the
//! function or type that they declare.

use provenance_macros::rule;

use super::Language;

/// The number of code lines below a binding that the scanner reads. Comment
/// lines and empty lines do not count.
const WINDOW: usize = 6;

/// What a declaration declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Function,
    Type,
}

/// The name of the function or type that the first code lines below a binding
/// declare, or else the name of the first function in the window.
#[rule("rule_site_item_name_is_the_declaration_below")]
pub(super) fn item_name(language: Language, following: &[&str]) -> Option<String> {
    let window = following
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !is_comment(language, line))
        .take(WINDOW)
        .collect::<Vec<_>>();
    let header = window.join("\n");
    if let Some((_, name)) = declaration(language, &tokens(language, &header)) {
        return Some(name);
    }
    window.iter().find_map(|line| {
        let tokens = tokens(language, line);
        match declaration(language, &tokens) {
            Some((Kind::Function, name)) => Some(name),
            _ if is_script(language) => function_expression(&tokens),
            _ => None,
        }
    })
}

/// The declaration at the start of `tokens`, in the grammar of `language`.
fn declaration(language: Language, tokens: &[&str]) -> Option<(Kind, String)> {
    match language {
        Language::Rust => rust(tokens),
        Language::Python => python(tokens),
        Language::Go => go(tokens),
        Language::JavaScript | Language::TypeScript => script(tokens),
        Language::Java => java(tokens),
    }
}

const fn is_script(language: Language) -> bool {
    matches!(language, Language::JavaScript | Language::TypeScript)
}

fn is_comment(language: Language, line: &str) -> bool {
    if language == Language::Python {
        return line.starts_with('#');
    }
    line.starts_with("//") || line.starts_with("/*") || line.starts_with('*')
}

/// `fn`, `struct`, `enum`, `union`, `trait`, and `type` after attributes,
/// visibility, and the function qualifiers.
fn rust(tokens: &[&str]) -> Option<(Kind, String)> {
    let mut index = 0;
    loop {
        let next = tokens.get(index + 1).copied();
        match *tokens.get(index)? {
            "#" => {
                let inner = usize::from(next == Some("!"));
                index = skip_group(tokens, index + 1 + inner, "[", "]");
            }
            "pub" => index = skip_group(tokens, index + 1, "(", ")"),
            "default" | "async" | "unsafe" => index += 1,
            "extern" => index += 1 + usize::from(next.is_some_and(|abi| abi.starts_with('"'))),
            "const" if matches!(next, Some("fn" | "async" | "unsafe" | "extern")) => index += 1,
            "fn" => return named(tokens, index + 1, Kind::Function),
            "struct" | "enum" | "union" | "trait" | "type" => {
                return named(tokens, index + 1, Kind::Type)
            }
            _ => return None,
        }
    }
}

/// `def` and `class` after decorators and `async`.
fn python(tokens: &[&str]) -> Option<(Kind, String)> {
    let mut index = 0;
    loop {
        match *tokens.get(index)? {
            "@" => index = skip_annotation(tokens, index + 1),
            "async" => index += 1,
            "def" => return named(tokens, index + 1, Kind::Function),
            "class" => return named(tokens, index + 1, Kind::Type),
            _ => return None,
        }
    }
}

/// `func`, with or without a receiver, and `type`.
fn go(tokens: &[&str]) -> Option<(Kind, String)> {
    match *tokens.first()? {
        "func" => named(tokens, skip_group(tokens, 1, "(", ")"), Kind::Function),
        "type" => named(tokens, 1, Kind::Type),
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
fn script(tokens: &[&str]) -> Option<(Kind, String)> {
    let mut index = 0;
    loop {
        let token = *tokens.get(index)?;
        let next = tokens.get(index + 1).copied();
        match token {
            "@" => index = skip_annotation(tokens, index + 1),
            "function" => {
                let generator = next == Some("*");
                return named(tokens, index + 1 + usize::from(generator), Kind::Function);
            }
            "class" | "interface" | "type" | "enum" => return named(tokens, index + 1, Kind::Type),
            "const" if next == Some("enum") => index += 1,
            "const" | "let" | "var" => return named(tokens, index + 1, Kind::Function),
            "*" => index += 1,
            _ if is_modifier(&SCRIPT_MODIFIERS, token, next) => index += 1,
            "get" | "set" if next.is_some_and(is_identifier) => index += 1,
            _ if is_identifier(token) && !SCRIPT_KEYWORDS.contains(&token) => {
                return script_method(tokens, index)
            }
            _ => return None,
        }
    }
}

/// A class method: a name, type parameters, and a parameter list that a body
/// or a return type follows.
fn script_method(tokens: &[&str], name: usize) -> Option<(Kind, String)> {
    let mut index = name + 1;
    index += usize::from(tokens.get(index) == Some(&"?"));
    index = skip_group(tokens, index, "<", ">");
    if tokens.get(index) != Some(&"(") {
        return None;
    }
    let after = skip_group(tokens, index, "(", ")");
    matches!(tokens.get(after), Some(&("{" | ":")))
        .then(|| (Kind::Function, tokens[name].to_string()))
}

/// The name of a named function expression, such as the test function that a
/// test runner call takes.
fn function_expression(tokens: &[&str]) -> Option<String> {
    let index = tokens.iter().position(|token| *token == "function")?;
    let index = index + 1 + usize::from(tokens.get(index + 1) == Some(&"*"));
    named(tokens, index, Kind::Function).map(|(_, name)| name)
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

const JAVA_KEYWORDS: [&str; 14] = [
    "return", "new", "throw", "if", "for", "while", "switch", "catch", "else", "do", "try",
    "assert", "case", "yield",
];

/// A class, an interface, an enum, a record, an annotation type, a method, or
/// a constructor, after annotations and modifiers.
fn java(tokens: &[&str]) -> Option<(Kind, String)> {
    let mut index = 0;
    let mut modified = false;
    loop {
        let token = *tokens.get(index)?;
        let next = tokens.get(index + 1).copied();
        match token {
            "@" if next == Some("interface") => return named(tokens, index + 2, Kind::Type),
            "@" => index = skip_annotation(tokens, index + 1),
            "class" | "interface" | "enum" | "record" => {
                return named(tokens, index + 1, Kind::Type)
            }
            "non" if next == Some("-") && tokens.get(index + 2) == Some(&"sealed") => {
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

/// A method: type parameters, a return type, a name, and a parameter list. A
/// constructor has no return type, so it needs a modifier or a body.
fn java_method(tokens: &[&str], start: usize, modified: bool) -> Option<(Kind, String)> {
    let start = skip_group(tokens, start, "<", ">");
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match *token {
            "<" => depth += 1,
            ">" => depth = depth.saturating_sub(1),
            "(" if depth == 0 => {
                let name = *tokens.get(index.checked_sub(1)?)?;
                let typed = index > start + 1 && tokens[index - 2] != ".";
                let after = tokens.get(skip_group(tokens, index, "(", ")")).copied();
                let body = matches!(after, Some("{" | "throws"));
                return (is_identifier(name) && (typed || modified || body))
                    .then(|| (Kind::Function, name.to_string()));
            }
            "=" | ";" | "{" | ")" => return None,
            _ if depth == 0 && JAVA_KEYWORDS.contains(token) => return None,
            _ => {}
        }
    }
    None
}

fn is_modifier(modifiers: &[&str], token: &str, next: Option<&str>) -> bool {
    modifiers.contains(&token) && next.is_some_and(|next| is_identifier(next) || next == "*")
}

/// The token at `index` as the name of a declaration of `kind`.
fn named(tokens: &[&str], index: usize, kind: Kind) -> Option<(Kind, String)> {
    let name = *tokens.get(index)?;
    is_identifier(name).then(|| (kind, name.to_string()))
}

/// The index after an annotation or a decorator name and its argument list.
fn skip_annotation(tokens: &[&str], mut index: usize) -> usize {
    while tokens.get(index).is_some_and(|token| is_identifier(token) || *token == ".") {
        index += 1;
    }
    skip_group(tokens, index, "(", ")")
}

/// The index after the group that opens at `index`, or `index` when no group
/// opens there.
fn skip_group(tokens: &[&str], index: usize, open: &str, close: &str) -> usize {
    if tokens.get(index) != Some(&open) {
        return index;
    }
    let mut depth = 0usize;
    for (position, token) in tokens.iter().enumerate().skip(index) {
        if *token == open {
            depth += 1;
        } else if *token == close {
            depth -= 1;
            if depth == 0 {
                return position + 1;
            }
        }
    }
    tokens.len()
}

fn is_identifier(token: &str) -> bool {
    token
        .chars()
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_' || first == '$')
}

fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == '$'
}

/// Words, quoted strings, and single punctuation characters. A Rust `'` starts
/// a lifetime, not a string.
fn tokens(language: Language, text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut characters = text.char_indices().peekable();
    while let Some((start, character)) = characters.next() {
        if character.is_whitespace() {
            continue;
        }
        let quote = character == '"' || (language != Language::Rust && "'`".contains(character));
        let mut end = start + character.len_utf8();
        if is_word(character) {
            while let Some((index, next)) = characters.next_if(|(_, next)| is_word(*next)) {
                end = index + next.len_utf8();
            }
        } else if quote {
            let mut escaped = false;
            for (index, next) in characters.by_ref() {
                end = index + next.len_utf8();
                if next == character && !escaped {
                    break;
                }
                escaped = next == '\\' && !escaped;
            }
        }
        tokens.push(&text[start..end]);
    }
    tokens
}
