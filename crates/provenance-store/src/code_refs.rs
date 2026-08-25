use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LineRange {
    pub start: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<u32>,
}

impl LineRange {
    pub const fn new(start: u32, end: Option<u32>) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeRef {
    pub path: String,
    pub lines: Vec<LineRange>,
}

/// Parses a code reference such as `src/UseCase.php:153-156`.
///
/// Returns `None` for anything that does not read as a file reference, so
/// every caller inherits the same prose/path decision. Line groups accept
/// single lines, `-`/en-dash ranges, and comma-separated lists.
pub fn parse_code_ref(text: &str) -> Option<CodeRef> {
    let text = text.trim();
    let (path, lines_part) = text
        .split_once(':')
        .map_or((text, None), |(path, lines)| (path, Some(lines)));
    let lines = match lines_part {
        Some(lines_part) => parse_line_ranges(lines_part)?,
        None => Vec::new(),
    };
    if !reads_as_file_reference(path, &lines) {
        return None;
    }
    Some(CodeRef {
        path: path.to_string(),
        lines,
    })
}

/// Finds code references inside a compound source-reference field.
pub fn parse_code_refs(text: &str) -> Vec<CodeRef> {
    if let Some(code_ref) = parse_code_ref(text) {
        return vec![code_ref];
    }
    text.split_whitespace()
        .filter_map(|token| {
            let token = token.trim_matches(|character: char| {
                matches!(character, '(' | ')' | '[' | ']' | '{' | '}' | ';')
            });
            parse_code_ref(token.strip_suffix('.').unwrap_or(token))
        })
        .collect()
}

/// The one place that decides whether text reads as a file reference rather
/// than prose. Both the whole-field surface and the free-text surface reach
/// it through [`parse_code_ref`], so neither can drift from the other.
///
/// A directory separator is enough on its own: nobody writes `src/foo` in a
/// sentence by accident. A bare dotted token is not, because English is full
/// of them — `e.g.`, `Fig.`, `etc.`, `v1.2` — and nothing in the text tells
/// `payroll.rs` apart from those. Such a token only reads as a file when a
/// line group is attached: `payroll.rs:12`.
fn reads_as_file_reference(path: &str, lines: &[LineRange]) -> bool {
    if path.is_empty() || path.contains("://") || path.chars().any(char::is_whitespace) {
        return false;
    }
    if path.contains('/') {
        return true;
    }
    path.contains('.') && !lines.is_empty()
}

/// Strips a leading "line"/"lines" word (case-insensitive), as in the
/// common human-written form `UseCase.php:lines 153-156`, so the numeric
/// parser underneath never has to know about it.
fn strip_leading_lines_word(part: &str) -> &str {
    let trimmed = part.trim_start();
    for word in ["lines", "line"] {
        if trimmed.len() > word.len()
            && trimmed.as_bytes()[word.len()].is_ascii_whitespace()
            && trimmed[..word.len()].eq_ignore_ascii_case(word)
        {
            return trimmed[word.len()..].trim_start();
        }
    }
    trimmed
}

fn parse_line_ranges(part: &str) -> Option<Vec<LineRange>> {
    let part = strip_leading_lines_word(part);
    part.split(',')
        .map(|group| {
            let group = group.trim();
            let (start, end) = group
                .split_once(['-', '\u{2013}'])
                .map_or((group, None), |(start, end)| {
                    (start.trim(), Some(end.trim()))
                });
            let start = start.parse().ok()?;
            let end = end.map(str::parse).transpose().ok()?;
            end.is_none_or(|end| end >= start)
                .then(|| LineRange::new(start, end))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_code_ref_reads_a_plain_path() {
        let code_ref = parse_code_ref("docs/save-invoice.md").unwrap();
        assert_eq!(code_ref.path, "docs/save-invoice.md");
        assert!(code_ref.lines.is_empty());
    }

    #[test]
    fn parse_code_ref_reads_a_single_line() {
        let code_ref = parse_code_ref("UseCase.php:153").unwrap();
        assert_eq!(code_ref.path, "UseCase.php");
        assert_eq!(code_ref.lines, vec![LineRange::new(153, None)]);
    }

    #[test]
    fn parse_code_ref_reads_a_line_range() {
        let code_ref = parse_code_ref("UseCase.php:153-156").unwrap();
        assert_eq!(code_ref.lines, vec![LineRange::new(153, Some(156))]);
    }

    #[test]
    fn parse_code_ref_rejects_a_descending_line_range() {
        assert!(parse_code_ref("UseCase.php:20-10").is_none());
    }

    #[test]
    fn parse_code_ref_accepts_a_leading_lines_word() {
        let code_ref = parse_code_ref("UseCase.php:lines 153-156").unwrap();
        assert_eq!(code_ref.lines, vec![LineRange::new(153, Some(156))]);
    }

    #[test]
    fn parse_code_ref_accepts_a_leading_line_word_singular() {
        let code_ref = parse_code_ref("UseCase.php:line 42").unwrap();
        assert_eq!(code_ref.lines, vec![LineRange::new(42, None)]);
    }

    #[test]
    fn parse_code_ref_accepts_lines_word_case_insensitively_with_extra_space() {
        let code_ref = parse_code_ref("UseCase.php: Lines  59-69").unwrap();
        assert_eq!(code_ref.lines, vec![LineRange::new(59, Some(69))]);
    }

    #[test]
    fn parse_code_ref_accepts_en_dash_ranges() {
        let code_ref = parse_code_ref("UseCase.php:59\u{2013}69").unwrap();
        assert_eq!(code_ref.lines, vec![LineRange::new(59, Some(69))]);
    }

    #[test]
    fn parse_code_ref_reads_comma_separated_line_groups() {
        let code_ref = parse_code_ref("UseCase.php:168, 193, 218").unwrap();
        assert_eq!(
            code_ref.lines,
            vec![
                LineRange::new(168, None),
                LineRange::new(193, None),
                LineRange::new(218, None),
            ]
        );
    }

    #[test]
    fn parse_code_ref_rejects_prose_urls_and_bare_words() {
        assert!(parse_code_ref("Section 7.2 of the award").is_none());
        assert!(parse_code_ref("https://example.com/handbook").is_none());
        assert!(parse_code_ref("README").is_none());
        assert!(parse_code_ref("12:30pm").is_none());
        assert!(parse_code_ref("").is_none());
    }

    #[test]
    fn parse_code_ref_rejects_bare_dotted_tokens() {
        // Prose punctuation and a real file name are indistinguishable
        // without a directory or a line group, so both stay prose.
        for text in ["e.g.", "Fig.", "etc.", "v1.2", "payroll.rs"] {
            assert!(parse_code_ref(text).is_none(), "`{text}` should be prose");
        }
    }

    #[test]
    fn parse_code_ref_reads_a_bare_file_name_with_a_line_group() {
        let code_ref = parse_code_ref("parser.rs:12").unwrap();
        assert_eq!(code_ref.path, "parser.rs");
        assert_eq!(code_ref.lines, vec![LineRange::new(12, None)]);
    }

    #[test]
    fn parse_code_ref_reads_a_directory_path_without_an_extension() {
        let code_ref = parse_code_ref("src/UseCase").unwrap();
        assert_eq!(code_ref.path, "src/UseCase");
        assert!(code_ref.lines.is_empty());
    }

    #[test]
    fn parse_code_refs_finds_a_path_inside_compound_source_text() {
        let refs = parse_code_refs("owner decision; docs/policy.md:2");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].path, "docs/policy.md");
        assert_eq!(refs[0].lines, vec![LineRange::new(2, None)]);
    }
}
