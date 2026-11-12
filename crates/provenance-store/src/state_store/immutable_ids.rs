//! The located refusal of an immutable ideation id on two lines of its file.
//!
//! A hand edit or a branch merge can put one id on two lines of the file of
//! a proposal, an assertion or a disposition. Each aggregate check of the
//! ideation records in the store goes through [`IdeationRecords::validate`],
//! which refuses that state with `invalid_state_file` at the second line.
//! The ideation scope check gives the same refusal. The message names the
//! two lines.

use super::readers::{read_jsonl_located, record_fault};
use super::StateStore;
use crate::shards;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, DuplicateImmutableId, IdeationAggregate,
    ProposalCard, ScopeId, SynthesisPacket,
};

/// The ideation records of one scope for the aggregate check of a write or
/// of the scope check.
pub(super) struct IdeationRecords<'a> {
    pub(super) store: &'a StateStore,
    pub(super) scope: &'a ScopeId,
    pub(super) disposition_actor_ids: &'a [String],
    pub(super) contributions: &'a [Contribution],
    pub(super) synthesis_packets: &'a [SynthesisPacket],
    pub(super) proposals: &'a [ProposalCard],
    pub(super) assertions: &'a [AssertionRecord],
    pub(super) dispositions: &'a [DispositionRecord],
}

impl IdeationRecords<'_> {
    /// [`provenance_core::validate_ideation_aggregate`] over these records,
    /// with the shipped legacy policy. When the aggregate check finds a
    /// duplicate proposal, assertion or disposition id, this reads the file
    /// of that family again with its lines. When that read shows the id on
    /// two lines, the refusal is the located refusal. The file can change
    /// between the read of the records and the second read. Then the second
    /// read can fail or show the id on one line, and the refusal is the
    /// refusal of the aggregate check, with no file and no line.
    pub(super) fn validate(&self) -> anyhow::Result<()> {
        provenance_core::validate_ideation_aggregate(IdeationAggregate {
            legacy_policy: provenance_core::LegacyProposalPolicy::ShippedV1,
            disposition_actor_ids: self.disposition_actor_ids,
            contributions: self.contributions,
            synthesis_packets: self.synthesis_packets,
            proposals: self.proposals,
            assertions: self.assertions,
            dispositions: self.dispositions,
        })
        .map_err(|error| {
            let located = error
                .downcast_ref::<DuplicateImmutableId>()
                .and_then(|duplicate| {
                    let path = family_path(self.store, self.scope, duplicate.kind)?;
                    located_duplicate(self.store, &path, duplicate.kind, &duplicate.id)
                });
            located.unwrap_or(error)
        })
    }
}

/// The file of the ideation family `kind` in `scope`.
fn family_path(store: &StateStore, scope: &ScopeId, kind: &str) -> Option<Utf8PathBuf> {
    match kind {
        "proposal" => Some(shards::proposal_cards_path(&store.layout, scope)),
        "assertion" => Some(shards::assertion_records_path(&store.layout, scope)),
        "disposition" => Some(shards::dispositions_path(&store.layout, scope)),
        _ => None,
    }
}

/// The refusal of the scope check for the id `id` of the family `kind`,
/// which the file at `path` holds two times. When the file does not show the
/// id on two lines, the refusal is the refusal of [`record_fault`].
pub(super) fn duplicate_fault(
    store: &StateStore,
    path: &Utf8Path,
    kind: &str,
    id: &str,
) -> anyhow::Error {
    located_duplicate(store, path, kind, id).unwrap_or_else(|| {
        record_fault(store, path, id, format!("duplicate immutable {kind} id {id}"))
    })
}

/// `invalid_state_file` at the second line with the id `id` in the file at
/// `path`. The message names the first line and the second line. `None`
/// when the read fails or does not show the id on two lines.
fn located_duplicate(
    store: &StateStore,
    path: &Utf8Path,
    kind: &str,
    id: &str,
) -> Option<anyhow::Error> {
    let rows = read_jsonl_located::<serde_json::Value>(store, path).ok()?;
    let mut lines = rows
        .into_iter()
        .filter(|(row, _)| row.get("id").and_then(serde_json::Value::as_str) == Some(id))
        .map(|(_, at)| at);
    let first = lines.next()?;
    let second = lines.next()?;
    let message = format!(
        "duplicate immutable {kind} id {id} at line {} and at line {}",
        first.line, second.line
    );
    Some(crate::refusal::state_fault(&second, message))
}
