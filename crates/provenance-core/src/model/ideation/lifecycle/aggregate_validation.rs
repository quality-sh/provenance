use super::{
    assertion_validation, effective_proposal_state, legacy_validation, lineage_validation,
    validate_proposal_intrinsic, AssertionRecord, IdeationAggregate,
};
use crate::model::{
    disposition_requires_prior_assertion, validate_disposition_intrinsic, DispositionRecord,
    PromotionState, ProposalCard, SchemaVersion,
};
use provenance_macros::rule;
use serde::Serialize;
use std::collections::BTreeMap;

/// The only record layout this codebase knows.
///
/// Every guard that refuses a record for its version reads this one value:
/// the ideation guard below, `provenance-store`'s read guard, the pinned
/// graph's per-record check, and the CLI's artifact validator. There is no
/// second copy of the number to update.
pub const SUPPORTED_SCHEMA_VERSION: SchemaVersion = SchemaVersion(1);

// The record kinds the guard names, single-homed so a call site and the
// exhaustion proof below cannot drift apart.
const CONTRIBUTION_KIND: &str = "contribution";
const SYNTHESIS_KIND: &str = "synthesis";
const PROPOSAL_KIND: &str = "proposal";
const DISPOSITION_KIND: &str = "disposition";
pub(super) const ASSERTION_KIND: &str = "assertion";

/// Records are read only at schema version 1.
///
/// Every ideation record carries a `schema_version`, and a later version will
/// mean a different layout: a field moved, dropped, or read with a new
/// meaning. Today's structs would deserialize such a record on whatever
/// fields still line up and misread the rest without saying so, and the
/// validated aggregate would then rest on a record nobody here understood.
/// Version 1 is the only layout this code knows, so anything else is refused
/// at the door rather than half-read.
///
/// `kind` names the record in the message and is the only thing that differs
/// between the call sites; the test they all state is the same one.
#[rule("rule_schema_version_one")]
pub fn ensure_supported_schema_version(kind: &str, version: SchemaVersion) -> anyhow::Result<()> {
    anyhow::ensure!(
        version == SUPPORTED_SCHEMA_VERSION,
        "{kind} schema_version must be {}",
        SUPPORTED_SCHEMA_VERSION.0
    );
    Ok(())
}

pub(super) fn validate(aggregate: IdeationAggregate<'_>) -> anyhow::Result<()> {
    validate_source_schema_versions(&aggregate)?;
    ensure_immutable_ids(
        "contribution",
        aggregate
            .contributions
            .iter()
            .map(|record| (record.id.as_str(), record)),
    )?;
    ensure_immutable_ids(
        "synthesis packet",
        aggregate
            .synthesis_packets
            .iter()
            .map(|record| (record.id.as_str(), record)),
    )?;
    ensure_immutable_ids(
        "proposal",
        aggregate
            .proposals
            .iter()
            .map(|record| (record.id.as_str(), record)),
    )?;
    legacy_validation::validate_records(
        aggregate.proposals,
        aggregate.dispositions,
        aggregate.legacy_policy,
    )?;
    validate_proposals(aggregate.proposals)?;
    ensure_immutable_ids(
        "assertion",
        aggregate
            .assertions
            .iter()
            .map(|record| (record.id.as_str(), record)),
    )?;
    ensure_immutable_ids(
        "disposition",
        aggregate
            .dispositions
            .iter()
            .map(|record| (record.id.as_str(), record)),
    )?;
    validate_disposition_intrinsics(aggregate.dispositions)?;

    let proposals = aggregate
        .proposals
        .iter()
        .map(|proposal| (proposal.id.as_str(), proposal))
        .collect::<BTreeMap<_, _>>();
    validate_actor_allowlists(&aggregate, &proposals)?;
    lineage_validation::validate(aggregate.proposals, aggregate.assertions)?;
    let synthesis_packets = aggregate
        .synthesis_packets
        .iter()
        .map(|packet| (packet.id.as_str(), packet))
        .collect::<BTreeMap<_, _>>();
    assertion_validation::validate_assertions(&aggregate, &proposals, &synthesis_packets)?;
    validate_disposition_links(&aggregate)?;
    assertion_validation::ensure_qualifying_assertions(
        aggregate.proposals,
        aggregate.synthesis_packets,
        aggregate.assertions,
        aggregate.dispositions,
    )
}

