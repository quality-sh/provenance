//! An import that the store can write as an index. Its data digest is not the
//! digest of its entries, so a load of its index fails.

use provenance_ste100::{DictionaryImport, DictionaryImportIdentity, StandardIssue};

/// An import with no entries.
pub fn import() -> DictionaryImport {
    DictionaryImport {
        identity: DictionaryImportIdentity {
            issue: StandardIssue::Nine,
            source_sha256: "source".to_owned(),
            data_sha256: "data".to_owned(),
            extractor_version: "0.0.0".to_owned(),
        },
        entries: Vec::new(),
    }
}
