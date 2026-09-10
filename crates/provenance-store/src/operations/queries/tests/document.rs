use super::{root_of, seeded_store};
use crate::operations::catalog::{
    self, ContextResolver, ExecutionNeeds, PreparedContext, PreparedRead, RequestedContext,
};
use crate::operations::read_policy::ReadPolicy;
use provenance_core::protocol::failure::OperationFailure;
use serde_json::{json, Value};
use std::sync::Arc;

struct Target(camino::Utf8PathBuf);
impl ContextResolver for Target {
    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        Ok(PreparedContext::read(PreparedRead {
            root: self.0.clone(),
            scope: provenance_core::ScopeId::new("default").unwrap(),
            policy: ReadPolicy::default(),
            requested_target: "test".into(),
            external: true,
        }))
    }
}
async fn read(root: camino::Utf8PathBuf) -> Value {
    catalog::invoke_with("read-document", provenance_core::SDK_PROTOCOL_VERSION,
        json!({"context":{"repository":"test","scope":"default","freshness":"catch_up"},"request":{"id":"req_overtime"}}),
        Arc::new(Target(root))).await.expect("complete document operation")
}

#[tokio::test]
async fn document_reads_all_saved_records_and_marks_failed_catch_up() {
    let (dir, store, scope) = seeded_store();
    let first = read(root_of(&dir)).await;
    assert_eq!(first["requirements"].as_array().unwrap().len(), 1);
    let mut child =
        serde_json::to_value(store.list_requirements(&scope).unwrap()[0].clone()).unwrap();
    child["refines"] = json!("req_overtime");
    let path = crate::shards::requirements_path(&store.layout, &scope);
    for i in 0..205 {
        child["id"] = json!(format!("req_child_{i:03}"));
        crate::cache::tests::fixtures::append_record(&path, &child);
    }
    let next = read(root_of(&dir)).await;
    assert_eq!(next["requirements"].as_array().unwrap().len(), 206);
    assert_eq!(next["root_id"], "req_overtime");
    assert_ne!(next["stamp"]["digest"], first["stamp"]["digest"]);
    assert_eq!(next["stamp"]["policy"], "catch_up");
    for family in [
        "requirements",
        "resolutions",
        "rules",
        "sources",
        "topics",
        "questions",
        "threads",
        "messages",
    ] {
        assert!(
            next["stamp"]["attested"]
                .as_array()
                .unwrap()
                .contains(&json!(family)),
            "{family}"
        );
    }
    assert_eq!(next["stamp"]["live"], json!([]));
    std::fs::write(path, "invalid JSON\n").unwrap();
    let failed = read(root_of(&dir)).await;
    assert_eq!(failed["stamp"]["policy"], "catch_up_failed");
    assert_eq!(failed["stamp"]["digest"], next["stamp"]["digest"]);
    assert_eq!(failed["freshness_cause"], "catch_up_failed");
    assert_eq!(
        failed["freshness_error"],
        "catch-up failed; answer uses the stored projection"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn document_keeps_graph_and_discussions_at_one_revision_during_publication() {
    use crate::operations::reader;
    use provenance_core::protocol::ReadDocumentQuery;
    let (dir, store, scope) = seeded_store();
    let root = root_of(&dir);
    let initial = read(root.clone()).await;
    let layout = store.layout.clone();
    let write_scope = scope.clone();
    let answer = reader::answer(&root, &scope, ReadPolicy::default(), move |ctx| {
        Box::pin(async move {
            assert_eq!(ctx.snapshot().table::<provenance_core::Requirement>().count().await?, 1);
            let path = crate::shards::requirements_path(&layout, &write_scope);
            let mut child: Value = serde_json::from_str(std::fs::read_to_string(&path)?.lines().next().unwrap())?;
            child["id"] = json!("req_concurrent");
            child["refines"] = json!("req_overtime");
            crate::cache::tests::fixtures::append_record(&path, &child);
            for (id, status, counter) in [("thread_one", "resolved", 1), ("thread_two", "active", 2)] {
                crate::cache::tests::fixtures::append_record(&crate::shards::threads_path(&layout, &write_scope), &json!({
                    "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION, "scope_id": "default", "id": id,
                    "parent": {"node_type":"requirement","node_id":"req_concurrent"}, "status":status, "created_at":counter
                }));
                let message_path = layout.state_dir().join("scopes/default/threads/2026-09.jsonl");
                crate::cache::tests::fixtures::append_record(&message_path, &json!({
                    "schema_version":provenance_core::SUPPORTED_SCHEMA_VERSION, "scope_id":"default", "id":format!("msg_{counter}"),
                    "thread_id":id,"role":"user","body":format!("Discussion {counter}"),"created_at":counter,
                    "ai_metadata":{"order":"logical"}
                }));
            }
            crate::cache::catch_up_state(&layout).await?;
            super::super::document::read(ctx, ReadDocumentQuery { id: "req_overtime".into() }).await
        })
    }).await.unwrap();
    assert_eq!(answer.result.requirements.len(), 1);
    assert!(answer.result.threads.is_empty());
    assert!(answer.result.messages.is_empty());
    assert_eq!(answer.stamp.digest, initial["stamp"]["digest"]);
    let refreshed = read(root).await;
    assert_eq!(refreshed["requirements"].as_array().unwrap().len(), 2);
    assert_eq!(refreshed["threads"].as_array().unwrap().len(), 2);
    assert_eq!(refreshed["messages"].as_array().unwrap().len(), 2);
    assert_eq!(
        refreshed["messages"][0]["ai_metadata"],
        json!({"order":"logical"})
    );
    assert_ne!(refreshed["stamp"]["digest"], initial["stamp"]["digest"]);
}

#[tokio::test]
async fn document_retains_retired_identity_and_references() {
    let (dir, store, scope) = seeded_store();
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let mut retired =
        serde_json::to_value(store.list_requirements(&scope).unwrap()[0].clone()).unwrap();
    retired["id"] = json!("req_retired");
    retired["retired"] = json!(true);
    retired["refines"] = json!("req_overtime");
    crate::cache::tests::fixtures::append_record(&path, &retired);
    let answer = read(root_of(&dir)).await;
    let record = answer["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "req_retired")
        .unwrap();
    assert_eq!(record["retired"], true);
    assert_eq!(record["refines"], "req_overtime");
}
