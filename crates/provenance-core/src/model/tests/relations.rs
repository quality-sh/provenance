
// The chain is derived from an exhaustive match, so a new node kind fails
// compilation here until the rank sweep below covers it.
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
fn node_type_rank_is_the_one_contract_ordering() {
    let ranks: Vec<u8> = every_node_type().iter().map(|kind| kind.rank()).collect();
    assert_eq!(ranks, [0, 1, 2, 3, 4, 5, 6, 7]);
}
