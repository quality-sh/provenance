//! Implementation aid: no Rule states the form of a store error. This check
//! finds a raw anyhow refusal in store code before review, so that each new
//! error has one of the declared forms of `src/refusal.rs`. It does not judge
//! if the kind of a refusal is correct; the review of each site does that.
//!
//! The check reads the Rust tokens of each file below `src`, without comments,
//! strings and character literals, and refuses these forms outside
//! `src/refusal.rs` and `src/write_error.rs`:
//!
//! - `bail!`, `anyhow!` and `format_err!`.
//! - anyhow `ensure!`: an `ensure!` whose arguments do not start with a kind
//!   name, a comma and a condition, as the typed `write_error::ensure!` does.
//! - `Error::msg`, which includes `anyhow::Error::msg`, `anyhow::Error::new`,
//!   and a call of `anyhow::Error::from`.
//! - `anyhow::ensure!`, with any arguments.
//! - A `use anyhow::...` that imports `bail`, `ensure`, `anyhow` or
//!   `format_err`.
//!
//! Test code is not read: test files and directories, `test_support.rs`, and
//! each item whose `cfg` needs `test` (`cfg(test)` or `cfg(all(test, ...))`).
//! An item under `cfg(any(test, ...))` is read, because a fixture build has it.
//! A `cfg` attribute on a match arm, a field, a variant or a statement hides
//! nothing: only an attribute before an item keyword hides the item.
//!
//! The check cannot find these errors with no declared kind:
//!
//! - `.context(...)` or `.with_context(...)` on an `Option`. A text rule cannot
//!   tell an `Option` from a `Result`, and on a `Result` the call keeps the
//!   native cause, which is correct.
//! - `?` on a native error, such as an I/O, serde or git error. By design, it
//!   gives a fault with its cause. `map_err(anyhow::Error::from)` and `into()`
//!   are the same conversion.
//! - Text in another error type: `std::io::Error::other("...")`,
//!   `io::Error::new(kind, "...")`, or a `thiserror` type that no probe in
//!   `WriteError::safe` or `ReadError::safe` knows.
//! - A name that an alias hides, such as `use anyhow::Error as E; E::msg(...)`,
//!   `use anyhow::Error; Error::new(...)` or `<anyhow::Error>::msg(...)`.
//! - `fault!(...)` for an error that a caller can cause. The check cannot judge
//!   a kind.
//! - Errors of other crates, such as the refusals of `provenance-core` and of
//!   the transport porcelain. The check reads only `provenance-store/src`.
//! - A bare `ensure!` whose first argument is a constant with an upper-case
//!   name, such as `ensure!(READY, MESSAGE)`. The check takes the first word
//!   as a kind name when it starts with an upper-case letter.
//! - A production file with a test name: the check skips each file or
//!   directory named `tests`, `tests.rs`, `test_support.rs`, `*_tests` or
//!   `*_tests.rs`, in any directory, and does not read the module.
//! - A file that a `#[path]` attribute brings in from outside `src`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const ALLOWED: [&str; 2] = ["refusal.rs", "write_error.rs"];
const RAW_MACROS: [&str; 3] = ["bail", "anyhow", "format_err"];
const RAW_IMPORTS: [&str; 4] = ["bail", "ensure", "anyhow", "format_err"];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Mark(char),
}

/// Splits Rust source into words and marks, each with its line. Comments,
/// strings and character literals give no token.
fn tokens(source: &str) -> Vec<(Token, usize)> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut line = 1;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        if c == '\n' {
            line += 1;
            at += 1;
        } else if c.is_whitespace() {
            at += 1;
        } else if c == '/' && next == Some('/') {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while at < chars.len() {
                if chars[at] == '\n' {
                    line += 1;
                }
                if chars[at] == '/' && chars.get(at + 1) == Some(&'*') {
                    depth += 1;
                    at += 2;
                } else if chars[at] == '*' && chars.get(at + 1) == Some(&'/') {
                    depth -= 1;
                    at += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    at += 1;
                }
            }
        } else if c == '"' {
            at += 1;
            while at < chars.len() && chars[at] != '"' {
                if chars[at] == '\\' {
                    at += 1;
                }
                if chars.get(at) == Some(&'\n') {
                    line += 1;
                }
                at += 1;
            }
            at += 1;
        } else if c == '\'' {
            // A character literal gives no token. A lifetime gives a mark.
            if next == Some('\\') {
                // Skip the quote, the backslash and the escaped character.
                at += 3;
                while at < chars.len() && chars[at] != '\'' {
                    at += 1;
                }
                at += 1;
            } else if chars.get(at + 2) == Some(&'\'') {
                at += 3;
            } else {
                tokens.push((Token::Mark(c), line));
                at += 1;
            }
        } else if c.is_alphanumeric() || c == '_' {
            let start = at;
            while at < chars.len() && (chars[at].is_alphanumeric() || chars[at] == '_') {
                at += 1;
            }
            let word: String = chars[start..at].iter().collect();
            let hashes = chars[at..].iter().take_while(|c| **c == '#').count();
            if matches!(word.as_str(), "r" | "br") && chars.get(at + hashes) == Some(&'"') {
                at += hashes + 1;
                let close: Vec<char> = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                while at < chars.len() && !chars[at..].starts_with(&close) {
                    if chars[at] == '\n' {
                        line += 1;
                    }
                    at += 1;
                }
                at += close.len();
                continue;
            }
            tokens.push((Token::Word(word), line));
        } else {
            tokens.push((Token::Mark(c), line));
            at += 1;
        }
    }
    tokens
}

