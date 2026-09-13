use std::{fmt, str::FromStr};

use crate::string_context::marker_is_inside_quoted_region;

pub(crate) const PRIMARY_ANNOTATION_MARKER: &str = "@provenance";
const LEGACY_ANNOTATION_MARKER: &str = "@statesman";
const ANNOTATION_MARKERS: [&str; 2] = [PRIMARY_ANNOTATION_MARKER, LEGACY_ANNOTATION_MARKER];

/// Said once per comment block, at the first legacy marker: repeating it on
/// every line of a ten-line block would bury the annotations it sits beside.
const LEGACY_MARKER_WARNING: &str = "@statesman is the legacy marker; use @provenance";

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

/// Which markers a comment block used.
#[derive(Default)]
struct MarkerLog {
    first: Option<&'static str>,
    warned_legacy: bool,
}

impl MarkerLog {
    /// Records a marker and hands back the deprecation warning owed for it.
    ///
    /// Only the first legacy marker in a block earns one. The block still
    /// parses either way; the warning is the only thing deprecation costs.
    fn record(&mut self, marker: &'static str, line: usize) -> Option<ParseWarning> {
        self.first.get_or_insert(marker);
        if marker != LEGACY_ANNOTATION_MARKER || self.warned_legacy {
            return None;
        }
        self.warned_legacy = true;
        Some(ParseWarning {
            line,
            message: LEGACY_MARKER_WARNING.to_string(),
        })
    }
}

