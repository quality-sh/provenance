use provenance_macros::verifies;
use provenance_ste100::{check_descriptive, FindingKind, RuleNumber, Span};

const MESSAGE: &str = "This paragraph has more than six sentences.";

fn sentence_sequence(count: usize) -> String {
    (0..count)
        .map(|index| format!("Sentence {index}."))
        .collect::<Vec<_>>()
        .join(" ")
}

fn rule_6_6_spans(text: &str) -> Vec<Span> {
    check_descriptive(text)
        .findings
        .into_iter()
        .filter(|finding| serde_json::to_value(finding.rule).unwrap() == serde_json::json!("6.6"))
        .map(|finding| {
            assert_eq!(finding.kind, FindingKind::Violation);
            assert_eq!(finding.message, MESSAGE);
            finding.span
        })
        .collect()
}

#[test]
#[verifies("rule_ste100_paragraph_sentence_limit", examples)]
fn a_paragraph_permits_six_sentences_and_reports_seven() {
    assert_eq!(rule_6_6_spans(&sentence_sequence(6)), [] as [provenance_ste100::Span; 0]);

    let text = sentence_sequence(7);
    assert_eq!(
        rule_6_6_spans(&text),
        vec![Span {
            start: 0,
            end: text.len(),
        }]
    );
}

#[test]
fn sentence_counts_restart_at_clear_paragraph_boundaries() {
    let text = format!("{}\n\n{}", sentence_sequence(6), sentence_sequence(6));
    assert_eq!(rule_6_6_spans(&text), [] as [provenance_ste100::Span; 0]);

    let first = sentence_sequence(7);
    let second = sentence_sequence(7);
    let text = format!("é.\n\n{first}\n \t\n{second}");
    let first_start = "é.\n\n".len();
    let second_start = first_start + first.len() + "\n \t\n".len();
    assert_eq!(
        rule_6_6_spans(&text),
        vec![
            Span {
                start: first_start,
                end: first_start + first.len(),
            },
            Span {
                start: second_start,
                end: text.len(),
            },
        ]
    );
}

#[test]
fn lowercase_text_in_the_next_paragraph_does_not_change_the_first_paragraph() {
    let first = sentence_sequence(7);
    let second = format!("lowercase start. {}", sentence_sequence(6));
    let text = format!("{first}\n\n{second}");
    let second_start = first.len() + "\n\n".len();

    assert_eq!(
        rule_6_6_spans(&text),
        vec![
            Span {
                start: 0,
                end: first.len(),
            },
            Span {
                start: second_start,
                end: text.len(),
            },
        ]
    );
}

#[test]
fn one_line_break_does_not_end_a_paragraph() {
    let text = format!("{}\n{}", sentence_sequence(3), sentence_sequence(4));
    assert_eq!(
        rule_6_6_spans(&text),
        vec![Span {
            start: 0,
            end: text.len(),
        }]
    );
}

#[test]
fn protected_text_does_not_define_sentence_or_paragraph_boundaries() {
    let text = format!(
        "\"Ignore. These. Marks.\n\nAlso. These.\" {}",
        sentence_sequence(6)
    );
    assert_eq!(rule_6_6_spans(&text), [] as [provenance_ste100::Span; 0]);
}

#[test]
fn indeterminate_sentence_or_paragraph_boundaries_produce_no_strict_finding() {
    let list_like = format!("Items:\n- one. {}", sentence_sequence(6));
    let unbalanced_parenthesis = format!("({}", sentence_sequence(7));
    let unbalanced_quote = format!("\"{}", sentence_sequence(7));

    assert_eq!(rule_6_6_spans(&list_like), [] as [provenance_ste100::Span; 0]);
    assert_eq!(rule_6_6_spans(&unbalanced_parenthesis), [] as [provenance_ste100::Span; 0]);
    assert_eq!(rule_6_6_spans(&unbalanced_quote), [] as [provenance_ste100::Span; 0]);
}

#[test]
fn parenthetical_sentence_counting_is_reused() {
    let text = format!(
        "{} Final sentence (Parenthetical sentence).",
        sentence_sequence(5)
    );
    assert_eq!(
        rule_6_6_spans(&text),
        vec![Span {
            start: 0,
            end: text.len(),
        }]
    );
}

#[test]
fn an_abbreviation_with_an_unclear_sentence_boundary_prevents_a_strict_finding() {
    let text = format!("Use the e.g. value. {}", sentence_sequence(5));
    assert_eq!(rule_6_6_spans(&text), [] as [provenance_ste100::Span; 0]);
}

#[test]
fn combined_findings_have_stable_source_order() {
    let long_sentence = format!("{}.", vec!["word"; 26].join(" "));
    let text = format!("{long_sentence} It isn't ready;! {}", sentence_sequence(5));
    let first = check_descriptive(&text);

    assert_eq!(
        first
            .findings
            .iter()
            .map(|finding| finding.rule)
            .collect::<Vec<_>>(),
        vec![
            RuleNumber::SixThree,
            RuleNumber::SixSix,
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
