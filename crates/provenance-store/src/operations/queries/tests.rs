use super::records;
use crate::layout::ProvenanceLayout;
use crate::operations::read_policy::ReadPolicy;
use crate::repository::Repository;
use crate::state_store::{CreateRequirementInput, StateStore};
use provenance_core::{Manifest, NodeType, RepoPathPrefix, RequirementStatus, ScopeId, StableId};

mod binding_pages;
mod budgets;
mod comparison;
mod concurrent;
mod cursor;
mod dangling_targets;
mod document;
mod document_lifecycle;
mod document_membership;
mod document_review;
mod document_roots;
mod evidence;
mod front_equivalence;
mod hydration;
mod hydration_support;
mod identity;
mod impact;
mod lifecycle_visibility;
mod limits;
mod live;
mod order;
mod pinned;
mod reader;
mod stamp;
mod symbol_pages;
mod symbols;
mod tables;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

/// One scope holding one requirement.
fn seeded_store() -> (tempfile::TempDir, StateStore, ScopeId) {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    let scope = ScopeId::new("default").unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_string(&Manifest::default_with_scope(
            scope.clone(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    let store = crate::test_support::store(&layout);
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: sid("req_overtime"),
            statement: "Overtime is paid".into(),
            description: None,
            status: RequirementStatus::Active,
            refines: Vec::new(),
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    (dir, store, scope)
}

/// The repository root of a seeded store, for the public operations.
fn root_of(dir: &tempfile::TempDir) -> camino::Utf8PathBuf {
    camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap()
}

/// The opened repository of a seeded store, for the public operations.
fn repository_of(dir: &tempfile::TempDir) -> Repository {
    Repository::open(dir.path(), "default").unwrap()
}

/// The opened repository at a root, for the public operations.
fn repository_at(root: &camino::Utf8Path) -> Repository {
    Repository::open(root, "default").unwrap()
}

#[tokio::test]
async fn filter_only_search_reaches_the_selected_kind() {
    let (dir, _store, _scope) = seeded_store();
    let answer = super::search(
        &repository_of(&dir),
        ReadPolicy::default(),
        serde_json::from_value(serde_json::json!({
            "node_types":["requirement"],
            "limit":10
        }))
        .unwrap(),
    )
    .await
    .unwrap()
    .result;
    let found: Vec<(NodeType, &str)> = answer
        .nodes
        .iter()
        .map(|node| (node.node_type(), node.id().as_str()))
        .collect();
    assert_eq!(found, [(NodeType::Requirement, "req_overtime")]);
}

#[test]
fn rank_is_the_declared_contract_order() {
    let pinned = [
        (NodeType::Source, 0),
        (NodeType::Requirement, 1),
        (NodeType::Resolution, 2),
        (NodeType::Rule, 3),
        (NodeType::Topic, 4),
        (NodeType::Question, 5),
    ];
    for (kind, expected) in pinned {
        assert_eq!(records::rank(kind), expected, "rank of {kind:?}");
    }
}

mod eight_readers;
