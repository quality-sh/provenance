use super::{CreateReviewRequirement, WriteDiscussion};
use crate::{layout::ProvenanceLayout, state_store::StateStore, test_probes};
use camino::Utf8Path;
use provenance_core::{threads::DiscussionEntry, ScopeId};
use serde_json::json;

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}
fn open(root: &Utf8Path) -> StateStore {
    StateStore::new(ProvenanceLayout::new(root))
}
fn root_input() -> WriteDiscussion {
    serde_json::from_value(json!({"scope_id":"default","parent":{"node_type":"requirement","node_id":"req_a"},"request_id":"root","actor":"ben","action":{"kind":"start","role":"user","body":"Concern"}})).unwrap()
}
fn mutation(root: &DiscussionEntry, operation: &str) -> WriteDiscussion {
    let action = if operation == "start" {
        json!({"kind":"start","role":"user","body":"New concern"})
    } else if operation == "reply" {
        json!({"kind":"reply","discussion_id":root.discussion_id,"expected_version":1,"role":"user","body":"Reply"})
    } else {
        json!({"kind":"set_status","discussion_id":root.discussion_id,"expected_version":1,"status":"resolved"})
    };
    serde_json::from_value(json!({"scope_id":"default","parent":root.parent,"request_id":"mutation","actor":"ben","action":action})).unwrap()
}
fn creation(root: &DiscussionEntry) -> CreateReviewRequirement {
    serde_json::from_value(json!({"request_id":"creation","actor":"ben","origin":{"discussion_id":root.discussion_id,"thread_id":root.thread_id,"message_id":root.message_id},
        "create":{"scope_id":"default","id":"req_new","statement":"The system stores evidence.","status":"discovery","depends_on":[],"supersedes":[]}})).unwrap()
}
fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let store = open(root);
    store.create_requirement(serde_json::from_value(json!({"scope_id":"default","id":"req_a","statement":"The system stores records.","status":"discovery","depends_on":[],"supersedes":[]})).unwrap()).unwrap();
    store.write_discussion(root_input()).unwrap();
    temp
}

#[test]
fn crash_child() {
    let Ok(root) = std::env::var("PROVENANCE_DISCUSSION_CRASH_ROOT") else {
        return;
    };
    let operation = std::env::var("PROVENANCE_DISCUSSION_CRASH_OPERATION").unwrap();
    let phase = std::env::var("PROVENANCE_DISCUSSION_CRASH_PHASE").unwrap();
    let phase = match phase.as_str() {
        "state_prepared" => "state_prepared",
        "state_marker_prepared" => "state_marker_prepared",
        "state_backup_created" => "state_backup_created",
        "state_installed" => "state_installed",
        "state_published" => "state_published",
        _ => panic!("unknown phase"),
    };
    let store = open(Utf8Path::new(&root));
    let root = store.discussion_receipt(&root_input()).unwrap().unwrap();
    test_probes::arm(phase, || std::process::exit(86));
    match operation.as_str() {
        "create" => {
            store.create_review_requirement(creation(&root)).unwrap();
        }
        "edit" => {
            store
                .save_requirement_from_discussion(edit(&store), origin(&root))
                .unwrap();
        }
        _ => {
            store.write_discussion(mutation(&root, &operation)).unwrap();
        }
    }
    panic!("crash phase was not reached");
}
fn origin(root: &DiscussionEntry) -> provenance_core::threads::DiscussionOrigin {
    provenance_core::threads::DiscussionOrigin {
        discussion_id: root.discussion_id.clone(),
        thread_id: root.thread_id.clone(),
        message_id: root.message_id.clone().unwrap(),
    }
}
fn edit(store: &StateStore) -> super::SaveRequirement {
    let id = provenance_core::StableId::new("req_a").unwrap();
    serde_json::from_value(json!({"request_id":"edit","actor":"ben","expected_etag":store.requirement_edit_state(&scope(),&id).unwrap().etag,"update":{"scope_id":"default","id":"req_a","description":"After"},"relationships":null})).unwrap()
}