fn word(token: Option<&(Token, usize)>) -> Option<&str> {
    match token {
        Some((Token::Word(word), _)) => Some(word),
        _ => None,
    }
}

const fn mark(token: Option<&(Token, usize)>, expected: char) -> bool {
    matches!(token, Some((Token::Mark(found), _)) if *found == expected)
}

/// The word after the `::` that starts at `at`.
fn path_word(tokens: &[(Token, usize)], at: usize) -> Option<&str> {
    (mark(tokens.get(at), ':') && mark(tokens.get(at + 1), ':'))
        .then(|| word(tokens.get(at + 2)))
        .flatten()
}

/// The index after the item that starts at `at`: after its first `;` at
/// depth zero, or after the block of its first `{`.
fn item_end(tokens: &[(Token, usize)], mut at: usize) -> usize {
    let mut depth = 0usize;
    while at < tokens.len() {
        match tokens[at].0 {
            Token::Mark('{' | '(' | '[') => depth += 1,
            Token::Mark('}' | ')' | ']') => {
                depth = depth.saturating_sub(1);
                if depth == 0 && tokens[at].0 == Token::Mark('}') {
                    return at + 1;
                }
            }
            Token::Mark(';') if depth == 0 => return at + 1,
            _ => {}
        }
        at += 1;
    }
    at
}

/// The index after the group that opens at `at`.
const fn group_end(tokens: &[(Token, usize)], mut at: usize) -> usize {
    let mut depth = 0usize;
    while at < tokens.len() {
        match tokens[at].0 {
            Token::Mark('{' | '(' | '[') => depth += 1,
            Token::Mark('}' | ')' | ']') => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return at + 1;
                }
            }
            _ => {}
        }
        at += 1;
    }
    at
}

/// The words that can start an item after its attributes.
const ITEM_WORDS: [&str; 15] = [
    "fn", "mod", "impl", "use", "struct", "enum", "const", "static", "type", "pub", "trait",
    "macro_rules", "extern", "unsafe", "async",
];

/// The index of the first token after the attributes that start at `at`.
fn after_attributes(tokens: &[(Token, usize)], mut at: usize) -> usize {
    while mark(tokens.get(at), '#') && mark(tokens.get(at + 1), '[') {
        at = group_end(tokens, at + 1);
    }
    at
}

/// The tokens without the items whose `cfg` attribute needs `test`.
fn without_test_items(tokens: &[(Token, usize)]) -> Vec<(Token, usize)> {
    let mut kept = Vec::new();
    let mut at = 0;
    while at < tokens.len() {
        if mark(tokens.get(at), '#')
            && mark(tokens.get(at + 1), '[')
            && word(tokens.get(at + 2)) == Some("cfg")
        {
            let close = group_end(tokens, at + 1);
            let words: Vec<&str> = (at + 2..close)
                .filter_map(|index| word(tokens.get(index)))
                .collect();
            let needs_test = matches!(words.as_slice(), ["cfg", "test"])
                || (words.get(1) == Some(&"all")
                    && words.contains(&"test")
                    && !words.contains(&"any")
                    && !words.contains(&"not"));
            let item = after_attributes(tokens, close);
            if needs_test
                && word(tokens.get(item)).is_some_and(|name| ITEM_WORDS.contains(&name))
            {
                at = item_end(tokens, item);
                continue;
            }
        }
        kept.push(tokens[at].clone());
        at += 1;
    }
    kept
}

/// The line of each raw anyhow refusal in `source`.
fn raw_refusals(source: &str) -> Vec<usize> {
    let tokens = without_test_items(&tokens(source));
    let mut found = Vec::new();
    for at in 0..tokens.len() {
        let line = tokens[at].1;
        let current = word(tokens.get(at));
        let bang = mark(tokens.get(at + 1), '!');
        if bang && current.is_some_and(|name| RAW_MACROS.contains(&name)) {
            found.push(line);
        } else if bang && current == Some("ensure") {
            // Strings give no token, so `ensure!(FLAG, "text")` has no
            // condition after its comma.
            let kind = word(tokens.get(at + 3))
                .is_some_and(|name| name.starts_with(|c: char| c.is_ascii_uppercase()));
            let condition = !mark(tokens.get(at + 5), ')') && !mark(tokens.get(at + 5), ',');
            let anyhow_path = at >= 3 && word(tokens.get(at - 3)) == Some("anyhow");
            if anyhow_path || !(kind && mark(tokens.get(at + 4), ',') && condition) {
                found.push(line);
            }
        } else if (current == Some("Error") && path_word(&tokens, at + 1) == Some("msg"))
            || (current == Some("anyhow")
                && path_word(&tokens, at + 1) == Some("Error")
                && (path_word(&tokens, at + 4) == Some("new")
                    || (path_word(&tokens, at + 4) == Some("from")
                        && mark(tokens.get(at + 7), '('))))
        {
            found.push(line);
        } else if current == Some("use") && word(tokens.get(at + 1)) == Some("anyhow") {
            let end = item_end(&tokens, at);
            if (at + 2..end)
                .filter_map(|index| word(tokens.get(index)))
                .any(|name| RAW_IMPORTS.contains(&name))
            {
                found.push(line);
            }
        }
    }
    found
}

