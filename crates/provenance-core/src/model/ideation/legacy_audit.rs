//! The shipped-v1 terminal proposals, frozen by fingerprint.
//!
//! One repository shipped with proposals already in terminal states. That
//! history is closed: the rows that shipped are the only terminal rows there
//! will ever be. They are pinned here as a SHA-256 of the records themselves,
//! so any edit, addition, or deletion changes the fingerprint and is refused.

use crate::model::ProposalCard;
use serde::Serialize;
use sha2::{Digest, Sha256};

const SHIPPED_TERMINAL_PROPOSAL_DIGEST_V1: &str =
    "92441c53bce652c363cb8e976ae325fe82e116472ec904d0c9728e54327a1681";

/// Are these terminal proposal rows exactly the ones that shipped?
pub fn is_shipped_terminal_proposal_set<'a>(
    proposals: impl IntoIterator<Item = &'a ProposalCard>,
) -> bool {
    frozen_digest(proposals, |proposal| proposal.id.as_str()) == SHIPPED_TERMINAL_PROPOSAL_DIGEST_V1
}

/// The fingerprint recipe: records
/// ordered by id, each serialized as JSON and closed with a newline.
fn frozen_digest<'a, T: Serialize + 'a>(
    records: impl IntoIterator<Item = &'a T>,
    id: impl Fn(&T) -> &str,
) -> String {
    let mut records = records.into_iter().collect::<Vec<_>>();
    records.sort_by(|left, right| id(left).cmp(id(right)));
    let mut hasher = Sha256::new();
    for record in records {
        hasher.update(serde_json::to_vec(record).expect("record serialization is infallible"));
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    // This test has no Rule. It keeps the digest text the same when the hash library changes.
    #[test]
    fn frozen_digest_hashes_sorted_json_lines_as_lowercase_hex() {
        let records = ["b".to_owned(), "a".to_owned()];
        assert_eq!(
            super::frozen_digest(&records, String::as_str),
            "14bdec8c77f9feebc3b047fa42657bac4babc6d05439548a956ee1a9edf35eab"
        );
    }
}
