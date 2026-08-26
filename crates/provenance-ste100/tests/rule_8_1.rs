use provenance_macros::verifies;
use provenance_ste100::{
    check_descriptive, Finding, FindingKind, Report, RuleNumber, Span, Standard, StandardIssue,
    ANALYZER_VERSION,
};
use serde_json::json;

fn violation(start: usize) -> Finding {
    Finding {
        rule: RuleNumber::EightOne,
        kind: FindingKind::Violation,
        span: Span {
            start,
            end: start + 1,
        },
        message: "Do not use semicolons in descriptive text.".to_owned(),
    }
}

fn report(findings: Vec<Finding>) -> Report {
    Report {
        standard: Standard::AsdSte100,
        issue: StandardIssue::Nine,
        analyzer_version: ANALYZER_VERSION.to_owned(),
        findings,
    }
}

#[test]
fn clean_text_has_no_findings() {
    assert_eq!(check_descriptive("Install the cover."), report(vec![]));
}

#[test]
fn one_semicolon_has_one_violation() {
    assert_eq!(check_descriptive("Stop; wait."), report(vec![violation(4)]));
}

#[test]
fn multiple_semicolons_have_one_ordered_violation_each() {
    assert_eq!(
        check_descriptive("A; B;; C;"),
        report(vec![violation(1), violation(4), violation(5), violation(8)])
    );
}

#[test]
fn span_offsets_are_utf8_bytes() {
    assert_eq!(check_descriptive("é;"), report(vec![violation(2)]));
}

#[test]
fn repeated_analysis_is_identical() {
    let text = "é; A;;";
    let first = check_descriptive(text);

    for _ in 0..10 {
        assert_eq!(check_descriptive(text), first);
    }
}

#[test]
fn report_serialization_is_stable_and_explicit() {
    assert_eq!(
        serde_json::to_value(check_descriptive("Stop;")).unwrap(),
        json!({
            "standard": "ASD-STE100",
            "issue": 9,
            "analyzer_version": "0.2.1",
            "findings": [{
                "rule": "8.1",
                "kind": "violation",
                "span": { "start": 4, "end": 5 },
                "message": "Do not use semicolons in descriptive text."
            }]
        })
    );
}

#[test]
#[verifies("rule_ste100_semicolon", exhaustion)]
fn every_short_string_reports_exactly_its_semicolons() {
    const ALPHABET: [char; 3] = ['a', ';', 'é'];

    for length in 0..=5 {
        for ordinal in 0..ALPHABET.len().pow(length) {
            let text = string_at(ordinal, length, &ALPHABET);
            let expected = text
                .char_indices()
                .filter(|&(_, character)| character == ';')
                .map(|(offset, _)| violation(offset))
                .collect();

            assert_eq!(check_descriptive(&text), report(expected), "input {text:?}");
        }
    }
}

fn string_at(mut ordinal: usize, length: u32, alphabet: &[char]) -> String {
    let mut text = String::new();
    for _ in 0..length {
        text.push(alphabet[ordinal % alphabet.len()]);
        ordinal /= alphabet.len();
    }
    text
}
