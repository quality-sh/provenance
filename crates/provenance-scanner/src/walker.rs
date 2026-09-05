use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::coverage::EvidenceAnchor;

use crate::binding_lexer::{block_comment_state, code_outside_multiline_string, MultilineStyle};
use crate::parser::{annotation_marker_position, parse_annotations, Verification};
use crate::{Annotation, ParseWarning};

use bindings::parse_binding_line;
pub use bounded::scan_path_bounded;
use rust_lines::{rust_annotation_marker_position, rust_line_states, RustLexicalState};

mod bindings;
mod bounded;
#[cfg(test)]
mod bounded_tests;
mod rust_lines;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Java,
    Go,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(Self::Rust),
            "py" => Some(Self::Python),
            "js" | "jsx" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "java" => Some(Self::Java),
            "go" => Some(Self::Go),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AnnotationLocation {
    pub file_path: Utf8PathBuf,
    pub line: usize,
    pub function_name: Option<String>,
    pub anchor: EvidenceAnchor,
    pub annotation: Annotation,
}

/// A rule or verification binding found in source.
///
/// `verification` is `None` for a `#[rule]` implementation binding and
/// `Some` for a `#[verifies]` site (the item checks the rule, the method says
/// how).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AttributeBinding {
    pub file_path: Utf8PathBuf,
    pub line: usize,
    pub item_name: Option<String>,
    pub rule_id: String,
    pub verification: Option<Verification>,
    pub anchor: EvidenceAnchor,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FileScan {
    pub file_path: Utf8PathBuf,
    pub language: Language,
    pub annotations: Vec<AnnotationLocation>,
    pub bindings: Vec<AttributeBinding>,
    pub warnings: Vec<ParseWarning>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FileScanWithContent {
    pub scan: FileScan,
    pub content: String,
}

pub fn scan_path(path: &Utf8Path) -> anyhow::Result<Vec<FileScan>> {
    Ok(scan_path_with_content(path)?
        .into_iter()
        .map(|file| file.scan)
        .collect())
}

pub fn scan_path_with_content(path: &Utf8Path) -> anyhow::Result<Vec<FileScanWithContent>> {
    let mut scans = Vec::new();
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !is_ignored_directory(entry))
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(file_path) = Utf8PathBuf::from_path_buf(entry.path().to_path_buf()).ok() else {
            continue;
        };
        let Some(language) = file_path.extension().and_then(Language::from_extension) else {
            continue;
        };
        let content = std::fs::read_to_string(&file_path)
            .with_context(|| format!("read source file {file_path}"))?;
        scans.push(FileScanWithContent {
            scan: scan_file(&file_path, language, &content),
            content,
        });
    }
    scans.sort_by(|a, b| a.scan.file_path.cmp(&b.scan.file_path));
    Ok(scans)
}

fn is_ignored_directory(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir()
        && matches!(
            entry.file_name().to_str(),
            Some(".git" | "node_modules" | "target")
        )
}

