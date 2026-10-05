mod discussion_support;
use discussion_support::{fixture, id, scope};
use provenance_core::protocol::failure::OperationError;
use provenance_core::threads::DiscussionStatus;
use provenance_store::{
    layout::ProvenanceLayout,
    operations::catalog::{
        invoke_typed, PreparedContext, PreparedScope, WriteTargetDiscussion,
        WriteTargetDiscussionRequest,
    },
    review::{TargetDiscussionWrite, WriteDiscussion},
    write_error::{WriteError, WriteFailure},
};
use serde_json::json;

fn request(fields: &serde_json::Value) -> TargetDiscussionWrite {
    let mut request = json!({
        "scope_id": "default", "actor": "ben",
        "declared_by": null, "allowed_parent_kinds": [
            "source", "requirement", "resolution", "rule", "topic", "question", "domain", "boundary"
        ]
    });
    request
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    serde_json::from_value(request).unwrap()
}

fn start(body: &str) -> WriteDiscussion {
    serde_json::from_value(json!({
        "scope_id": "default", "actor": "ben",
        "declared_by": null,
        "parent": {"node_type": "requirement", "node_id": "req_a"},
        "action": {"kind": "start", "role": "user", "body": body}
    }))
    .unwrap()
}

fn reply(body: &str, discussion: &provenance_core::threads::Discussion) -> TargetDiscussionWrite {
    request(&json!({
        "discussion_id": discussion.discussion_id,
        "expected_version": discussion.version, "role": "user", "body": body
    }))
}

fn catalog_reply(
    body: &str,
    discussion: &provenance_core::threads::Discussion,
) -> WriteTargetDiscussionRequest {
    serde_json::from_value(json!({
        "scope_id":"default", "actor":"ben", "declared_by":null,
        "allowed_parent_kinds":["requirement"],
        "discussion_id":discussion.discussion_id,
        "expected_version":discussion.version, "role":"user", "body":body
    }))
    .unwrap()
}

fn failure(result: anyhow::Result<provenance_core::threads::Discussion>) -> WriteFailure {
    WriteError(result.unwrap_err()).safe()
}

#[test]
fn addressed_reply_contract_has_only_reply_fields() {
    let mut request = json!({
        "scope_id":"default", "actor":"ben",
        "declared_by":null, "allowed_parent_kinds":["requirement"],
        "discussion_id":"discussion_a", "expected_version":1,
        "role":"user", "body":"A reply."
    });
    assert!(serde_json::from_value::<TargetDiscussionWrite>(request.clone()).is_ok());
    request["request_id"] = json!("reply_request");
    assert!(serde_json::from_value::<TargetDiscussionWrite>(request).is_err());
}

#[test]
fn target_reply_changes_only_its_discussion() {
    let (_temp, store) = fixture();
    let a = store.write_discussion(start("a")).unwrap();
    let b = store.write_discussion(start("b")).unwrap();
    let changed = store.write_target_discussion(reply("reply_a", &a)).unwrap();
    assert_eq!(changed.discussion_id, a.discussion_id);
    assert_eq!(changed.version, 2);
    assert_eq!(b.version, 1);
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 3);
}

#[test]
fn target_reply_checks_host_parent_grants_inside_publication() {
    let (_temp, store) = fixture();
    let original = store.write_discussion(start("first")).unwrap();
    let mut denied_reply = reply("denied_reply", &original);
    denied_reply.allowed_parent_kinds = vec![provenance_core::NodeType::Source];
    assert!(matches!(
        failure(store.write_target_discussion(denied_reply)),
        WriteFailure::ResourceNotFound
    ));
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 1);
}

#[test]
fn target_refusals_do_not_append_messages() {
    let (_temp, store) = fixture();
    let a = store.write_discussion(start("a")).unwrap();
    let missing =
        request(&json!({"discussion_id":"missing","expected_version":1,"role":"user","body":"x"}));
    assert!(matches!(
        failure(store.write_target_discussion(missing)),
        WriteFailure::ResourceNotFound
    ));
    let stale = request(
        &json!({"discussion_id":a.discussion_id,"expected_version":0,"role":"user","body":"x"}),
    );
    assert!(matches!(
        failure(store.write_target_discussion(stale)),
        WriteFailure::DiscussionVersionConflict
    ));
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 1);
}

#[test]
fn one_expected_version_wins_concurrent_replies() {
    let (_temp, store) = fixture();
    let a = store.write_discussion(start("a")).unwrap();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|threads| {
        let one = threads.spawn(|| {
            barrier.wait();
            store.write_target_discussion(reply("r1", &a))
        });
        let two = threads.spawn(|| {
            barrier.wait();
            store.write_target_discussion(reply("r2", &a))
        });
        let results = [one.join().unwrap(), two.join().unwrap()];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert!(results
            .into_iter()
            .filter(Result::is_err)
            .any(|result| matches!(failure(result), WriteFailure::DiscussionVersionConflict)));
    });
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 2);
}

