//! Addressed Discussions on all six parent kinds: per-kind ownership, the
//! journaled Thread and Message outcomes, and the legacy containers that stay
//! readable beside them.

#[allow(dead_code)]
mod review_support;
use provenance_core::threads::DiscussionStatus;
use provenance_core::{NodeType, ScopeId};
use provenance_store::review::WriteDiscussion;
use provenance_store::state_store::StateStore;
use review_support::*;
use serde_json::json;

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn write_for(
    kind: NodeType,
    node: &str,
    request: &str,
    declared_by: &serde_json::Value,
) -> WriteDiscussion {
    serde_json::from_value(json!({
        "scope_id": "default",
        "parent": {"node_type": kind, "node_id": node},
        "request_id": request,
        "actor": "ben",
        "declared_by": declared_by,
        "action": {"kind": "start", "role": "user", "body": "A concern"}
    }))
    .unwrap()
}

fn every_kind() -> Vec<(NodeType, &'static str)> {
    vec![
        (NodeType::Source, "source_award"),
        (NodeType::Requirement, "req_a"),
        (NodeType::Resolution, "res_award"),
        (NodeType::Rule, "rule_award"),
        (NodeType::Topic, "topic_award"),
        (NodeType::Question, "question_award"),
    ]
}

/// Seeds one parent record per kind: source, requirement, resolution, rule,
/// topic, and question.
fn seed_kinds(store: &StateStore) {
    store
        .create_source(
            serde_json::from_value(json!({
                "scope_id": "default", "id": "source_award", "name": "Award",
                "source_type": "policy", "url": null, "reference": "docs/award.md",
                "commit_pin": null, "effective_date": null, "review_date": null,
                "supersedes": [], "origin_thread": null, "origin_message": null
            }))
            .unwrap(),
        )
        .unwrap();
    // req_a comes from the fixture.
    store
        .create_resolution(
            serde_json::from_value(json!({
                "scope_id": "default", "id": "res_award", "title": "Award",
                "requirement_ids": ["req_a"], "supersedes": [],
                "position": "Adopted", "rationale": "The award says so",
                "status": "proposed", "context": null, "enforcement": null,
                "confidence": null, "inputs": [], "made_by": "maker",
                "approved_by": null, "approved_at": null,
                "origin_thread": null, "origin_message": null
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .create_rule(
            serde_json::from_value(json!({
                "scope_id": "default", "id": "rule_award",
                "name": null, "description": null,
                "requirement_ids": ["req_a"], "resolution_ids": [],
                "statement": "The system pays the award rate.",
                "status": "active", "archived_in_commit": null,
                "severity": "high", "source_document": null,
                "source_section": null, "origin_thread": null, "origin_message": null
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .create_topic(
            serde_json::from_value(json!({
                "scope_id": "default", "id": "topic_award",
                "requirement_id": "req_a", "title": "Award",
                "status": "open", "links": []
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .create_question(
            serde_json::from_value(json!({
                "scope_id": "default", "id": "question_award",
                "topic_id": "topic_award",
                "question": "Which rate applies?", "resolution_method": "research",
                "status": "open", "answer": null, "links": [],
                "resolution_id": null, "contradicts": null
            }))
            .unwrap(),
        )
        .unwrap();
}

#[test]
fn every_kind_starts_replies_and_resolves_a_concern() {
    let (_temp, store) = fixture();
    seed_kinds(&store);
    for (kind, node) in every_kind() {
        let parent = json!({"node_type": kind, "node_id": node});
        // The resolution parent carries its maker as the owner fact.
        let declared_by = if node == "res_award" {
            json!("maker")
        } else {
            json!(null)
        };
        let started: provenance_core::threads::DiscussionEntry = store
            .write_discussion(
                serde_json::from_value(json!({
                    "scope_id": "default", "parent": parent,
                    "request_id": format!("{node}_start"), "actor": "ben",
                    "declared_by": declared_by,
                    "action": {"kind": "start", "role": "user", "body": "A concern"}
                }))
                .unwrap(),
            )
            .unwrap_or_else(|error| panic!("{node}: {error}"));
        assert_eq!(started.parent.node_type, kind, "{node}");
        assert_eq!(started.version, 1, "{node}");
        let replied = store
            .write_discussion(
                serde_json::from_value(json!({
                    "scope_id": "default", "parent": parent,
                    "request_id": format!("{node}_reply"), "actor": "ben",
                    "declared_by": declared_by,
                    "action": {"kind": "reply", "discussion_id": started.discussion_id,
                               "expected_version": 1, "role": "user", "body": "A reply"}
                }))
                .unwrap(),
            )
            .unwrap_or_else(|error| panic!("{node}: {error}"));
        assert_eq!(replied.status, DiscussionStatus::Active, "{node}");
        let resolved = store
            .write_discussion(
                serde_json::from_value(json!({
                    "scope_id": "default", "parent": parent,
                    "request_id": format!("{node}_status"), "actor": "ben",
                    "declared_by": declared_by,
                    "action": {"kind": "set_status", "discussion_id": started.discussion_id,
                               "expected_version": 2, "status": "resolved"}
                }))
                .unwrap(),
            )
            .unwrap_or_else(|error| panic!("{node}: {error}"));
        assert_eq!(resolved.status, DiscussionStatus::Resolved, "{node}");
    }
    // Six kinds, one canonical container each; every Message stays readable.
    let threads = store.list_threads(&scope()).unwrap();
    assert_eq!(threads.len(), 6);
    assert!(threads
        .iter()
        .all(|thread| thread.schema_version == provenance_core::review::REVIEW_SCHEMA_VERSION));
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 12);
}

#[test]
fn parent_ownership_and_existence_are_per_kind() {
    let (_temp, store) = fixture();
    seed_kinds(&store);
    // An owner fact the parent does not carry refuses on every authored kind.
    for (kind, node) in [
        (NodeType::Source, "source_award"),
        (NodeType::Requirement, "req_a"),
        (NodeType::Resolution, "res_award"),
        (NodeType::Rule, "rule_award"),
    ] {
        assert!(
            store
                .write_discussion(write_for(
                    kind,
                    node,
                    &format!("wrong_{node}"),
                    &json!("other")
                ))
                .is_err(),
            "{node} refuses a foreign declared owner"
        );
    }
    // The resolution owner matches its maker.
    store
        .write_discussion(write_for(
            NodeType::Resolution,
            "res_award",
            "owned",
            &json!("maker"),
        ))
        .unwrap();
    // Claims are work locks, not authorship: topics and questions take no
    // declared owner at all.
    for (kind, node) in [
        (NodeType::Topic, "topic_award"),
        (NodeType::Question, "question_award"),
    ] {
        assert!(
            store
                .write_discussion(write_for(
                    kind,
                    node,
                    &format!("claimed_{node}"),
                    &json!("someone")
                ))
                .is_err(),
            "{node} refuses a declared owner"
        );
        store
            .write_discussion(write_for(kind, node, &format!("open_{node}"), &json!(null)))
            .unwrap_or_else(|error| panic!("{node}: {error}"));
    }
    // A parent that does not exist refuses before any write.
    assert!(store
        .write_discussion(write_for(
            NodeType::Source,
            "source_missing",
            "ghost",
            &json!(null)
        ))
        .is_err());
    // A kind that takes no Discussions refuses before any lookup.
    assert!(store
        .write_discussion(write_for(
            NodeType::Domain,
            "domain_missing",
            "domain",
            &json!(null)
        ))
        .is_err());
}

#[test]
fn legacy_containers_stay_readable_beside_addressed_discussions() {
    let (_temp, store) = fixture();
    seed_kinds(&store);
    // The legacy posting path still works, and its container keeps its identity.
    let legacy = store
        .post_thread_message(
            serde_json::from_value(json!({
                "scope_id": "default",
                "parent": {"node_type": "question", "node_id": "question_award"},
                "role": "user", "body": "A legacy comment"
            }))
            .unwrap(),
        )
        .unwrap();
    assert_eq!(legacy.thread.parent.node_id.as_str(), "question_award");
    // The addressed Discussion adopts the canonical container, so the legacy
    // identity keeps its history instead of splitting it.
    let addressed = store
        .write_discussion(write_for(
            NodeType::Question,
            "question_award",
            "addressed",
            &json!(null),
        ))
        .unwrap();
    assert_eq!(addressed.thread_id, legacy.thread.id);
    let containers = store
        .list_threads(&scope())
        .unwrap()
        .into_iter()
        .filter(|thread| thread.parent.node_id.as_str() == "question_award")
        .collect::<Vec<_>>();
    assert_eq!(containers.len(), 1);
    assert_eq!(
        containers[0].schema_version,
        provenance_core::review::REVIEW_SCHEMA_VERSION
    );
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 2);
}
