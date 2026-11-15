//! The item below a binding: which declaration a binding comment or a binding
//! attribute marks, and the name of that declaration.
//!
//! The scanner does not parse a syntax tree. Each language has one function
//! that reads the tokens of the code below a binding and gives the name of the
//! function or type that they declare.

use provenance_macros::rule;

use super::{block_comment_state, is_annotation_comment_line, Language};

/// The number of code lines, from the first line of a declaration, that can
/// hold its name. In a script class, the parameter list of a method must also
/// close in these lines. A declaration that takes more lines has no name.
const HEADER_LINES: usize = 6;

/// The name of the declaration that starts on the first code line below a
/// binding, when it is a function, a method, or a type.
#[rule("rule_site_item_name_is_the_declaration_below")]
pub(super) fn item_name(
    language: Language,
    following: &[&str],
    in_block_comment: bool,
) -> Option<String> {
    let mut lines = code_lines(language, following, in_block_comment);
    let mut tokens = Vec::new();
    let start = loop {
        tokens.extend(tokens_of(language, lines.next()?));
        let start = attributes_end(language, &tokens);
        if start < tokens.len() {
            break start;
        }
    };
    for line in lines.take(HEADER_LINES - 1) {
        tokens.extend(tokens_of(language, line));
    }
    declaration(language, &tokens[start..])
}

/// The lines below a binding that are not empty and not comment lines.
fn code_lines<'a>(
    language: Language,
    following: &'a [&'a str],
    mut in_block_comment: bool,
) -> impl Iterator<Item = &'a str> + 'a {
    following.iter().copied().filter(move |line| {
        let comment = is_annotation_comment_line(language, line, in_block_comment);
        if language != Language::Python {
            in_block_comment =
                block_comment_state(line, in_block_comment, language == Language::Rust);
        }
        !comment && !line.trim().is_empty()
    })
}

/// The index after the attributes, decorators, or annotations at the start
/// of `tokens`, with their argument lists.
fn attributes_end(language: Language, tokens: &[&str]) -> usize {
    let mut index = 0;
    loop {
        let next = tokens.get(index + 1).copied();
        index = match (language, tokens.get(index).copied()) {
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
fn declaration(language: Language, tokens: &[&str]) -> Option<String> {
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
fn rust(tokens: &[&str]) -> Option<String> {
    let mut index = 0;
    loop {
        let next = tokens.get(index + 1).copied();
        match *tokens.get(index)? {
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
fn python(tokens: &[&str]) -> Option<String> {
    let index = usize::from(tokens.first() == Some(&"async"));
    match *tokens.get(index)? {
        "def" | "class" => named(tokens, index + 1),
        _ => None,
    }
}

/// `func`, with or without a receiver, and `type`.
fn go(tokens: &[&str]) -> Option<String> {
    match *tokens.first()? {
        "func" => named(tokens, skip_group(tokens, 1, "(", ")")),
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
fn script(tokens: &[&str]) -> Option<String> {
    let mut index = 0;
    loop {
        let token = *tokens.get(index)?;
        let next = tokens.get(index + 1).copied();
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
fn script_method(tokens: &[&str], name: usize) -> Option<String> {
    let mut index = name + 1;
    index += usize::from(tokens.get(index) == Some(&"?"));
    index = skip_group(tokens, index, "<", ">");
    if tokens.get(index) != Some(&"(") {
        return None;
    }
    let after = skip_group(tokens, index, "(", ")");
    let body = matches!(tokens.get(after), Some(&("{" | ":")));
    body.then(|| tokens[name].to_string())
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
fn java(tokens: &[&str]) -> Option<String> {
    let mut index = 0;
    let mut modified = false;
    loop {
        let token = *tokens.get(index)?;
        let next = tokens.get(index + 1).copied();
        match token {
            "@" if next == Some("interface") => return named(tokens, index + 2),
            "@" => index = skip_annotation(tokens, index + 1),
            "class" | "interface" | "enum" | "record" => return named(tokens, index + 1),
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
fn java_method(tokens: &[&str], start: usize, modified: bool) -> Option<String> {
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
                    .then(|| name.to_string());
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

/// The token at `index` as the name of a declaration. The name of a Rust raw
/// identifier `r#match` is `match`.
fn named(tokens: &[&str], index: usize) -> Option<String> {
    let name = *tokens.get(index)?;
    let name = name.strip_prefix("r#").unwrap_or(name);
    is_identifier(name).then(|| name.to_string())
}

/// The index after an annotation or a decorator name, such as
/// `pytest.mark.parametrize`, and its argument list.
fn skip_annotation(tokens: &[&str], mut index: usize) -> usize {
    while tokens.get(index).is_some_and(|token| is_identifier(token)) {
        index += 1;
        if tokens.get(index) != Some(&".") {
            break;
        }
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

/// Words, quoted strings, and single punctuation characters of one line. A
/// Rust `'` starts a lifetime, not a string, and `r#` with a word is one raw
/// identifier.
fn tokens_of(language: Language, line: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut characters = line.char_indices().peekable();
    while let Some((start, character)) = characters.next() {
        if character.is_whitespace() {
            continue;
        }
        let quote = character == '"' || (language != Language::Rust && "'`".contains(character));
        let mut end = start + character.len_utf8();
        if is_word(character) {
            if language == Language::Rust && character == 'r' && is_raw_prefix(&line[end..]) {
                characters.next();
            }
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
        tokens.push(&line[start..end]);
    }
    tokens
}

/// Whether `rest`, the text after an `r`, continues a Rust raw identifier.
fn is_raw_prefix(rest: &str) -> bool {
    let mut characters = rest.chars();
    characters.next() == Some('#') && characters.next().is_some_and(is_word)
}
