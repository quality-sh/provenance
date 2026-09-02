mod relation_traversal {
use crate::model::relations::{related_nodes, RelationDirection, RelationKind, RelationSource};
use crate::model::{ArtifactLinkTargetType, EdgeType, NodeType, StableId};

use super::relation_fixtures::*;


#[test]
fn an_edge_row_relation_walks_out_and_back() {
    let records = fixture();
    assert_eq!(
        related(
            &records,
            RelationKind::References,
            NodeType::Source,
            "source_award",
            RelationDirection::Out,
        ),
        [(NodeType::Requirement, "req_overtime".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::References,
            NodeType::Requirement,
            "req_overtime",
            RelationDirection::In,
        ),
        [(NodeType::Source, "source_award".to_string())]
    );
}

#[test]
fn a_foreign_key_relation_walks_out_and_scans_back() {
    let records = fixture();
    assert_eq!(
        related(
            &records,
            RelationKind::RequirementInDomain,
            NodeType::Requirement,
            "req_overtime",
            RelationDirection::Out,
        ),
        [(NodeType::Domain, "domain_payroll".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::RequirementInDomain,
            NodeType::Domain,
            "domain_payroll",
            RelationDirection::In,
        ),
        [(NodeType::Requirement, "req_overtime".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::BoundaryConstrains,
            NodeType::Requirement,
            "req_overtime",
            RelationDirection::In,
        ),
        [(NodeType::Boundary, "boundary_no_backpay".to_string())]
    );
}

#[test]
fn an_embedded_collection_relation_walks_both_directions() {
    let records = fixture();
    assert_eq!(
        related(
            &records,
            RelationKind::RequirementCitesSource,
            NodeType::Requirement,
            "req_overtime",
            RelationDirection::Out,
        ),
        [(NodeType::Source, "source_award".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::RequirementCitesSource,
            NodeType::Source,
            "source_award",
            RelationDirection::In,
        ),
        [(NodeType::Requirement, "req_overtime".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::TopicLinks,
            NodeType::Rule,
            "rule_pay",
            RelationDirection::In,
        ),
        [(NodeType::Topic, "topic_rates".to_string())]
    );
}

#[test]
fn the_core_walks_every_declared_relation_around_one_node() {
    let records = fixture();
    let reached = related_nodes(
        &front(&records),
        NodeType::Requirement,
        &sid("req_overtime"),
    );
    let labels: Vec<(&str, RelationDirection, String)> = reached
        .iter()
        .map(|related| {
            (
                related.relation.name(),
                related.direction,
                related.endpoint.id.as_str().to_string(),
            )
        })
        .collect();
    // The citation appears once: the declared duality lets the References
    // edge speak for the embedded source_refs entry. Every other declared
    // relation that touches the requirement appears exactly once, in
    // vocabulary order, out before in.
    let expected: Vec<(&str, RelationDirection, String)> = [
        ("references", RelationDirection::In, "source_award"),
        ("refines_into", RelationDirection::Out, "req_penalty"),
        ("depends_on", RelationDirection::Out, "req_penalty"),
        ("contradicts", RelationDirection::Out, "req_penalty"),
        ("supersedes", RelationDirection::Out, "req_penalty"),
        ("needs", RelationDirection::Out, "res_threshold"),
        ("resolves", RelationDirection::In, "res_threshold"),
        ("spawns", RelationDirection::In, "res_threshold"),
        ("produces", RelationDirection::Out, "rule_pay"),
        ("boundary_constrains", RelationDirection::In, "boundary_no_backpay"),
        ("topic_shapes", RelationDirection::In, "topic_rates"),
        ("question_refines", RelationDirection::In, "question_threshold"),
        ("requirement_in_domain", RelationDirection::Out, "domain_payroll"),
    ]
    .into_iter()
    .map(|(name, direction, id)| (name, direction, id.to_string()))
    .collect();
    assert_eq!(labels, expected);
}

#[test]
fn every_question_relation_walks_its_own_field() {
    let records = fixture();
    assert_eq!(
        related(
            &records,
            RelationKind::QuestionBelongsToTopic,
            NodeType::Question,
            "question_threshold",
            RelationDirection::Out,
        ),
        [(NodeType::Topic, "topic_rates".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::QuestionRefines,
            NodeType::Question,
            "question_threshold",
            RelationDirection::Out,
        ),
        [(NodeType::Requirement, "req_overtime".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::QuestionSettledBy,
            NodeType::Question,
            "question_threshold",
            RelationDirection::Out,
        ),
        [(NodeType::Resolution, "res_threshold".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::QuestionLinks,
            NodeType::Question,
            "question_threshold",
            RelationDirection::Out,
        ),
        [(NodeType::Source, "source_award".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::QuestionSettledBy,
            NodeType::Resolution,
            "res_threshold",
            RelationDirection::In,
        ),
        [(NodeType::Question, "question_threshold".to_string())]
    );
}

#[test]
fn a_stored_edge_with_an_illegal_endpoint_does_not_traverse() {
    let mut records = fixture();
    records.edges.push(edge(
        "edge_bad",
        EdgeType::References,
        (NodeType::Source, "source_award"),
        (NodeType::Rule, "rule_pay"),
    ));
    let out = related(
        &records,
        RelationKind::References,
        NodeType::Source,
        "source_award",
        RelationDirection::Out,
    );
    assert_eq!(
        out,
        [(NodeType::Requirement, "req_overtime".to_string())],
        "an endpoint outside the declared set must not be presented as this relation"
    );
}

#[test]
fn one_standard_path_citation_yields_one_source_neighbor() {
    // The standard write path records a citation twice: the embedded
    // source_refs entry and the References edge. The fixture mirrors that.
    let records = fixture();
    let reached = related_nodes(
        &front(&records),
        NodeType::Requirement,
        &sid("req_overtime"),
    );
    let citations = reached
        .iter()
        .filter(|related| related.endpoint.id.as_str() == "source_award")
        .count();
    assert_eq!(citations, 1, "one fact must not be presented twice: {reached:?}");
}

#[test]
fn a_front_answers_in_rank_then_id_order_regardless_of_storage_order() {
    let mut records = fixture();
    records.boundaries = vec![
        boundary("boundary_zz", "req_overtime"),
        boundary("boundary_aa", "req_overtime"),
    ];
    assert_eq!(
        related(
            &records,
            RelationKind::BoundaryConstrains,
            NodeType::Requirement,
            "req_overtime",
            RelationDirection::In,
        ),
        [
            (NodeType::Boundary, "boundary_aa".to_string()),
            (NodeType::Boundary, "boundary_zz".to_string()),
        ],
        "reverse scans answer in canonical id order, not storage order"
    );
    records.topics = vec![topic(
        "topic_mixed",
        "req_overtime",
        &[
            ("rule_pay", ArtifactLinkTargetType::Rule),
            ("source_award", ArtifactLinkTargetType::Source),
        ],
    )];
    assert_eq!(
        related(
            &records,
            RelationKind::TopicLinks,
            NodeType::Topic,
            "topic_mixed",
            RelationDirection::Out,
        ),
        [
            (NodeType::Source, "source_award".to_string()),
            (NodeType::Rule, "rule_pay".to_string()),
        ],
        "mixed-kind link lists answer in rank then id order"
    );
}

/// Records every ask so the executor's legality filter is visible.
struct ProbeSource(std::cell::RefCell<Vec<(RelationKind, NodeType, RelationDirection)>>);

impl RelationSource for ProbeSource {
    fn related(
        &self,
        relation: RelationKind,
        node_type: NodeType,
        _id: &StableId,
        direction: RelationDirection,
    ) -> Vec<crate::model::relations::RelationEndpoint> {
        self.0.borrow_mut().push((relation, node_type, direction));
        Vec::new()
    }
}

#[test]
fn the_executor_owns_iteration_legality_and_asks_nothing_illegal() {
    let probe = ProbeSource(std::cell::RefCell::new(Vec::new()));
    related_nodes(&probe, NodeType::Rule, &sid("rule_pay"));
    let asks = probe.0.into_inner();
    assert!(!asks.is_empty(), "the executor must ask something for a rule");
    for (relation, node_type, direction) in asks {
        let legal = match direction {
            RelationDirection::Out => relation.from_types().contains(&node_type),
            RelationDirection::In => relation.to_types().contains(&node_type),
        };
        assert!(
            legal,
            "illegal ask reached the seam: {relation:?} {direction:?}"
        );
    }
}

#[test]
fn a_front_answers_an_illegal_direct_ask_with_empty() {
    // Direct callers can bypass the executor; the front stays a total
    // function and answers empty rather than guessing.
    let records = fixture();
    assert!(front(&records)
        .related(
            RelationKind::References,
            NodeType::Rule,
            &sid("rule_pay"),
            RelationDirection::Out,
        )
        .is_empty());
}

fn all_nodes(records: &Fixture) -> Vec<(NodeType, StableId)> {
    let mut nodes = Vec::new();
    nodes.extend(records.sources.iter().map(|r| (NodeType::Source, r.id.clone())));
    nodes.extend(records.requirements.iter().map(|r| (NodeType::Requirement, r.id.clone())));
    nodes.extend(records.resolutions.iter().map(|r| (NodeType::Resolution, r.id.clone())));
    nodes.extend(records.rules.iter().map(|r| (NodeType::Rule, r.id.clone())));
    nodes.extend(records.topics.iter().map(|r| (NodeType::Topic, r.id.clone())));
    nodes.extend(records.questions.iter().map(|r| (NodeType::Question, r.id.clone())));
    nodes.extend(records.domains.iter().map(|r| (NodeType::Domain, r.id.clone())));
    nodes.extend(records.boundaries.iter().map(|r| (NodeType::Boundary, r.id.clone())));
    nodes
}

#[test]
fn every_declared_relation_traverses_somewhere_in_the_shared_corpus() {
    // Structural obligation on the fixture: a relation nobody can walk
    // here is a relation nobody has tested. Adding a kind without a
    // corpus row fails this, in both directions.
    let records = fixture();
    let front = front(&records);
    let nodes = all_nodes(&records);
    let mut untraversed = Vec::new();
    for relation in RelationKind::ALL {
        for direction in [RelationDirection::Out, RelationDirection::In] {
            let walked = nodes.iter().any(|(node_type, id)| {
                !front.related(relation, *node_type, id, direction).is_empty()
            });
            if !walked {
                untraversed.push((relation.name(), direction));
            }
        }
    }
    assert!(
        untraversed.is_empty(),
        "relations with no exercised traversal path: {untraversed:?}"
    );
}

#[test]
fn a_superseded_source_walks_to_its_successor_and_back() {
    let records = fixture();
    assert_eq!(
        related(&records, RelationKind::SourceSupersededBy, NodeType::Source, "source_award_2019", RelationDirection::Out),
        [(NodeType::Source, "source_award".to_string())]
    );
    assert_eq!(
        related(&records, RelationKind::SourceSupersededBy, NodeType::Source, "source_award", RelationDirection::In),
        [(NodeType::Source, "source_award_2019".to_string())]
    );
}

#[test]
fn a_superseded_resolution_walks_to_its_successor_and_back() {
    let records = fixture();
    assert_eq!(
        related(&records, RelationKind::ResolutionSupersededBy, NodeType::Resolution, "res_threshold_draft", RelationDirection::Out),
        [(NodeType::Resolution, "res_threshold".to_string())]
    );
    assert_eq!(
        related(&records, RelationKind::ResolutionSupersededBy, NodeType::Resolution, "res_threshold", RelationDirection::In),
        [(NodeType::Resolution, "res_threshold_draft".to_string())]
    );
}

#[test]
fn a_boundary_cites_its_source_and_the_source_scans_back() {
    let records = fixture();
    assert_eq!(
        related(&records, RelationKind::BoundaryCitesSource, NodeType::Boundary, "boundary_no_backpay", RelationDirection::Out),
        [(NodeType::Source, "source_award".to_string())]
    );
    assert_eq!(
        related(&records, RelationKind::BoundaryCitesSource, NodeType::Source, "source_award", RelationDirection::In),
        [(NodeType::Boundary, "boundary_no_backpay".to_string())]
    );
}
}
