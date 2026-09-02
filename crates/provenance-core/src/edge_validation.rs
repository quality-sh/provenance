use crate::{EdgeType, NodeType};
use provenance_macros::rule;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EdgeValidationError {
    #[error("invalid {edge_type:?} edge endpoint: {from:?} -> {to:?}")]
    InvalidEdgeEndpoint {
        edge_type: EdgeType,
        from: NodeType,
        to: NodeType,
    },
}

#[rule("rule_prov_edge_endpoint_table")]
pub fn validate_edge_endpoint(
    edge_type: EdgeType,
    from: NodeType,
    to: NodeType,
) -> Result<(), EdgeValidationError> {
    let valid = match edge_type {
        EdgeType::References => from == NodeType::Source && to == NodeType::Requirement,
        EdgeType::RefinesInto => from == NodeType::Requirement && to == NodeType::Requirement,
        EdgeType::DependsOn | EdgeType::Contradicts | EdgeType::Supersedes => {
            from == NodeType::Requirement && to == NodeType::Requirement
        }
        EdgeType::Needs => from == NodeType::Requirement && to == NodeType::Resolution,
        EdgeType::Resolves | EdgeType::Spawns => {
            from == NodeType::Resolution && to == NodeType::Requirement
        }
        EdgeType::Produces => {
            (from == NodeType::Resolution || from == NodeType::Requirement) && to == NodeType::Rule
        }
    };
    if valid {
        Ok(())
    } else {
        Err(EdgeValidationError::InvalidEdgeEndpoint {
            edge_type,
            from,
            to,
        })
    }
}

#[cfg(test)]
mod tests {
    use provenance_macros::verifies;

    use super::*;

    // The variant lists are derived from exhaustive matches so that adding an
    // EdgeType or NodeType variant fails compilation until the new variant
    // joins the chain, keeping the exhaustion proofs below complete.
    fn all_edge_types() -> Vec<EdgeType> {
        let mut all = vec![EdgeType::References];
        while let Some(next) = match all.last().unwrap() {
            EdgeType::References => Some(EdgeType::RefinesInto),
            EdgeType::RefinesInto => Some(EdgeType::DependsOn),
            EdgeType::DependsOn => Some(EdgeType::Contradicts),
            EdgeType::Contradicts => Some(EdgeType::Supersedes),
            EdgeType::Supersedes => Some(EdgeType::Needs),
            EdgeType::Needs => Some(EdgeType::Resolves),
            EdgeType::Resolves => Some(EdgeType::Spawns),
            EdgeType::Spawns => Some(EdgeType::Produces),
            EdgeType::Produces => None,
        } {
            all.push(next);
        }
        all
    }

    fn all_node_types() -> Vec<NodeType> {
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

    // Independent restatement of the rule-leaf property ("no edge may leave a
    // rule, and only a produces edge may end at one"), used as the oracle
    // below. Must not be implemented by calling the endpoint table.
    fn edge_may_touch_rule(edge_type: EdgeType, from: NodeType, to: NodeType) -> bool {
        from != NodeType::Rule && (to != NodeType::Rule || edge_type == EdgeType::Produces)
    }

    #[test]
    #[verifies("rule_prov_edge_endpoint_table", exhaustion)]
    fn rejects_every_edge_leaving_a_rule() {
        for edge_type in all_edge_types() {
            for &to in &all_node_types() {
                assert!(
                    validate_edge_endpoint(edge_type, NodeType::Rule, to).is_err(),
                    "endpoint table accepts {edge_type:?} leaving a rule to {to:?}"
                );
            }
        }
    }

    #[test]
    #[verifies("rule_prov_edge_endpoint_table", exhaustion)]
    fn endpoint_table_conforms_to_rule_leaf() {
        for edge_type in all_edge_types() {
            for &from in &all_node_types() {
                for &to in &all_node_types() {
                    if validate_edge_endpoint(edge_type, from, to).is_ok() {
                        assert!(
                            edge_may_touch_rule(edge_type, from, to),
                            "endpoint table accepts {edge_type:?} {from:?} -> {to:?}, \
                             which the rule-leaf property forbids"
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[verifies("rule_prov_edge_endpoint_table", examples)]
    fn source_requirement_reference_is_valid() {
        validate_edge_endpoint(
            EdgeType::References,
            NodeType::Source,
            NodeType::Requirement,
        )
        .unwrap();
    }

    #[test]
    #[verifies("rule_prov_edge_endpoint_table", examples)]
    fn rejects_source_to_rule_reference_edge() {
        let err = validate_edge_endpoint(EdgeType::References, NodeType::Source, NodeType::Rule)
            .unwrap_err();
        assert!(matches!(
            err,
            EdgeValidationError::InvalidEdgeEndpoint { .. }
        ));
    }

    #[test]
    #[verifies("rule_prov_edge_endpoint_table", examples)]
    fn accepts_phase_three_traceability_edges() {
        validate_edge_endpoint(EdgeType::Needs, NodeType::Requirement, NodeType::Resolution)
            .unwrap();
        validate_edge_endpoint(
            EdgeType::Resolves,
            NodeType::Resolution,
            NodeType::Requirement,
        )
        .unwrap();
        validate_edge_endpoint(EdgeType::Produces, NodeType::Resolution, NodeType::Rule).unwrap();
        validate_edge_endpoint(EdgeType::Produces, NodeType::Requirement, NodeType::Rule).unwrap();
    }

    #[test]
    #[verifies("rule_prov_edge_endpoint_table", examples)]
    fn rejects_structural_shaping_records_as_explicit_edges() {
        assert!(
            validate_edge_endpoint(EdgeType::DependsOn, NodeType::Topic, NodeType::Topic).is_err()
        );
        assert!(validate_edge_endpoint(
            EdgeType::DependsOn,
            NodeType::Question,
            NodeType::Question,
        )
        .is_err());
    }
}
