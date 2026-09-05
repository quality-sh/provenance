mod relation_traversal {
use super::relation_fixtures::{fixture, front, sid};
use crate::model::relations::{
    flow_neighbors, related_nodes, RelatedNode, RelationDirection, RelationEndpoint, LINKS,
};
use crate::model::NodeType;

type Reached = Vec<(&'static str, RelationDirection, NodeType, String)>;

fn flatten(nodes: Vec<RelatedNode>) -> Reached {
    nodes
        .into_iter()
        .map(|node| {
            (
                node.relation,
                node.direction,
                node.endpoint.node_type,
                node.endpoint.id.as_str().to_string(),
            )
        })
        .collect()
}

fn out(relation: &'static str, node_type: NodeType, id: &str) -> (&'static str, RelationDirection, NodeType, String) {
    (relation, RelationDirection::Out, node_type, id.to_string())
}

fn inbound(relation: &'static str, node_type: NodeType, id: &str) -> (&'static str, RelationDirection, NodeType, String) {
    (relation, RelationDirection::In, node_type, id.to_string())
}

#[test]
fn a_single_field_reads_out_from_the_owner_and_in_from_the_target() {
    let records = fixture();
    let front = front(&records);
    let from_child = flatten(related_nodes(&front, NodeType::Requirement, &sid("req_penalty")));
    assert!(from_child.contains(&out("refines", NodeType::Requirement, "req_overtime")));
    assert!(from_child.contains(&out("depends_on", NodeType::Requirement, "req_overtime")));
    let from_parent = flatten(related_nodes(&front, NodeType::Requirement, &sid("req_overtime")));
    assert!(from_parent.contains(&inbound("refines", NodeType::Requirement, "req_penalty")));
    assert!(from_parent.contains(&inbound("depends_on", NodeType::Requirement, "req_penalty")));
}

#[test]
fn the_hub_lists_every_neighbour_in_rank_id_declaration_direction_order() {
    let records = fixture();
    let front = front(&records);
    let reached = flatten(related_nodes(&front, NodeType::Requirement, &sid("req_overtime")));
    assert_eq!(
        reached,
        vec![
            out("cites", NodeType::Source, "source_award"),
            inbound("refines", NodeType::Requirement, "req_penalty"),
            inbound("depends_on", NodeType::Requirement, "req_penalty"),
            out("supersedes", NodeType::Requirement, "req_penalty"),
            out("spawned_by", NodeType::Resolution, "res_threshold"),
            inbound("requirement_ids", NodeType::Resolution, "res_threshold"),
            inbound("requirement_ids", NodeType::Resolution, "res_threshold_draft"),
            inbound("requirement_ids", NodeType::Rule, "rule_pay"),
            inbound("requirement_id", NodeType::Topic, "topic_rates"),
            inbound("requirement_id", NodeType::Question, "question_threshold"),
            out("domain_id", NodeType::Domain, "domain_payroll"),
            inbound("requirement_id", NodeType::Boundary, "boundary_no_backpay"),
        ]
    );
}

#[test]
fn a_source_sees_every_citation_and_its_own_supersession() {
    let records = fixture();
    let front = front(&records);
    let reached = flatten(related_nodes(&front, NodeType::Source, &sid("source_award")));
    assert_eq!(
        reached,
        vec![
            out("supersedes", NodeType::Source, "source_award_2019"),
            inbound("cites", NodeType::Requirement, "req_overtime"),
            inbound(LINKS, NodeType::Question, "question_threshold"),
            inbound("cites", NodeType::Boundary, "boundary_no_backpay"),
        ]
    );
    let superseded = flatten(related_nodes(&front, NodeType::Source, &sid("source_award_2019")));
    assert_eq!(superseded, vec![inbound("supersedes", NodeType::Source, "source_award")]);
}

#[test]
fn links_are_walked_both_ways_under_their_own_name() {
    let records = fixture();
    let front = front(&records);
    let from_rule = flatten(related_nodes(&front, NodeType::Rule, &sid("rule_pay")));
    assert!(from_rule.contains(&inbound(LINKS, NodeType::Topic, "topic_rates")));
    let from_topic = flatten(related_nodes(&front, NodeType::Topic, &sid("topic_rates")));
    assert_eq!(
        from_topic,
        vec![
            out("requirement_id", NodeType::Requirement, "req_overtime"),
            out(LINKS, NodeType::Rule, "rule_pay"),
            inbound("topic_id", NodeType::Question, "question_threshold"),
        ]
    );
}

#[test]
fn a_question_reads_its_contradiction_and_resolution_outward() {
    let records = fixture();
    let front = front(&records);
    let reached = flatten(related_nodes(&front, NodeType::Question, &sid("question_threshold")));
    assert_eq!(
        reached,
        vec![
            out(LINKS, NodeType::Source, "source_award"),
            out("requirement_id", NodeType::Requirement, "req_overtime"),
            out("contradicts", NodeType::Requirement, "req_penalty"),
            out("resolution_id", NodeType::Resolution, "res_threshold"),
            out("topic_id", NodeType::Topic, "topic_rates"),
        ]
    );
    let contradicted = flatten(related_nodes(&front, NodeType::Requirement, &sid("req_penalty")));
    assert!(contradicted.contains(&inbound("contradicts", NodeType::Question, "question_threshold")));
}

#[test]
fn a_domain_only_ever_appears_as_a_target() {
    let records = fixture();
    let front = front(&records);
    let reached = flatten(related_nodes(&front, NodeType::Domain, &sid("domain_payroll")));
    assert_eq!(reached, vec![inbound("domain_id", NodeType::Requirement, "req_overtime")]);
}

#[test]
fn an_unknown_record_reaches_nothing() {
    let records = fixture();
    let front = front(&records);
    assert!(related_nodes(&front, NodeType::Rule, &sid("rule_missing")).is_empty());
}

#[test]
fn flow_follows_each_declared_flow_and_skips_none_relations() {
    let records = fixture();
    let front = front(&records);
    let downstream = flatten(flow_neighbors(&front, NodeType::Source, &sid("source_award"), true));
    assert_eq!(
        downstream,
        vec![
            out("supersedes", NodeType::Source, "source_award_2019"),
            inbound("cites", NodeType::Requirement, "req_overtime"),
        ]
    );
    let upstream = flatten(flow_neighbors(&front, NodeType::Rule, &sid("rule_pay"), false));
    assert_eq!(
        upstream,
        vec![
            out("requirement_ids", NodeType::Requirement, "req_overtime"),
            out("resolution_ids", NodeType::Resolution, "res_threshold"),
        ]
    );
    let requirement_downstream = flatten(flow_neighbors(
        &front,
        NodeType::Requirement,
        &sid("req_overtime"),
        true,
    ));
    assert_eq!(
        requirement_downstream,
        vec![
            inbound("refines", NodeType::Requirement, "req_penalty"),
            out("supersedes", NodeType::Requirement, "req_penalty"),
            inbound("requirement_ids", NodeType::Resolution, "res_threshold"),
            inbound("requirement_ids", NodeType::Resolution, "res_threshold_draft"),
            inbound("requirement_ids", NodeType::Rule, "rule_pay"),
        ]
    );
    let requirement_upstream = flatten(flow_neighbors(
        &front,
        NodeType::Requirement,
        &sid("req_overtime"),
        false,
    ));
    assert_eq!(
        requirement_upstream,
        vec![
            out("cites", NodeType::Source, "source_award"),
            inbound("depends_on", NodeType::Requirement, "req_penalty"),
            out("spawned_by", NodeType::Resolution, "res_threshold"),
        ]
    );
}

#[test]
fn a_dangling_reference_is_still_reported_as_stored() {
    let mut records = fixture();
    records.requirements[1].refines = Some(sid("req_vanished"));
    let front = front(&records);
    let reached = flatten(related_nodes(&front, NodeType::Requirement, &sid("req_penalty")));
    assert!(reached.contains(&out("refines", NodeType::Requirement, "req_vanished")));
    let endpoint = RelationEndpoint {
        node_type: NodeType::Requirement,
        id: sid("req_vanished"),
    };
    assert!(related_nodes(&front, endpoint.node_type, &endpoint.id)
        .iter()
        .any(|node| node.relation == "refines" && node.direction == RelationDirection::In));
}

/// A requirement citing one source under two clauses stores two
/// references and answers one `cites` neighbour each way: one neighbour
/// per (relation, direction, endpoint).
#[test]
fn a_source_cited_under_two_clauses_is_one_neighbour() {
    let mut records = fixture();
    records.requirements[0].source_refs.push(crate::model::SourceReference {
        source_id: sid("source_award"),
        clause: Some("clause 2".into()),
    });
    let front = front(&records);
    let from_requirement = flatten(related_nodes(&front, NodeType::Requirement, &sid("req_overtime")));
    assert_eq!(
        from_requirement.iter().filter(|node| **node == out("cites", NodeType::Source, "source_award")).count(),
        1,
        "{from_requirement:?}"
    );
    let from_source = flatten(related_nodes(&front, NodeType::Source, &sid("source_award")));
    assert_eq!(
        from_source.iter().filter(|node| **node == inbound("cites", NodeType::Requirement, "req_overtime")).count(),
        1,
        "{from_source:?}"
    );
    let upstream = flatten(flow_neighbors(&front, NodeType::Requirement, &sid("req_overtime"), false));
    assert_eq!(
        upstream.iter().filter(|node| **node == out("cites", NodeType::Source, "source_award")).count(),
        1,
        "{upstream:?}"
    );
}
}
