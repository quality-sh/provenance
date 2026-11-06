//! Implementation aid: no Rule states where Git starts. Each Git start goes through
//! `provenance_store::git_process::git`, so a Windows process without a console opens no
//! window for Git (bead provenance-a5ru).
//!
//! The check reads the Rust tokens of each workspace source file, without comments, and
//! refuses these forms outside the helper:
//!
//! - `Command::new` with `"git"` or `"git.exe"` as a string or a raw string.
//! - `Command::new` with a name that a `let`, `const` or `static` sets to one of these
//!   strings in the same file.
//! - The same calls through a `type` alias or a `use ... Command as` name.
//! - A shell (`sh`, `bash`, `cmd`, `powershell`, `pwsh`) started with a string in the
//!   same statement that is a Git command line, for example `"git status"` or
//!   `"cd x && git log"`. A command line that a variable holds is not found.
//! - A Git library crate (`git2`, `gix`, `libgit2-sys`) in `Cargo.lock`.
//!
//! It does not find a Git path that the code builds at run time, for example from
//! `which`, from a format string, or from a path to `git.exe`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const HELPER: &str = "crates/provenance-store/src/git_process.rs";
const THIS_FILE: &str = "crates/provenance-store/tests/git_starts_use_the_helper.rs";
const GIT_PROGRAMS: [&str; 2] = ["git", "git.exe"];
const SHELLS: [&str; 7] = [
    "sh",
    "bash",
    "cmd",
    "cmd.exe",
    "powershell",
    "pwsh",
    "sh.exe",
];
const GIT_LIBRARIES: [&str; 3] = ["git2", "gix", "libgit2-sys"];

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Word(String),
    Text(String),
    Mark(char),
}

/// Splits Rust source into words, string contents and marks. Comments, spaces, numbers
/// inside words and character literals do not give tokens.
fn tokens(source: &str) -> Vec<Token> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        if c.is_whitespace() {
            at += 1;
        } else if c == '/' && next == Some('/') {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while at < chars.len() {
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
            let (text, end) = quoted(&chars, at + 1);
            tokens.push(Token::Text(text));
            at = end;
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
                tokens.push(Token::Mark(c));
                at += 1;
            }
        } else if c.is_alphanumeric() || c == '_' {
            let start = at;
            while at < chars.len() && (chars[at].is_alphanumeric() || chars[at] == '_') {
                at += 1;
            }
            let word: String = chars[start..at].iter().collect();
            if matches!(word.as_str(), "r" | "br") && matches!(chars.get(at), Some('"' | '#')) {
                let hashes = chars[at..].iter().take_while(|c| **c == '#').count();
                if chars.get(at + hashes) == Some(&'"') {
                    let (text, end) = raw(&chars, at + hashes + 1, hashes);
                    tokens.push(Token::Text(text));
                    at = end;
                    continue;
                }
            }
            tokens.push(Token::Word(word));
        } else {
            tokens.push(Token::Mark(c));
            at += 1;
        }
    }
    tokens
}

/// The text of a string that starts at `at`, after its opening quote, and the index
/// after its closing quote.
fn quoted(chars: &[char], mut at: usize) -> (String, usize) {
    let mut text = String::new();
    while at < chars.len() && chars[at] != '"' {
        if chars[at] == '\\' {
            at += 1;
            match chars.get(at) {
                Some('n') => text.push('\n'),
                Some(other) => text.push(*other),
                None => {}
            }
        } else {
            text.push(chars[at]);
        }
        at += 1;
    }
    (text, at + 1)
}

/// The text of a raw string with `hashes` marks, and the index after it.
fn raw(chars: &[char], mut at: usize, hashes: usize) -> (String, usize) {
    let mut text = String::new();
    while at < chars.len() {
        if chars[at] == '"'
            && chars[at + 1..]
                .iter()
                .take(hashes)
                .filter(|c| **c == '#')
                .count()
                == hashes
        {
            return (text, at + 1 + hashes);
        }
        text.push(chars[at]);
        at += 1;
    }
    (text, at)
}

fn word(token: Option<&Token>) -> Option<&str> {
    match token {
        Some(Token::Word(word)) => Some(word),
        _ => None,
    }
}

fn names_git(token: &Token) -> bool {
    matches!(token, Token::Text(text) if GIT_PROGRAMS.contains(&text.as_str()))
}

