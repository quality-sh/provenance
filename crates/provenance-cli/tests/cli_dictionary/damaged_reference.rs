//! The answers of the dictionary commands when the dictionary reference does
//! not parse, for example after a bad merge or a hand edit: an empty file, or a
//! file with half of a JSON object.
//!
//! The import and the init refuse, keep the file and write no index, as
//! `rule_ste_dictionary_reference_parse_refusal` states. The statement check
//! warns that Rule 1.1 did not run, as `rule_ste_dictionary_index_missing_warning`
//! states. The strict check fails, as `rule_ste_strict_dictionary_index_gate`
//! states. The tests of an init with no PDF are in `cli_ste_onboarding`.

use super::support::{
    half_reference, reference_problem, reference_refusal, unloaded_check, unloaded_status,
    unloaded_strict_check, ReferenceProject, HALF_REFERENCE_MESSAGE,
};
use provenance_macros::verifies;

/// A reference with no bytes.
const EMPTY: &str = "";

/// The parse message of [`EMPTY`].
const EMPTY_MESSAGE: &str = "EOF while parsing a value";

/// The refusal that a write gives for a reference whose first line does
/// not parse with `message`, the unchanged `contents`, and no index file.
fn refused(message: &str, contents: &str) -> (Option<i32>, String, String, Vec<String>) {
    (
        Some(1),
        reference_refusal(message),
        contents.to_owned(),
        Vec::new(),
    )
}

/// Implementation aid with no Rule: no Rule states the `load_problem` of the
/// dictionary status. The test records the answer with its repair step.
#[test]
fn the_status_of_an_empty_reference_names_the_parse_error() {
    let damaged = ReferenceProject::with_reference(EMPTY);

    assert_eq!(
        damaged.status(),
        unloaded_status(&damaged, &reference_problem(EMPTY_MESSAGE))
    );
}

/// Implementation aid with no Rule, as the test above.
#[test]
fn the_status_of_half_of_a_reference_names_the_parse_error() {
    let damaged = ReferenceProject::with_reference(&half_reference());

    assert_eq!(
        damaged.status(),
        unloaded_status(&damaged, &reference_problem(HALF_REFERENCE_MESSAGE))
    );
}

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_statement_check_with_an_empty_reference_warns_that_rule_1_1_did_not_run() {
    let damaged = ReferenceProject::with_reference(EMPTY);

    assert_eq!(
        damaged.statement_check(&[]),
        unloaded_check(&reference_problem(EMPTY_MESSAGE))
    );
}

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_statement_check_with_half_of_a_reference_warns_that_rule_1_1_did_not_run() {
    let damaged = ReferenceProject::with_reference(&half_reference());

    assert_eq!(
        damaged.statement_check(&[]),
        unloaded_check(&reference_problem(HALF_REFERENCE_MESSAGE))
    );
}

#[test]
#[verifies("rule_ste_strict_dictionary_index_gate", examples)]
fn a_strict_statement_check_with_half_of_a_reference_fails() {
    let damaged = ReferenceProject::with_reference(&half_reference());

    assert_eq!(
        damaged.statement_check(&["--strict"]),
        unloaded_strict_check(&reference_problem(HALF_REFERENCE_MESSAGE))
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_import_over_an_empty_reference_refuses_and_keeps_the_file() {
    let damaged = ReferenceProject::with_reference(EMPTY);

    let import = damaged.import();

    assert_eq!(damaged.refusal(&import), refused(EMPTY_MESSAGE, EMPTY));
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_import_over_half_of_a_reference_refuses_and_keeps_the_technical_names() {
    let damaged = ReferenceProject::with_reference(&half_reference());

    let import = damaged.import();

    assert_eq!(
        damaged.refusal(&import),
        refused(HALF_REFERENCE_MESSAGE, &half_reference())
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_init_over_an_empty_reference_refuses_and_keeps_the_file() {
    let damaged = ReferenceProject::with_reference(EMPTY);

    let init = damaged.init();

    assert_eq!(damaged.refusal(&init), refused(EMPTY_MESSAGE, EMPTY));
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_init_over_half_of_a_reference_refuses_and_keeps_the_technical_names() {
    let damaged = ReferenceProject::with_reference(&half_reference());

    let init = damaged.init();

    assert_eq!(
        damaged.refusal(&init),
        refused(HALF_REFERENCE_MESSAGE, &half_reference())
    );
}
