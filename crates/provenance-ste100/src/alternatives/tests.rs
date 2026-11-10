//! Implementation-aid tests: no active Rule states the dictionary text in a
//! Rule 1.1 finding yet. Each test pins the full message for one shape of
//! match, with an in-memory dictionary.

use crate::{
    check_descriptive_with_dictionary, DictionaryEntry, DictionaryImport, DictionaryImportIdentity,
    DictionaryStatus, PartOfSpeech, RuleNumber, StandardIssue,
};

fn entry(
    form: &str,
    part_of_speech: PartOfSpeech,
    status: DictionaryStatus,
    text: &str,
) -> DictionaryEntry {
    DictionaryEntry {
        headword: form.to_owned(),
        word_forms: vec![form.to_owned()],
        part_of_speech,
        status,
        approved_meaning_or_alternatives: text.to_owned(),
        ste_example: "USE THE ITEM.".to_owned(),
        non_ste_example: None,
    }
}

fn dictionary() -> DictionaryImport {
    use DictionaryStatus::{Approved, Unapproved};
    use PartOfSpeech::{Noun, Verb};
    DictionaryImport {
        identity: DictionaryImportIdentity {
            issue: StandardIssue::Nine,
            source_sha256: "0".repeat(64),
            data_sha256: "0".repeat(64),
            extractor_version: "test".to_owned(),
        },
        entries: vec![
            entry("bzorp", Noun, Unapproved, "ITEM (n)"),
            entry("bzorp", Verb, Unapproved, "MOVE (v)."),
            entry("bglint", Noun, Unapproved, "LIGHT (n)"),
            entry("bglint", Verb, Unapproved, "SHINE (v)"),
            entry("bzax", Noun, Unapproved, "PART (n)"),
            entry("bb far from", Noun, Unapproved, "AWAY FROM (prep)"),
            entry("CCPAIR", Noun, Approved, "Two items of one type"),
            entry("ccpair", Verb, Unapproved, "CONNECT (v)"),
            entry("bzas", Noun, Unapproved, " "),
            entry("bdup", Noun, Unapproved, "TRY (v)"),
            entry("bdup", Verb, Unapproved, "TRY (v)."),
            entry("bplum", Noun, Unapproved, "UNIT (n)"),
            entry("bqq", Noun, Unapproved, "ONE (n)"),
            entry("bqq far", Noun, Unapproved, "NEAR (prep)"),
            entry("bqq far from", Noun, Unapproved, "OUT OF (prep)"),
        ],
    }
}

fn messages(text: &str) -> Vec<String> {
    check_descriptive_with_dictionary(text, &dictionary(), &[])
        .findings
        .into_iter()
        .filter(|finding| finding.rule == RuleNumber::OneOne)
        .map(|finding| finding.message)
        .collect()
}

fn message(rejected: &str, given: &str) -> String {
    format!(
        "The word or phrase \"{rejected}\" is not approved by ASD-STE100 Rule 1.1.\
         {given}"
    )
}

#[test]
fn two_entries_give_their_text_in_dictionary_order_with_a_separator() {
    assert_eq!(
        messages("The bzorp stops."),
        [message(
            "bzorp",
            " The dictionary gives: ITEM (n); MOVE (v)."
        )]
    );
}

#[test]
fn an_inflected_form_gives_only_the_entries_of_its_word_class() {
    assert_eq!(
        messages("The lamp bglinted."),
        [message("bglinted", " The dictionary gives: SHINE (v).")]
    );
}

#[test]
fn a_possessive_gives_the_text_of_its_base_word() {
    assert_eq!(
        messages("The bzax's cover stops."),
        [message("bzax's", " The dictionary gives: PART (n).")]
    );
}

#[test]
fn a_phrase_gives_the_text_of_the_phrase_entry() {
    assert_eq!(
        messages("Stop bb far from the door."),
        [message(
            "bb far from",
            " The dictionary gives: AWAY FROM (prep)."
        )]
    );
}

#[test]
fn a_word_with_an_approved_entry_gives_only_the_unapproved_entry() {
    assert_eq!(
        messages("You must ccpair the panel."),
        [message("ccpair", " The dictionary gives: CONNECT (v).")]
    );
}

#[test]
fn an_entry_with_no_text_gives_no_dictionary_sentence() {
    assert_eq!(messages("The bzas stops."), [message("bzas", "")]);
}

#[test]
fn text_that_two_entries_give_occurs_one_time() {
    assert_eq!(
        messages("The bdup stops."),
        [message("bdup", " The dictionary gives: TRY (v).")]
    );
}

#[test]
fn a_plural_gives_the_text_of_its_noun() {
    assert_eq!(
        messages("The bplums stop."),
        [message("bplums", " The dictionary gives: UNIT (n).")]
    );
}

#[test]
fn the_longest_phrase_gives_its_text_and_not_the_text_of_a_shorter_match() {
    assert_eq!(
        messages("Stop bqq far from the door."),
        [message(
            "bqq far from",
            " The dictionary gives: OUT OF (prep)."
        )]
    );
}
