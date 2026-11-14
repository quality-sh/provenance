//! An import that the store can write as an index and load again, with no PDF.

use provenance_ste100::{
    normalized_data_digest, DictionaryImport, DictionaryImportIdentity, StandardIssue,
};

/// An import with no entries and the true data digest of its entries. A load
/// by identity checks the identity and the digest, so a stored index of this
/// import loads again.
pub fn import() -> DictionaryImport {
    DictionaryImport {
        identity: DictionaryImportIdentity {
            issue: StandardIssue::Nine,
            source_sha256: "source".to_owned(),
            data_sha256: normalized_data_digest(&[]),
            extractor_version: "0.0.0".to_owned(),
        },
        entries: Vec::new(),
    }
}