fn validate_source_schema_versions(aggregate: &IdeationAggregate<'_>) -> anyhow::Result<()> {
    for contribution in aggregate.contributions {
        ensure_supported_schema_version(CONTRIBUTION_KIND, contribution.schema_version)?;
    }
    for packet in aggregate.synthesis_packets {
        ensure_supported_schema_version(SYNTHESIS_KIND, packet.schema_version)?;
    }
    Ok(())
}

fn validate_proposals(proposals: &[ProposalCard]) -> anyhow::Result<()> {
    for proposal in proposals {
        ensure_supported_schema_version(PROPOSAL_KIND, proposal.schema_version)?;
        if proposal.promotion_state == PromotionState::Proposed {
            validate_proposal_intrinsic(proposal)?;
        }
    }
    Ok(())
}

fn validate_disposition_intrinsics(dispositions: &[DispositionRecord]) -> anyhow::Result<()> {
    for disposition in dispositions {
        validate_disposition_intrinsic(disposition)?;
    }
    Ok(())
}

/// Who may dispose of a live proposal.
///
/// A disposition ends a proposal's life, so only an actor named in the
/// repository's manifest may record one. The check bites on a proposal whose
/// effective pre-disposition state is live (`proposed` or `asserted`); a legacy
/// terminal row is frozen by its shipped fingerprint and carries its own audit.
///
/// The allowlist is manifest state, written at `provenance init` and read
/// on every write path that validates the aggregate. An empty list therefore
/// blocks every disposition, which is the safe default for a repository that
/// has not yet said who decides.
///
/// The actor id is an attestation, not proof of identity. Nothing here checks
/// a key or a signature: the id records who claims to have decided, and write
/// access to the repository is the only gate behind that claim.
fn validate_actor_allowlists(
    aggregate: &IdeationAggregate<'_>,
    proposals: &BTreeMap<&str, &ProposalCard>,
) -> anyhow::Result<()> {
    for disposition in aggregate.dispositions {
        if let Some(proposal) = proposals.get(disposition.proposal_id.as_str()) {
            validate_actor_allowlist(
                proposal,
                aggregate.assertions,
                &disposition.actor.id,
                aggregate.disposition_actor_ids,
            )?;
        }
    }
    Ok(())
}

#[rule("rule_disposition_actor_allowlist")]
fn validate_actor_allowlist(
    proposal: &ProposalCard,
    assertions: &[AssertionRecord],
    actor_id: &str,
    disposition_actor_ids: &[String],
) -> anyhow::Result<()> {
    let requires_allowlisted_actor = matches!(
        effective_proposal_state(proposal, assertions, &[]),
        PromotionState::Proposed | PromotionState::Asserted
    );
    if requires_allowlisted_actor {
        anyhow::ensure!(
            disposition_actor_ids.iter().any(|id| id == actor_id),
            "{}",
            allowlist_refusal(disposition_actor_ids)
        );
    }
    Ok(())
}

/// An empty allowlist means configuration is absent, not that every possible
/// actor was deliberately denied, so its refusal names the field and setup
/// command rather than blaming the supplied actor ID.
const fn allowlist_refusal(disposition_actor_ids: &[String]) -> &'static str {
    if disposition_actor_ids.is_empty() {
        "no disposition actors configured: repository manifest disposition_actor_ids is empty; \
         set it with provenance init --disposition-actor-id <ACTOR_ID>"
    } else {
        "disposition actor is not in the repository allowlist"
    }
}

