//! The scanner, macro, and canonical contract use the same method words.
//! The SDK derives its method type from this contract. Its compiler fixtures
//! check the public TypeScript type in both assignment directions.

use std::str::FromStr;

use provenance_macros::verifies;
use provenance_scanner::Verification;

/// Every variant, and the compiler refuses this list going stale: adding a
/// variant breaks the match, and the match names exactly this list.
fn all_verifications() -> Vec<Verification> {
    let listed = [
        Verification::Exhaustion,
        Verification::Property,
        Verification::Examples,
        Verification::Conformance,
        Verification::Construction,
        Verification::Proof,
    ];
    for variant in listed {
        match variant {
            Verification::Exhaustion
            | Verification::Property
            | Verification::Examples
            | Verification::Conformance
            | Verification::Construction
            | Verification::Proof => {}
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

fn core_method_words() -> Vec<String> {
    use provenance_core::VerificationMethod;

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
    listed.iter().map(ToString::to_string).collect()
}

#[test]
#[verifies("rule_verification_method_words", conformance)]
fn every_macro_method_word_is_a_scanner_verification() {
    for word in macro_method_words() {
        let parsed = Verification::from_str(&word)
            .unwrap_or_else(|_| panic!("the macro accepts `{word}`; the scanner refuses it"));
        assert_eq!(
            parsed.to_string(),
            word,
            "`{word}` does not round-trip through the scanner"
        );
    }
}

#[test]
#[verifies("rule_verification_method_words", conformance)]
fn the_scanner_knows_no_method_word_the_macro_refuses() {
    let macro_words = macro_method_words();
    for variant in all_verifications() {
        assert!(
            macro_words.contains(&variant.to_string()),
            "the scanner parses `{variant}`; the macro would refuse it at the attribute"
        );
    }
    assert_eq!(
        macro_words.len(),
        all_verifications().len(),
        "the two method lists differ in size"
    );
}

#[test]
#[verifies("rule_verification_method_words", conformance)]
fn core_uses_exactly_the_macro_and_scanner_method_words() {
    let core_words = core_method_words();
    let macro_words = macro_method_words();
    let scanner_words = all_verifications()
        .into_iter()
        .map(|verification| verification.to_string())
        .collect::<Vec<_>>();

    assert_eq!(core_words, macro_words);
    assert_eq!(core_words, scanner_words);
}
