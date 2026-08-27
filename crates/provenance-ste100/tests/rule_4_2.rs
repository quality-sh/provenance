use provenance_macros::verifies;
use provenance_ste100::check_descriptive;
use serde_json::{json, Value};

fn report(text: &str) -> Value {
    serde_json::to_value(check_descriptive(text)).unwrap()
}

fn finding(rule: &str, start: usize, end: usize, message: &str) -> Value {
    json!({
        "rule": rule,
        "kind": "violation",
        "span": { "start": start, "end": end },
        "message": message,
    })
}

fn findings(text: &str) -> Value {
    report(text)["findings"].clone()
}

const RECOGNIZED_FORMS: &[&str] = &[
    "I'm",
    "you're",
    "we're",
    "they're",
    "who're",
    "what're",
    "there're",
    "I've",
    "you've",
    "we've",
    "they've",
    "who've",
    "what've",
    "could've",
    "should've",
    "would've",
    "might've",
    "must've",
    "I'll",
    "you'll",
    "he'll",
    "she'll",
    "it'll",
    "we'll",
    "they'll",
    "that'll",
    "who'll",
    "what'll",
    "there'll",
    "I'd",
    "you'd",
    "he'd",
    "she'd",
    "it'd",
    "we'd",
    "they'd",
    "that'd",
    "who'd",
    "what'd",
    "where'd",
    "when'd",
    "why'd",
    "how'd",
    "there'd",
    "he's",
    "she's",
    "it's",
    "that's",
    "what's",
    "who's",
    "where's",
    "when's",
    "why's",
    "how's",
    "there's",
    "here's",
    "ain't",
    "amn't",
    "aren't",
    "can't",
    "couldn't",
    "daren't",
    "didn't",
    "doesn't",
    "don't",
    "hadn't",
    "hasn't",
    "haven't",
    "isn't",
    "mayn't",
    "mightn't",
    "mustn't",
    "needn't",
    "oughtn't",
    "shan't",
    "shouldn't",
    "wasn't",
    "weren't",
    "won't",
    "wouldn't",
];

#[test]
#[verifies("rule_ste100_contracted_verb", examples)]
fn contracted_verbs_have_exact_ordered_utf8_spans() {
    assert_eq!(
        findings("é CAN’T; we aren't and won’t."),
        json!([
            finding("4.2", 3, 10, "Use the full verb form in descriptive text."),
            finding("8.1", 10, 11, "Do not use semicolons in descriptive text."),
            finding("4.2", 15, 21, "Use the full verb form in descriptive text."),
            finding("4.2", 26, 33, "Use the full verb form in descriptive text."),
        ])
    );
}

#[test]
#[verifies("rule_ste100_contracted_verb", examples)]
fn recognized_form_regression_set_is_case_and_apostrophe_insensitive() {
    for form in RECOGNIZED_FORMS {
        for spelling in [
            (*form).to_owned(),
            form.to_uppercase(),
            form.replace('\'', "’"),
        ] {
            assert_eq!(
                findings(&spelling),
                json!([finding(
                    "4.2",
                    0,
                    spelling.len(),
                    "Use the full verb form in descriptive text."
                )]),
                "spelling {spelling:?}"
            );
        }
    }
}

#[test]
#[verifies("rule_ste100_contracted_verb", examples)]
fn unambiguous_positive_form_classes_produce_findings() {
    assert_eq!(
        findings("I'm ready. You're ready. We've started. They'll go. I'd go. It's ready."),
        json!([
            finding("4.2", 0, 3, "Use the full verb form in descriptive text."),
            finding("4.2", 11, 17, "Use the full verb form in descriptive text."),
            finding("4.2", 25, 30, "Use the full verb form in descriptive text."),
            finding("4.2", 40, 47, "Use the full verb form in descriptive text."),
            finding("4.2", 52, 55, "Use the full verb form in descriptive text."),
            finding("4.2", 60, 64, "Use the full verb form in descriptive text."),
        ])
    );
}

#[test]
#[verifies("rule_ste100_contracted_verb", examples)]
fn noncontracted_apostrophe_uses_do_not_produce_rule_4_2_findings() {
    let text = "The actuator's label is 'OPEN'. Its value is 5′. Keep ‘OPEN’.";

    assert_eq!(findings(text), json!([]));
}

#[test]
#[verifies("rule_ste100_contracted_verb", examples)]
fn ambiguous_and_non_token_forms_do_not_produce_findings() {
    let text = "The pump's label is John's. xaren't aren'tx aren't_ready O’Neill.";

    assert_eq!(findings(text), json!([]));
}

#[test]
fn adjacent_punctuation_preserves_complete_token_matching() {
    assert_eq!(
        findings("(Didn't), [DOESN’T]."),
        json!([
            finding("4.2", 1, 7, "Use the full verb form in descriptive text."),
            finding("4.2", 11, 20, "Use the full verb form in descriptive text."),
        ])
    );
}

#[test]
fn rule_4_2_report_serialization_is_stable_and_explicit() {
    let expected = json!({
        "standard": "ASD-STE100",
        "issue": 9,
        "analyzer_version": "0.2.2",
        "findings": [finding(
            "4.2",
            0,
            5,
            "Use the full verb form in descriptive text."
        )],
    });

    assert_eq!(report("isn't"), expected);
    assert_eq!(report("isn't"), expected);
}