/// Walks a file line by line, feeding two channels: bindings (`#[rule]`,
/// `#[verifies]`, decorators, and calls) and comment annotations.
///
/// Two lexers cooperate. The binding lexer tracks multiline strings and block
/// comments for every language; for Rust the per-line lexical states add
/// honest string, raw-string, and nested-comment tracking on top, gating both
/// channels.
pub fn scan_file(file_path: &Utf8Path, language: Language, content: &str) -> FileScan {
    let mut annotations = Vec::new();
    let mut bindings = Vec::new();
    let mut warnings = Vec::new();
    let lines = content.lines().collect::<Vec<_>>();
    let rust_states = rust_line_states(language, &lines);
    let mut idx = 0;
    let mut in_block_comment = false;
    let mut multiline_delimiter = None;
    while idx < lines.len() {
        let started_in_multiline_string = multiline_delimiter.is_some();
        let Some(line) = code_outside_multiline_string(
            lines[idx],
            multiline_style(language),
            &mut multiline_delimiter,
            in_block_comment,
        ) else {
            idx += 1;
            continue;
        };
        let started_in_block_comment = in_block_comment;
        if language != Language::Python {
            in_block_comment =
                block_comment_state(line, in_block_comment, language == Language::Rust);
        }
        let binding = (language != Language::Rust || rust_states[idx] == RustLexicalState::Code)
            .then(|| parse_binding_line(language, line, started_in_block_comment))
            .flatten();
        if let Some((rule_id, verification)) = binding {
            let item_name = binding_item_name(language, &lines, idx, verification);
            bindings.push(AttributeBinding {
                file_path: file_path.to_path_buf(),
                line: idx + 1,
                anchor: EvidenceAnchor::new(item_name.clone(), lines[idx]),
                item_name,
                rule_id,
                verification,
            });
            idx += 1;
            continue;
        }
        let marker_position = annotation_marker_start(
            language,
            lines[idx],
            rust_states[idx],
            started_in_block_comment,
            started_in_multiline_string,
        );
        let Some(marker_position) = marker_position else {
            idx += 1;
            continue;
        };
        let (comment, end_idx) = collect_annotation_comment(&lines, idx, marker_position);
        let parsed = parse_annotations(&comment);
        warnings.extend(parsed.warnings);
        let function_name = next_function_name(language, &lines[end_idx.saturating_add(1)..]);
        for annotation in parsed.annotations {
            annotations.push(AnnotationLocation {
                file_path: file_path.to_path_buf(),
                line: idx + 1,
                function_name: function_name.clone(),
                anchor: EvidenceAnchor::new(function_name.clone(), lines[idx]),
                annotation,
            });
        }
        if language != Language::Python {
            for consumed_line in lines.iter().take(end_idx + 1).skip(idx + 1) {
                in_block_comment = block_comment_state(
                    consumed_line,
                    in_block_comment,
                    language == Language::Rust,
                );
            }
        }
        idx = end_idx + 1;
    }
    FileScan {
        file_path: file_path.to_path_buf(),
        language,
        annotations,
        bindings,
        warnings,
    }
}

const fn multiline_style(language: Language) -> MultilineStyle {
    match language {
        Language::JavaScript | Language::TypeScript | Language::Go => MultilineStyle::Backtick,
        Language::Python => MultilineStyle::TripleBoth,
        Language::Java => MultilineStyle::TripleDouble,
        Language::Rust => MultilineStyle::RustRaw,
    }
}

/// Finds the annotation marker on a line the comment-line gate admits.
///
/// The gate is ratified behavior (`rule_prov_annot_014`): a directive binds
/// only when, after leading whitespace, the line starts with `//`, `/*`, or
/// `*` (or `#` in Python), or continues an open block comment. A marker
/// trailing code on the same line never binds. Past the gate, markers sitting
/// inside string literals quoted within the comment are still skipped.
fn annotation_marker_start(
    language: Language,
    line: &str,
    rust_state: RustLexicalState,
    started_in_block_comment: bool,
    started_in_multiline_string: bool,
) -> Option<usize> {
    if language == Language::Rust {
        let in_string = matches!(
            rust_state,
            RustLexicalState::Quoted | RustLexicalState::Raw(_)
        );
        let in_comment = matches!(rust_state, RustLexicalState::BlockComment(_));
        return (!in_string && is_annotation_comment_line(language, line, in_comment))
            .then(|| rust_annotation_marker_position(line, rust_state))
            .flatten();
    }
    (!started_in_multiline_string
        && is_annotation_comment_line(language, line, started_in_block_comment))
    .then(|| annotation_marker_position(line))
    .flatten()
}

fn is_annotation_comment_line(
    language: Language,
    line: &str,
    started_in_block_comment: bool,
) -> bool {
    if started_in_block_comment {
        return true;
    }
    let trimmed = line.trim_start();
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || (language == Language::Python && trimmed.starts_with('#'))
}

