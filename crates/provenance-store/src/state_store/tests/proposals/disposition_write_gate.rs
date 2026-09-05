use super::{
    super::{initialized_store, seeded_requirement_store},
    proposal_input,
};
use crate::state_store::proposal_writers::validate_disposition_write_gate;
use crate::state_store::{CreateDispositionInput, StateStore};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    AssertionId, AssertionRecord, CanonicalArtifact, CanonicalArtifactType, DispositionActor,
    DispositionDecision, DispositionRecord, IdentityType, PromotionState, ScopeId, StableId,
};
use provenance_macros::verifies;

#[test]
#[verifies("rule_disposition_write_gate", examples)]
fn refuses_a_disposition_naming_a_proposal_that_was_never_created() {
    let (_dir, store, scope) = initialized_store();
    allow_actor(&store);

    let error = store
        .create_disposition(rejected_input(
            &scope,
            "disposition_ghost",
            "proposal_ghost",
        ))
        .unwrap_err()
        .to_string();

    assert!(error.contains("proposal does not exist"), "{error}");
    assert!(store.list_dispositions(&scope).unwrap().is_empty());
}

#[test]
#[verifies("rule_disposition_write_gate", examples)]
fn refuses_a_second_disposition_on_a_proposal_already_disposed() {
    let (_dir, store, scope) = initialized_store();
    allow_actor(&store);
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_overtime",
            "Overtime",
            PromotionState::Proposed,
        ))
        .unwrap();
    store
        .create_disposition(rejected_input(
            &scope,
            "disposition_first",
            "proposal_overtime",
        ))
        .unwrap();

    let error = store
        .create_disposition(rejected_input(
            &scope,
            "disposition_second",
            "proposal_overtime",
        ))
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("proposal already has an authoritative disposition"),
        "{error}"
    );
    assert_eq!(store.list_dispositions(&scope).unwrap().len(), 1);
}