fn is_test_path(relative: &Path) -> bool {
    relative.components().any(|part| {
        let part = part.as_os_str().to_string_lossy();
        part == "tests"
            || part.ends_with("_tests")
            || part.ends_with("_tests.rs")
            || matches!(part.as_ref(), "tests.rs" | "test_support.rs")
    })
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

/// The raw refusal lines of each store file that has one, by the path below
/// `src` with `/` separators.
fn store_raw_refusals() -> BTreeMap<String, Vec<usize>> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&source, &mut files);
    files
        .into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(&source).unwrap().to_owned();
            let name = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            if is_test_path(&relative) || ALLOWED.contains(&name.as_str()) {
                return None;
            }
            let lines = raw_refusals(&std::fs::read_to_string(&path).unwrap());
            (!lines.is_empty()).then_some((name, lines))
        })
        .collect()
}

#[test]
fn store_code_raises_no_raw_anyhow_refusal() {
    let faults: Vec<String> = store_raw_refusals()
        .iter()
        .map(|(file, lines)| {
            format!("src/{file} lines {lines:?}: use a declared form of src/refusal.rs")
        })
        .collect();
    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

#[test]
fn the_scan_finds_each_raw_form_and_skips_test_code() {
    let source = r#"
use anyhow::{bail, Context};
fn a() -> anyhow::Result<()> {
    anyhow::ensure!(true, "text");
    ensure!(AlreadyExists, true, "typed");
    crate::write_error::ensure!(InvalidUpdate, true, "typed");
    let _ = anyhow::anyhow!("text");
    let _ = anyhow::Error::msg("text");
    let _ = "bail!(\"in a string\")";
    // bail!("in a comment")
    let _ = refuse!(AlreadyExists, "declared");
    let _ = fault!("declared");
    bail!("text")
}
#[cfg(test)]
mod tests {
    fn b() { bail!("test code") }
}
#[cfg(not(test))]
fn c() { let _ = anyhow::format_err!("text"); }
#[cfg(all(test, feature = "host"))]
fn d() { bail!("test code") }
#[cfg(any(test, feature = "test-fixture"))]
fn e() { bail!("fixture code") }
"#;
    assert_eq!(raw_refusals(source), vec![2, 4, 7, 8, 13, 20, 24]);
}

#[test]
fn the_scan_finds_each_import_and_each_untyped_ensure() {
    let source = r#"
use anyhow::ensure;
use anyhow::anyhow;
use anyhow::Context as _;
fn a(condition: bool) -> anyhow::Result<()> {
    ensure!(condition, "text");
    ensure!(FLAG, "text");
    ensure!(FLAG, "text {}", 1);
    ensure!(Fault, condition, "typed");
    let _ = anyhow::Error::new(std::io::Error::other("x"));
    let _ = std::io::Error::new(std::io::ErrorKind::Other, "x");
    Ok(())
}
"#;
    assert_eq!(raw_refusals(source), vec![2, 3, 6, 7, 8, 10]);
}

#[test]
fn raw_strings_character_literals_and_lifetimes_hide_no_form() {
    let source = r##"
fn a<'a>(text: &'a str) -> &'a str {
    let _ = r#"bail!("in a raw string")"#;
    let _ = br"anyhow!(1)";
    let _ = '"';
    let _ = '\'';
    let _ = anyhow::anyhow!("after the literals");
    text
}
"##;
    assert_eq!(raw_refusals(source), vec![7]);
}

#[test]
fn a_test_cfg_hides_only_an_item() {
    let source = r#"
fn a(x: u8) -> anyhow::Result<u8> {
    let value = match x {
        #[cfg(test)]
        0 => 1,
        _ => { bail!("after a test arm") }
    };
    #[cfg(test)]
    let _ = 1;
    let _ = anyhow::Error::from(std::io::Error::other("x"));
    let _ = std::io::Error::from(std::io::ErrorKind::NotFound);
    let _ = std::fs::read("x").map_err(anyhow::Error::from);
    anyhow::ensure!(READY, MESSAGE);
    Ok(value)
}
#[cfg(test)]
#[allow(dead_code)]
fn b() { bail!("test code") }
"#;
    assert_eq!(raw_refusals(source), vec![6, 10, 13]);
}
