use provenance_macros::verifies;
use provenance_ste100::{check_descriptive, RuleNumber, Span};

fn findings(text: &str) -> Vec<(RuleNumber, Span)> {
    check_descriptive(text)
        .findings
        .into_iter()
        .map(|finding| (finding.rule, finding.span))
        .collect()
}

fn words(count: usize) -> String {
    vec!["word"; count].join(" ")
}

#[test]
#[verifies("rule_ste100_quoted_text_protection", examples)]
#[verifies("rule_ste100_contracted_verb", examples)]
#[verifies("rule_ste100_semicolon", examples)]
fn straight_quotes_protect_only_their_contents() {
    assert_eq!(
        findings("é \"can't; stop\" can't;"),
        vec![
            (RuleNumber::FourTwo, Span { start: 17, end: 22 }),
            (RuleNumber::EightOne, Span { start: 22, end: 23 }),
        ]
    );
}

#[test]
#[verifies("rule_ste100_quoted_text_protection", examples)]
fn curly_quotes_and_unicode_prefixes_preserve_byte_spans() {
    assert_eq!(
        findings("α “won’t;” β;"),
        vec![(RuleNumber::EightOne, Span { start: 20, end: 21 })]
    );
}

#[test]
#[verifies("rule_ste100_quoted_text_protection", examples)]
fn several_balanced_quotes_protect_each_delimited_span() {
    let text = "\"can't;\" outside; “won’t;” can't";
    let outside_semicolon = text.find("outside;").unwrap() + "outside".len();
    let outside_contraction = text.rfind("can't").unwrap();

    assert_eq!(
        findings(text),
        vec![
            (
                RuleNumber::EightOne,
                Span {
                    start: outside_semicolon,
                    end: outside_semicolon + 1,
                },
            ),
            (
                RuleNumber::FourTwo,
                Span {
                    start: outside_contraction,
                    end: outside_contraction + "can't".len(),
                },
            ),
        ]
    );
}

#[test]
#[verifies("rule_ste100_quoted_text_protection", examples)]
fn unmatched_opening_quote_makes_strict_checks_indeterminate() {
    assert_eq!(
        findings("\"can't; outside;"),
        [] as [(provenance_ste100::RuleNumber, provenance_ste100::Span); 0]
    );
}

#[test]
#[verifies("rule_ste100_quoted_text_protection", examples)]
fn unmatched_curly_close_makes_strict_checks_indeterminate() {
    assert_eq!(
        findings("can't;” outside;"),
        [] as [(provenance_ste100::RuleNumber, provenance_ste100::Span); 0]
    );
}

#[test]
#[verifies("rule_ste100_quoted_text_protection", examples)]
fn mixed_delimiters_make_strict_checks_indeterminate() {
    assert_eq!(
        findings("“can't;\" outside;"),
        [] as [(provenance_ste100::RuleNumber, provenance_ste100::Span); 0]
    );
}

#[test]
#[verifies("rule_ste100_quoted_text_protection", examples)]
fn nested_quotes_make_strict_checks_indeterminate() {
    assert_eq!(
        findings("“outer “can't;” outside;”"),
        [] as [(provenance_ste100::RuleNumber, provenance_ste100::Span); 0]
    );
}

#[test]
fn indeterminate_quotes_prevent_a_strict_sentence_length_finding() {
    let text = format!("“{} “can't;” outside;”.", words(26));
    assert_eq!(
        findings(&text),
        [] as [(provenance_ste100::RuleNumber, provenance_ste100::Span); 0]
    );
}

#[test]
#[verifies("rule_ste100_explicit_quotation_counting", examples)]
fn protected_quotation_still_counts_as_one_word() {
    let at_limit = format!("{} \"can't; use these eight source words\".", words(24));
    assert_eq!(
        findings(&at_limit),
        [] as [(provenance_ste100::RuleNumber, provenance_ste100::Span); 0]
    );

    let above_limit = format!("{} “won’t; use these eight source words”.", words(25));
    assert_eq!(
        findings(&above_limit),
        vec![(
            RuleNumber::SixThree,
            Span {
                start: 0,
                end: above_limit.len(),
            },
        )]
    );
}
