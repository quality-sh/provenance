//! What a schema rebuild of the file cache leaves behind.
//!
//! No Rule: these tests are characterizations of the rebuild. The first
//! holds that a rebuild leaves every table of the store as the first
//! build made it, so the order in which the rebuild drops the objects of
//! the store is not visible to a reader. The second holds what a foreign
//! key from a table of another name does to the rebuild.

use super::fixtures::pinned_store::pinned_store_layout;
use super::fixtures::{open, seeded_layout, sid};
use crate::cache::{catch_up_state, materialize_state, open_cache, ProjectionFamily};
use crate::layout::ProvenanceLayout;
use crate::state_store::{CreateQuestionInput, CreateTopicInput, StateStore};
use provenance_core::{QuestionStatus, ResolutionMethod, ScopeId, TopicStatus};
use serde_json::json;

/// The tables of the store that hold one row by design.
const ONE_ROW: [&str; 4] = [
    "_cache_metadata",
    "projection_instance",
    "projection_revision",
    "projection_validation",
];

/// Each object of the catalog, then each row of each table, in a fixed
/// order, and the number of rows of each table.
async fn everything(layout: &ProvenanceLayout) -> (Vec<String>, Vec<(String, usize)>) {
    let cache = open_cache(layout).await.unwrap();
    let pool = cache.pool();
    let mut dump: Vec<String> = sqlx::query_scalar(
        "SELECT type || '|' || name || '|' || tbl_name || '|' || COALESCE(sql, '') \
         FROM sqlite_schema ORDER BY type, name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_schema WHERE type = 'table' \
         AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut counts = Vec::new();
    for table in tables {
        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
                .bind(&table)
                .fetch_all(pool)
                .await
                .unwrap();
        // The time of a revision is the second that it was written in.
        let quoted: Vec<String> = columns
            .iter()
            .filter(|column| !(table == "projection_revision" && *column == "created_at"))
            .map(|column| format!("quote(\"{column}\")"))
            .collect();
        let select = format!(
            "SELECT {} FROM \"{table}\" ORDER BY 1",
            quoted.join(" || '|' || ")
        );
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(select))
            .fetch_all(pool)
            .await
            .unwrap();
        counts.push((table.clone(), rows.len()));
        dump.extend(rows.into_iter().map(|row| format!("{table}: {row}")));
    }
    cache.close().await.unwrap();
    (dump, counts)
}

/// Appends `records` to the shard of `family`.
fn append(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
    family: ProjectionFamily,
    records: &[serde_json::Value],
) {
    let path = family.shard_path(layout, scope);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    for record in records {
        text.push_str(&record.to_string());
        text.push('\n');
    }
    std::fs::write(path, text).unwrap();
}

/// Two ideation aggregates on `req_overtime`, `a` and `b`: each with a
/// contribution and its claim, a synthesis packet, a proposal, an
/// assertion, and a deferred disposition by `reviewer`.
fn two_ideation_aggregates(layout: &ProvenanceLayout, scope: &ScopeId) {
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let target = json!({"artifact_type": "requirement", "artifact_id": "req_overtime"});
    let scope = scope.as_str();
    for (family, make) in [
        (
            ProjectionFamily::Contributions,
            contribution as fn(&str, &str, &serde_json::Value) -> serde_json::Value,
        ),
        (ProjectionFamily::SynthesisPackets, packet),
        (ProjectionFamily::ProposalCards, proposal),
        (ProjectionFamily::AssertionRecords, assertion),
        (ProjectionFamily::Dispositions, disposition),
    ] {
        let records: Vec<_> = ["a", "b"]
            .iter()
            .map(|key| make(scope, key, &target))
            .collect();
        append(layout, &ScopeId::new(scope).unwrap(), family, &records);
    }
}

