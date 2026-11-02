use provenance_macros::verifies;
use provenance_ste100::{
    check_descriptive_with_dictionary, classify_vocabulary, DictionaryEntry, DictionaryImport,
    DictionaryImportIdentity, DictionaryStatus, FindingKind, PartOfSpeech, RuleNumber, Span,
    StandardIssue, VocabularyCategory,
};

fn entry(
    headword: &str,
    forms: &[&str],
    part_of_speech: PartOfSpeech,
    status: DictionaryStatus,
) -> DictionaryEntry {
    DictionaryEntry {
        headword: headword.to_owned(),
        word_forms: forms.iter().map(|form| (*form).to_owned()).collect(),
        part_of_speech,
        status,
        approved_meaning_or_alternatives: "A meaning".to_owned(),
        ste_example: "USE THE ITEM.".to_owned(),
        non_ste_example: None,
    }
}

fn noun(headword: &str, forms: &[&str], status: DictionaryStatus) -> DictionaryEntry {
    entry(headword, forms, PartOfSpeech::Noun, status)
}

fn verb(headword: &str, forms: &[&str], status: DictionaryStatus) -> DictionaryEntry {
    entry(headword, forms, PartOfSpeech::Verb, status)
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
            noun("AAGOOD", &["AAGOOD", "AAGOODS"], DictionaryStatus::Approved),
            noun("bbbad", &["bbbad"], DictionaryStatus::Unapproved),
            noun(
                "bb far from",
                &["bb far from"],
                DictionaryStatus::Unapproved,
            ),
            noun("CCBOTH", &["CCBOTH"], DictionaryStatus::Approved),
            noun("ccboth", &["ccboth"], DictionaryStatus::Unapproved),
            verb(
                "bfrobnicate",
                &["bfrobnicate"],
                DictionaryStatus::Unapproved,
            ),
            verb("bzap", &["bzap"], DictionaryStatus::Unapproved),
            verb("bzorply", &["bzorply"], DictionaryStatus::Unapproved),
            noun("bzorp", &["bzorp"], DictionaryStatus::Unapproved),
            noun("AAWIDGET", &["AAWIDGET"], DictionaryStatus::Approved),
            verb(
                "AAMOVE",
                &["AAMOVE", "AAMOVES", "AAMOVED"],
                DictionaryStatus::Approved,
            ),
            verb("ccwin", &["ccwin"], DictionaryStatus::Unapproved),
            noun("CCWINS", &["CCWINS"], DictionaryStatus::Approved),
            noun("CCPAIR", &["CCPAIR"], DictionaryStatus::Approved),
            verb("ccpair", &["ccpair"], DictionaryStatus::Unapproved),
            entry(
                "CCFULL",
                &["CCFULL"],
                PartOfSpeech::Adjective,
                DictionaryStatus::Approved,
            ),
            verb("ccfull", &["ccfull"], DictionaryStatus::Unapproved),
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

fn categories(text: &str) -> Vec<(String, VocabularyCategory)> {
    classify_vocabulary(text, &fixture_dictionary())
        .into_iter()
        .map(|word_use| {
            (
                text[word_use.span.start..word_use.span.end].to_owned(),
                word_use.category,
            )
        })
        .collect()
}

fn category_of(text: &str, word: &str) -> VocabularyCategory {
    categories(text)
        .into_iter()
        .find(|(found, _)| found == word)
        .unwrap_or_else(|| panic!("{word} must be classified in {text}"))
        .1
}

fn rule_one_one_words(text: &str) -> Vec<String> {
    check_descriptive_with_dictionary(text, &fixture_dictionary(), &[])
        .findings
        .into_iter()
        .filter(|finding| finding.rule == RuleNumber::OneOne)
        .map(|finding| text[finding.span.start..finding.span.end].to_owned())
        .collect()
}

#[test]
#[verifies("rule_ste_dictionary_inflected_unapproved_form", examples)]
fn each_regular_inflection_of_an_unapproved_verb_produces_a_finding() {
    for (text, word) in [
        ("The registry bfrobnicates the command.", "bfrobnicates"),
        ("The registry bfrobnicated the command.", "bfrobnicated"),
        (
            "The registry is bfrobnicating the command.",
            "bfrobnicating",
        ),
        ("The registry bzaps the command.", "bzaps"),
        ("The registry bzapped the command.", "bzapped"),
        ("The registry is bzapping the command.", "bzapping"),
        ("The registry bzorplies the command.", "bzorplies"),
        ("The registry bzorplied the command.", "bzorplied"),
    ] {
        assert_eq!(rule_one_one_words(text), [word], "text: {text}");
    }
}

#[test]
#[verifies("rule_ste_dictionary_inflected_unapproved_form", examples)]
fn the_plural_of_an_unapproved_noun_produces_a_finding() {
    assert_eq!(rule_one_one_words("Bzorps stay here."), ["Bzorps"]);
    assert_eq!(rule_one_one_words("The bzorps's door opens."), ["bzorps's"]);
}

#[test]
#[verifies("rule_ste_dictionary_inflected_unapproved_form", examples)]
fn the_plural_of_an_approved_noun_is_approved() {
    assert_eq!(
        category_of("The aawidgets stay.", "aawidgets"),
        VocabularyCategory::Approved
    );
}

#[test]
#[verifies("rule_ste_dictionary_inflected_unapproved_form", examples)]
fn an_approved_verb_keeps_only_its_printed_forms() {
    assert_eq!(
        category_of("The unit aamoved.", "aamoved"),
        VocabularyCategory::Approved
    );
    assert_eq!(
        category_of("The unit is aamoving.", "aamoving"),
        VocabularyCategory::Unknown
    );
}

#[test]
#[verifies("rule_ste_dictionary_inflected_unapproved_form", examples)]
fn a_printed_form_wins_over_a_derived_form() {
    assert_eq!(
        category_of("The ccwins stay.", "ccwins"),
        VocabularyCategory::Approved
    );
}

#[test]
#[verifies("rule_ste_dictionary_sentence_word_class", examples)]
fn a_noun_position_picks_the_approved_noun_entry() {
    for text in [
        "The ccpair is open.",
        "The unit's ccpair is open.",
        "Stop with ccpair here.",
        "The ccpairs stay.",
    ] {
        assert_eq!(
            rule_one_one_words(text),
            Vec::<String>::new(),
            "text: {text}"
        );
    }
    assert_eq!(
        category_of("The ccpair is open.", "ccpair"),
        VocabularyCategory::Approved
    );
}

#[test]
#[verifies("rule_ste_dictionary_sentence_word_class", examples)]
fn a_verb_position_picks_the_unapproved_verb_entry() {
    for (text, word) in [
        ("Ccpair the panel.", "Ccpair"),
        ("It ccpairs the panel.", "ccpairs"),
        ("You must ccpair the panel.", "ccpair"),
        ("Make sure to ccpair the panel.", "ccpair"),
    ] {
        assert_eq!(rule_one_one_words(text), [word], "text: {text}");
    }
}

#[test]
#[verifies("rule_ste_dictionary_sentence_word_class", examples)]
fn an_unsettled_word_class_stays_uncertain() {
    for (text, word) in [
        ("Ccpairs stay here.", "Ccpairs"),
        ("The bbbad ccpair stays.", "ccpair"),
        ("Stop, ccpair here.", "ccpair"),
        ("The ccboth is open.", "ccboth"),
        ("Ccpair output stops.", "Ccpair"),
        ("The tank is ccfull.", "ccfull"),
        ("Ccfull the tank.", "Ccfull"),
    ] {
        assert_eq!(
            category_of(text, word),
            VocabularyCategory::Uncertain,
            "text: {text}"
        );
    }
}

#[test]
#[verifies("rule_ste_dictionary_unapproved_word", examples)]
fn a_word_with_only_unapproved_entries_stays_unapproved_in_any_position() {
    assert_eq!(
        rule_one_one_words("The bfrobnicate shows it."),
        ["bfrobnicate"]
    );
}

fn rule_one_one_words_with_names(text: &str, technical_names: &[&str]) -> Vec<String> {
    let names: Vec<String> = technical_names
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    check_descriptive_with_dictionary(text, &fixture_dictionary(), &names)
        .findings
        .into_iter()
        .filter(|finding| finding.rule == RuleNumber::OneOne)
        .map(|finding| text[finding.span.start..finding.span.end].to_owned())
        .collect()
}

#[test]
#[verifies("rule_ste_technical_names_accepted", examples)]
fn a_technical_name_accepts_its_regular_inflections() {
    for text in [
        "Bzorps stay here.",
        "The bzorps's door opens.",
        "The registry bzaps the command.",
        "The registry bzapped the command.",
        "The registry is bzapping the command.",
        "The registry bzorplies the command.",
        "The registry bzorplied the command.",
    ] {
        assert_eq!(
            rule_one_one_words_with_names(text, &["bzorp", "bzap", "bzorply"]),
            Vec::<String>::new(),
            "text: {text}"
        );
    }
}

#[test]
#[verifies("rule_ste_technical_names_accepted", examples)]
fn the_last_word_of_a_technical_phrase_accepts_its_plural() {
    assert_eq!(
        rule_one_one_words_with_names("The aagood bzorps stay.", &["aagood bzorp"]),
        Vec::<String>::new()
    );
    assert_eq!(
        rule_one_one_words_with_names("The bzorps aagood stay.", &["bzorp aagood"]),
        ["bzorps"]
    );
}

#[test]
#[verifies("rule_ste_technical_names_accepted", examples)]
fn an_unrelated_unapproved_word_still_gives_a_finding_next_to_an_inflected_name() {
    assert_eq!(
        rule_one_one_words_with_names("The bzorps and the bbbad stay.", &["bzorp"]),
        ["bbbad"]
    );
    assert_eq!(
        rule_one_one_words_with_names("The registry bfrobnicates the bzorps.", &["bzorp"]),
        ["bfrobnicates"]
    );
}
