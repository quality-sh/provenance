use provenance_porcelain::discussion::{ListInput, render_readable};
use provenance_core::threads::DiscussionStatusFilter;

#[test]
fn list_input_defaults_to_active_and_rejects_host_grants() {
    let input: ListInput = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(input.status, DiscussionStatusFilter::Active);
    assert!(serde_json::from_value::<ListInput>(serde_json::json!({
        "allowed_parent_kinds": ["source"]
    })).is_err());
}

#[test]
fn readable_list_reports_bounds_and_true_entry_fields() {
    let result = serde_json::json!({
        "kind": "list",
        "result": {"entries": [{
            "discussion_id": "discussion_a",
            "parent": {"node_type": "requirement", "node_id": "req_a"},
            "status": "active", "version": 2,
            "opening_excerpt": "Opening text", "excerpt_truncated": true
        }], "limit": 1, "has_more": true, "next_cursor": "next-page"},
        "stamp": null, "freshness_error": null
    });
    let readable = render_readable(&result);
    for expected in ["discussion_a", "req_a", "active", "version=2", "Opening text", "truncated=true", "limit=1", "next-page"] {
        assert!(readable.contains(expected), "missing {expected}: {readable}");
    }
}