fn binding_item_name(
    language: Language,
    lines: &[&str],
    idx: usize,
    verification: Option<Verification>,
) -> Option<String> {
    if matches!(language, Language::Rust | Language::Python) {
        return next_item_name(language, &lines[idx + 1..]);
    }
    assignment_name(lines[idx]).or_else(|| {
        if verification.is_some() {
            enclosing_script_function(lines, idx)
        } else {
            preceding_assignment_name(lines, idx)
        }
    })
}

fn assignment_name(line: &str) -> Option<String> {
    let assigned = line.split_once('=')?.0.trim_end();
    let before_equals = assigned.trim_end_matches(':').trim_end();
    if let Some(name) = ["const ", "let ", "var "]
        .iter()
        .find_map(|marker| token_after(before_equals, marker))
    {
        return Some(name);
    }
    before_equals
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next_back()
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

fn preceding_assignment_name(lines: &[&str], idx: usize) -> Option<String> {
    lines[..idx]
        .iter()
        .rev()
        .find(|line| !line.trim().is_empty())
        .filter(|line| line.trim_end().ends_with('='))
        .and_then(|line| assignment_name(line))
}

fn enclosing_script_function(lines: &[&str], idx: usize) -> Option<String> {
    let mut depth = 0;
    for line in lines[..idx].iter().rev() {
        let line = line.trim();
        depth += line.matches('}').count();
        let item_name = if line.contains("function ") {
            token_after(line, "function ")
        } else if line.contains("=>") {
            assignment_name(line)
        } else {
            None
        };
        let openings = line.matches('{').count();
        if item_name.is_some() && openings > depth {
            return item_name;
        }
        depth = depth.saturating_sub(openings);
    }
    None
}

/// Like `next_function_name`, but looks past other attribute lines such as
/// `#[test]`, and also accepts type definitions (`construction` bindings sit
/// on types, not functions).
fn next_item_name(language: Language, following: &[&str]) -> Option<String> {
    following
        .iter()
        .filter(|line| !line.trim_start().starts_with("#["))
        .take(6)
        .find_map(|line| {
            let line = line.trim();
            function_name(language, line).or_else(|| type_name(line))
        })
}

fn type_name(line: &str) -> Option<String> {
    ["struct ", "enum ", "type "]
        .iter()
        .find(|marker| line.contains(*marker))
        .and_then(|marker| token_after(line, marker))
}

fn collect_annotation_comment(
    lines: &[&str],
    start: usize,
    marker_position: usize,
) -> (String, usize) {
    let mut end = start;
    while end + 1 < lines.len() && is_comment_continuation(lines[end + 1]) {
        end += 1;
    }
    let mut comment = lines[start][marker_position..].to_string();
    for continuation in lines.iter().take(end + 1).skip(start + 1) {
        comment.push('\n');
        comment.push_str(continuation);
    }
    (comment, end)
}

fn is_comment_continuation(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with('*')
        || trimmed.starts_with("/*")
        || trimmed.starts_with("*/")
}

fn next_function_name(language: Language, following: &[&str]) -> Option<String> {
    following
        .iter()
        .take(6)
        .find_map(|line| function_name(language, line.trim()))
}

fn function_name(language: Language, line: &str) -> Option<String> {
    let marker = match language {
        Language::Rust => "fn ",
        Language::Python => "def ",
        Language::Go => "func ",
        Language::JavaScript | Language::TypeScript | Language::Java => " ",
    };
    if matches!(language, Language::JavaScript | Language::TypeScript) {
        if line.contains("function ") {
            return token_after(line, "function ");
        }
        for declaration in ["const ", "let ", "var "] {
            if line.contains(declaration) {
                return token_after(line, declaration);
            }
        }
    }
    if language == Language::Go && line.starts_with("func (") {
        let after_receiver = line.split_once(") ")?.1;
        return token_after(after_receiver, "");
    }
    token_after(line, marker)
}

fn token_after(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    let name = line[start..]
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()?;
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
mod tests;
