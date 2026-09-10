use super::{root_of, seeded_store};
use crate::operations::catalog::{
    self, ContextResolver, ExecutionNeeds, PreparedContext, PreparedRead, RequestedContext,
};
use crate::operations::read_policy::ReadPolicy;
use provenance_core::protocol::failure::OperationFailure;
use provenance_core::protocol::{read_failure::ReadFailure, ReadDocumentQuery};
use provenance_macros::verifies;
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

fn collections(mut page: Value) -> Value {
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
        page[family] = json!([]);
    }
    for entry in page["entries"].as_array().unwrap().clone() {
        let (family, row) = match entry["kind"].as_str().unwrap() {
            "thread" => ("threads".to_owned(), entry["thread"].clone()),
            "message" => ("messages".to_owned(), entry["message"].clone()),
            _ => (
                format!("{}s", entry["node"]["node_type"].as_str().unwrap()),
                entry["node"].clone(),
            ),
        };
        if let Some(rows) = page.get_mut(&family).and_then(Value::as_array_mut) {
            rows.push(row);
        }
    }
    page
}
async fn complete(root: camino::Utf8PathBuf) -> Value {
    let mut all = read(root.clone()).await;
    let stamp = all["stamp"].clone();
    while all["next_cursor"].is_string() {
        let next = catalog::invoke_with("read-document", provenance_core::SDK_PROTOCOL_VERSION,
            json!({"context":{"repository":"test","scope":"default"},"request":{"id":"req_overtime","cursor":all["next_cursor"]}}),
            Arc::new(Target(root.clone()))).await.unwrap();
        for field in ["serial", "digest", "instance_id", "derivation"] {
            assert_eq!(next["stamp"][field], stamp[field]);
        }
        all["entries"]
            .as_array_mut()
            .unwrap()
            .extend(next["entries"].as_array().unwrap().clone());
        all["next_cursor"] = next["next_cursor"].clone();
        all["has_more"] = next["has_more"].clone();
    }
    collections(all)
}

#[tokio::test]
async fn document_reads_all_saved_records_and_marks_failed_catch_up() {
    let (dir, store, scope) = seeded_store();
    let first = complete(root_of(&dir)).await;
    assert_eq!(first["requirements"].as_array().unwrap().len(), 1);
    let mut child =
        serde_json::to_value(store.list_requirements(&scope).unwrap()[0].clone()).unwrap();
    child["refines"] = json!("req_overtime");
    let path = crate::shards::requirements_path(&store.layout, &scope);
    for i in 0..205 {
        child["id"] = json!(format!("req_child_{i:03}"));
        crate::cache::tests::fixtures::append_record(&path, &child);
    }
    let next = complete(root_of(&dir)).await;
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
    let failed = catalog::invoke_with(
        "read-document",
        provenance_core::SDK_PROTOCOL_VERSION,
        json!({"context":{"repository":"test","scope":"default"},"request":{"id":"req_overtime"}}),
        Arc::new(Target(root_of(&dir))),
    )
    .await
    .unwrap_err();
    assert!(serde_json::to_string(&failed)
        .unwrap()
        .contains("document_catch_up_failed"));
}

#[tokio::test]
#[verifies("rule_review_partial_reads_remain_explicit", examples)]
async fn document_failed_catch_up_precedes_missing_root_in_old_projection() {
    use crate::cache::tests::fixtures::create_requirement;
    use crate::operations::read_policy::FreshnessPolicy;

    let (dir, store, scope) = seeded_store();
    read(root_of(&dir)).await;
    create_requirement(
        &store,
        &scope,
        "req_new",
        provenance_core::RequirementStatus::Active,
    );
    let path = crate::shards::domains_path(&store.layout, &scope);
    let saved = std::fs::read(&path).unwrap();
    std::fs::write(&path, "invalid JSON\n").unwrap();
    let request = ReadDocumentQuery {
        id: "req_new".into(),
        cursor: None,
        limit: 50,
    };
    let stale = super::super::read_document(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
        request.clone(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        stale.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::DocumentRootMissing)
    );

    let failed = catalog::invoke_with(
        "read-document", provenance_core::SDK_PROTOCOL_VERSION,
        json!({"context":{"repository":"test","scope":"default","freshness":"catch_up"},"request":{"id":"req_new"}}),
        Arc::new(Target(root_of(&dir))),
    ).await.unwrap_err();
    assert_eq!(failed.error, json!({"kind":"document_catch_up_failed"}));

    std::fs::write(path, saved).unwrap();
    let recovered =
        super::super::read_document(Some(root_of(&dir)), &scope, ReadPolicy::default(), request)
            .await
            .unwrap();
    assert_eq!(recovered.result.root_id.as_str(), "req_new");
    assert_eq!(
        recovered.stamp.policy,
        provenance_core::protocol::StampPolicy::CatchUp
    );
}

