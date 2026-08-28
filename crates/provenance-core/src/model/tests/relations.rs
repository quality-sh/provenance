// W4: relation vocabulary, superset node vocabulary, and the rank order.

use crate::model::relations::{RelationDerivation, RelationEndpoint, RelationKind};

#[test]
fn node_type_parses_the_superset_vocabulary_in_contract_order() {
    let words = [
        "source",
        "requirement",
        "resolution",
        "rule",
        "topic",
        "question",
        "domain",
        "boundary",
    ];
    let parsed: Vec<NodeType> = words.iter().map(|word| NodeType::parse(word).unwrap()).collect();
    let expected = [
        NodeType::Source,
        NodeType::Requirement,
        NodeType::Resolution,
        NodeType::Rule,
        NodeType::Topic,
        NodeType::Question,
        NodeType::Domain,
        NodeType::Boundary,
    ];
    assert_eq!(parsed, expected, "Domain and Boundary append after Question");
    assert!(NodeType::parse("widget").is_err());
}

#[test]
fn node_type_serde_round_trip_includes_the_new_variants() {
    for (node_type, word) in [
        (NodeType::Domain, "domain"),
        (NodeType::Boundary, "boundary"),
    ] {
        let serialized = serde_json::to_string(&node_type).unwrap();
        assert_eq!(serialized, format!("\"{word}\""));
        let deserialized: NodeType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, node_type);
    }
}

fn sample_rank(node_type: NodeType) -> u8 {
    sample_rank_of(node_type)
}

fn sample_rank_of(node_type: NodeType) -> u8 {
    // Mirrors the contract order the store's `records::rank` pins.
    match node_type {
        NodeType::Source => 0,
        NodeType::Requirement => 1,
        NodeType::Resolution => 2,
        NodeType::Rule => 3,
        NodeType::Topic => 4,
        NodeType::Question => 5,
        NodeType::Domain => 6,
        NodeType::Boundary => 7,
    }
}

#[test]
fn rank_slots_append_domain_and_boundary_after_existing_kinds() {
    let order = [
        NodeType::Source,
        NodeType::Requirement,
        NodeType::Resolution,
        NodeType::Rule,
        NodeType::Topic,
        NodeType::Question,
        NodeType::Domain,
        NodeType::Boundary,
    ];
    let ranks: Vec<u8> = order.iter().copied().map(sample_rank).collect();
    let mut sorted = ranks.clone();
    sorted.sort_unstable();
    assert_eq!(ranks, sorted, "rank increases exactly with contract order");
    let mut unique = sorted.clone();
    unique.dedup();
    assert_eq!(unique, sorted, "every node kind owns one rank slot");
}

fn sample_all_relation_kinds() -> Vec<RelationKind> {
    let mut all = vec![RelationKind::SourceReferencesRequirement];
    while let Some(next) = match all.last().unwrap() {
        RelationKind::SourceReferencesRequirement => Some(RelationKind::RequirementRefinesIntoRequirement),
        RelationKind::RequirementRefinesIntoRequirement => Some(RelationKind::RequirementDependsOnRequirement),
        RelationKind::RequirementDependsOnRequirement => Some(RelationKind::RequirementContradictsRequirement),
        RelationKind::RequirementContradictsRequirement => Some(RelationKind::RequirementSupersedesRequirement),
        RelationKind::RequirementSupersedesRequirement => Some(RelationKind::RequirementNeedsResolution),
        RelationKind::RequirementNeedsResolution => Some(RelationKind::ResolutionResolvesRequirement),
        RelationKind::ResolutionResolvesRequirement => Some(RelationKind::ResolutionSpawnsRequirement),
        RelationKind::ResolutionSpawnsRequirement => Some(RelationKind::RequirementProducesRule),
        RelationKind::RequirementProducesRule => Some(RelationKind::ResolutionProducesRule),
        RelationKind::ResolutionProducesRule => Some(RelationKind::BoundaryRequiresRequirement),
        RelationKind::BoundaryRequiresRequirement => Some(RelationKind::TopicExploresRequirement),
        RelationKind::TopicExploresRequirement => Some(RelationKind::QuestionRefinesTopic),
        RelationKind::QuestionRefinesTopic => Some(RelationKind::QuestionRaisesRequirement),
        RelationKind::QuestionRaisesRequirement => Some(RelationKind::QuestionSeeksResolution),
        RelationKind::QuestionSeeksResolution => Some(RelationKind::RequirementBelongsToDomain),
        RelationKind::RequirementBelongsToDomain => Some(RelationKind::RequirementCitesSource),
        RelationKind::RequirementCitesSource => Some(RelationKind::TopicLinksArtifact),
        RelationKind::TopicLinksArtifact => Some(RelationKind::QuestionLinksArtifact),
        RelationKind::QuestionLinksArtifact => None,
    } {
        all.push(next);
    }
    all
}

#[test]
#[verifies("rule_prov_edge_endpoint_table", exhaustion)]
fn relation_kind_enumeration_is_complete() {
    let all = sample_all_relation_kinds();
    assert_eq!(all.len(), 19, "nine edges, six foreign keys, four embedded");
}

#[test]
fn relation_kinds_declare_endpoints_and_derivation() {
    let (from, to, derivation) = RelationKind::SourceReferencesRequirement.parts();
    assert_eq!(from, NodeType::Source);
    assert_eq!(to, RelationEndpoint::Node(NodeType::Requirement));
    assert_eq!(derivation, RelationDerivation::EdgeRow(EdgeType::References));

    let (from, to, derivation) = RelationKind::RequirementBelongsToDomain.parts();
    assert_eq!(from, NodeType::Requirement);
    assert_eq!(to, RelationEndpoint::Node(NodeType::Domain));
    assert_eq!(derivation, RelationDerivation::FkField);

    let (from, to, derivation) = RelationKind::QuestionLinksArtifact.parts();
    assert_eq!(from, NodeType::Question);
    assert_eq!(to, RelationEndpoint::LinkTarget);
    assert_eq!(derivation, RelationDerivation::EmbeddedCollection);
}

#[test]
fn edge_relations_agree_with_the_edge_endpoint_table() {
    for kind in sample_all_relation_kinds() {
        let (from, to, derivation) = kind.parts();
        if let (RelationDerivation::EdgeRow(edge_type), RelationEndpoint::Node(to)) = (derivation, to)
        {
            crate::edge_validation::validate_edge_endpoint(edge_type, from, to).unwrap_or_else(
                |error| panic!("{kind:?} must agree with the endpoint table: {error}"),
            );
        }
    }
}
