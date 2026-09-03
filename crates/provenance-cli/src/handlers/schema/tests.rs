use super::{
    common::model::{
        enum_names, ARTIFACT_CHANGE_TYPES, CONTRIBUTION_STANCES, EVIDENCE_QUALITIES,
        IDEATION_EVIDENCE_TYPES, IDEATION_TARGET_TYPES, PROMOTION_STATES, PROPOSAL_TYPES,
        SPECULATION_MARKERS, UNCERTAINTY_LEVELS,
    },
    schema_for,
};
use crate::cli::IdeationArtifactKind;
use provenance_core::{
    ArtifactChangeType, ContributionStance, EvidenceQuality, IdeationEvidenceType,
    IdeationTargetType, PromotionState, ProposalType, SpeculationMarker, UncertaintyLevel,
};
use provenance_macros::verifies;
use provenance_store::graph_reference::{graph_digest, GraphExport};
use serde_json::{json, Value};

fn enum_values_at(schema: &Value, pointer: &str) -> Vec<String> {
    schema
        .pointer(pointer)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect()
}

fn assert_ideation_target_type_array_is_exhaustive(value: IdeationTargetType) {
    match value {
        IdeationTargetType::Source
        | IdeationTargetType::Requirement
        | IdeationTargetType::Resolution
        | IdeationTargetType::Rule
        | IdeationTargetType::Topic
        | IdeationTargetType::Question
        | IdeationTargetType::Domain => {}
    }
}

fn assert_ideation_evidence_type_array_is_exhaustive(value: IdeationEvidenceType) {
    match value {
        IdeationEvidenceType::Source
        | IdeationEvidenceType::Artifact
        | IdeationEvidenceType::ThreadMessage
        | IdeationEvidenceType::DomainKnowledge
        | IdeationEvidenceType::Unsupported
        | IdeationEvidenceType::Exploratory => {}
    }
}

fn assert_artifact_change_type_array_is_exhaustive(value: ArtifactChangeType) {
    match value {
        ArtifactChangeType::Create
        | ArtifactChangeType::Update
        | ArtifactChangeType::Remove
        | ArtifactChangeType::None => {}
    }
}

fn assert_contribution_stance_array_is_exhaustive(value: ContributionStance) {
    match value {
        ContributionStance::Support
        | ContributionStance::Oppose
        | ContributionStance::Mixed
        | ContributionStance::NeedsMoreEvidence => {}
    }
}

fn assert_speculation_marker_array_is_exhaustive(value: SpeculationMarker) {
    match value {
        SpeculationMarker::Unsupported | SpeculationMarker::Exploratory => {}
    }
}

fn all_ideation_evidence_types() -> Vec<IdeationEvidenceType> {
    let mut all = vec![IdeationEvidenceType::Source];
    while let Some(next) = match all.last().unwrap() {
        IdeationEvidenceType::Source => Some(IdeationEvidenceType::Artifact),
        IdeationEvidenceType::Artifact => Some(IdeationEvidenceType::ThreadMessage),
        IdeationEvidenceType::ThreadMessage => Some(IdeationEvidenceType::DomainKnowledge),
        IdeationEvidenceType::DomainKnowledge => Some(IdeationEvidenceType::Unsupported),
        IdeationEvidenceType::Unsupported => Some(IdeationEvidenceType::Exploratory),
        IdeationEvidenceType::Exploratory => None,
    } {
        all.push(next);
    }
    all
}

fn all_speculation_markers() -> Vec<SpeculationMarker> {
    let mut all = vec![SpeculationMarker::Unsupported];
    while let Some(next) = match all.last().unwrap() {
        SpeculationMarker::Unsupported => Some(SpeculationMarker::Exploratory),
        SpeculationMarker::Exploratory => None,
    } {
        all.push(next);
    }
    all
}

fn assert_uncertainty_level_array_is_exhaustive(value: UncertaintyLevel) {
    match value {
        UncertaintyLevel::Low | UncertaintyLevel::Medium | UncertaintyLevel::High => {}
    }
}

fn assert_evidence_quality_array_is_exhaustive(value: EvidenceQuality) {
    match value {
        EvidenceQuality::Strong
        | EvidenceQuality::Mixed
        | EvidenceQuality::Weak
        | EvidenceQuality::Unsupported => {}
    }
}

fn assert_proposal_type_array_is_exhaustive(value: ProposalType) {
    match value {
        ProposalType::RequirementCandidate
        | ProposalType::ResolutionCandidate
        | ProposalType::RuleCandidate
        | ProposalType::SourceGap
        | ProposalType::Question
        | ProposalType::NoAction => {}
    }
}

fn assert_promotion_state_array_is_exhaustive(value: PromotionState) {
    match value {
        PromotionState::Proposed
        | PromotionState::Asserted
        | PromotionState::Accepted
        | PromotionState::Rejected
        | PromotionState::Deferred
        | PromotionState::Duplicate
        | PromotionState::Superseded => {}
    }
}

