use super::super::*;
use super::fixtures::*;

const PROJECTION_TABLES: [&str; 7] = [
    "implementation_bindings",
    "verification_bindings",
    "requirement_reviews",
    "projection_instance",
    "projection_revision",
    "projection_family_digests",
    "projection_unit_digests",
];

async fn table_exists(pool: &sqlx::SqlitePool, table: &str) -> bool {
    let found: Option<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(table)
            .fetch_optional(pool)
            .await
            .unwrap();
    found.is_some()
}

#[tokio::test]
async fn migration_creates_the_projection_stamp_and_family_tables() {
    let (_dir, layout, _scope) = empty_layout();
    materialize_empty_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    for table in PROJECTION_TABLES {
        assert!(table_exists(&pool, table).await, "missing table {table}");
    }
}

#[test]
fn projection_family_table_names_every_stored_family_once() {
    let names: Vec<&str> = ProjectionFamily::ALL
        .iter()
        .map(|family| family.family_name())
        .collect();
    assert_eq!(names.len(), 19);
    let mut unique = names.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 19, "family names must be unique");
    for expected in [
        "sources",
        "domains",
        "requirements",
        "boundaries",
        "topics",
        "questions",
        "edges",
        "resolutions",
        "rules",
        "messages",
        "threads",
        "contributions",
        "synthesis_packets",
        "proposal_cards",
        "assertion_records",
        "dispositions",
        "implementation_bindings",
        "verification_bindings",
        "requirement_reviews",
    ] {
        assert!(names.contains(&expected), "missing family {expected}");
    }
}

#[test]
fn only_the_edges_family_is_global() {
    for family in ProjectionFamily::ALL {
        assert_eq!(
            family.is_scoped(),
            family.family_name() != "edges",
            "scoping wrong for {}",
            family.family_name()
        );
    }
}

pub(super) fn seed_integration_shards(layout: &crate::layout::ProvenanceLayout, scope: &str) {
    let write = |path: camino::Utf8PathBuf, line: &str| {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("{line}\n")).unwrap();
    };
    let scope_id = provenance_core::ScopeId::new(scope).unwrap();
    write(
        crate::shards::implementation_bindings_path(layout, &scope_id),
        &format!(
            r#"{{"schema_version":1,"scope_id":"{scope}","id":"bind_impl_a","rule_id":"rule_schads_pay_001","declared_by":"agent","file":"src/pay.rs","symbol":"pay"}}"#
        ),
    );
    write(
        crate::shards::verification_bindings_path(layout, &scope_id),
        &format!(
            r#"{{"schema_version":1,"scope_id":"{scope}","id":"bind_ver_a","rule_id":"rule_schads_pay_001","key":"pay_examples","method":"examples","declared_by":"agent","file":"tests/pay.rs","symbol":"pay_works"}}"#
        ),
    );
    write(
        crate::shards::requirement_reviews_path(layout, &scope_id),
        &format!(
            r#"{{"schema_version":1,"scope_id":"{scope}","id":"review_a","rule_id":"rule_schads_pay_001","requirement_id":"req_schads_overtime","field":"statement","before":"Overtime","after":"Overtime pay","changed_at":1}}"#
        ),
    );
}

#[tokio::test]
async fn materialization_loads_binding_and_review_families_into_their_tables() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    for (table, id) in [
        ("implementation_bindings", "bind_impl_a"),
        ("verification_bindings", "bind_ver_a"),
        ("requirement_reviews", "review_a"),
    ] {
        let found: Option<String> = sqlx::query_scalar(&format!(
            "SELECT id FROM {table} WHERE scope_id = ? AND rule_id = 'rule_schads_pay_001'"
        ))
        .bind(scope.as_str())
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(found.as_deref(), Some(id), "missing row in {table}");
    }
}

