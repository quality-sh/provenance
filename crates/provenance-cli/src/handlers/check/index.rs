use provenance_core::{ScopeId, StableId};
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct CheckIndex {
    scoped_nodes: BTreeSet<(String, String, String)>,
}

impl CheckIndex {
    pub(super) fn add_node(&mut self, scope_id: &ScopeId, node_type: &str, id: &StableId) {
        let node_type = node_type.to_string();
        let id = id.as_str().to_string();
        self.scoped_nodes
            .insert((scope_id.as_str().to_string(), node_type, id));
    }

    pub(super) fn has_scoped_node(
        &self,
        scope_id: &ScopeId,
        node_type: &str,
        id: &StableId,
    ) -> bool {
        self.scoped_nodes.contains(&(
            scope_id.as_str().to_string(),
            node_type.to_string(),
            id.as_str().to_string(),
        ))
    }
}
