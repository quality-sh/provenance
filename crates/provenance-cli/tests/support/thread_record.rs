//! A thread record of a scope export, for the import tests.

/// A resolved thread `id` of the scope `scope_id` on the Requirement `parent_id`.
pub fn thread_in(id: &str, scope_id: &str, parent_id: &str, created_at: i64) -> serde_json::Value {
    serde_json::json!({
        "scope_id": scope_id,
        "id": id,
        "parent": {"node_type": "requirement", "node_id": parent_id},
        "status": "resolved",
        "created_at": created_at
    })
}
