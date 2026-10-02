use provenance_macros::verifies;
use provenance_ste100::{check_descriptive, FindingKind, RuleNumber, Span};

const MESSAGE: &str = "This descriptive sentence has more than 25 words.";

fn words(count: usize) -> String {
    (0..count).map(|_| "word").collect::<Vec<_>>().join(" ")
}

fn sentence(count: usize) -> String {
    format!("{}.", words(count))
}

fn rule_6_3_spans(text: &str) -> Vec<Span> {
    check_descriptive(text)
        .findings
        .into_iter()
        .filter(|finding| finding.rule == RuleNumber::SixThree)
        .map(|finding| {
            assert_eq!(finding.kind, FindingKind::Violation);
            assert_eq!(finding.message, MESSAGE);
            finding.span
        })
        .collect()
}

#[test]
#[verifies("rule_ste100_descriptive_sentence_length", property)]
fn determinate_limit_is_exactly_25_words() {
    for count in 0..=40 {
        let text = sentence(count);
        let expected = if count > 25 {
            vec![Span {
                start: 0,
                end: text.len(),
            }]
        } else {
            vec![]
        };
        assert_eq!(rule_6_3_spans(&text), expected, "count {count}");
    }
}

#[test]
fn exactly_25_passes_and_exactly_26_has_one_finding() {
    assert_eq!(
        check_descriptive(&sentence(25)).findings,
        [] as [provenance_ste100::Finding; 0]
    );

    let text = sentence(26);
    let report = check_descriptive(&text);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].rule, RuleNumber::SixThree);
    assert_eq!(
        report.findings[0].span,
        Span {
            start: 0,
            end: text.len()
        }
    );
}

#[test]
#[verifies("rule_ste100_ordinary_colon_continuity", examples)]
fn ordinary_colon_does_not_end_a_sentence() {
    let text = format!("{}: {}.", words(13), words(13));
    assert_eq!(
        rule_6_3_spans(&text),
        vec![Span {
            start: 0,
            end: text.len()
        }]
    );
}

#[test]
fn list_like_colon_and_newline_is_not_counted_as_one_ordinary_sentence() {
    let text = format!("{}:\n  {}.", words(13), words(13));
    assert_eq!(rule_6_3_spans(&text), [] as [provenance_ste100::Span; 0]);
}

#[test]
fn terminal_punctuation_and_unicode_offsets_define_sentence_spans() {
    let first = "é is ready! ";
    let second = format!("{}? ", words(26));
    let third = sentence(26);
    let text = format!("{first}{second}{third}");
    assert_eq!(
        rule_6_3_spans(&text),
        vec![
            Span {
                start: first.len(),
                end: first.len() + second.trim_end().len(),
            },
            Span {
                start: first.len() + second.len(),
                end: text.len(),
            },
        ]
    );
}

#[test]
fn six_short_sentences_are_not_a_sentence_length_violation() {
    let text = (0..6).map(|_| sentence(25)).collect::<Vec<_>>().join(" ");
    assert_eq!(rule_6_3_spans(&text), [] as [provenance_ste100::Span; 0]);
}

#[test]
#[verifies("rule_ste100_parenthetical_counting", examples)]
fn parenthetical_text_counts_once_outside_and_as_a_sentence_inside() {
    let outer_passes = format!("{} ({}).", words(24), words(10));
    assert_eq!(
        rule_6_3_spans(&outer_passes),
        [] as [provenance_ste100::Span; 0]
    );

    let inner = words(26);
    let text = format!("{} ({inner}).", words(2));
    let start = text.find('(').unwrap() + 1;
    assert_eq!(
        rule_6_3_spans(&text),
        vec![Span {
            start,
            end: start + inner.len(),
        }]
    );
}

#[test]
#[verifies("rule_ste100_explicit_quotation_counting", examples)]
fn a_balanced_explicit_quotation_counts_as_one_word() {
    let text = format!("{} \"{}\".", words(24), words(8));
    assert_eq!(rule_6_3_spans(&text), [] as [provenance_ste100::Span; 0]);
}

#[test]
#[verifies("rule_ste100_hyphenated_group_counting", property)]
fn hyphenated_groups_each_count_as_one_word() {
    for count in 1..=30 {
        let text = format!("{}.", vec!["alpha-beta-gamma"; count].join(" "));
        assert_eq!(rule_6_3_spans(&text).is_empty(), count <= 25);
    }
}

#[test]
#[verifies("rule_ste100_number_counting", property)]
fn mechanically_unambiguous_numbers_each_count_as_one_word() {
    for count in 1..=30 {
        let text = format!("{}.", vec!["123"; count].join(" "));
        assert_eq!(rule_6_3_spans(&text).is_empty(), count <= 25);
    }
}

#[test]
#[verifies("rule_ste100_semantic_count_indeterminate", examples)]
fn unresolved_rule_8_6_meaning_prevents_a_strict_finding() {
    let possible_unit = format!("{} 10 widgets remain.", words(24));
    let possible_multiword_name = format!("{} Alpha Beta remain.", words(24));
    assert_eq!(
        rule_6_3_spans(&possible_unit),
        [] as [provenance_ste100::Span; 0]
    );
    assert_eq!(
        rule_6_3_spans(&possible_multiword_name),
        [] as [provenance_ste100::Span; 0]
    );
}

