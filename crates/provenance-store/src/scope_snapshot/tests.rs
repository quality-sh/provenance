use super::snapshot;
use crate::repository::{Repository, RepositoryIdentity};
use crate::settings::Settings;
use crate::storage::memory::{MemorySectionLock, MemoryStateStorage};
use crate::storage::StateKey;
use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

fn shard(row: &serde_json::Value) -> (StateKey, Vec<u8>) {
    let mut bytes = serde_json::to_vec(row).unwrap();
    bytes.push(b'\n');
    let family = match row["id"].as_str().unwrap() {
        "source_policy" => "sources/source.jsonl",
        "req_policy" => "requirements/req.jsonl",
        _ => "rules/rule.jsonl",
    };
    (
        StateKey::new(format!("scopes/default/{family}")).unwrap(),
        bytes,
    )
}

/// A memory repository whose default scope holds one Source, one
/// Requirement and one Rule.
fn seeded_repository() -> Repository {
    let scope = ScopeId::new("default").unwrap();
    let manifest = Manifest::default_with_scope(scope.clone(), RepoPathPrefix::new("."));
    let mut entries = BTreeMap::from([(
        StateKey::new("manifest.json").unwrap(),
        serde_json::to_vec(&manifest).unwrap(),
    )]);
    entries.extend([
        shard(
            &json!({"scope_id": "default", "id": "source_policy", "name": "Policy",
            "source_type": "policy"}),
        ),
        shard(&json!({"scope_id": "default", "id": "req_policy",
            "statement": "Share links expire", "status": "active"})),
        shard(&json!({"scope_id": "default", "id": "rule_policy",
            "statement": "Share links expire", "status": "active", "severity": "medium",
            "requirement_ids": ["req_policy"]})),
    ]);
    Repository::with_storage(
        RepositoryIdentity::new("/provenance-snapshot".into(), scope, Settings::default()),
        Arc::new(MemoryStateStorage::new(entries)),
        Arc::new(MemorySectionLock::default()),
        None,
    )
    .unwrap()
}

#[test]
fn graph_records_borrows_the_graph_families_from_the_full_snapshot() {
    let repository = seeded_repository();

    let snapshot = snapshot(&repository.store(), repository.scope()).unwrap();
    let graph = snapshot.graph_records();

    assert_eq!(graph.sources[0].id.as_str(), "source_policy");
    assert_eq!(graph.requirements[0].id.as_str(), "req_policy");
    assert_eq!(graph.rules[0].id.as_str(), "rule_policy");
    assert!(std::ptr::eq(graph.rules, snapshot.rules.as_slice()));
    assert_eq!(snapshot.threads, [] as [provenance_core::Thread; 0]);
}
