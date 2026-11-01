use provenance_macros::verifies;
use provenance_ste100::{
    check_descriptive_with_dictionary, classify_vocabulary, DictionaryEntry, DictionaryImport,
    DictionaryImportIdentity, DictionaryStatus, FindingKind, PartOfSpeech, RuleNumber, Span,
    StandardIssue, VocabularyCategory,
};

fn entry(headword: &str, forms: &[&str], status: DictionaryStatus) -> DictionaryEntry {
    DictionaryEntry {
        headword: headword.to_owned(),
        word_forms: forms.iter().map(|form| (*form).to_owned()).collect(),
        part_of_speech: PartOfSpeech::Noun,
        status,
        approved_meaning_or_alternatives: "A meaning".to_owned(),
        ste_example: "USE THE ITEM.".to_owned(),
        non_ste_example: None,
    }
}

fn fixture_dictionary() -> DictionaryImport {
    DictionaryImport {
        identity: DictionaryImportIdentity {
            issue: StandardIssue::Nine,
            source_sha256: "0".repeat(64),
            data_sha256: "0".repeat(64),
            extractor_version: "test".to_owned(),
        },
        entries: vec![
            entry("AAGOOD", &["AAGOOD", "AAGOODS"], DictionaryStatus::Approved),
            entry("bbbad", &["bbbad"], DictionaryStatus::Unapproved),
            entry(
                "bb far from",
                &["bb far from"],
                DictionaryStatus::Unapproved,
            ),
            entry("CCBOTH", &["CCBOTH"], DictionaryStatus::Approved),
            entry("ccboth", &["ccboth"], DictionaryStatus::Unapproved),
        ],
    }
}

#[test]
#[verifies("rule_ste_dictionary_unapproved_word", examples)]
fn a_word_with_only_unapproved_forms_produces_a_rule_1_1_finding() {
    let dictionary = fixture_dictionary();
    let text = "The bbbad output stops.";

    let report = check_descriptive_with_dictionary(text, &dictionary, &[]);

    assert_eq!(report.findings.len(), 1, "findings: {:?}", report.findings);
    let finding = &report.findings[0];
    assert_eq!(finding.rule, RuleNumber::OneOne);
    assert_eq!(finding.kind, FindingKind::Violation);
    assert_eq!(finding.span, Span { start: 4, end: 9 });
    assert_eq!(&text[finding.span.start..finding.span.end], "bbbad");
}

#[test]
#[verifies("rule_ste_dictionary_unapproved_word", examples)]
fn matching_folds_case() {
    let dictionary = fixture_dictionary();

    let report = check_descriptive_with_dictionary("Bbbad output stops.", &dictionary, &[]);

    assert_eq!(report.findings.len(), 1, "findings: {:?}", report.findings);
    assert_eq!(report.findings[0].rule, RuleNumber::OneOne);
}

#[test]
#[verifies("rule_ste_dictionary_unapproved_word", examples)]
fn an_unapproved_phrase_produces_one_finding_over_its_full_span() {
    let dictionary = fixture_dictionary();
    let text = "Stop bb far from the door.";

    let report = check_descriptive_with_dictionary(text, &dictionary, &[]);

    assert_eq!(report.findings.len(), 1, "findings: {:?}", report.findings);
    assert_eq!(
        &text[report.findings[0].span.start..report.findings[0].span.end],
        "bb far from"
    );
}

#[test]
#[verifies("rule_ste_dictionary_unapproved_word", examples)]
fn quoted_text_produces_no_vocabulary_finding() {
    let dictionary = fixture_dictionary();

    let report =
        check_descriptive_with_dictionary("The \"bbbad\" light comes on.", &dictionary, &[]);

    assert!(
        report.findings.is_empty(),
        "findings: {:?}",
        report.findings
    );
}

#[test]
#[verifies("rule_ste_dictionary_unapproved_word", examples)]
fn identifier_tokens_produce_no_result() {
    let dictionary = fixture_dictionary();

    let report = check_descriptive_with_dictionary(
        "The req_bbbad record and bbbad2 stay.",
        &dictionary,
        &[],
    );

    assert!(
        report.findings.is_empty(),
        "findings: {:?}",
        report.findings
    );
    assert!(
        classify_vocabulary("The req_bbbad record stays.", &dictionary)
            .iter()
            .all(|word_use| word_use.category != VocabularyCategory::Unapproved),
        "identifier parts must not classify as unapproved"
    );
}

#[test]
#[verifies("rule_ste_dictionary_finding_categories", examples)]
fn classification_separates_the_four_categories() {
    let dictionary = fixture_dictionary();
    let text = "aagoods bbbad ccboth ddnew";

    let uses = classify_vocabulary(text, &dictionary);

    let categories: Vec<(&str, VocabularyCategory)> = uses
        .iter()
        .map(|word_use| {
            (
                &text[word_use.span.start..word_use.span.end],
                word_use.category,
            )
        })
        .collect();
    assert_eq!(
        categories,
        [
            ("aagoods", VocabularyCategory::Approved),
            ("bbbad", VocabularyCategory::Unapproved),
            ("ccboth", VocabularyCategory::Uncertain),
            ("ddnew", VocabularyCategory::Unknown),
        ]
    );
}

#[test]
#[verifies("rule_ste_dictionary_finding_categories", examples)]
fn uncertain_and_unknown_uses_produce_no_finding() {
    let dictionary = fixture_dictionary();

    let report = check_descriptive_with_dictionary("The ccboth ddnew stays.", &dictionary, &[]);

    assert!(
        report.findings.is_empty(),
        "findings: {:?}",
        report.findings
    );
}

#[test]
fn the_data_free_checks_still_run_with_a_dictionary() {
    let dictionary = fixture_dictionary();

    let report = check_descriptive_with_dictionary("The pump runs; it is loud.", &dictionary, &[]);

    assert_eq!(report.findings.len(), 1, "findings: {:?}", report.findings);
    assert_eq!(report.findings[0].rule, RuleNumber::EightOne);
}
