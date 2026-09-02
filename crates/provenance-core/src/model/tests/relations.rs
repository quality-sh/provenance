use super::relations::{RelationDerivation, RelationKind};

// The chain is derived from an exhaustive match, so adding a RelationKind
// variant fails compilation until the new variant joins the chain, keeping
// the completeness assertions below honest.
fn all_relation_kinds() -> Vec<RelationKind> {
    let mut all = vec![RelationKind::References];
    while let Some(next) = match all.last().unwrap() {
        RelationKind::References => Some(RelationKind::RefinesInto),
        RelationKind::RefinesInto => Some(RelationKind::DependsOn),
        RelationKind::DependsOn => Some(RelationKind::Contradicts),
        RelationKind::Contradicts => Some(RelationKind::Supersedes),
        RelationKind::Supersedes => Some(RelationKind::Needs),
        RelationKind::Needs => Some(RelationKind::Resolves),
        RelationKind::Resolves => Some(RelationKind::Spawns),
        RelationKind::Spawns => Some(RelationKind::Produces),
        RelationKind::Produces => Some(RelationKind::BoundaryConstrains),
        RelationKind::BoundaryConstrains => Some(RelationKind::TopicShapes),
        RelationKind::TopicShapes => Some(RelationKind::QuestionBelongsToTopic),
        RelationKind::QuestionBelongsToTopic => Some(RelationKind::QuestionRefines),
        RelationKind::QuestionRefines => Some(RelationKind::QuestionSettledBy),
        RelationKind::QuestionSettledBy => Some(RelationKind::RequirementInDomain),
        RelationKind::RequirementInDomain => Some(RelationKind::RequirementCitesSource),
        RelationKind::RequirementCitesSource => Some(RelationKind::TopicLinks),
        RelationKind::TopicLinks => Some(RelationKind::QuestionLinks),
        RelationKind::QuestionLinks => Some(RelationKind::SourceSupersededBy),
        RelationKind::SourceSupersededBy => Some(RelationKind::ResolutionSupersededBy),
        RelationKind::ResolutionSupersededBy => Some(RelationKind::BoundaryCitesSource),
        RelationKind::BoundaryCitesSource => None,
    } {
        all.push(next);
    }
    all
}

// Same chain trick for NodeType, local to this file: a new node kind fails
// compilation here until the endpoint sweep below covers it.
fn every_node_type() -> Vec<NodeType> {
    let mut all = vec![NodeType::Source];
    while let Some(next) = match all.last().unwrap() {
        NodeType::Source => Some(NodeType::Requirement),
        NodeType::Requirement => Some(NodeType::Resolution),
        NodeType::Resolution => Some(NodeType::Rule),
        NodeType::Rule => Some(NodeType::Topic),
        NodeType::Topic => Some(NodeType::Question),
        NodeType::Question => Some(NodeType::Domain),
        NodeType::Domain => Some(NodeType::Boundary),
        NodeType::Boundary => None,
    } {
        all.push(next);
    }
    all
}

#[test]
#[verifies("rule_prov_relation_vocabulary_closed", exhaustion)]
fn the_relation_vocabulary_is_closed_at_twenty_one_declared_kinds() {
    let all = all_relation_kinds();
    assert_eq!(all.len(), 21);
    assert_eq!(RelationKind::ALL.to_vec(), all);
    assert_eq!(super::relations::declared_relations(), all.as_slice());
    let mut names: Vec<&str> = all.iter().map(|kind| kind.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 21, "relation names must be unique");
}

#[test]
fn nine_edge_relations_cover_the_nine_edge_types_exactly_once() {
    let mut covered: Vec<EdgeType> = all_relation_kinds()
        .into_iter()
        .filter_map(RelationKind::edge_type)
        .collect();
    assert_eq!(covered.len(), 9);
    covered.dedup();
    assert_eq!(covered.len(), 9, "each edge type maps to one relation");
    for kind in all_relation_kinds() {
        assert_eq!(
            kind.edge_type().is_some(),
            kind.derivation() == RelationDerivation::EdgeRow,
            "{kind:?} edge mapping must match its derivation",
        );
    }
}

