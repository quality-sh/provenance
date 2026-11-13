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

/// A user reads these texts as the load problem of the dictionary status and in the
/// failure of a strict check. No Rule states the texts, so this test has no Rule binding.
/// It makes each fault of the bytes of an index and shows its text. The store tests the
/// texts of a file that is missing or that cannot be read.
#[test]
fn each_fault_of_the_index_bytes_gives_its_diagnostic() {
    let import = fixture_import();
    let path = Path::new(PATH);
    let at = path.display();
    let decode = |contents: &[u8]| {
        decode_dictionary_index(contents, &import.identity, path)
            .expect_err("the index has a fault")
            .to_string()
    };

    let malformed = decode(b"{");
    let cause = malformed.strip_prefix(&format!("malformed index file at {at}: "));
    assert!(cause.is_some_and(|cause| !cause.is_empty()), "{malformed}");

    let mut other = fixture_import();
    other.identity.extractor_version.push_str("-other");
    assert_eq!(
        decode(&encoded(&other)),
        format!("the index file at {at} records a different import identity")
    );

    let stored = String::from_utf8(encoded(&import)).expect("the index is UTF-8");
    assert_eq!(
        decode(stored.replace("AAWORD", "ABWORD").as_bytes()),
        format!("the index file at {at} does not match its recorded data digest")
    );

    let rows = super::super::validate_entries(&import.entries).expect_err("two rows");
    let error =
        decode_dictionary_index_for_source(&encoded(&import), b"synthetic source bytes", path)
            .expect_err("an incomplete index must not load");
    assert_eq!(
        error.to_string(),
        format!("the index file at {at} has invalid dictionary structure: {rows}")
    );
}