pub fn parse_annotations(comment_text: &str) -> ParseResult {
    let mut warnings = Vec::new();
    let mut annotations: Vec<Annotation> = Vec::new();
    let mut shared = Annotation::default();
    let mut markers = MarkerLog::default();

    for (line_idx, raw_line) in comment_text.lines().enumerate() {
        let line = line_idx + 1;
        let stripped = strip_comment_prefix(raw_line);
        let Some((marker, after_marker)) = split_annotation_marker(stripped) else {
            continue;
        };
        warnings.extend(markers.record(marker, line));
        let Some((key, value)) = after_marker.split_once(':') else {
            warnings.push(ParseWarning {
                line,
                message: format!("malformed directive: expected `key: value` after {marker}"),
            });
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if value.is_empty() && key != "tags" {
            warnings.push(ParseWarning {
                line,
                message: format!("empty value for field `{}`", key.trim()),
            });
            continue;
        }
        match key.as_str() {
            "rule" => annotations.push(Annotation {
                rule: value.to_string(),
                name: shared.name.clone(),
                description: shared.description.clone(),
                tags: shared.tags.clone(),
                coverage: shared.coverage,
                confidence: shared.confidence,
                intent: shared.intent.clone(),
                verification: shared.verification,
            }),
            "name" => set_field(&mut annotations, &mut shared, |ann| {
                ann.name = Some(value.to_string());
            }),
            "description" => set_field(&mut annotations, &mut shared, |ann| {
                ann.description = Some(value.to_string());
            }),
            "tags" => {
                let tags = value
                    .split(',')
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                set_field(&mut annotations, &mut shared, |ann| {
                    ann.tags.clone_from(&tags);
                });
            }
            "coverage" => match CoverageLevel::from_str(value) {
                Ok(level) => set_field(&mut annotations, &mut shared, |ann| ann.coverage = level),
                Err(_) => warnings.push(ParseWarning {
                    line,
                    message: format!("invalid coverage level `{value}`, using default"),
                }),
            },
            "confidence" => match parse_confidence(value) {
                Ok(confidence) => set_field(&mut annotations, &mut shared, |ann| {
                    ann.confidence = confidence;
                }),
                Err(rejection) => warnings.push(ParseWarning {
                    line,
                    message: rejection.warning(value),
                }),
            },
            "intent" => set_field(&mut annotations, &mut shared, |ann| {
                ann.intent = Some(value.to_string());
            }),
            "verification" => match Verification::from_str(value) {
                Ok(method) => set_field(&mut annotations, &mut shared, |ann| {
                    ann.verification = Some(method);
                }),
                Err(_) => warnings.push(ParseWarning {
                    line,
                    message: format!("invalid verification method `{value}`, ignoring"),
                }),
            },
            other => warnings.push(ParseWarning {
                line,
                message: format!("unknown field `{other}`"),
            }),
        }
    }

    if let Some(marker) = markers.first.filter(|_| annotations.is_empty()) {
        warnings.push(ParseWarning {
            line: 0,
            message: format!("found {marker} directives but no rule annotations"),
        });
    }

    ParseResult {
        annotations,
        warnings,
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

pub(crate) fn annotation_marker_position(line: &str) -> Option<usize> {
    annotation_marker(line).map(|(_, position)| position)
}

pub(crate) fn annotation_marker_positions(line: &str) -> impl Iterator<Item = usize> + '_ {
    ANNOTATION_MARKERS
        .iter()
        .flat_map(|marker| line.match_indices(marker).map(|(position, _)| position))
}

fn split_annotation_marker(line: &str) -> Option<(&'static str, &str)> {
    let (marker, position) = annotation_marker(line)?;
    Some((marker, line[position + marker.len()..].trim()))
}

fn annotation_marker(line: &str) -> Option<(&'static str, usize)> {
    ANNOTATION_MARKERS
        .iter()
        .flat_map(|marker| {
            line.match_indices(marker)
                .filter(|(position, _)| !marker_is_inside_quoted_region(line, *position))
                .map(|(position, _)| (*marker, position))
        })
        .min_by_key(|(_, position)| *position)
}

fn set_field<F>(annotations: &mut [Annotation], shared: &mut Annotation, apply: F)
where
    F: Fn(&mut Annotation),
{
    if let Some(annotation) = annotations.last_mut() {
        apply(annotation);
    } else {
        apply(shared);
    }
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
mod tests {
    use super::*;

    #[test]
    fn parses_shared_fields_across_multiple_provenance_rules() {
        let parsed = parse_annotations(
            r"
            @provenance name: Payroll thresholds
            @provenance coverage: full
            @provenance rule: SCHADS-PAY-001
            @provenance rule: SCHADS-PAY-002
            ",
        );

        assert_eq!(parsed.annotations.len(), 2);
        assert_eq!(
            parsed.annotations[0].name.as_deref(),
            Some("Payroll thresholds")
        );
        assert_eq!(parsed.annotations[1].coverage, CoverageLevel::Full);
    }

    #[test]
    fn parses_statesman_marker_as_legacy_alias() {
        let parsed = parse_annotations("@statesman rule: SCHADS-PAY-001");

        assert_eq!(parsed.annotations.len(), 1);
        assert_eq!(parsed.annotations[0].rule, "SCHADS-PAY-001");
    }

    /// The legacy marker still parses; it just says so on the way through.
    #[test]
    fn statesman_marker_warns_but_keeps_the_annotation() {
        let parsed = parse_annotations("@statesman rule: SCHADS-PAY-001");

        assert_eq!(parsed.annotations.len(), 1);
        assert_eq!(
            parsed.warnings,
            vec![ParseWarning {
                line: 1,
                message: "@statesman is the legacy marker; use @provenance".to_string(),
            }]
        );
    }

    #[test]
    fn statesman_marker_warns_once_per_comment_block() {
        let parsed = parse_annotations(
            r"
            @statesman name: Payroll thresholds
            @statesman rule: SCHADS-PAY-001
            @statesman rule: SCHADS-PAY-002
            ",
        );

        assert_eq!(parsed.annotations.len(), 2);
        assert_eq!(parsed.warnings.len(), 1);
        assert_eq!(parsed.warnings[0].line, 2);
    }

    #[test]
    fn provenance_marker_draws_no_legacy_warning() {
        let parsed = parse_annotations("@provenance rule: SCHADS-PAY-001");

        assert!(parsed.warnings.is_empty());
    }

    /// A block that mixes markers is warned about once, at the first legacy
    /// line, whichever marker opened the block.
    #[test]
    fn mixed_markers_warn_once_at_the_legacy_line() {
        let parsed = parse_annotations(
            r"
            @provenance rule: SCHADS-PAY-001
            @statesman rule: SCHADS-PAY-002
            ",
        );

        assert_eq!(parsed.annotations.len(), 2);
        assert_eq!(parsed.warnings.len(), 1);
        assert_eq!(parsed.warnings[0].line, 3);
    }

    #[test]
    fn rejects_non_finite_confidence_with_warning() {
        for value in ["NaN", "inf", "-inf"] {
            let parsed = parse_annotations(&format!(
                "@provenance confidence: {value}\n@provenance rule: RULE-001"
            ));

            assert!((parsed.annotations[0].confidence - 1.0).abs() < f64::EPSILON);
            assert_eq!(
                parsed.warnings,
                vec![ParseWarning {
                    line: 1,
                    message: format!("invalid confidence `{value}`, using default"),
                }]
            );
        }
    }

    /// Out of range is refused, not pulled to the nearest end: the graph
    /// rejects such a score outright (`rule_confidence_range`), and a scan
    /// that quietly clamped would report a confidence nobody wrote.
    #[test]
    fn refuses_out_of_range_confidence_and_keeps_the_default() {
        for value in ["-0.25", "1.25", "-1", "2", "100"] {
            let parsed = parse_annotations(&format!(
                "@provenance confidence: {value}\n@provenance rule: RULE-001"
            ));

            assert!((parsed.annotations[0].confidence - 1.0).abs() < f64::EPSILON);
            assert_eq!(
                parsed.warnings,
                vec![ParseWarning {
                    line: 1,
                    message: format!("confidence `{value}` is outside 0.0-1.0, using default"),
                }]
            );
        }
    }

    #[test]
    fn preserves_in_range_confidence() {
        let parsed = parse_annotations("@provenance confidence: 0.75\n@provenance rule: RULE-001");

        assert!((parsed.annotations[0].confidence - 0.75).abs() < f64::EPSILON);
        assert!(parsed.warnings.is_empty());
    }
}