fn contribution(scope: &str, key: &str, target: &serde_json::Value) -> serde_json::Value {
    json!({"scope_id": scope, "id": format!("contribution_{key}"), "target": target,
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Observed",
        "evidence_references": [{"reference_id": format!("evidence_{key}"),
            "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": format!("claim_{key}"), "statement": "Observed",
            "evidence_type": "source", "evidence_reference_ids": [format!("evidence_{key}")]}],
        "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [],
        "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []})
}

fn packet(scope: &str, key: &str, target: &serde_json::Value) -> serde_json::Value {
    json!({"scope_id": scope, "id": format!("packet_{key}"), "target": target,
        "summary": "Adjudicated", "consensus": [], "contested_claims": [],
        "minority_objections": [], "evidence_gaps": [], "unsupported_speculation": [],
        "open_questions": [],
        "suggested_artifacts": [{"proposal_id": format!("proposal_{key}"),
            "proposal_key": key, "proposal_type": "requirement_candidate",
            "summary": "Candidate", "origin_participant_slots": ["reviewer"]}],
        "required_human_decisions": []})
}

fn proposal(scope: &str, key: &str, target: &serde_json::Value) -> serde_json::Value {
    json!({"scope_id": scope, "id": format!("proposal_{key}"), "proposal_key": key,
        "proposal_type": "requirement_candidate", "title": key, "summary": key,
        "traceability": {"target": target, "source_ids": [], "evidence_references": [],
            "supporting_claim_ids": [format!("claim_{key}")]},
        "promotion_state": "proposed"})
}

fn assertion(scope: &str, key: &str, _target: &serde_json::Value) -> serde_json::Value {
    json!({"scope_id": scope, "id": format!("assertion_{key}"),
        "proposal_id": format!("proposal_{key}"), "synthesis_packet_id": format!("packet_{key}"),
        "supporting_claim_ids": [format!("claim_{key}")]})
}

fn disposition(scope: &str, key: &str, _target: &serde_json::Value) -> serde_json::Value {
    json!({"scope_id": scope, "id": format!("disposition_{key}"),
        "proposal_id": format!("proposal_{key}"), "decision": "deferred",
        "rationale": "Deferred for now", "actor": {"identity_type": "human", "id": "reviewer"}})
}

/// A second topic and question, two Discussions, and two withdrawn
/// reviews, written through the store.
fn second_records(store: &StateStore, scope: &ScopeId) {
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: sid("topic_second"),
            requirement_id: sid("req_overtime"),
            title: "Second".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .unwrap();
    store
        .create_question(CreateQuestionInput {
            depends_on: Vec::new(),
            scope_id: scope.clone(),
            id: sid("question_second"),
            topic_id: sid("topic_second"),
            question: "Which one?".into(),
            resolution_method: ResolutionMethod::Ask,
            status: QuestionStatus::Open,
            answer: None,
            links: Vec::new(),
            resolution_id: None,
            contradicts: None,
        })
        .unwrap();
    for parent in ["req_overtime", "req_top"] {
        store
            .write_discussion(
                serde_json::from_value(json!({
                    "scope_id": scope, "actor": "reviewer", "declared_by": null,
                    "parent": {"node_type": "requirement", "node_id": parent},
                    "action": {"kind": "start", "role": "user", "body": "A concern"}
                }))
                .unwrap(),
            )
            .unwrap();
    }
    for id in ["req_top", "req_left"] {
        let id = sid(id);
        let etag = store.requirement_edit_state(scope, &id).unwrap().etag;
        store
            .save_requirement(
                serde_json::from_value(json!({
                    "actor": "reviewer", "expected_etag": etag,
                    "update": {"scope_id": scope, "id": id, "description": "Reviewed"},
                    "relationships": null
                }))
                .unwrap(),
            )
            .unwrap();
        let pending = store
            .requirement_decision_state(scope, &id)
            .unwrap()
            .pending
            .unwrap();
        store
            .withdraw_record_review(
                serde_json::from_value(json!({
                    "scope_id": scope, "actor": "reviewer",
                    "proposal_id": pending.proposal_id, "declared_by": null, "reason": null
                }))
                .unwrap(),
            )
            .unwrap();
    }
}

async fn execute(layout: &ProvenanceLayout, statements: &[&str]) {
    let cache = open_cache(layout).await.unwrap();
    for sql in statements {
        sqlx::query(sqlx::AssertSqlSafe(*sql))
            .execute(cache.pool())
            .await
            .unwrap();
    }
    cache.close().await.unwrap();
}

/// The rows of `dump` of the table `table`, and the other rows.
fn take(dump: Vec<String>, table: &str) -> (Vec<String>, Vec<String>) {
    let prefix = format!("{table}: ");
    dump.into_iter().partition(|row| row.starts_with(&prefix))
}

/// A schema rebuild and a full rebuild of a cache with more than one row
/// in each table of the store give the catalog and every row of every
/// table that the first build gave, with two exceptions. The schema
/// rebuild makes a new instance id. The full rebuild over a current schema
/// keeps the instance and adds revision 2 with the digest of revision 1.
#[tokio::test]
async fn a_rebuild_leaves_every_table_as_the_first_build_made_it() {
    let (_dir, layout, scope) = pinned_store_layout();
    two_ideation_aggregates(&layout, &scope);
    second_records(&crate::test_support::store(&layout), &scope);
    let repository = open(&layout);
    let first = catch_up_state(&repository).await.unwrap();
    assert!(first.rebuilt);
    let (made, counts) = everything(&layout).await;
    let thin: Vec<&(String, usize)> = counts
        .iter()
        .filter(|(table, rows)| *rows < 2 && !ONE_ROW.contains(&table.as_str()))
        .collect();
    assert_eq!(thin, Vec::<&(String, usize)>::new(), "{counts:?}");
    let (instance, made) = take(made, "projection_instance");
    let (revision, made) = take(made, "projection_revision");
    assert_eq!((instance.len(), revision.len()), (1, 1));
    execute(
        &layout,
        &["UPDATE _cache_metadata SET schema_digest = 'another'"],
    )
    .await;
    let pass = catch_up_state(&repository).await.unwrap();
    assert!(pass.rebuilt && pass.cache_recreated);
    let (new_instance, rebuilt) = take(everything(&layout).await.0, "projection_instance");
    let (new_revision, rebuilt) = take(rebuilt, "projection_revision");
    assert_eq!(rebuilt, made, "the schema rebuild");
    assert_eq!(new_revision, revision, "the schema rebuild");
    assert_ne!(
        new_instance, instance,
        "the schema rebuild kept the instance id"
    );
    materialize_state(&repository).await.unwrap();
    let (kept_instance, rebuilt) = take(everything(&layout).await.0, "projection_instance");
    let (revisions, rebuilt) = take(rebuilt, "projection_revision");
    assert_eq!(rebuilt, made, "the full rebuild");
    assert_eq!(kept_instance, new_instance, "the full rebuild");
    let second = revision[0].replacen("projection_revision: 1|", "projection_revision: 2|", 1);
    assert_eq!(
        revisions,
        vec![revision[0].clone(), second],
        "the full rebuild"
    );
}

/// A table of another name with a foreign key into a table of the store
/// stops each rebuild while it holds a row that refers to a row of the
/// store: the file checks foreign keys, so the drop of the table of the
/// store fails. The table and its row stay. Once the row is removed, the
/// next catch-up rebuilds.
#[tokio::test]
async fn a_foreign_key_into_a_table_of_the_store_stops_each_rebuild() {
    let (_dir, layout, _scope) = seeded_layout();
    let repository = open(&layout);
    catch_up_state(&repository).await.unwrap();
    execute(
        &layout,
        &[
            "CREATE TABLE embedder_links (scope_id TEXT, id TEXT, \
             FOREIGN KEY (scope_id, id) REFERENCES requirements (scope_id, id))",
            "INSERT INTO embedder_links VALUES ('default', 'req_schads_overtime')",
            "UPDATE _cache_metadata SET schema_digest = 'another'",
        ],
    )
    .await;
    for pass in 0..2 {
        let failed = catch_up_state(&repository).await.unwrap_err();
        assert!(
            format!("{failed:#}").contains("FOREIGN KEY constraint failed"),
            "pass {pass}: {failed:#}"
        );
    }
    let links: Vec<String> = everything(&layout)
        .await
        .0
        .into_iter()
        .filter(|row| row.starts_with("embedder_links: "))
        .collect();
    assert_eq!(
        links,
        vec!["embedder_links: 'default'|'req_schads_overtime'"]
    );
    execute(&layout, &["DELETE FROM embedder_links"]).await;
    let pass = catch_up_state(&repository).await.unwrap();
    assert!(pass.rebuilt && pass.cache_recreated);
}
