//! Repository ignore rules for the scan walk.
//!
//! The scan covers what the repository tracks. Directories named like
//! dependency, build, or metadata trees are always skipped, and the
//! `.gitignore` file of the scanned root — or of any directory below it —
//! prunes the output trees the repository itself declares generated. A
//! directory name alone is never proof that its content is generated: an
//! unignored `dist` directory of legitimate source stays in the scan.
//!
//! The walk prunes directories; `.gitignore` rules that name single files
//! are not consulted. The matcher implements the working subset of
//! gitignore semantics this scanner needs: comments, blank lines, `!`
//! negation, a trailing `/` for directory-only patterns, anchoring through
//! a leading or interior `/`, and `*`, `**`, and `?` globs. It does not
//! implement escaped characters, character classes, or exclusion files
//! outside the scanned root.

use std::path::Path;

use walkdir::DirEntry;

use super::is_ignored_directory;

/// Decides which walk entries the repository scan admits.
///
/// Entries arrive parent before child, so a stack of per-directory pattern
/// levels mirrors the ancestor chain: a level is pushed when a directory is
/// admitted and popped once the walk leaves its subtree.
#[derive(Default)]
pub(super) struct RepositoryFilter {
    levels: Vec<IgnoreLevel>,
}

struct IgnoreLevel {
    depth: usize,
    directory: std::path::PathBuf,
    patterns: Vec<Pattern>,
}

impl RepositoryFilter {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn admits(&mut self, entry: &DirEntry) -> bool {
        let depth = entry.depth();
        while self.levels.last().is_some_and(|level| level.depth >= depth) {
            self.levels.pop();
        }
        if depth == 0 {
            self.push_level(depth, entry.path());
            return true;
        }
        if is_ignored_directory(entry) {
            return false;
        }
        if entry.file_type().is_dir() {
            if self.is_ignored(entry.path()) {
                return false;
            }
            self.push_level(depth, entry.path());
        }
        true
    }

    fn push_level(&mut self, depth: usize, directory: &Path) {
        let patterns: Vec<Pattern> = std::fs::read_to_string(directory.join(".gitignore"))
            .map(|text| text.lines().filter_map(parse_pattern).collect())
            .unwrap_or_default();
        if !patterns.is_empty() {
            self.levels.push(IgnoreLevel {
                depth,
                directory: directory.to_path_buf(),
                patterns,
            });
        }
    }

    /// Git precedence: deeper `.gitignore` files win, and within one file
    /// the last matching pattern decides.
    fn is_ignored(&self, path: &Path) -> bool {
        let mut ignored = false;
        for level in &self.levels {
            let Ok(relative) = path.strip_prefix(&level.directory) else {
                continue;
            };
            let segments: Vec<String> = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy().to_string())
                .collect();
            for pattern in &level.patterns {
                if pattern.matches(&segments) {
                    ignored = !pattern.negated;
                }
            }
        }
        ignored
    }
}

#[derive(Clone, Copy)]
enum Scope {
    AnyDepth,
    Anchored,
}

struct Pattern {
    negated: bool,
    scope: Scope,
    segments: Vec<String>,
}

fn parse_pattern(line: &str) -> Option<Pattern> {
    let trimmed = line.trim_end();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (negated, rest) = trimmed
        .strip_prefix('!')
        .map_or((false, trimmed), |rest| (true, rest));
    let rest = rest.trim_end_matches('/');
    let scope = if rest.contains('/') {
        Scope::Anchored
    } else {
        Scope::AnyDepth
    };
    let segments: Vec<String> = rest
        .trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    if segments.is_empty() {
        return None;
    }
    Some(Pattern {
        negated,
        scope,
        segments,
    })
}

impl Pattern {
    fn matches(&self, relative: &[String]) -> bool {
        let pattern: Vec<&str> = self.segments.iter().map(String::as_str).collect();
        match self.scope {
            Scope::Anchored => match_segments(&pattern, relative),
            Scope::AnyDepth => {
                (0..relative.len()).any(|start| match_segments(&pattern, &relative[start..]))
            }
        }
    }
}