/// The direct Git starts in `source`.
fn direct_git_starts(source: &str) -> usize {
    let tokens = tokens(source);
    let mut commands = BTreeSet::from(["Command".to_owned()]);
    let mut git_names = BTreeSet::new();
    for (index, token) in tokens.iter().enumerate() {
        // `use ... Command as Name`
        if word(Some(token)) == Some("as")
            && word(tokens.get(index.wrapping_sub(1))) == Some("Command")
        {
            if let Some(name) = word(tokens.get(index + 1)) {
                commands.insert(name.to_owned());
            }
        }
        // `type Name = ...Command;`
        if word(Some(token)) == Some("type") {
            if let (Some(name), Some(end)) = (
                word(tokens.get(index + 1)),
                tokens[index..]
                    .iter()
                    .position(|token| *token == Token::Mark(';')),
            ) {
                if word(tokens.get(index + end - 1)) == Some("Command") {
                    commands.insert(name.to_owned());
                }
            }
        }
        // `let name = "git"`, `const NAME: &str = "git"`, `static NAME: &str = "git"`
        if names_git(token) && tokens.get(index.wrapping_sub(1)) == Some(&Token::Mark('=')) {
            let start = index.saturating_sub(10);
            if let Some(binding) = tokens[start..index].iter().rposition(|token| {
                matches!(word(Some(token)), Some("let" | "const" | "static" | "mut"))
            }) {
                if let Some(name) = word(tokens.get(start + binding + 1)) {
                    git_names.insert(name.to_owned());
                }
            }
        }
    }
    let mut starts = 0;
    for (index, window) in tokens.windows(6).enumerate() {
        let [Token::Word(command), Token::Mark(':'), Token::Mark(':'), Token::Word(new), Token::Mark('('), argument] =
            window
        else {
            continue;
        };
        if new != "new" || !commands.contains(command) {
            continue;
        }
        let argument_is_git = names_git(argument)
            || (word(Some(argument)).is_some_and(|name| git_names.contains(name))
                && tokens.get(index + 6) == Some(&Token::Mark(')')));
        if argument_is_git {
            starts += 1;
        }
        if matches!(argument, Token::Text(text) if SHELLS.contains(&text.as_str())) {
            starts += tokens[index + 6..]
                .iter()
                .take_while(|token| **token != Token::Mark(';'))
                .filter(|token| matches!(token, Token::Text(text) if is_git_command_line(text)))
                .count();
        }
    }
    starts
}

/// A shell command line that starts Git: `git` as the first word of the line or after
/// `&&`, `||`, `;` or `|`.
fn is_git_command_line(text: &str) -> bool {
    let mut first = true;
    for part in text.split_whitespace() {
        if first && GIT_PROGRAMS.contains(&part) {
            return true;
        }
        first = matches!(part, "&&" | "||" | ";" | "|") || part.ends_with(';');
    }
    false
}

#[test]
fn direct_git_starts_are_found_in_each_form() {
    for source in [
        "Command::new(\n    \"git\"\n)",
        "std::process::Command::new(\"git.exe\")",
        "tokio::process::Command::new(r\"git\")",
        "Command::new(r#\"git\"#)",
        "const GIT: &str = \"git\";\nfn f() { Command::new(GIT); }",
        "let program = \"git\"; Command::new(program).arg(\"log\");",
        "type Git = std::process::Command;\nGit::new(\"git\")",
        "use std::process::{Command as Raw, Stdio};\nRaw::new(\"git\")",
        "Command::new(\"sh\").args([\"-c\", \"cd repo && git log\"])",
        "Command::new(\"cmd\").args([\"/c\", \"git status\"])",
    ] {
        assert_eq!(direct_git_starts(source), 1, "{source}");
    }
    for source in [
        "Command::new(\"cargo\")",
        "// Command::new(\"git\")",
        "/* Command::new(\"git\") */",
        "let text = \"Command::new(\\\"git\\\")\";",
        "Command::new(\"sh\").args([\"-c\", \"command -v git\"])",
        "Command::new(\"bash\").arg(\"x.sh\");\nlet text = \"git diff\";",
        "type Shape = ScopedCommand; Shape::new(\"git\")",
        "let c = '\"'; Command::new(\"cargo\")",
    ] {
        assert_eq!(direct_git_starts(source), 0, "{source}");
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.file_type().unwrap().is_dir() {
            if !name.starts_with('.') && !matches!(name.as_ref(), "target" | "node_modules") {
                rust_files(&path, files);
            }
        } else if name.ends_with(".rs") {
            files.push(path);
        }
    }
}

#[test]
fn only_the_git_process_helper_starts_git() {
    let root = workspace_root();
    let mut files = Vec::new();
    for directory in ["crates", "examples", "tools", "fixtures"] {
        if root.join(directory).is_dir() {
            rust_files(&root.join(directory), &mut files);
        }
    }
    assert!(files.iter().any(|file| file.ends_with(HELPER)));
    let offenders: Vec<String> = files
        .iter()
        .filter(|file| !file.ends_with(HELPER) && !file.ends_with(THIS_FILE))
        .filter(|file| direct_git_starts(&std::fs::read_to_string(file).unwrap()) > 0)
        .map(|file| file.strip_prefix(&root).unwrap().display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "start Git with provenance_store::git_process::git() in: {offenders:?}"
    );
}

#[test]
fn no_git_library_crate_is_in_the_lock_file() {
    let lock = std::fs::read_to_string(workspace_root().join("Cargo.lock")).unwrap();
    let found: Vec<&str> = lock
        .lines()
        .filter_map(|line| line.strip_prefix("name = \"")?.strip_suffix('"'))
        .filter(|name| GIT_LIBRARIES.contains(name) || name.starts_with("gix-"))
        .collect();
    assert!(found.is_empty(), "Git library crates: {found:?}");
}
