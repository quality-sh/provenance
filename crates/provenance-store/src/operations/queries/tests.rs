use super::records;
use crate::layout::ProvenanceLayout;
use crate::state_store::{
    CreateBoundaryInput, CreateDomainInput, CreateRequirementInput, StateStore,
};
use provenance_core::protocol::{GetQuery, SearchQuery};
use provenance_core::{
    Manifest, NodeType, RepoPathPrefix, RequirementStatus, ScopeId, StableId, SDK_PROTOCOL_VERSION,
};

mod differential;
mod golden;
mod oracle;
mod reader;
mod stamp;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

/// One scope holding a requirement, its domain, and one boundary on it.
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
    let store = StateStore::new(layout);
    store
        .create_domain(CreateDomainInput {
            scope_id: scope.clone(),
            id: sid("domain_payroll"),
            name: "Payroll".into(),
            description: Some("Wages and awards".into()),
            color: None,
        })
        .unwrap();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: sid("req_overtime"),
            statement: "Overtime is paid".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: Some(sid("domain_payroll")),
            refines: None,
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_boundary(CreateBoundaryInput {
            scope_id: scope.clone(),
            id: sid("boundary_no_backpay"),
            requirement_id: sid("req_overtime"),
            statement: "Back pay is out of scope".into(),
            source_ref: None,
        })
        .unwrap();
    (dir, store, scope)
}

/// The repository root of a seeded store, for the public operations.
fn root_of(dir: &tempfile::TempDir) -> camino::Utf8PathBuf {
    camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap()
}

#[tokio::test]
async fn get_answers_a_domain_and_a_boundary_by_id() {
    let (dir, _store, scope) = seeded_store();
    for (kind, id, wire) in [
        (NodeType::Domain, "domain_payroll", "domain"),
        (NodeType::Boundary, "boundary_no_backpay", "boundary"),
    ] {
        let answer = super::get(
            Some(root_of(&dir)),
            &scope,
            GetQuery {
                protocol_version: Some(SDK_PROTOCOL_VERSION),
                node_type: kind,
                id: id.into(),
                include_retired: false,
            },
        )
        .await
        .unwrap()
        .result;
        assert!(answer.found, "get must find the {wire} record");
        let node = serde_json::to_value(answer.node.unwrap()).unwrap();
        assert_eq!(node["node_type"], wire);
        assert_eq!(node["id"], id);
    }
}

#[tokio::test]
async fn search_reaches_domains_and_boundaries_by_kind_and_text() {
    let (dir, _store, scope) = seeded_store();
    let answer = super::search(
        Some(root_of(&dir)),
        &scope,
        SearchQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            text: "pay".into(),
            node_types: vec![NodeType::Domain, NodeType::Boundary],
            limit: 10,
            include_retired: false,
        },
    )
    .await
    .unwrap()
    .result;
    let kinds: Vec<String> = answer
        .nodes
        .iter()
        .map(|node| {
            serde_json::to_value(node).unwrap()["node_type"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(kinds, ["domain", "boundary"]);
}

#[tokio::test]
async fn default_search_keeps_the_six_settled_kinds_under_protocol_five() {
    let (dir, _store, scope) = seeded_store();
    let answer = super::search(
        Some(root_of(&dir)),
        &scope,
        SearchQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            text: "pay".into(),
            node_types: Vec::new(),
            limit: 10,
            include_retired: false,
        },
    )
    .await
    .unwrap()
    .result;
    for node in &answer.nodes {
        let kind = node.node_type();
        assert!(
            !matches!(kind, NodeType::Domain | NodeType::Boundary),
            "a default search must not emit {kind:?} while the protocol stays at version 5"
        );
    }
}

#[test]
fn rank_appends_the_new_kinds_after_the_six_settled_positions() {
    let pinned = [
        (NodeType::Source, 0),
        (NodeType::Requirement, 1),
        (NodeType::Resolution, 2),
        (NodeType::Rule, 3),
        (NodeType::Topic, 4),
        (NodeType::Question, 5),
        (NodeType::Domain, 6),
        (NodeType::Boundary, 7),
    ];
    for (kind, expected) in pinned {
        assert_eq!(records::rank(kind), expected, "rank of {kind:?}");
    }
}

#[test]
fn load_orders_new_kinds_after_every_settled_kind() {
    let (_dir, store, scope) = seeded_store();
    let nodes = records::load(&store, &scope, false).unwrap();
    let order: Vec<NodeType> = nodes
        .iter()
        .map(provenance_core::protocol::GraphNode::node_type)
        .collect();
    assert_eq!(
        order,
        [NodeType::Requirement, NodeType::Domain, NodeType::Boundary]
    );
}
