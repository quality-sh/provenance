#[allow(dead_code)]
mod review_support;
use provenance_store::{layout::ProvenanceLayout, shards};
use review_support::*;
use serde_json::json;

#[test]
fn legacy_partial_origins_keep_their_existing_crud_shapes() {
    let (_temp, store) = fixture();
    let old = store
        .post_thread_message(
            serde_json::from_value(json!({
                "scope_id":"default", "parent":{"node_type":"requirement","node_id":"req_a"},
                "role":"user", "body":"Legacy concern"
            }))
            .unwrap(),
        )
        .unwrap();
    for field in ["origin_thread", "origin_message"] {
        let mut input = json!({
            "scope_id":"default", "id":format!("req_{field}"),
            "statement":"The system retains origins.", "status":"discovery",
            "depends_on":[], "supersedes":[]
        });
        input[field] = if field == "origin_thread" {
            json!(old.thread.id)
        } else {
            json!(old.message.id)
        };
        let created = store
            .create_requirement(serde_json::from_value(input).unwrap())
            .unwrap();
        assert_eq!(
            created.origin_thread,
            (field == "origin_thread").then(|| old.thread.id.clone())
        );
        assert_eq!(
            created.origin_message,
            (field == "origin_message").then(|| old.message.id.clone())
        );
    }
}

#[test]
fn malformed_enrollment_keeps_the_existing_version_refusal_context() {
    let (temp, store) = fixture();
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    for (path, kind) in [
        (shards::threads_path(&layout, &scope()), "thread"),
        (shards::messages_path(&layout, &scope()), "message"),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            format!("{{\"schema_version\":3,\"id\":\"{kind}_future\"}}\n"),
        )
        .unwrap();
        let error = if kind == "thread" {
            store.list_threads(&scope()).unwrap_err()
        } else {
            store.list_messages(&scope()).unwrap_err()
        };
        assert!(
            format!("{error:#}").contains("has schema_version 3"),
            "{error}"
        );
        assert!(
            format!("{error:#}").contains(&format!("record {kind}_future")),
            "{error}"
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn a_message_read_failure_after_legacy_thread_publication_remains_uncertain() {
    let (temp, store) = fixture();
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    let path = shards::messages_path(&layout, &scope());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "invalid JSON\n").unwrap();
    let error = store
        .post_thread_message(
            serde_json::from_value(json!({
                "scope_id":"default", "parent":{"node_type":"requirement","node_id":"req_a"},
                "role":"user", "body":"Legacy concern"
            }))
            .unwrap(),
        )
        .unwrap_err();
    assert!(matches!(
        provenance_store::write_error::WriteError::from(error).safe(),
        provenance_store::write_error::WriteFailure::UncertainWrite
    ));
    assert_eq!(store.list_threads(&scope()).unwrap().len(), 1);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid JSON\n");
}