#[test]
fn process_restart_recovers_membership_status_and_outcome_as_one_state() {
    for operation in ["start", "reply", "resolve", "create", "edit"] {
        for (phase, committed) in [
            ("state_prepared", false),
            ("state_marker_prepared", false),
            ("state_backup_created", false),
            ("state_installed", true),
            ("state_published", true),
        ] {
            let temp = fixture();
            let root = Utf8Path::from_path(temp.path()).unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "review::discussion_recovery_tests::crash_child",
                    "--nocapture",
                ])
                .env("PROVENANCE_DISCUSSION_CRASH_ROOT", root.as_str())
                .env("PROVENANCE_DISCUSSION_CRASH_OPERATION", operation)
                .env("PROVENANCE_DISCUSSION_CRASH_PHASE", phase)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(86), "{operation}/{phase}");
            let store = open(root);
            // This receipt read must recover an absent live state before reporting absence.
            let original = store.discussion_receipt(&root_input()).unwrap().unwrap();
            if operation == "create" {
                let receipt = store
                    .requirement_creation_receipt(creation(&original))
                    .unwrap();
                assert_eq!(receipt.is_some(), committed, "creation/{phase}");
                let entries = store.review_entries(&scope()).unwrap();
                assert_eq!(entries.len(), usize::from(committed));
                assert_eq!(
                    store.list_requirements(&scope()).unwrap().len(),
                    if committed { 2 } else { 1 }
                );
                let result = store
                    .create_review_requirement(creation(&original))
                    .unwrap();
                assert!(result.before.is_none());
                assert_eq!(result.origin, Some(origin(&original)));
                if committed {
                    assert_eq!(result, entries[0]);
                }
            } else if operation == "edit" {
                let receipt = store
                    .requirement_save_receipt(
                        &scope(),
                        &provenance_core::StableId::new("req_a").unwrap(),
                        &provenance_core::StableId::new("edit").unwrap(),
                        "ben",
                        None,
                    )
                    .unwrap();
                assert_eq!(receipt.is_some(), committed);
                assert_eq!(
                    store.list_requirements(&scope()).unwrap()[0]
                        .description
                        .as_deref(),
                    committed.then_some("After")
                );
                if let Some(receipt) = receipt {
                    assert!(receipt.before.is_some());
                    assert_eq!(receipt.origin, Some(origin(&original)));
                }
            } else {
                let input = mutation(&original, operation);
                let receipt = store.discussion_receipt(&input).unwrap();
                assert_eq!(receipt.is_some(), committed, "{operation}/{phase}");
                assert_eq!(
                    store.list_messages(&scope()).unwrap().len(),
                    if committed && matches!(operation, "reply" | "start") {
                        2
                    } else {
                        1
                    }
                );
                let result = store.write_discussion(input).unwrap();
                if let Some(receipt) = receipt {
                    assert_eq!(result, receipt);
                }
                assert_eq!(result.version, if operation == "start" { 1 } else { 2 });
            }
            store.validated_journal_entries(&scope()).unwrap();
            assert!(!ProvenanceLayout::new(root)
                .publication_marker_path()
                .exists());
        }
    }
}

#[test]
fn lost_response_reconciles_receipt_after_failed_cleanup() {
    let temp = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let store = open(root);
    let original = store.discussion_receipt(&root_input()).unwrap().unwrap();
    let input = mutation(&original, "reply");
    test_probes::crash_at("state_published");
    let result = store.write_discussion(input.clone());
    test_probes::disarm("state_published");
    assert!(matches!(
        crate::write_error::WriteError(result.unwrap_err()).safe(),
        crate::write_error::WriteFailure::UncertainWrite
    ));
    let reopened = open(root);
    let receipt = reopened.discussion_receipt(&input).unwrap().unwrap();
    assert_eq!(receipt, reopened.write_discussion(input).unwrap());
    assert_eq!(reopened.list_messages(&scope()).unwrap().len(), 2);
}