/// Matches a slash-free glob against one path segment.
fn match_segment(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    match_chars(&pattern, &text)
}

fn match_chars(pattern: &[char], text: &[char]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some(('?', rest)) => !text.is_empty() && match_chars(rest, &text[1..]),
        Some(('*', rest)) => (0..=text.len()).any(|skip| match_chars(rest, &text[skip..])),
        Some((first, rest)) => text.first() == Some(first) && match_chars(rest, &text[1..]),
    }
}

/// Matches glob segments against path segments; `**` spans any number of
/// segments, including none.
fn match_segments(pattern: &[&str], text: &[String]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((first, rest)) if *first == "**" => {
            (0..=text.len()).any(|skip| match_segments(rest, &text[skip..]))
        }
        Some((first, rest)) => {
            !first.is_empty()
                && text.first().is_some_and(|head| match_segment(first, head))
                && match_segments(rest, &text[1..])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(text: &str) -> Pattern {
        parse_pattern(text).unwrap()
    }

    fn matches(pattern_text: &str, relative: &[&str]) -> bool {
        pattern(pattern_text).matches(
            &relative
                .iter()
                .map(|segment| (*segment).to_string())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn plain_patterns_match_at_any_depth() {
        assert!(matches("dist", &["dist"]));
        assert!(matches("dist", &["packages", "web", "dist"]));
        assert!(!matches("dist", &["dist", "generated.js"]));
        assert!(!matches("dist", &["distribution"]));
    }

    #[test]
    fn anchored_and_directory_only_patterns_match_their_paths() {
        assert!(matches("/dist", &["dist"]));
        assert!(!matches("/dist", &["packages", "dist"]));
        assert!(matches("dist/", &["packages", "dist"]));
        assert!(matches("**/out", &["packages", "web", "out"]));
        assert!(matches("**/out", &["out"]));
        assert!(!matches("build/*.tmp", &["build", "a", "x.tmp"]));
        assert!(matches("build/*.tmp", &["build", "x.tmp"]));
    }

    #[test]
    fn globs_match_within_a_segment() {
        assert!(matches("*.cache", &["notes.cache"]));
        assert!(!matches("*.cache", &["cache"]));
        assert!(matches("tmp?", &["tmp1"]));
        assert!(!matches("tmp?", &["tmp12"]));
    }

    #[test]
    fn comments_blanks_and_unparsable_lines_are_dropped() {
        assert!(parse_pattern("# comment").is_none());
        assert!(parse_pattern("   ").is_none());
        assert!(parse_pattern("!").is_none());
        let negated = pattern("!tmp_fixtures");
        assert!(negated.negated);
        assert_eq!(negated.segments, ["tmp_fixtures"]);
    }

    #[test]
    fn segment_globs_and_literal_segments_match() {
        let segments: Vec<String> = ["a", "b"].iter().map(|s| (*s).to_string()).collect();
        let deep: Vec<String> = ["a", "b", "c", "d"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let bridged: Vec<String> = ["a", "d"].iter().map(|s| (*s).to_string()).collect();
        assert!(match_segment("*.cache", "notes.cache"));
        assert!(match_segment("a", "a"));
        assert!(match_segments(&["a", "b"], &segments));
        assert!(!match_segments(&["a"], &segments));
        assert!(
            !match_segments(&["a", "b"], &deep),
            "a pattern matches a whole path, not a prefix"
        );
        assert!(match_segments(&["**"], &segments));
        assert!(match_segments(&["a", "**", "d"], &bridged));
        assert!(!match_segments(&["a", "b", "e"], &deep));
    }

    #[test]
    fn empty_relative_paths_never_match() {
        let any_depth = pattern("dist");
        assert!(!any_depth.matches(&[]));
        let anchored = pattern("/dist");
        assert!(!anchored.matches(&[]));
    }
}
