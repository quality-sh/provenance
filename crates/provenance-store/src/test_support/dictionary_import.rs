//! An import that the store can write as an index and load again, with no PDF.

use provenance_ste100::{DictionaryImport, DictionaryImportIdentity, StandardIssue};

/// The data digest that `provenance-ste100` gives to a list of no entries.
const NO_ENTRIES_DIGEST: &str = "6e085e74cf76f5c335d6f6e636f457497c040fbe50521e449ca4872ac0b2a684";

/// An import with no entries and the true data digest of its entries. A load
/// by identity checks the identity and the digest, so a stored index of this
/// import loads again.
pub fn import() -> DictionaryImport {
    DictionaryImport {
        identity: DictionaryImportIdentity {
            issue: StandardIssue::Nine,
            source_sha256: "source".to_owned(),
            data_sha256: NO_ENTRIES_DIGEST.to_owned(),
            extractor_version: "0.0.0".to_owned(),
        },
        entries: Vec::new(),
    }
}