/// What every disposition must satisfy, wherever it is judged.
///
/// A disposition is the record that ends a proposal's life, and two readers
/// judge one: this aggregate, which reads a whole scope, and
/// `provenance-store`'s disposition write gate, which reads a single
/// disposition against what the scope already holds. Three of the tests are
/// the same test in both, so they are stated once here and both callers ask
/// them:
///
/// 1. the proposal named exists, so no decision lands on nothing;
/// 2. an acceptance names a proposal that already carries an assertion, unless
///    [`disposition_requires_prior_assertion`] says a person's ratified
///    artifact stands where the assertion would. A proposal that has already
///    left `proposed` is shipped history, frozen by its own fingerprint and
///    carrying its own audit, so it is not asked again;
/// 3. nothing has ended this proposal already and the disposition id is still
///    free, so a proposal is disposed of once and no decision overwrites
///    another.
///
/// `recorded` is the dispositions the caller already holds: every disposition
/// in the scope for the write gate, and the ones already walked for the
/// aggregate. The proposal found by the first test is returned, so a caller
/// that needs the record does not look it up twice.
///
/// One test the write gate makes is not here: that the canonical artifact a
/// disposition names exists in the scope under the kind it claims. Only the
/// store can see artifacts.
pub fn validate_disposition_admissible<'a>(
    disposition: &DispositionRecord,
    proposals: &'a [ProposalCard],
    assertions: &[AssertionRecord],
    recorded: &[DispositionRecord],
) -> anyhow::Result<&'a ProposalCard> {
    let proposal = proposals
        .iter()
        .find(|proposal| proposal.id == disposition.proposal_id)
        .ok_or_else(|| anyhow::anyhow!("proposal does not exist"))?;
    if proposal.promotion_state == PromotionState::Proposed {
        anyhow::ensure!(
            !disposition_requires_prior_assertion(disposition)
                || assertions
                    .iter()
                    .any(|assertion| assertion.proposal_id == disposition.proposal_id),
            "accepted proposal must be asserted before disposition"
        );
    }
    anyhow::ensure!(
        !recorded
            .iter()
            .any(|record| record.id == disposition.id
                || record.proposal_id == disposition.proposal_id),
        "proposal already has an authoritative disposition"
    );
    Ok(proposal)
}

/// Which proposal each disposition ends, and what had to be recorded first.
///
/// [`validate_disposition_admissible`] holds everything this shares with
/// `provenance-store`'s write gate. What is left is what only a reader of
/// whole records can ask: the record layout, and that a disposition sits in
/// the scope its proposal sits in.
fn validate_disposition_links(aggregate: &IdeationAggregate<'_>) -> anyhow::Result<()> {
    for (index, disposition) in aggregate.dispositions.iter().enumerate() {
        ensure_supported_schema_version(DISPOSITION_KIND, disposition.schema_version)?;
        let proposal = validate_disposition_admissible(
            disposition,
            aggregate.proposals,
            aggregate.assertions,
            &aggregate.dispositions[..index],
        )?;
        anyhow::ensure!(
            disposition.scope_id == proposal.scope_id,
            "disposition must share the proposal scope"
        );
    }
    Ok(())
}

