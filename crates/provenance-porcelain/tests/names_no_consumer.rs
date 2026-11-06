//! The porcelain sources name no consumer of the commands: no identifier outside
//! comments and literals has the word `cli`, `mcp`, `clap`, or `rmcp`.

use provenance_macros::verifies;
use std::path::{Path, PathBuf};

const CONSUMER_WORDS: &[&str] = &["cli", "mcp", "clap", "rmcp"];

/// The source text with each comment, string, and character literal removed.
fn code(source: &str) -> String {
    let chars = source.chars().collect::<Vec<_>>();
    let mut code = String::with_capacity(source.len());
    let mut index = 0;
    while index < chars.len() {
        let rest = &chars[index..];
        if rest.starts_with(&['/', '/']) {
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
        } else if rest.starts_with(&['/', '*']) {
            index = block_comment_end(&chars, index);
        } else if let Some(end) = raw_string_end(&chars, index) {
            index = end;
        } else if chars[index] == '"' {
            index = string_end(&chars, index + 1);
            code.push(' ');
        } else if let Some(end) = char_literal_end(&chars, index) {
            index = end;
            code.push(' ');
        } else {
            code.push(chars[index]);
            index += 1;
        }
    }
    code
}

/// The index after the nested block comment that starts at `start`.
fn block_comment_end(chars: &[char], start: usize) -> usize {
    let mut depth = 0;
    let mut index = start;
    while index < chars.len() {
        if chars[index..].starts_with(&['/', '*']) {
            depth += 1;
            index += 2;
        } else if chars[index..].starts_with(&['*', '/']) {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    index
}

/// The index after the string whose text starts at `start`.
const fn string_end(chars: &[char], start: usize) -> usize {
    let mut index = start;
    while index < chars.len() {
        match chars[index] {
            '\\' => index += 2,
            '"' => return index + 1,
            _ => index += 1,
        }
    }
    index
}

/// The index after the raw string that starts at `start`, when one starts there.
fn raw_string_end(chars: &[char], start: usize) -> Option<usize> {
    let mut index = start;
    if chars.get(index) == Some(&'b') {
        index += 1;
    }
    if chars.get(index) != Some(&'r') || (start > 0 && is_ident(chars[start - 1])) {
        return None;
    }
    index += 1;
    let hashes = chars[index..].iter().take_while(|c| **c == '#').count();
    index += hashes;
    if chars.get(index) != Some(&'"') {
        return None;
    }
    index += 1;
    while index < chars.len() {
        if chars[index] == '"' && chars[index + 1..].iter().take(hashes).all(|c| *c == '#') {
            return Some(index + 1 + hashes);
        }
        index += 1;
    }
    Some(index)
}

/// The index after the character literal that starts at `start`. A lifetime is not one.
fn char_literal_end(chars: &[char], start: usize) -> Option<usize> {
    if chars[start] != '\'' {
        return None;
    }
    match chars.get(start + 1) {
        Some('\\') => chars[start + 2..]
            .iter()
            .position(|c| *c == '\'')
            .map(|offset| start + 3 + offset),
        Some(_) if chars.get(start + 2) == Some(&'\'') => Some(start + 3),
        _ => None,
    }
}

const fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The lowercase words of one identifier, split at `_` and at each capital letter.
fn words(identifier: &str) -> Vec<String> {
    let mut words = Vec::new();
    for part in identifier.split('_').filter(|part| !part.is_empty()) {
        let mut word = String::new();
        let chars = part.chars().collect::<Vec<_>>();
        for (index, c) in chars.iter().enumerate() {
            let next_lower = chars.get(index + 1).is_some_and(char::is_ascii_lowercase);
            let previous_lower = index > 0 && !chars[index - 1].is_ascii_uppercase();
            let boundary = previous_lower || next_lower;
            if boundary && c.is_ascii_uppercase() && !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            word.push(c.to_ascii_lowercase());
        }
        words.push(word);
    }
    words
}

/// The identifiers of `source` that have a consumer word.
fn consumer_identifiers(source: &str) -> Vec<String> {
    code(source)
        .split(|c: char| !is_ident(c))
        .filter(|identifier| {
            words(identifier)
                .iter()
                .any(|word| CONSUMER_WORDS.contains(&word.as_str()))
        })
        .map(str::to_owned)
        .collect()
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

/// No porcelain source names a consumer.
#[test]
#[verifies("rule_porcelain_names_no_consumer", exhaustion)]
fn porcelain_sources_name_no_consumer() {
    let mut files = Vec::new();
    rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let found = files
        .iter()
        .flat_map(|path| {
            let source = std::fs::read_to_string(path).unwrap();
            consumer_identifiers(&source)
                .into_iter()
                .map(move |identifier| format!("{}: {identifier}", path.display()))
        })
        .collect::<Vec<_>>();
    assert_eq!(found, Vec::<String>::new());
}

/// The scan refuses a consumer word in an identifier, and accepts it in comments,
/// strings, and other words.
#[test]
#[verifies("rule_porcelain_names_no_consumer", examples)]
fn scan_refuses_only_identifiers_with_a_consumer_word() {
    let source = r#"
        // A cli_input comment.
        /* An McpInput /* nested */ comment. */
        /// The CLI help.
        const TEXT: &str = "cli_form and MCP";
        const RAW: &str = r"McpTool cli";
        const QUOTE: char = '"';
        fn shared<'a>(value: &'a str, clients: u8, climb: u8) -> &'a str { value }
        struct CheckCliInput;
        fn mcp_schema() {}
        enum Form { ClapArg, RmcpTool, CLIForm }
    "#;
    assert_eq!(
        consumer_identifiers(source),
        [
            "CheckCliInput",
            "mcp_schema",
            "ClapArg",
            "RmcpTool",
            "CLIForm"
        ]
    );
}
