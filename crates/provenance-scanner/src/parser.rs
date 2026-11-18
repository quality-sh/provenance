use std::{fmt, str::FromStr};

use crate::string_context::marker_is_inside_quoted_region;

const ANNOTATION_MARKER: &str = "@provenance";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageLevel {
    #[default]
    Full,
    Partial,
    Indirect,
}

impl fmt::Display for CoverageLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "full"),
            Self::Partial => write!(f, "partial"),
            Self::Indirect => write!(f, "indirect"),
        }
    }
}

impl FromStr for CoverageLevel {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "full" => Ok(Self::Full),
            "partial" => Ok(Self::Partial),
            "indirect" => Ok(Self::Indirect),
            other => Err(format!("invalid coverage level: {other}")),
        }
    }
}

/// The method words the `verifies` macro accepts. The scanner keeps core's
/// name so its parsed bindings read as before; the word list has one
/// definition, and `tests/method_word_conformance.rs` pins the macro list to
/// it.
pub use provenance_core::VerificationMethod as Verification;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Annotation {
    pub rule: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub coverage: CoverageLevel,
    pub confidence: f64,
    pub intent: Option<String>,
    pub verification: Option<Verification>,
}

impl Default for Annotation {
    fn default() -> Self {
        Self {
            rule: String::new(),
            name: None,
            description: None,
            tags: Vec::new(),
            coverage: CoverageLevel::Full,
            confidence: 1.0,
            intent: None,
            verification: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ParseWarning {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub annotations: Vec<Annotation>,
    pub warnings: Vec<ParseWarning>,
}

pub fn parse_annotations(comment_text: &str) -> ParseResult {
    let mut block = Block::default();
    let mut found_marker = false;

    for (line_idx, raw_line) in comment_text.lines().enumerate() {
        let line = line_idx + 1;
        let stripped = strip_comment_prefix(raw_line);
        let Some(after_marker) = split_annotation_marker(stripped) else {
            continue;
        };
        found_marker = true;
        match split_directive(after_marker) {
            Ok((key, value)) => block.apply(&key, value, line),
            Err(message) => block.warn(line, message),
        }
    }

    if found_marker && block.annotations.is_empty() {
        block.warn(
            0,
            format!("found {ANNOTATION_MARKER} directives but no rule annotations"),
        );
    }

    ParseResult {
        annotations: block.annotations,
        warnings: block.warnings,
    }
}

/// Splits the `key: value` text after a marker. The key is trimmed and
/// lowercase, and the value is trimmed. Only `tags` can have an empty value.
fn split_directive(after_marker: &str) -> Result<(String, &str), String> {
    let Some((key, value)) = after_marker.split_once(':') else {
        return Err(format!(
            "malformed directive: expected `key: value` after {ANNOTATION_MARKER}"
        ));
    };
    let key = key.trim().to_ascii_lowercase();
    let value = value.trim();
    if value.is_empty() && key != "tags" {
        return Err(format!("empty value for field `{key}`"));
    }
    Ok((key, value))
}

/// The annotations of one comment block, the fields that the block gives
/// before its first rule, and the warnings that its lines earned.
#[derive(Default)]
struct Block {
    annotations: Vec<Annotation>,
    shared: Annotation,
    warnings: Vec<ParseWarning>,
}

impl Block {
    fn warn(&mut self, line: usize, message: String) {
        self.warnings.push(ParseWarning { line, message });
    }

    /// Sets a field on the latest rule. Before the first rule, the field is
    /// shared by each rule that follows.
    fn set(&mut self, apply: impl FnOnce(&mut Annotation)) {
        apply(self.annotations.last_mut().unwrap_or(&mut self.shared));
    }

    fn apply(&mut self, key: &str, value: &str, line: usize) {
        match key {
            "rule" => self.annotations.push(Annotation {
                rule: value.to_string(),
                ..self.shared.clone()
            }),
            "name" => self.set(|ann| ann.name = Some(value.to_string())),
            "description" => self.set(|ann| ann.description = Some(value.to_string())),
            "tags" => {
                let tags = value
                    .split(',')
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                self.set(|ann| ann.tags = tags);
            }
            "coverage" => self.apply_coverage(value, line),
            "confidence" => self.apply_confidence(value, line),
            "intent" => self.set(|ann| ann.intent = Some(value.to_string())),
            "verification" => self.apply_verification(value, line),
            other => self.warn(line, format!("unknown field `{other}`")),
        }
    }

    fn apply_coverage(&mut self, value: &str, line: usize) {
        match CoverageLevel::from_str(value) {
            Ok(level) => self.set(|ann| ann.coverage = level),
            Err(_) => self.warn(
                line,
                format!("invalid coverage level `{value}`, using default"),
            ),
        }
    }

    fn apply_confidence(&mut self, value: &str, line: usize) {
        match parse_confidence(value) {
            Ok(confidence) => self.set(|ann| ann.confidence = confidence),
            Err(rejection) => self.warn(line, rejection.warning(value)),
        }
    }

    fn apply_verification(&mut self, value: &str, line: usize) {
        match Verification::from_str(value) {
            Ok(method) => self.set(|ann| ann.verification = Some(method)),
            Err(_) => self.warn(
                line,
                format!("invalid verification method `{value}`, ignoring"),
            ),
        }
    }
}

/// Why an annotated confidence was not used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfidenceRejection {
    /// The text is not a number, or is a NaN or an infinity.
    Unreadable,
    /// A real number, but not one between 0.0 and 1.0.
    OutOfRange,
}

impl ConfidenceRejection {
    fn warning(self, value: &str) -> String {
        match self {
            Self::Unreadable => format!("invalid confidence `{value}`, using default"),
            Self::OutOfRange => format!("confidence `{value}` is outside 0.0-1.0, using default"),
        }
    }
}

/// The scanner's copy of `rule_confidence_range`, whose primary implementation is
/// `validate_confidence_score` in `provenance-core/src/model/validation.rs`.
/// The scanner needs the range before any record exists, so it holds the range
/// itself; it must agree with core on the verdict and may differ only in what
/// it does about it. The conformance test in
/// `provenance-scanner/tests/confidence_conformance.rs` holds the two in step.
fn confidence_in_range(confidence: f64) -> bool {
    confidence.is_finite() && (0.0..=1.0).contains(&confidence)
}

/// Reads an annotated confidence, refusing anything outside 0.0 to 1.0.
///
/// An out-of-range score is refused rather than pulled to the nearest end: the
/// caller warns and keeps the default, so a reader of the report sees that the
/// annotation said something the graph would not accept, instead of a 1.0 that
/// looks like a choice somebody made.
fn parse_confidence(value: &str) -> Result<f64, ConfidenceRejection> {
    let confidence = value
        .parse::<f64>()
        .ok()
        .filter(|confidence| confidence.is_finite())
        .ok_or(ConfidenceRejection::Unreadable)?;
    if confidence_in_range(confidence) {
        Ok(confidence)
    } else {
        Err(ConfidenceRejection::OutOfRange)
    }
}

/// The position of the first marker in the line that is not in a quoted region.
pub(crate) fn annotation_marker_position(line: &str) -> Option<usize> {
    annotation_marker_positions(line)
        .find(|position| !marker_is_inside_quoted_region(line, *position))
}

pub(crate) fn annotation_marker_positions(line: &str) -> impl Iterator<Item = usize> + '_ {
    line.match_indices(ANNOTATION_MARKER)
        .map(|(position, _)| position)
}

fn split_annotation_marker(line: &str) -> Option<&str> {
    let position = annotation_marker_position(line)?;
    Some(line[position + ANNOTATION_MARKER.len()..].trim())
}

fn strip_comment_prefix(line: &str) -> &str {
    line.trim_start()
        .trim_start_matches('/')
        .trim_start_matches('*')
        .trim_start_matches('#')
        .trim_start_matches('-')
        .trim()
}

#[cfg(test)]
mod tests;
