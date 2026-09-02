use super::graph::{EdgeType, NodeType};

#[test]
fn domain_and_boundary_are_node_types_with_stable_wire_names() {
    for (name, expected) in [
        ("domain", NodeType::Domain),
        ("boundary", NodeType::Boundary),
    ] {
        assert_eq!(NodeType::parse(name).unwrap(), expected);
        assert_eq!(serde_json::to_value(expected).unwrap(), name);
        assert_eq!(
            serde_json::from_value::<NodeType>(serde_json::Value::String(name.into())).unwrap(),
            expected
        );
    }
}

#[test]
fn no_edge_may_touch_a_domain_or_boundary() {
    for kind in [NodeType::Domain, NodeType::Boundary] {
        assert!(crate::edge_validation::validate_edge_endpoint(
            EdgeType::References,
            NodeType::Source,
            kind,
        )
        .is_err());
        assert!(crate::edge_validation::validate_edge_endpoint(
            EdgeType::RefinesInto,
            kind,
            NodeType::Requirement,
        )
        .is_err());
    }
}

#[test]
fn a_domain_node_searches_by_name_and_description() {
    let domain = crate::model::Domain {
        schema_version: crate::model::SchemaVersion(1),
        scope_id: crate::model::ScopeId::new("default").unwrap(),
        id: crate::model::StableId::new("domain_payroll").unwrap(),
        name: "Payroll".into(),
        description: Some("Wages and awards".into()),
        color: None,
    };
    let node = crate::protocol::GraphNode::Domain(Box::new(domain));
    let text = node.searchable_text();
    assert!(text.contains(&"Payroll"));
    assert!(
        text.contains(&"Wages and awards"),
        "the description must be searchable: {text:?}"
    );
}

#[test]
fn topic_and_question_are_thread_parent_node_types_but_not_edge_endpoints() {
    assert_eq!(NodeType::parse("topic").unwrap(), NodeType::Topic);
    assert_eq!(NodeType::parse("question").unwrap(), NodeType::Question);
    assert!(crate::edge_validation::validate_edge_endpoint(
        EdgeType::DependsOn,
        NodeType::Topic,
        NodeType::Topic,
    )
    .is_err());
}
