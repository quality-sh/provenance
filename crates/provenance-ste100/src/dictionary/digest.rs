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