#[test]
fn unbalanced_parentheses_and_quotes_cannot_produce_strict_findings() {
    let unbalanced_parenthesis = format!("({}.", words(26));
    let unbalanced_quote = format!("\"{}.", words(26));
    let unbalanced_curly_quote = format!("“{}.", words(26));
    assert_eq!(
        rule_6_3_spans(&unbalanced_parenthesis),
        [] as [provenance_ste100::Span; 0]
    );
    assert_eq!(
        rule_6_3_spans(&unbalanced_quote),
        [] as [provenance_ste100::Span; 0]
    );
    assert_eq!(
        rule_6_3_spans(&unbalanced_curly_quote),
        [] as [provenance_ste100::Span; 0]
    );
}

#[test]
fn nested_parentheses_and_unclear_internal_punctuation_are_indeterminate() {
    let nested = format!("({} (note)).", words(26));
    let apostrophes = format!("{}.", vec!["operator's"; 13].join(" "));
    let slashes = format!("{}.", vec!["input/output"; 13].join(" "));
    assert_eq!(rule_6_3_spans(&nested), [] as [provenance_ste100::Span; 0]);
    assert_eq!(
        rule_6_3_spans(&apostrophes),
        [] as [provenance_ste100::Span; 0]
    );
    assert_eq!(rule_6_3_spans(&slashes), [] as [provenance_ste100::Span; 0]);
}

#[test]
fn curly_apostrophes_and_prime_like_marks_are_indeterminate() {
    for token in ["operator’s", "operator‘s", "operator′s"] {
        let text = format!("{}.", vec![token; 13].join(" "));
        assert!(rule_6_3_spans(&text).is_empty(), "token {token:?}");
    }
}

#[test]
fn digit_first_alphanumeric_candidates_are_indeterminate() {
    let text = format!("{}.", vec!["1A"; 13].join(" "));
    assert_eq!(rule_6_3_spans(&text), [] as [provenance_ste100::Span; 0]);
}

#[test]
fn lowercase_underscore_identifier_candidates_are_indeterminate() {
    let text = format!("{}.", vec!["pump_id"; 13].join(" "));
    assert_eq!(rule_6_3_spans(&text), [] as [provenance_ste100::Span; 0]);
}

#[test]
fn internal_colon_alphanumeric_candidates_are_indeterminate() {
    let text = format!("{}.", vec!["port:1"; 13].join(", "));
    assert_eq!(rule_6_3_spans(&text), [] as [provenance_ste100::Span; 0]);
}

#[test]
fn digit_hyphen_letter_measurement_candidates_are_indeterminate() {
    let text = format!("{}.", vec!["10-mm"; 13].join(" "));
    assert_eq!(rule_6_3_spans(&text), [] as [provenance_ste100::Span; 0]);
}

#[test]
fn unresolved_multiword_names_are_indeterminate_without_grouping_them() {
    let uppercase = format!("{}.", vec!["ALPHA BETA"; 13].join(" "));
    let lowercase_connector = format!("{} Bank of America.", words(23));
    assert_eq!(
        rule_6_3_spans(&uppercase),
        [] as [provenance_ste100::Span; 0]
    );
    assert_eq!(
        rule_6_3_spans(&lowercase_connector),
        [] as [provenance_ste100::Span; 0]
    );
}

#[test]
fn balanced_curly_double_quotation_counts_as_one_word() {
    let text = format!("{} “{}”.", words(24), words(8));
    assert_eq!(rule_6_3_spans(&text), [] as [provenance_ste100::Span; 0]);
}

#[test]
fn parenthetical_text_with_internal_sentence_punctuation_is_indeterminate() {
    let text = format!("note ({}. {}).", words(13), words(13));
    assert_eq!(rule_6_3_spans(&text), [] as [provenance_ste100::Span; 0]);
}

#[test]
fn rule_8_1_and_rule_6_3_findings_have_stable_source_order() {
    let text = format!("{}; word.", words(25));
    let first = check_descriptive(&text);
    assert_eq!(first.findings.len(), 2);
    assert_eq!(first.findings[0].rule, RuleNumber::SixThree);
    assert_eq!(
        first.findings[0].span,
        Span {
            start: 0,
            end: text.len()
        }
    );
    assert_eq!(first.findings[1].rule, RuleNumber::EightOne);
    assert_eq!(first.findings[1].span.start, words(25).len());

    for _ in 0..10 {
        assert_eq!(check_descriptive(&text), first);
    }
}

#[test]
fn all_implemented_rule_findings_have_stable_source_order() {
    let long_sentence = sentence(26);
    let text = format!("{long_sentence} It isn't ready;");
    let first = check_descriptive(&text);

    assert_eq!(
        first
            .findings
            .iter()
            .map(|finding| finding.rule)
            .collect::<Vec<_>>(),
        vec![
            RuleNumber::SixThree,
            RuleNumber::FourTwo,
            RuleNumber::EightOne,
        ]
    );
    assert!(first
        .findings
        .windows(2)
        .all(|pair| pair[0].span.start <= pair[1].span.start));

    for _ in 0..10 {
        assert_eq!(check_descriptive(&text), first);
    }
}