// The gate's four conditions range over a finite domain: whether the proposal
// exists, what the scope already recorded about it, which decision the person
// made, who made it, whether an assertion exists, and which of the three
// canonical artifact cases the disposition names. Every point in that domain
// is tried here against an independently stated expectation. The identity and
// artifact axes are what an acceptance without an assertion turns on: a human
// who names a canonical artifact has ratified one, and the ratified artifact
// stands where the swarm's assertion would. The canonical artifact index is
// the real one, loaded from a store holding requirement `req_overtime`, so the
// existing and missing cases are decided by the same index the writer uses.
#[test]
#[verifies("rule_disposition_write_gate", exhaustion)]
fn admits_exactly_the_writes_the_four_conditions_allow() {
    let (_dir, store, scope) = seeded_requirement_store();
    let canonical_artifacts = store.canonical_artifact_index(&scope).unwrap();
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_overtime",
            "Overtime",
            PromotionState::Proposed,
        ))
        .unwrap();
    let proposals = store.list_proposal_definitions(&scope).unwrap();
    let assertion = AssertionRecord {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: AssertionId::new("assertion_overtime").unwrap(),
        proposal_id: StableId::new("proposal_overtime").unwrap(),
        synthesis_packet_id: StableId::new("synthesis_overtime").unwrap(),
        supporting_claim_ids: vec![StableId::new("claim_overtime").unwrap()],
    };

    for proposal_exists in [false, true] {
        for prior in all_priors() {
            for decision in all_decisions() {
                for identity in all_identities() {
                    for asserted in [false, true] {
                        for artifact in all_artifacts() {
                            let candidate = disposition_by(
                                &scope,
                                "disposition_candidate",
                                "proposal_overtime",
                                decision,
                                identity,
                                named_artifact(artifact),
                            );
                            let admitted = validate_disposition_write_gate(
                                &candidate,
                                if proposal_exists { &proposals } else { &[] },
                                if asserted {
                                    std::slice::from_ref(&assertion)
                                } else {
                                    &[]
                                },
                                &prior_dispositions(&scope, prior),
                                &canonical_artifacts,
                            )
                            .is_ok();

                            // Independent restatement of the decision. An
                            // acceptance rests on an assertion, unless a person
                            // ratified an artifact and named it, in which case
                            // it rests on that.
                            let ratified = identity == IdentityType::Human
                                && artifact != ArtifactCase::Unnamed;
                            let allowed = proposal_exists
                                && (decision != DispositionDecision::Accepted
                                    || asserted
                                    || ratified)
                                && artifact != ArtifactCase::Missing
                                && prior == PriorCase::Nothing;
                            assert_eq!(
                                admitted, allowed,
                                "proposal_exists={proposal_exists} prior={prior:?} \
                                 decision={decision:?} identity={identity:?} \
                                 asserted={asserted} artifact={artifact:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
#[verifies("rule_disposition_write_gate", examples)]
fn a_person_accepts_a_tournament_winner_without_a_swarm_assertion() {
    let (_dir, store, scope) = seeded_requirement_store();
    let canonical_artifacts = store.canonical_artifact_index(&scope).unwrap();
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_overtime",
            "Overtime",
            PromotionState::Proposed,
        ))
        .unwrap();
    let proposals = store.list_proposal_definitions(&scope).unwrap();
    let accepted = disposition_by(
        &scope,
        "disposition_winner",
        "proposal_overtime",
        DispositionDecision::Accepted,
        IdentityType::Human,
        named_artifact(ArtifactCase::Existing),
    );

    validate_disposition_write_gate(&accepted, &proposals, &[], &[], &canonical_artifacts).unwrap();
}

#[test]
#[verifies("rule_disposition_write_gate", examples)]
fn acceptance_without_an_assertion_still_needs_a_person_and_an_artifact() {
    let (_dir, store, scope) = seeded_requirement_store();
    let canonical_artifacts = store.canonical_artifact_index(&scope).unwrap();
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_overtime",
            "Overtime",
            PromotionState::Proposed,
        ))
        .unwrap();
    let proposals = store.list_proposal_definitions(&scope).unwrap();
    for (identity, artifact) in [
        // A person who ratified nothing has nothing to stand the acceptance on.
        (IdentityType::Human, ArtifactCase::Unnamed),
        // An agent or a service naming an artifact has not ratified it.
        (IdentityType::Agent, ArtifactCase::Existing),
        (IdentityType::Service, ArtifactCase::Existing),
    ] {
        let accepted = disposition_by(
            &scope,
            "disposition_candidate",
            "proposal_overtime",
            DispositionDecision::Accepted,
            identity,
            named_artifact(artifact),
        );

        let error =
            validate_disposition_write_gate(&accepted, &proposals, &[], &[], &canonical_artifacts)
                .unwrap_err()
                .to_string();

        assert_eq!(
            error, "accepted proposal must be asserted before disposition",
            "{identity:?} naming {artifact:?}"
        );
    }
}

// What the scope already recorded about the proposal being disposed of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PriorCase {
    Nothing,
    SameProposal,
    SameDispositionId,
}

// Which canonical artifact the disposition names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtifactCase {
    Unnamed,
    Existing,
    Missing,
}

// The variant lists walk exhaustive matches, so a new DispositionDecision or
// case fails compilation until it joins the chain and the domain above stays
// complete.
fn all_decisions() -> Vec<DispositionDecision> {
    let mut all = vec![DispositionDecision::Accepted];
    while let Some(next) = match all.last().unwrap() {
        DispositionDecision::Accepted => Some(DispositionDecision::Rejected),
        DispositionDecision::Rejected => Some(DispositionDecision::Deferred),
        DispositionDecision::Deferred => None,
    } {
        all.push(next);
    }
    all
}

fn all_identities() -> Vec<IdentityType> {
    let mut all = vec![IdentityType::Human];
    while let Some(next) = match all.last().unwrap() {
        IdentityType::Human => Some(IdentityType::Agent),
        IdentityType::Agent => Some(IdentityType::Service),
        IdentityType::Service => None,
    } {
        all.push(next);
    }
    all
}

fn all_priors() -> Vec<PriorCase> {
    let mut all = vec![PriorCase::Nothing];
    while let Some(next) = match all.last().unwrap() {
        PriorCase::Nothing => Some(PriorCase::SameProposal),
        PriorCase::SameProposal => Some(PriorCase::SameDispositionId),
        PriorCase::SameDispositionId => None,
    } {
        all.push(next);
    }
    all
}

fn all_artifacts() -> Vec<ArtifactCase> {
    let mut all = vec![ArtifactCase::Unnamed];
    while let Some(next) = match all.last().unwrap() {
        ArtifactCase::Unnamed => Some(ArtifactCase::Existing),
        ArtifactCase::Existing => Some(ArtifactCase::Missing),
        ArtifactCase::Missing => None,
    } {
        all.push(next);
    }
    all
}

fn named_artifact(case: ArtifactCase) -> Option<CanonicalArtifact> {
    let artifact_id = match case {
        ArtifactCase::Unnamed => return None,
        ArtifactCase::Existing => "req_overtime",
        ArtifactCase::Missing => "req_missing",
    };
    Some(CanonicalArtifact {
        artifact_type: CanonicalArtifactType::Requirement,
        artifact_id: StableId::new(artifact_id).unwrap(),
    })
}

fn prior_dispositions(scope: &ScopeId, case: PriorCase) -> Vec<DispositionRecord> {
    let (id, proposal_id) = match case {
        PriorCase::Nothing => return Vec::new(),
        PriorCase::SameProposal => ("disposition_prior", "proposal_overtime"),
        PriorCase::SameDispositionId => ("disposition_candidate", "proposal_other"),
    };
    vec![disposition(
        scope,
        id,
        proposal_id,
        DispositionDecision::Rejected,
        None,
    )]
}

fn disposition(
    scope: &ScopeId,
    id: &str,
    proposal_id: &str,
    decision: DispositionDecision,
    canonical_artifact: Option<CanonicalArtifact>,
) -> DispositionRecord {
    disposition_by(
        scope,
        id,
        proposal_id,
        decision,
        IdentityType::Human,
        canonical_artifact,
    )
}

fn disposition_by(
    scope: &ScopeId,
    id: &str,
    proposal_id: &str,
    decision: DispositionDecision,
    identity_type: IdentityType,
    canonical_artifact: Option<CanonicalArtifact>,
) -> DispositionRecord {
    DispositionRecord {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        proposal_id: StableId::new(proposal_id).unwrap(),
        decision,
        rationale: "Reviewed".into(),
        actor: DispositionActor {
            identity_type,
            id: "reviewer".into(),
            name: None,
        },
        canonical_artifact,
        external_action: None,
    }
}

fn rejected_input(scope: &ScopeId, id: &str, proposal_id: &str) -> CreateDispositionInput {
    CreateDispositionInput {
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        proposal_id: StableId::new(proposal_id).unwrap(),
        decision: DispositionDecision::Rejected,
        rationale: "Reviewed".into(),
        actor: DispositionActor {
            identity_type: IdentityType::Human,
            id: "reviewer".into(),
            name: None,
        },
        canonical_artifact: None,
        external_action: None,
    }
}

fn allow_actor(store: &StateStore) {
    let mut manifest = store.manifest().unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}