#[test]
#[verifies("rule_prov_relation_vocabulary_closed", exhaustion)]
fn edge_relation_endpoints_agree_with_the_edge_endpoint_table() {
    for kind in all_relation_kinds() {
        let Some(edge_type) = kind.edge_type() else {
            continue;
        };
        for &from in &every_node_type() {
            for &to in &every_node_type() {
                let declared = kind.from_types().contains(&from) && kind.to_types().contains(&to);
                let legal =
                    crate::edge_validation::validate_edge_endpoint(edge_type, from, to).is_ok();
                assert_eq!(
                    declared, legal,
                    "{kind:?} declares {from:?} -> {to:?} = {declared}, table says {legal}"
                );
            }
        }
    }
}

#[test]
#[verifies("rule_prov_relation_vocabulary_closed", exhaustion)]
fn every_foreign_key_field_maps_to_one_declared_relation() {
    let declared: Vec<(RelationKind, NodeType, &[NodeType])> = all_relation_kinds()
        .into_iter()
        .filter(|kind| kind.derivation() == RelationDerivation::FkField)
        .map(|kind| (kind, kind.from_types()[0], kind.to_types()))
        .collect();
    assert_eq!(
        declared.len(),
        9,
        "nine single-target reference fields exist in the data model"
    );
    let expect = [
        (
            RelationKind::BoundaryConstrains,
            NodeType::Boundary,
            NodeType::Requirement,
        ),
        (
            RelationKind::TopicShapes,
            NodeType::Topic,
            NodeType::Requirement,
        ),
        (
            RelationKind::QuestionBelongsToTopic,
            NodeType::Question,
            NodeType::Topic,
        ),
        (
            RelationKind::QuestionRefines,
            NodeType::Question,
            NodeType::Requirement,
        ),
        (
            RelationKind::QuestionSettledBy,
            NodeType::Question,
            NodeType::Resolution,
        ),
        (
            RelationKind::RequirementInDomain,
            NodeType::Requirement,
            NodeType::Domain,
        ),
        (
            RelationKind::SourceSupersededBy,
            NodeType::Source,
            NodeType::Source,
        ),
        (
            RelationKind::ResolutionSupersededBy,
            NodeType::Resolution,
            NodeType::Resolution,
        ),
        (
            RelationKind::BoundaryCitesSource,
            NodeType::Boundary,
            NodeType::Source,
        ),
    ];
    for (kind, from, to) in expect {
        assert!(
            declared
                .iter()
                .any(|(declared_kind, declared_from, declared_to)| *declared_kind == kind
                    && *declared_from == from
                    && *declared_to == [to]),
            "missing FK relation {kind:?}: {from:?} -> {to:?}"
        );
    }
}

#[test]
fn embedded_collections_declare_their_target_sets() {
    assert_eq!(
        RelationKind::RequirementCitesSource.to_types(),
        [NodeType::Source]
    );
    for kind in [RelationKind::TopicLinks, RelationKind::QuestionLinks] {
        assert_eq!(kind.derivation(), RelationDerivation::EmbeddedCollection);
        assert_eq!(
            kind.to_types(),
            [
                NodeType::Source,
                NodeType::Requirement,
                NodeType::Resolution,
                NodeType::Rule
            ],
            "artifact links reach the four linkable kinds"
        );
    }
}

#[test]
fn the_citation_duality_is_declared_and_symmetric() {
    assert_eq!(
        RelationKind::References.same_fact_as(),
        Some(RelationKind::RequirementCitesSource)
    );
    assert_eq!(
        RelationKind::RequirementCitesSource.same_fact_as(),
        Some(RelationKind::References)
    );
    for kind in all_relation_kinds() {
        if let Some(partner) = kind.same_fact_as() {
            assert_eq!(
                partner.same_fact_as(),
                Some(kind),
                "{kind:?} duality must be symmetric"
            );
        }
    }
}

#[test]
fn node_type_rank_is_the_one_contract_ordering() {
    let ranks: Vec<u8> = every_node_type().iter().map(|kind| kind.rank()).collect();
    assert_eq!(ranks, [0, 1, 2, 3, 4, 5, 6, 7]);
}
