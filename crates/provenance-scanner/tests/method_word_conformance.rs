//! The `verifies` macro and the canonical method type use the same six words.
//! A proc-macro cannot depend on provenance-core, so this test reads the
//! macro list from its source and pins it against core's enum. The SDK
//! derives its method type from core's contract; its compiler fixtures check
//! the public TypeScript type in both assignment directions.

use std::str::FromStr;

use provenance_core::VerificationMethod;
use provenance_macros::verifies;

/// Every variant, and the compiler refuses this list going stale: adding a
/// variant breaks the match, and the match names exactly this list.
fn core_method_words() -> Vec<VerificationMethod> {
    let listed = [
        VerificationMethod::Exhaustion,
        VerificationMethod::Property,
        VerificationMethod::Examples,
        VerificationMethod::Conformance,
        VerificationMethod::Construction,
        VerificationMethod::Proof,
    ];
    for variant in listed {
        match variant {
            VerificationMethod::Exhaustion
            | VerificationMethod::Property
            | VerificationMethod::Examples
            | VerificationMethod::Conformance
            | VerificationMethod::Construction
            | VerificationMethod::Proof => {}
        }
    }
    listed.to_vec()
}

fn macro_method_words() -> Vec<String> {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../provenance-macros/src/lib.rs"),
    )
    .expect("read provenance-macros source");
    let (_, tail) = source
        .split_once("const VERIFICATION_METHODS")
        .expect("provenance-macros no longer declares VERIFICATION_METHODS");
    let (_, initializer) = tail
        .split_once('=')
        .expect("VERIFICATION_METHODS has no initializer");
    let (list, _) = initializer
        .split_once(']')
        .expect("VERIFICATION_METHODS list is unterminated");
    let words = list
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert!(
        !words.is_empty(),
        "parsed no method words out of provenance-macros"
    );
    words
}

#[test]
#[verifies("rule_verification_method_words", conformance)]
fn every_macro_method_word_is_a_core_method_word() {
    for word in macro_method_words() {
        let parsed = VerificationMethod::from_str(&word)
            .unwrap_or_else(|_| panic!("the macro accepts `{word}`; core refuses it"));
        assert_eq!(
            parsed.to_string(),
            word,
            "`{word}` does not round-trip through core"
        );
    }
}

#[test]
#[verifies("rule_verification_method_words", conformance)]
fn core_knows_no_method_word_the_macro_refuses() {
    let macro_words = macro_method_words();
    for variant in core_method_words() {
        assert!(
            macro_words.contains(&variant.to_string()),
            "core accepts `{variant}`; the macro would refuse it at the attribute"
        );
    }
    assert_eq!(
        macro_words.len(),
        core_method_words().len(),
        "the two method lists differ in size"
    );
}
