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

/// A field after a rule belongs to that rule only. A field before the
/// first rule is shared by each rule that follows.
#[test]
fn a_field_after_a_rule_sets_that_rule_only() {
    let parsed = parse_annotations(
        "@provenance description: Shared text\n\
         @provenance rule: RULE-001\n\
         @provenance tags: pay, , award\n\
         @provenance intent: Keep pay right\n\
         @provenance verification: Property\n\
         @provenance coverage: partial\n\
         @provenance rule: RULE-002",
    );

    assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
    let [first, second] = parsed.annotations.as_slice() else {
        panic!("two annotations: {:?}", parsed.annotations);
    };
    assert_eq!(first.description.as_deref(), Some("Shared text"));
    assert_eq!(first.tags, ["pay", "award"]);
    assert_eq!(first.intent.as_deref(), Some("Keep pay right"));
    assert_eq!(first.verification, Some(Verification::Property));
    assert_eq!(first.coverage, CoverageLevel::Partial);
    assert_eq!(second.description.as_deref(), Some("Shared text"));
    assert!(second.tags.is_empty());
    assert_eq!(second.intent, None);
    assert_eq!(second.verification, None);
    assert_eq!(second.coverage, CoverageLevel::Full);
}

#[test]
fn each_unusable_directive_warns_on_its_line_and_changes_nothing() {
    let parsed = parse_annotations(
        "@provenance rule: RULE-001\n\
         @provenance no separator\n\
         @provenance name:\n\
         @provenance tags:\n\
         @provenance coverage: most\n\
         @provenance verification: guess\n\
         @provenance owner: payroll",
    );

    assert_eq!(parsed.annotations.len(), 1);
    assert_eq!(parsed.annotations[0].name, None);
    assert!(parsed.annotations[0].tags.is_empty());
    assert_eq!(parsed.annotations[0].coverage, CoverageLevel::Full);
    assert_eq!(parsed.annotations[0].verification, None);
    let warnings: Vec<(usize, &str)> = parsed
        .warnings
        .iter()
        .map(|warning| (warning.line, warning.message.as_str()))
        .collect();
    assert_eq!(
        warnings,
        [
            (
                2,
                "malformed directive: expected `key: value` after @provenance"
            ),
            (3, "empty value for field `name`"),
            (5, "invalid coverage level `most`, using default"),
            (6, "invalid verification method `guess`, ignoring"),
            (7, "unknown field `owner`"),
        ]
    );
}

#[test]
fn directives_without_a_rule_warn_that_no_rule_was_found() {
    let parsed = parse_annotations("@provenance name: Payroll thresholds");

    assert!(parsed.annotations.is_empty());
    assert_eq!(
        parsed.warnings,
        vec![ParseWarning {
            line: 0,
            message: "found @provenance directives but no rule annotations".to_string(),
        }]
    );
}
