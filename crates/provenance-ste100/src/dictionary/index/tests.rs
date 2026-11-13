use std::path::Path;

use provenance_macros::verifies;

use super::super::{digest, DictionaryEntry, DictionaryImport, DictionaryStatus, PartOfSpeech};
use super::{
    decode_dictionary_index, decode_dictionary_index_for_source, dictionary_index_file_name,
    encode_dictionary_index, DictionaryIndexError,
};

const PATH: &str = "index.json";

fn fixture_import() -> DictionaryImport {
    let entries = vec![
        DictionaryEntry {
            headword: "AAWORD".to_owned(),
            word_forms: vec!["AAWORD".to_owned()],
            part_of_speech: PartOfSpeech::Noun,
            status: DictionaryStatus::Approved,
            approved_meaning_or_alternatives: "A meaning".to_owned(),
            ste_example: "USE THE ITEM.".to_owned(),
            non_ste_example: None,
        },
        DictionaryEntry {
            headword: "bword".to_owned(),
            word_forms: vec!["bword".to_owned()],
            part_of_speech: PartOfSpeech::Verb,
            status: DictionaryStatus::Unapproved,
            approved_meaning_or_alternatives: "USE (v)".to_owned(),
            ste_example: "USE THE ITEM.".to_owned(),
            non_ste_example: Some("Bword the item.".to_owned()),
        },
    ];
    DictionaryImport {
        identity: digest::identity(b"synthetic source bytes", &entries),
        entries,
    }
}

fn encoded(import: &DictionaryImport) -> Vec<u8> {
    encode_dictionary_index(import).expect("encode the index")
}

#[test]
fn a_decoded_index_is_the_encoded_import() {
    let import = fixture_import();

    let decoded = decode_dictionary_index(&encoded(&import), &import.identity, Path::new(PATH));

    assert_eq!(decoded, Ok(import));
}

#[test]
#[verifies("rule_ste_dictionary_index_digest_verification", examples)]
fn an_edited_index_file_fails_closed() {
    let import = fixture_import();
    let stored = String::from_utf8(encoded(&import)).expect("the index is UTF-8");
    let edited = stored.replace("AAWORD", "ABWORD");
    assert_ne!(stored, edited, "the edit must change the file");

    let error = decode_dictionary_index(edited.as_bytes(), &import.identity, Path::new(PATH));

    assert_eq!(
        error,
        Err(DictionaryIndexError::DigestMismatch { path: PATH.into() })
    );
}

#[test]
#[verifies("rule_ste_dictionary_index_digest_verification", examples)]
fn an_index_for_a_different_identity_has_a_different_file() {
    let import = fixture_import();
    let mut other = import.identity.clone();
    other.extractor_version = format!("{}-other", other.extractor_version);

    assert_ne!(
        dictionary_index_file_name(&other),
        dictionary_index_file_name(&import.identity)
    );
}

#[test]
#[verifies("rule_ste_dictionary_index_digest_verification", examples)]
fn an_index_file_with_a_different_recorded_identity_fails_closed() {
    let import = fixture_import();
    let mut other = import.identity.clone();
    other.extractor_version = format!("{}-other", other.extractor_version);

    let error = decode_dictionary_index(&encoded(&import), &other, Path::new(PATH));

    assert_eq!(
        error,
        Err(DictionaryIndexError::IdentityMismatch { path: PATH.into() })
    );
}

#[test]
#[verifies("rule_ste_dictionary_structure_validation", examples)]
fn a_source_match_does_not_accept_an_incomplete_index() {
    let import = fixture_import();

    let error = decode_dictionary_index_for_source(
        &encoded(&import),
        b"synthetic source bytes",
        Path::new(PATH),
    )
    .expect_err("an incomplete index must not load for a new project");

    assert!(
        matches!(error, DictionaryIndexError::InvalidStructure { .. }),
        "unexpected error: {error:?}"
    );
}