#[test]
fn schema_enum_variant_arrays_are_exhaustive() {
    for variant in IDEATION_TARGET_TYPES {
        assert_ideation_target_type_array_is_exhaustive(variant);
    }
    for variant in IDEATION_EVIDENCE_TYPES {
        assert_ideation_evidence_type_array_is_exhaustive(variant);
    }
    for variant in ARTIFACT_CHANGE_TYPES {
        assert_artifact_change_type_array_is_exhaustive(variant);
    }
    for variant in CONTRIBUTION_STANCES {
        assert_contribution_stance_array_is_exhaustive(variant);
    }
    for variant in SPECULATION_MARKERS {
        assert_speculation_marker_array_is_exhaustive(variant);
    }
    for variant in UNCERTAINTY_LEVELS {
        assert_uncertainty_level_array_is_exhaustive(variant);
    }
    for variant in EVIDENCE_QUALITIES {
        assert_evidence_quality_array_is_exhaustive(variant);
    }
    for variant in PROPOSAL_TYPES {
        assert_proposal_type_array_is_exhaustive(variant);
    }
    for variant in PROMOTION_STATES {
        assert_promotion_state_array_is_exhaustive(variant);
    }
}

#[test]
fn schema_show_enum_values_match_model_serialization() {
    let contribution = schema_for(IdeationArtifactKind::Contribution);
    let synthesis = schema_for(IdeationArtifactKind::SynthesisPacket);
    let proposal = schema_for(IdeationArtifactKind::Proposal);
    let target_types = enum_names(&IDEATION_TARGET_TYPES);
    let evidence_types = enum_names(&IDEATION_EVIDENCE_TYPES);
    let change_types = enum_names(&ARTIFACT_CHANGE_TYPES);
    let contribution_stances = enum_names(&CONTRIBUTION_STANCES);
    let speculation_markers = enum_names(&SPECULATION_MARKERS);
    let uncertainty_levels = enum_names(&UNCERTAINTY_LEVELS);
    let evidence_qualities = enum_names(&EVIDENCE_QUALITIES);
    let proposal_types = enum_names(&PROPOSAL_TYPES);

    assert_eq!(
        enum_values_at(
            &contribution,
            "/$defs/ideationTarget/properties/artifact_type/enum"
        ),
        target_types
    );
    assert_eq!(
        enum_values_at(
            &contribution,
            "/$defs/evidenceReference/properties/evidence_type/enum"
        ),
        evidence_types
    );
    assert_eq!(
        enum_values_at(
            &contribution,
            "/$defs/materialClaim/properties/evidence_type/enum"
        ),
        evidence_types
    );
    assert_eq!(
        enum_values_at(
            &contribution,
            "/$defs/suggestedArtifactChange/properties/change_type/enum"
        ),
        change_types
    );
    assert_eq!(
        enum_values_at(&contribution, "/schema/properties/stance/enum"),
        contribution_stances
    );
    assert_eq!(
        enum_values_at(
            &contribution,
            "/$defs/unsupportedRecommendation/properties/marker/enum"
        ),
        speculation_markers
    );
    assert_eq!(
        enum_values_at(&contribution, "/$defs/uncertainty/properties/level/enum"),
        uncertainty_levels
    );
    assert_eq!(
        enum_values_at(
            &synthesis,
            "/$defs/evidenceGap/properties/needed_evidence_type/enum"
        ),
        evidence_types
    );
    assert_eq!(
        enum_values_at(
            &synthesis,
            "/$defs/contestedClaim/properties/evidence_quality/enum"
        ),
        evidence_qualities
    );
    assert_eq!(
        enum_values_at(
            &synthesis,
            "/$defs/unsupportedSpeculation/properties/marker/enum"
        ),
        speculation_markers
    );
    assert_eq!(
        enum_values_at(
            &synthesis,
            "/$defs/suggestedArtifact/properties/proposal_type/enum"
        ),
        proposal_types
    );
    assert_eq!(
        enum_values_at(&proposal, "/schema/properties/proposal_type/enum"),
        proposal_types
    );
    assert_eq!(
        proposal.pointer("/schema/properties/promotion_state/const"),
        Some(&json!("proposed"))
    );
}

#[test]
#[verifies("rule_positive_evidence", conformance)]
fn schema_evidence_vocabularies_match_the_positive_evidence_domain() {
    let core_evidence_types = enum_names(&all_ideation_evidence_types());
    let schema_evidence_types = enum_names(&IDEATION_EVIDENCE_TYPES);
    assert_eq!(
        schema_evidence_types, core_evidence_types,
        "schema evidence types drifted from the core domain"
    );

    let core_speculation_markers = enum_names(&all_speculation_markers());
    let schema_speculation_markers = enum_names(&SPECULATION_MARKERS);
    assert_eq!(
        schema_speculation_markers, core_speculation_markers,
        "schema speculation markers drifted from the core domain"
    );

    let non_positive_evidence = enum_names(&[
        IdeationEvidenceType::Unsupported,
        IdeationEvidenceType::Exploratory,
    ]);
    assert_eq!(
        schema_speculation_markers, non_positive_evidence,
        "schema speculation markers must name exactly the non-positive evidence types"
    );
}