#[test]
fn resolved_discussion_refuses_target_reply() {
    let (_temp, store) = fixture();
    let a = store.write_discussion(start("a")).unwrap();
    let resolved = store
        .write_discussion(discussion_support::status(&a, "resolved"))
        .unwrap();
    assert_eq!(resolved.status, DiscussionStatus::Resolved);
    assert!(matches!(
        failure(store.write_target_discussion(reply("reply", &resolved))),
        WriteFailure::DiscussionResolved
    ));
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 1);
}

#[test]
fn owner_and_parent_membership_refuse_without_publication() {
    let (_temp, store) = fixture();
    let a = store.write_discussion(start("a")).unwrap();
    let mut wrong_owner = start("owner");
    wrong_owner.declared_by = Some("other".into());
    assert!(matches!(
        failure(store.write_discussion(wrong_owner)),
        WriteFailure::RecordOwnershipConflict
    ));
    store
        .create_review_requirement(
            serde_json::from_value(json!({
                "request_id":"create_b","actor":"ben","origin":null,
                "create": {
                    "scope_id": "default", "id": "req_b",
                    "statement": "The system stores notes.", "status": "discovery",
                    "depends_on": [], "supersedes": []
                }
            }))
            .unwrap(),
        )
        .unwrap();
    let mut wrong_parent = discussion_support::reply(&a, "wrong_parent");
    wrong_parent.parent.node_id = provenance_core::StableId::new("req_b").unwrap();
    assert!(matches!(
        failure(store.write_discussion(wrong_parent)),
        WriteFailure::DiscussionMembershipMismatch
    ));
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 1);
}

#[test]
fn closed_container_refuses_target_reply() {
    let (temp, store) = fixture();
    let a = store.write_discussion(start("a")).unwrap();
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    let path = layout.scopes_dir().join("default/threads/threads.jsonl");
    let line = std::fs::read_to_string(&path).unwrap();
    let mut thread: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    thread["status"] = json!("resolved");
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string(&thread).unwrap()),
    )
    .unwrap();
    assert!(matches!(
        failure(store.write_target_discussion(reply("closed", &a))),
        WriteFailure::DiscussionClosed
    ));
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 1);
}

#[tokio::test]
async fn catalog_operation_uses_the_target_write_path() {
    let (temp, store) = fixture();
    let started = store.write_discussion(start("a")).unwrap();
    assert!(provenance_store::operations::catalog::contains(
        "write-target-discussion"
    ));
    let context = PreparedContext::for_scope(PreparedScope {
        root: camino::Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap(),
        scope: scope(),
        requested_target: "selected".into(),
    });
    let receipt =
        invoke_typed::<WriteTargetDiscussion>(context, catalog_reply("catalog", &started))
            .await
            .unwrap();
    assert_eq!(receipt.version, 2);
    assert_eq!(store.list_messages(&scope()).unwrap().len(), 2);
}

#[tokio::test]
async fn catalog_scope_mismatch_is_safe_and_does_not_write() {
    let (temp, store) = fixture();
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    let journal = layout.scopes_dir().join("default/review/journal");
    let receipt_count = std::fs::read_dir(&journal).unwrap().count();
    let requirement = store.requirement_edit_state(&scope(), &id()).unwrap();
    let context = PreparedContext::for_scope(PreparedScope {
        root: camino::Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap(),
        scope: scope(),
        requested_target: "selected".into(),
    });
    let mut mismatched: WriteTargetDiscussionRequest = serde_json::from_value(json!({
        "scope_id":"default", "actor":"ben", "declared_by":null,
        "allowed_parent_kinds":["requirement"], "discussion_id":"missing",
        "expected_version":1, "role":"user", "body":"x"
    }))
    .unwrap();
    mismatched.scope_id = provenance_core::ScopeId::new("other").unwrap();

    let error = invoke_typed::<WriteTargetDiscussion>(context, mismatched)
        .await
        .unwrap_err();
    let OperationError::Handler(error) = error else {
        panic!("expected a write refusal");
    };
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        json!({"kind":"scope_mismatch"})
    );
    assert_eq!(error.status(), 400);
    assert_eq!(std::fs::read_dir(journal).unwrap().count(), receipt_count);
    assert_eq!(
        store.list_threads(&scope()).unwrap(),
        [] as [provenance_core::Thread; 0]
    );
    assert_eq!(
        store.list_messages(&scope()).unwrap(),
        [] as [provenance_core::Message; 0]
    );
    assert_eq!(
        store.requirement_edit_state(&scope(), &id()).unwrap().etag,
        requirement.etag
    );
    assert!(!layout.scopes_dir().join("other").exists());
}
