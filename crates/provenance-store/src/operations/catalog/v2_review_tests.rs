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
    update_request_with_etag(
        request_id,
        &store.requirement_edit_state(&scope, &id).unwrap().etag,
        "Saved text.",
    )
}

fn update_request_with_etag(
    request_id: &str,
    expected_etag: &str,
    description: &str,
) -> UpdateRequirementRequest {
    serde_json::from_value(json!({
        "request_id": request_id,
        "actor": "ben",
        "expected_etag": expected_etag,
        "declared_by": null,
        "statement": null,
        "description": description,
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

    let committed = CreateRequirementV2::run(
        PreparedContext::for_scope(PreparedScope {
            root: store.layout.root().to_owned(),
            scope: scope.clone(),
            requested_target: "fixture".into(),
        }),
        create_request("create_a"),
    )
    .await
    .unwrap();
    assert_eq!(committed.record.id.as_str(), "req_a");
    assert_eq!(store.review_entries(&scope).unwrap().len(), 1);
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
    let receipts_before = store.review_entries(&scope).unwrap();
    crate::test_probes::arm("requirement_resource_snapshot", || {
        anyhow::bail!("injected response construction failure")
    });

    let result = UpdateRequirementV2::run(context, request).await;
    crate::test_probes::disarm("requirement_resource_snapshot");

    assert!(result.is_err());
    let record = store.list_requirements(&scope).unwrap().remove(0);
    assert_eq!(record.description, None);
    assert_eq!(store.review_entries(&scope).unwrap(), receipts_before);

    let committed = UpdateRequirementV2::run(
        PreparedContext::for_scope(PreparedScope {
            root: store.layout.root().to_owned(),
            scope: scope.clone(),
            requested_target: "fixture".into(),
        }),
        update_request(&store, "update_a"),
    )
    .await
    .unwrap();
    assert_eq!(committed.record.description.as_deref(), Some("Saved text."));
    assert_eq!(
        store.review_entries(&scope).unwrap().len(),
        receipts_before.len() + 1
    );
}

#[tokio::test]
async fn replay_precedes_stale_precondition_and_returns_current_state() {
    let (_temp, context, store, scope) = fixture();
    CreateRequirementV2::run(context.clone(), create_request("create_a"))
        .await
        .unwrap();
    let id = StableId::new("req_a").unwrap();
    let first_etag = store.requirement_edit_state(&scope, &id).unwrap().etag;
    UpdateRequirementV2::run(
        context.clone(),
        update_request_with_etag("update_a", &first_etag, "First text."),
    )
    .await
    .unwrap();
    let second_etag = store.requirement_edit_state(&scope, &id).unwrap().etag;
    UpdateRequirementV2::run(
        context.clone(),
        update_request_with_etag("update_b", &second_etag, "Second text."),
    )
    .await
    .unwrap();

    let replay = UpdateRequirementV2::run(
        context,
        update_request_with_etag("update_a", &first_etag, "First text."),
    )
    .await
    .unwrap();

    assert_eq!(replay.record.description.as_deref(), Some("Second text."));
    assert_eq!(replay.edit.etag, store.requirement_edit_state(&scope, &id).unwrap().etag);
    assert_eq!(store.review_entries(&scope).unwrap().len(), 3);
}

#[test]
fn concurrent_resource_writes_with_one_etag_commit_once() {
    let (_temp, _context, store, scope) = fixture();
    store
        .create_review_requirement(serde_json::from_value(json!({
            "request_id": "create_a",
            "actor": "ben",
            "origin": null,
            "create": {
                "scope_id": "default",
                "id": "req_a",
                "statement": "The system stores records.",
                "status": "discovery",
                "depends_on": [],
                "supersedes": []
            }
        })).unwrap())
        .unwrap();
    let id = StableId::new("req_a").unwrap();
    let etag = store.requirement_edit_state(&scope, &id).unwrap().etag;
    let input = |request: &str, description: &str| {
        serde_json::from_value(json!({
            "request_id": request,
            "actor": "ben",
            "expected_etag": etag,
            "update": {
                "scope_id": "default",
                "id": "req_a",
                "description": description
            },
            "relationships": null
        }))
        .unwrap()
    };
    let first = input("update_a", "First text.");
    let second = input("update_b", "Second text.");

    std::thread::scope(|threads| {
        let first = threads.spawn(|| store.save_requirement_resource(first));
        let second = threads.spawn(|| store.save_requirement_resource(second));
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert!(results
            .iter()
            .find_map(|result| result.as_ref().err())
            .unwrap()
            .to_string()
            .contains("etag"));
    });
    assert_eq!(store.review_entries(&scope).unwrap().len(), 2);
}