async fn stamp(pool: &sqlx::SqlitePool) -> (i64, String, String) {
    let (serial, digest): (i64, String) = sqlx::query_as(
        "SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let instance: String = sqlx::query_scalar("SELECT instance_id FROM projection_instance")
        .fetch_one(pool)
        .await
        .unwrap();
    (serial, digest, instance)
}

#[tokio::test]
async fn materialization_stores_a_revision_stamp_with_instance_identity() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();

    let (serial, digest, instance) = stamp(&pool).await;
    // The first materialization of a fresh instance is serial one: no
    // journal consumes sequences ahead of it.
    assert_eq!(serial, 1);
    assert!(
        digest.starts_with("sha256:") && digest.len() == 71,
        "{digest}"
    );
    assert_eq!(
        instance.parse::<uuid::Uuid>().unwrap().get_version(),
        Some(uuid::Version::Random),
        "instance id must be an OS-entropy UUID: {instance}"
    );

    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT scope_id, family, content_digest, record_count FROM projection_family_digests ORDER BY family, scope_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 19, "one row per family for the one scope");
    for (scope_id, family, digest, count) in &rows {
        let expected_scope = if family == "edges" {
            ""
        } else {
            scope.as_str()
        };
        assert_eq!(scope_id, expected_scope, "scope for {family}");
        assert!(digest.starts_with("sha256:"), "digest for {family}");
        let expected_count = match family.as_str() {
            "sources"
            | "domains"
            | "requirements"
            | "resolutions"
            | "rules"
            | "implementation_bindings"
            | "verification_bindings"
            | "requirement_reviews" => 1,
            "edges" => 5,
            _ => 0,
        };
        assert_eq!(*count, expected_count, "count for {family}");
    }
}

#[tokio::test]
async fn rematerialization_of_unchanged_state_keeps_digest_and_instance_and_advances_serial() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (first_serial, first_digest, first_instance) = stamp(&pool).await;
    drop(pool);

    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (second_serial, second_digest, second_instance) = stamp(&pool).await;

    assert!(second_serial > first_serial, "serials only move forward");
    assert_eq!(first_digest, second_digest);
    assert_eq!(first_instance, second_instance);
}

#[test]
fn revision_digest_reproduces_from_a_walk_of_the_family_table() {
    let (_dir, layout, scope) = seeded_layout();
    seed_integration_shards(&layout, scope.as_str());
    let store = crate::state_store::StateStore::new(layout);
    let scopes = vec![scope.clone()];
    let assembled = revision_digest(&family_content_digests(&store, &scopes).unwrap()).unwrap();

    let mut walked = Vec::new();
    for family in ProjectionFamily::ALL {
        let keys: Vec<Option<&provenance_core::ScopeId>> = if family.is_scoped() {
            vec![Some(&scope)]
        } else {
            vec![None]
        };
        for key in keys {
            let (bytes, record_count) = family.canonical_records(&store, key).unwrap();
            walked.push(serde_json::json!({
                "family": family.family_name(),
                "scope_id": key.map_or("", provenance_core::ScopeId::as_str),
                "digest": crate::canonical_digest::digest(&bytes),
                "record_count": record_count,
            }));
        }
    }
    let reproduced = crate::canonical_digest::digest(
        &crate::canonical_digest::canonical_bytes(&walked).unwrap(),
    );
    assert_eq!(
        assembled, reproduced,
        "the family table is the digest domain"
    );
}

#[tokio::test]
async fn identical_repositories_agree_on_digest_but_never_on_instance() {
    let (_dir_a, layout_a, scope_a) = seeded_layout();
    let (_dir_b, layout_b, scope_b) = seeded_layout();
    seed_integration_shards(&layout_a, scope_a.as_str());
    seed_integration_shards(&layout_b, scope_b.as_str());
    materialize_state(&layout_a).await.unwrap();
    materialize_state(&layout_b).await.unwrap();

    let pool_a = open_cache(&layout_a).await.unwrap();
    let pool_b = open_cache(&layout_b).await.unwrap();
    let (_, digest_a, instance_a) = stamp(&pool_a).await;
    let (_, digest_b, instance_b) = stamp(&pool_b).await;

    assert_eq!(digest_a, digest_b, "same canonical state, same digest");
    assert_ne!(instance_a, instance_b, "each database is its own instance");
}

#[tokio::test]
async fn empty_state_materialization_stores_no_revision_and_no_instance() {
    let (_dir, layout, _scope) = empty_layout();
    materialize_empty_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let revisions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projection_revision")
        .fetch_one(&pool)
        .await
        .unwrap();
    let instances: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projection_instance")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(revisions, 0);
    assert_eq!(instances, 0);
}
