use super::*;
use crate::operations::catalog::PreparedScope;
use provenance_core::{Manifest, RepoPathPrefix};
use serde_json::json;

fn fixture() -> (tempfile::TempDir, PreparedContext, StateStore, ScopeId) {
    let temp = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let scope = ScopeId::new("default").unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            scope.clone(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    let context = PreparedContext::for_scope(PreparedScope {
        root,
        scope: scope.clone(),
        requested_target: "fixture".into(),
    });
    (temp, context, StateStore::new(layout), scope)
}

fn create_request(request_id: &str) -> CreateRequirementRequest {
    serde_json::from_value(json!({
        "request_id": request_id,
        "actor": "ben",
        "id": "req_a",
        "statement": "The system stores records.",
        "description": null,
        "status": "discovery",
        "domain_id": null,
        "refines": null,
        "depends_on": [],
        "supersedes": [],
        "spawned_by": null,
        "origin_thread": null,
        "origin_message": null,
        "origin": null
    }))
    .unwrap()
}

fn update_request(store: &StateStore, request_id: &str) -> UpdateRequirementRequest {
    let scope = ScopeId::new("default").unwrap();
    let id = StableId::new("req_a").unwrap();
    serde_json::from_value(json!({
        "request_id": request_id,
        "actor": "ben",
        "expected_etag": store.requirement_edit_state(&scope, &id).unwrap().etag,
        "declared_by": null,
        "statement": null,
        "description": "Saved text.",
        "fog": null,
        "status": null,
        "domain_id": null,
        "clear_fields": [],
        "relationships": null,
        "id": "req_a"
    }))
    .unwrap()
}

#[tokio::test]
async fn create_response_failure_refuses_before_publication() {
    let (_temp, context, store, scope) = fixture();
    crate::test_probes::arm("requirement_resource_snapshot", || {
        anyhow::bail!("injected response construction failure")
    });

    let result = CreateRequirementV2::run(context, create_request("create_a")).await;
    crate::test_probes::disarm("requirement_resource_snapshot");

    assert!(result.is_err());
    assert!(store.list_requirements(&scope).unwrap().is_empty());
    assert!(store.review_entries(&scope).unwrap().is_empty());
}

#[tokio::test]
async fn update_response_failure_refuses_before_publication() {
    let (_temp, context, store, scope) = fixture();
    store
        .create_requirement(serde_json::from_value(json!({
            "scope_id": "default",
            "id": "req_a",
            "statement": "The system stores records.",
            "status": "discovery",
            "depends_on": [],
            "supersedes": []
        })).unwrap())
        .unwrap();
    let request = update_request(&store, "update_a");
    crate::test_probes::arm("requirement_resource_snapshot", || {
        anyhow::bail!("injected response construction failure")
    });

    let result = UpdateRequirementV2::run(context, request).await;
    crate::test_probes::disarm("requirement_resource_snapshot");

    assert!(result.is_err());
    let record = store.list_requirements(&scope).unwrap().remove(0);
    assert_eq!(record.description, None);
    assert!(store.review_entries(&scope).unwrap().is_empty());
}