/// The digest a document must carry for `graph`, derived rather than written
/// down so the fixture cannot claim a hash its graph does not have.
fn derived_digest(graph: &Value) -> String {
    let graph: GraphExport =
        serde_json::from_value(graph.clone()).expect("the fixture graph is a pinned graph");
    graph_digest(&graph).expect("a graph can be canonicalized")
}

fn minimal_exact_export() -> Value {
    let mut document = minimal_exact_export_without_digest();
    document["graph_digest"] = json!(derived_digest(&document["graph"]));
    document
}

fn minimal_exact_export_without_digest() -> Value {
    json!({
        "schema_version": 1,
        "operation": "exact-export",
        "reference_id": format!("grf1_{}", "0".repeat(64)),
        "graph": {
            "schema_version": 1,
            "scope": {"id": "default", "path_prefix": "."},
            "sources": [{
                "schema_version": 1, "scope_id": "default", "id": "source_policy",
                "name": "Policy", "source_type": "policy", "url": null
            }],
            "domains": [],
            "requirements": [{
                "schema_version": 1, "scope_id": "default", "id": "req_policy",
                "statement": "Follow policy", "status": "active",
                "source_refs": [{"source_id": "source_policy", "clause": "1.1"}]
            }],
            "boundaries": [],
            "topics": [{
                "schema_version": 1, "scope_id": "default", "id": "topic_policy",
                "requirement_id": "req_policy", "title": "Policy details", "status": "open",
                "links": [{"target_type": "source", "target_id": "source_policy"}]
            }],
            "questions": [],
            "resolutions": [{
                "schema_version": 1, "scope_id": "default", "id": "resolution_policy",
                "title": "Apply policy", "position": "Apply it", "rationale": "Required",
                "status": "approved", "inputs": [{
                    "input_type": "source_material", "reference": "source_policy", "summary": "Policy"
                }], "review_on": null, "requirement_ids": ["req_policy"]
            }],
            "rules": []
        }
    })
}

#[test]
fn graph_reference_export_schema_validates_record_structure() {
    let shown = schema_for(IdeationArtifactKind::GraphReferenceExport);
    let schema = shown.get("schema").unwrap();
    let validator = jsonschema::JSONSchema::compile(schema).unwrap();
    let valid = minimal_exact_export();
    assert!(validator.is_valid(&valid));

    let mut with_implementation = minimal_exact_export_without_digest();
    with_implementation["graph"]["rules"] = json!([{
        "schema_version": 1, "scope_id": "default", "id": "rule_runtime",
        "statement": "Accepted workflows start", "status": "active", "severity": "medium",
        "requirement_ids": ["req_policy"]
    }]);
    with_implementation["graph"]["implementation_bindings"] = json!([{
        "schema_version": 1, "scope_id": "default",
        "id": "implementation_binding_runtime", "rule_id": "rule_runtime",
        "declared_by": "spec://typescript/workflows",
        "retired": true,
        "file": "src/runtime.ts", "symbol": "startWorkflow"
    }]);
    with_implementation["graph_digest"] = json!(derived_digest(&with_implementation["graph"]));
    assert!(validator.is_valid(&with_implementation));

    let malformed_cases = [
        // The digest is not optional: a document without it cannot be checked
        // against the graph it carries, and one that is not a sha256 digest
        // cannot be compared with the reference's.
        minimal_exact_export_without_digest(),
        {
            let mut value = minimal_exact_export();
            value["graph_digest"] = json!("0".repeat(64));
            value
        },
        {
            let mut value = minimal_exact_export();
            value["graph"]["sources"] = json!([{}]);
            value
        },
        {
            let mut value = minimal_exact_export();
            value["graph"]["sources"] = json!([{
                "schema_version": 2, "scope_id": "default", "id": "source_policy",
                "name": "Policy", "source_type": "policy", "url": null
            }]);
            value
        },
        {
            let mut value = minimal_exact_export();
            value["graph"]["sources"] = json!([{
                "schema_version": 1, "scope_id": "default", "id": "source_policy",
                "name": "Policy", "source_type": "policy", "url": null,
                "origin_thread": "thread_private"
            }]);
            value
        },
        {
            let mut value = minimal_exact_export();
            value["graph"]["sources"][0]["commit_pin"] = json!("not-a-commit");
            value
        },
        {
            let mut value = minimal_exact_export();
            value["graph"]["scope"]["workflow_id"] = json!("workflowd-123");
            value
        },
        {
            let mut value = with_implementation;
            value["graph"]["implementation_bindings"][0]["symbol"] = json!(null);
            value
        },
    ];
    for malformed in malformed_cases {
        assert!(
            !validator.is_valid(&malformed),
            "schema accepted {malformed}"
        );
    }
}
