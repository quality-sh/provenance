//! The cause text of a refusal of no known kind, with no absolute host path.
use provenance_macros::rule;
use std::path::Path;

/// A native error of an operation on one repository, with the spellings of
/// the repository root that a path in the error can start with.
#[derive(Debug)]
pub(crate) struct InRepository {
    roots: Vec<String>,
    source: anyhow::Error,
}
impl std::fmt::Display for InRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.source, f)
    }
}
impl std::error::Error for InRepository {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.source()
    }
}

/// Puts the repository root on `error`, so that its cause can give each path
/// relative to the repository.
pub(crate) fn in_repository(root: &Path, error: anyhow::Error) -> anyhow::Error {
    if error.downcast_ref::<InRepository>().is_some() {
        return error;
    }
    InRepository {
        roots: root_spellings(root),
        source: error,
    }
    .into()
}

/// The native error without the repository root.
pub(crate) fn native(error: &anyhow::Error) -> &anyhow::Error {
    error
        .downcast_ref::<InRepository>()
        .map_or(error, |located| &located.source)
}

/// The display chain of `error` for a refusal of no known kind. A path below
/// the repository root is relative to that root, with `/` between its parts.
/// Any other absolute path is only its file name.
#[rule("rule_unclassified_refusal_gives_cause")]
pub(crate) fn failure_cause(error: &anyhow::Error) -> String {
    let roots = error
        .downcast_ref::<InRepository>()
        .map_or(&[][..], |located| &located.roots);
    let text = format!("{:#}", native(error));
    file_names_only(&relative_to(&text, roots))
}

/// Each spelling of `root` that an error can hold: as given, canonical, with
/// and without the `\\?\` prefix, and with either separator. The longest
/// spelling is first.
fn root_spellings(root: &Path) -> Vec<String> {
    let mut given = vec![root.to_string_lossy().into_owned()];
    if let Ok(canonical) = std::fs::canonicalize(root) {
        given.push(canonical.to_string_lossy().into_owned());
    }
    let mut spellings = Vec::new();
    for path in &given {
        let path = path.trim_end_matches(['/', '\\']);
        let plain = path.strip_prefix(r"\\?\").unwrap_or(path);
        if plain.is_empty() {
            continue;
        }
        spellings.extend([
            plain.to_owned(),
            plain.replace('\\', "/"),
            plain.replace('/', "\\"),
        ]);
        if has_drive(plain) {
            spellings.push(format!(r"\\?\{}", plain.replace('/', "\\")));
        }
    }
    spellings.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    spellings.dedup();
    spellings
}

/// `text` with each path below one of `roots` made relative to that root.
fn relative_to(text: &str, roots: &[String]) -> String {
    let mut cause = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((at, root)) = first_root(rest, roots) {
        cause.push_str(&rest[..at]);
        rest = &rest[at + root.len()..];
        if rest.starts_with(['/', '\\']) {
            rest = &rest[1..];
            let end = rest.find(ends_path).unwrap_or(rest.len());
            cause.push_str(&rest[..end].replace('\\', "/"));
            rest = &rest[end..];
        } else {
            cause.push('.');
        }
    }
    cause.push_str(rest);
    cause
}

/// The first place in `text` where a whole root starts, and that root. At
/// one place the longest root wins.
fn first_root<'a>(text: &str, roots: &'a [String]) -> Option<(usize, &'a str)> {
    roots
        .iter()
        .filter_map(|root| {
            text.match_indices(root.as_str())
                .find(|&(at, _)| {
                    let before = text[..at].chars().next_back();
                    let after = text[at + root.len()..].chars().next();
                    !before.is_some_and(in_path)
                        && after.is_none_or(|next| next == '/' || next == '\\' || !in_path(next))
                })
                .map(|(at, _)| (at, root.as_str()))
        })
        .min_by_key(|&(at, root)| (at, std::cmp::Reverse(root.len())))
}

/// `text` with each absolute path replaced by its file name.
fn file_names_only(text: &str) -> String {
    let mut cause = String::with_capacity(text.len());
    let mut previous = None;
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        if previous.is_none_or(opens_path) && is_absolute(rest) {
            let end = path_end(rest);
            let path = &rest[..end];
            let name = path.rsplit(['/', '\\']).find(|part| !part.is_empty());
            cause.push_str(name.unwrap_or(path));
            previous = path.chars().next_back();
            index += end;
            continue;
        }
        let character = rest.chars().next().unwrap_or_default();
        cause.push(character);
        previous = Some(character);
        index += character.len_utf8();
    }
    cause
}

/// Whether `text` starts with an absolute path: a `/` or `\` root, or a drive.
fn is_absolute(text: &str) -> bool {
    let mut characters = text.chars();
    match characters.next() {
        Some('/' | '\\') => characters.next().is_some_and(|next| !ends_path(next)),
        Some(_) => has_drive(text),
        None => false,
    }
}

/// The end of the path at the start of `text`: before a character that ends
/// a path, or before a `:` that ends the text or comes before a space.
fn path_end(text: &str) -> usize {
    text.char_indices()
        .find(|&(at, character)| {
            ends_path(character)
                || (character == ':'
                    && text[at + 1..]
                        .chars()
                        .next()
                        .is_none_or(char::is_whitespace))
        })
        .map_or(text.len(), |(at, _)| at)
}

fn has_drive(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

/// A character that can come just before the start of a path.
fn opens_path(character: char) -> bool {
    character.is_whitespace() || matches!(character, '"' | '\'' | '`' | '(' | '[' | '<' | '=')
}

/// A character that can be part of a path name.
fn in_path(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '.' | '_' | '-' | '~' | '/' | '\\')
}

const fn ends_path(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | ';'
        )
}

#[cfg(test)]
#[path = "cause_tests.rs"]
mod tests;