fn ensure_immutable_ids<'a, T: Serialize + 'a>(
    kind: &str,
    records: impl IntoIterator<Item = (&'a str, &'a T)>,
) -> anyhow::Result<()> {
    let mut seen = BTreeMap::new();
    for (id, record) in records {
        let value = serde_json::to_value(record)?;
        anyhow::ensure!(
            seen.insert(id, value).is_none(),
            "duplicate immutable {kind} id {id}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod schema_version_tests {
    use provenance_macros::verifies;

    use super::{
        ensure_supported_schema_version, ASSERTION_KIND, CONTRIBUTION_KIND, DISPOSITION_KIND,
        PROPOSAL_KIND, SYNTHESIS_KIND,
    };
    use crate::model::SchemaVersion;

    // Every record kind the five guarded call sites name. A sixth guarded
    // kind has to join this list before the proofs below cover it.
    const GUARDED_RECORD_KINDS: [&str; 5] = [
        CONTRIBUTION_KIND,
        SYNTHESIS_KIND,
        PROPOSAL_KIND,
        DISPOSITION_KIND,
        ASSERTION_KIND,
    ];

    // The version range the exhaustion below runs. The guard is an equality
    // test on the version and nothing else: it is monotone in nothing, so no
    // interpolation between tried values is being claimed, and every version
    // that is not 1 takes the same branch. These four are the value below the
    // accepted one, the accepted one, the value above, and the top of the u32
    // domain, which is the whole of the behaviour there is to see.
    const VERSION_RANGE: [u32; 4] = [0, 1, 2, u32::MAX];

    // Independent restatement of the decision, listed rather than compared so
    // the oracle does not repeat the primary implementation's shape: these are the
    // record layouts this codebase knows how to read. Must not be implemented
    // by calling the primary implementation.
    const READABLE_LAYOUT_VERSIONS: [u32; 1] = [1];

    fn layout_is_readable_by_oracle(version: SchemaVersion) -> bool {
        READABLE_LAYOUT_VERSIONS.contains(&version.0)
    }

    // The messages the five sites shipped before they shared a function,
    // copied byte for byte. `provenance-cli/tests/cli_ideation_schema_versions.rs`
    // matches on these strings from outside the crate.
    const SHIPPED_MESSAGES: [(&str, &str); 5] = [
        (CONTRIBUTION_KIND, "contribution schema_version must be 1"),
        (SYNTHESIS_KIND, "synthesis schema_version must be 1"),
        (PROPOSAL_KIND, "proposal schema_version must be 1"),
        (DISPOSITION_KIND, "disposition schema_version must be 1"),
        (ASSERTION_KIND, "assertion schema_version must be 1"),
    ];

    #[test]
    #[verifies("rule_schema_version_one", exhaustion)]
    fn only_version_one_is_read() {
        for kind in GUARDED_RECORD_KINDS {
            for version in VERSION_RANGE.map(SchemaVersion) {
                assert_eq!(
                    ensure_supported_schema_version(kind, version).is_ok(),
                    layout_is_readable_by_oracle(version),
                    "the rule and the decision disagree on {kind} at {version:?}"
                );
            }
        }
    }

    #[test]
    #[verifies("rule_schema_version_one", examples)]
    fn rejection_names_the_record_kind_as_it_always_did() {
        for (kind, message) in SHIPPED_MESSAGES {
            let error = ensure_supported_schema_version(kind, SchemaVersion(2)).unwrap_err();
            assert_eq!(error.to_string(), message);
        }
    }
}

#[cfg(test)]
mod disposition_actor_trigger_tests {
    use provenance_macros::verifies;
    use serde_json::json;

    use super::validate_actor_allowlist;
    use crate::model::{AssertionRecord, PromotionState, ProposalCard};

    #[test]
    #[verifies("rule_disposition_actor_allowlist", examples)]
    fn trigger_uses_the_proposals_derived_effective_state() {
        let mut proposal: ProposalCard = serde_json::from_value(json!({
            "schema_version": 1, "scope_id": "default", "id": "proposal_a",
            "proposal_key": "proposal_a", "proposal_type": "question",
            "title": "Proposal", "summary": "Proposal",
            "traceability": {
                "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
                "source_ids": [], "evidence_references": [], "supporting_claim_ids": []
            },
            "promotion_state": "proposed"
        }))
        .unwrap();
        let assertion: AssertionRecord = serde_json::from_value(json!({
            "schema_version": 1, "scope_id": "default", "id": "assertion_a",
            "proposal_id": "proposal_a", "synthesis_packet_id": "synthesis_a",
            "supporting_claim_ids": ["claim_a"]
        }))
        .unwrap();

        let error = validate_actor_allowlist(&proposal, &[assertion], "reviewer", &[])
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("no disposition actors configured"));

        proposal.promotion_state = PromotionState::Accepted;
        validate_actor_allowlist(&proposal, &[], "reviewer", &[]).unwrap();
    }
}
