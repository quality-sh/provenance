//! The shipped-v1 legacy audit, frozen by fingerprint.
//!
//! One repository shipped with proposals already in terminal states and with a
//! `promotion_decisions.jsonl` shard recording who ended them. That history is
//! closed: the rows that shipped are the only rows the audit will ever have.
//! Both halves of it are pinned here as a SHA-256 of the records themselves, so
//! any edit, addition, or deletion changes the fingerprint and is refused.
//!
//! Two readers ask the same question of the same constants. The ideation
//! aggregate asks whether the terminal rows and their dispositions inside a
//! validated aggregate are still the shipped ones, and `provenance-store`'s
//! shard admission gate asks whether the rows sitting in the retired shard
//! belong to that audit at all. The freeze lives here so there is one of it.

use crate::model::{DispositionRecord, ProposalCard};
use serde::Serialize;
use sha2::{Digest, Sha256};

const SHIPPED_TERMINAL_PROPOSAL_DIGEST_V1: &str =
    "eb8f351c26b3894197e2d5748eed0e62d0b12ca015a309553fce237909462843";
const SHIPPED_DISPOSITION_AUDIT_DIGEST_V1: &str =
    "995184bcded00c0c42c5bb0709189bdb34851e26981e82e997fbca6c7823b69b";

/// Are these terminal proposal rows exactly the ones that shipped?
///
/// Only the ideation aggregate asks, so this one is not re-exported from
/// `model`; the disposition half below is, because `provenance-store`'s shard
/// gate asks it too.
pub fn is_shipped_terminal_proposal_set<'a>(
    proposals: impl IntoIterator<Item = &'a ProposalCard>,
) -> bool {
    frozen_digest(proposals, |proposal| proposal.id.as_str()) == SHIPPED_TERMINAL_PROPOSAL_DIGEST_V1
}

/// Are these disposition rows exactly the shipped-v1 audit?
///
/// The answer is about the set, not about any one row: a row belongs to the
/// audit only when the whole set it arrives in fingerprints as the audit. An
/// empty set is not the audit, and callers who mean "no legacy history here"
/// say so themselves rather than reading it out of this answer.
pub fn is_shipped_legacy_disposition_audit<'a>(
    dispositions: impl IntoIterator<Item = &'a DispositionRecord>,
) -> bool {
    frozen_digest(dispositions, |disposition| disposition.id.as_str())
        == SHIPPED_DISPOSITION_AUDIT_DIGEST_V1
}

/// The fingerprint recipe both halves of the audit were taken with: records
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
    format!("{:x}", hasher.finalize())
}
