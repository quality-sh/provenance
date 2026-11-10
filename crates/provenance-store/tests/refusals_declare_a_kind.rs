//! Implementation aid: no Rule states the form of a store error. This check
//! finds a raw anyhow refusal in store code before review, so that each new
//! error is a declared refusal, a declared fault or a located state fault
//! (`src/refusal.rs`). It does not judge if the kind of a refusal is correct;
//! the review of each converted site does that.
//!
//! The check reads the Rust tokens of each file below `src`, without comments
//! and strings, and refuses these forms outside `src/refusal.rs` and
//! `src/write_error.rs`:
//!
//! - `bail!`, `anyhow!` and `format_err!`.
//! - anyhow `ensure!`: an `ensure!` whose first argument is not a kind name
//!   followed by a comma, as the typed `write_error::ensure!` has.
//! - `Error::msg`, which includes `anyhow::Error::msg`.
//! - A `use anyhow::...` that imports `bail`, `ensure`, `anyhow` or
//!   `format_err`.
//!
//! Test code is not read: test files and directories, and each item that has
//! a `#[cfg(test)]` attribute. The baseline file gives the count of raw forms
//! that each file has before its conversion. A count can only go down: the
//! check fails when a file has more raw forms than its baseline line, or fewer.

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
                at += 2;
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

/// The tokens without the items that a `#[cfg(test)]` attribute selects.
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
            if words.contains(&"test") && !words.contains(&"not") {
                at = item_end(tokens, close);
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
            let kind = word(tokens.get(at + 3))
                .is_some_and(|name| name.starts_with(|c: char| c.is_ascii_uppercase()));
            if !(kind && mark(tokens.get(at + 4), ',')) {
                found.push(line);
            }
        } else if current == Some("Error")
            && mark(tokens.get(at + 1), ':')
            && mark(tokens.get(at + 2), ':')
            && word(tokens.get(at + 3)) == Some("msg")
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
            || matches!(
                part.as_ref(),
                "tests.rs" | "test_support.rs" | "test_probes.rs" | "fixture_probe.rs"
            )
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

fn baseline() -> BTreeMap<String, usize> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/refusals_declare_a_kind/baseline.txt");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (file, count) = line.rsplit_once(' ').unwrap();
            (file.to_owned(), count.parse().unwrap())
        })
        .collect()
}

#[test]
fn store_code_raises_no_raw_anyhow_refusal() {
    let found = store_raw_refusals();
    let baseline = baseline();
    let mut faults = Vec::new();
    for (file, lines) in &found {
        match baseline.get(file) {
            None => faults.push(format!(
                "src/{file} lines {lines:?}: use refuse!, fault! or state_fault (src/refusal.rs)"
            )),
            Some(count) if *count != lines.len() => faults.push(format!(
                "src/{file} lines {lines:?}: the baseline gives {count}; it must give {}",
                lines.len()
            )),
            Some(_) => {}
        }
    }
    for file in baseline.keys().filter(|file| !found.contains_key(*file)) {
        faults.push(format!(
            "src/{file} has no raw refusal; remove its baseline line"
        ));
    }
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
"#;
    assert_eq!(raw_refusals(source), vec![2, 4, 7, 8, 13, 20]);
}
