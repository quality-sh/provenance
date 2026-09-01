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
    // edge speak for the embedded source_refs entry.
    assert_eq!(
        labels,
        [
            ("references", RelationDirection::In, "source_award".into()),
            (
                "boundary_constrains",
                RelationDirection::In,
                "boundary_no_backpay".to_string()
            ),
            ("topic_shapes", RelationDirection::In, "topic_rates".into()),
            (
                "question_refines",
                RelationDirection::In,
                "question_threshold".to_string()
            ),
            (
                "requirement_in_domain",
                RelationDirection::Out,
                "domain_payroll".to_string()
            ),
        ]
    );
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
}
