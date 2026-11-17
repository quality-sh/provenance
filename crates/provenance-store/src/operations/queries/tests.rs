use super::records;
use crate::layout::ProvenanceLayout;
use crate::operations::read_policy::ReadPolicy;
use crate::repository::Repository;
use crate::state_store::{CreateRequirementInput, StateStore};
use provenance_core::{Manifest, NodeType, RepoPathPrefix, RequirementStatus, ScopeId, StableId};

mod binding_pages;
mod budgets;
mod catch_up_rows;
mod comparison;
mod concurrent;
mod cursor;
mod dangling_targets;
mod discussion_chunks;
mod discussion_outputs;
mod document;
mod document_lifecycle;
mod document_membership;
mod document_review;
mod document_roots;
mod evidence;
mod front_equivalence;
mod history_index_work;
mod hydration;
mod hydration_support;
mod identity;
mod impact;
mod lifecycle_visibility;
mod limits;
mod live;
mod message_order;
mod order;
mod page_budget;
mod page_interrupt;
mod pinned;
mod reader;
mod reader_outputs;
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
    let mut manifest = Manifest::default_with_scope(scope.clone(), RepoPathPrefix::new("."));
    manifest.disposition_actor_ids = vec!["reviewer".to_owned()];
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_string(&manifest).unwrap(),
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

/// The search text that a separator requirement holds only across two pieces.
const SEPARATOR_TEXT: &str = "\u{1}plain";

/// Appends the requirements `req_00000` up to `req_{count - 1}` in one write.
/// A requirement that `matches` selects holds [`SEPARATOR_TEXT`] inside one
/// piece. Each other one holds it only across two pieces: the SQL test of the
/// search text selects it and the check of each piece refuses it.
fn append_separator_requirements(
    store: &StateStore,
    scope: &ScopeId,
    count: usize,
    matches: impl Fn(usize) -> bool,
) {
    let path = crate::shards::requirements_path(&store.layout, scope);
    let mut record = serde_json::json!(store.list_requirements(scope).unwrap()[0]);
    let mut contents = std::fs::read_to_string(&path).unwrap();
    for i in 0..count {
        record["id"] = serde_json::json!(format!("req_{i:05}"));
        record["description"] = serde_json::json!(if matches(i) {
            "the \u{1}plain needle"
        } else {
            "plain"
        });
        contents.push_str(&record.to_string());
        contents.push('\n');
    }
    std::fs::write(&path, contents).unwrap();
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