#[tokio::test]
async fn document_valid_freshness_preserves_page_refusals() {
    use crate::operations::read_policy::FreshnessPolicy;

    let (dir, store, scope) = seeded_store();
    let mut retired = json!(store.list_requirements(&scope).unwrap()[0]);
    retired["id"] = json!("req_retired");
    retired["retired"] = json!(true);
    crate::cache::tests::fixtures::append_record(
        &crate::shards::requirements_path(&store.layout, &scope),
        &retired,
    );
    read(root_of(&dir)).await;
    for freshness in [
        FreshnessPolicy::CatchUp,
        FreshnessPolicy::AnnotateOnly,
        FreshnessPolicy::RefuseStale,
    ] {
        for (id, cursor, expected) in [
            ("req_absent", None, ReadFailure::DocumentRootMissing),
            ("req_retired", None, ReadFailure::DocumentRootRetired),
            ("req_overtime", Some("invalid"), ReadFailure::CursorInvalid),
        ] {
            let error = super::super::read_document(
                Some(root_of(&dir)),
                &scope,
                ReadPolicy::with_freshness(freshness),
                ReadDocumentQuery {
                    id: id.into(),
                    cursor: cursor.map(str::to_owned),
                    limit: 50,
                },
            )
            .await
            .unwrap_err();
            assert_eq!(
                error.downcast_ref::<ReadFailure>(),
                Some(&expected),
                "{freshness:?}: {id}"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn document_keeps_graph_and_discussions_at_one_revision_during_publication() {
    use crate::operations::reader;
    use provenance_core::protocol::ReadDocumentQuery;
    let (dir, store, scope) = seeded_store();
    let root = root_of(&dir);
    let initial = complete(root.clone()).await;
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
            super::super::document::read(ctx, ReadDocumentQuery { id: "req_overtime".into(), cursor: None, limit: 50 }).await
        })
    }).await.unwrap();
    let old = collections(serde_json::to_value(&answer.result).unwrap());
    assert_eq!(old["requirements"].as_array().unwrap().len(), 1);
    assert!(old["threads"].as_array().unwrap().is_empty());
    assert!(old["messages"].as_array().unwrap().is_empty());
    assert_eq!(answer.stamp.digest, initial["stamp"]["digest"]);
    let refreshed = complete(root).await;
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
    use crate::cache::tests::fixtures::{attach_source, create_rule_of, create_source};
    let (dir, store, scope) = seeded_store();
    create_source(&store, &scope, "source_retired");
    create_rule_of(&store, &scope, "rule_retired", "req_overtime");
    attach_source(&store, &scope, "req_overtime", "source_retired");
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let mut retired =
        serde_json::to_value(store.list_requirements(&scope).unwrap()[0].clone()).unwrap();
    retired["id"] = json!("req_retired");
    retired["retired"] = json!(true);
    retired["refines"] = json!("req_overtime");
    crate::cache::tests::fixtures::append_record(&path, &retired);
    let mut declarations = vec![("requirements", retired)];
    for (family, path, mut record) in [
        (
            "sources",
            crate::shards::sources_path(&store.layout, &scope),
            json!(store.list_sources(&scope).unwrap()[0]),
        ),
        (
            "rules",
            crate::shards::rules_path(&store.layout, &scope),
            json!(store.list_rules(&scope).unwrap()[0]),
        ),
    ] {
        record["retired"] = json!(true);
        std::fs::write(path, format!("{record}\n")).unwrap();
        declarations.push((family, record));
    }
    let answer = complete(root_of(&dir)).await;
    for (family, expected) in declarations {
        let record = answer[family]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == expected["id"])
            .unwrap();
        let mut canonical = record.clone();
        canonical.as_object_mut().unwrap().remove("node_type");
        assert_eq!(canonical, expected, "{family}");
        assert_eq!(record["retired"], true);
    }
    assert_eq!(
        answer["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "req_overtime")
            .unwrap()["source_refs"][0]["source_id"],
        "source_retired"
    );
}

#[tokio::test]
async fn document_page_requires_continuation_and_excludes_other_roots() {
    let (dir, store, scope) = seeded_store();
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let mut child = json!(store.list_requirements(&scope).unwrap()[0]);
    for i in 0..205 {
        child["id"] = json!(format!("req_aaa_{i:03}"));
        child["refines"] = json!("req_overtime");
        crate::cache::tests::fixtures::append_record(&path, &child);
    }
    child["id"] = json!("req_outside");
    child["refines"] = Value::Null;
    crate::cache::tests::fixtures::append_record(&path, &child);
    let first = read(root_of(&dir)).await;
    assert_eq!(first["entries"][0]["node"]["id"], "req_overtime");
    assert_eq!(first["has_more"], true);
    assert!(first["next_cursor"].is_string());
    assert!(first["entries"].as_array().unwrap().len() <= 50);
}

#[tokio::test]
async fn search_returns_a_revision_bound_cursor() {
    let (dir, store, scope) = seeded_store();
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let mut row = json!(store.list_requirements(&scope).unwrap()[0]);
    for i in 0..205 {
        row["id"] = json!(format!("req_search_{i:03}"));
        crate::cache::tests::fixtures::append_record(&path, &row);
    }
    let first = catalog::invoke_with("search", provenance_core::SDK_PROTOCOL_VERSION,
        json!({"context":{"repository":"test","scope":"default"},"request":{"text":"req_","limit":200}}),
        Arc::new(Target(root_of(&dir)))).await.unwrap();
    assert_eq!(first["nodes"].as_array().unwrap().len(), 200);
    assert!(first["next_cursor"].is_string());
}
