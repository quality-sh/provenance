use provenance_macros::rule;
use sha2::{Digest, Sha256};

use super::{
    DictionaryEntry, DictionaryImportIdentity, DictionaryStatus, PartOfSpeech,
    DICTIONARY_EXTRACTOR_VERSION,
};
use crate::StandardIssue;

/// Records the source and normalized-data identity without storing either asset.
#[rule("rule_ste_dictionary_import_identity")]
pub(super) fn identity(source: &[u8], entries: &[DictionaryEntry]) -> DictionaryImportIdentity {
    DictionaryImportIdentity {
        issue: StandardIssue::Nine,
        source_sha256: source_digest(source),
        data_sha256: normalized_data_digest(entries),
        extractor_version: DICTIONARY_EXTRACTOR_VERSION.to_owned(),
    }
}

pub(super) fn source_digest(source: &[u8]) -> String {
    format!("{:x}", Sha256::digest(source))
}

pub(super) fn normalized_data_digest(entries: &[DictionaryEntry]) -> String {
    let mut digest = Sha256::new();
    put(&mut digest, "provenance-ste100-dictionary-import-v1");
    for entry in entries {
        put(&mut digest, &entry.headword);
        for form in &entry.word_forms {
            put(&mut digest, form);
        }
        put(&mut digest, part_label(entry.part_of_speech));
        put(&mut digest, status_label(entry.status));
        put(&mut digest, &entry.approved_meaning_or_alternatives);
        put(&mut digest, &entry.ste_example);
        put(&mut digest, entry.non_ste_example.as_deref().unwrap_or(""));
    }
    format!("{:x}", digest.finalize())
}

fn put(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value.as_bytes());
}

const fn status_label(status: DictionaryStatus) -> &'static str {
    match status {
        DictionaryStatus::Approved => "approved",
        DictionaryStatus::Unapproved => "unapproved",
    }
}

const fn part_label(part: PartOfSpeech) -> &'static str {
    match part {
        PartOfSpeech::Adjective => "adjective",
        PartOfSpeech::Adverb => "adverb",
        PartOfSpeech::Article => "article",
        PartOfSpeech::Conjunction => "conjunction",
        PartOfSpeech::Noun => "noun",
        PartOfSpeech::Prefix => "prefix",
        PartOfSpeech::Preposition => "preposition",
        PartOfSpeech::Pronoun => "pronoun",
        PartOfSpeech::Verb => "verb",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_digest_is_the_lowercase_sha256_hex_of_the_bytes() {
        assert_eq!(
            source_digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn normalized_data_digest_hashes_length_prefixed_fields() {
        let entry = DictionaryEntry {
            headword: "test".to_owned(),
            word_forms: vec!["test".to_owned(), "tests".to_owned()],
            part_of_speech: PartOfSpeech::Verb,
            status: DictionaryStatus::Approved,
            approved_meaning_or_alternatives: "To examine.".to_owned(),
            ste_example: "Test the pump.".to_owned(),
            non_ste_example: None,
        };
        assert_eq!(
            normalized_data_digest(&[entry]),
            "e655bf7150a2a54afceb231da1cf31af7027fb5b40c4d6f6f94a8fcfee1b2be1"
        );
    }
}
