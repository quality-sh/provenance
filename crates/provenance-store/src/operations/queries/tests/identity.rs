use super::{root_of, seeded_store};
use crate::cache::{open_cache, catch_up_state};
use crate::operations::queries;
use crate::operations::read_policy::ReadPolicy;
use provenance_core::protocol::{RecordResolution, ResolveRecordQuery};
use provenance_core::{NodeType, SDK_PROTOCOL_VERSION};

fn request(id: &str, allowed_node_types: Vec<NodeType>) -> ResolveRecordQuery {
    ResolveRecordQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: id.into(),
        allowed_node_types,
    }
}

#[tokio::test]
async fn repository_identity_resolves_a_typed_node_from_one_snapshot() {
    let (dir, _store, scope) = seeded_store();

    let answer = queries::resolve_record(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        request("req_overtime", NodeType::ALL.to_vec()),
    )
    .await
    .unwrap();

    let RecordResolution::Found(node) = answer.result.resolution else {
        panic!("record must resolve")
    };
    assert_eq!(node.node_type(), NodeType::Requirement);
    assert_eq!(node.id().as_str(), "req_overtime");
    assert_eq!(answer.stamp.attested, ["record_identities", "requirements"]);
}

#[tokio::test]
async fn identity_resolution_hides_disallowed_kinds_as_missing() {
    let (dir, _store, scope) = seeded_store();

    let answer = queries::resolve_record(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        request("domain_payroll", vec![NodeType::Requirement]),
    )
    .await
    .unwrap();

    assert!(matches!(
        answer.result.resolution,
        RecordResolution::Missing
    ));
    assert_eq!(answer.stamp.attested, ["record_identities"]);
}

#[tokio::test]
async fn identity_resolution_does_not_reveal_another_bound_scope() {
    let (dir, _store, scope) = seeded_store();
    let layout = crate::layout::ProvenanceLayout::new(root_of(&dir));
    catch_up_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    sqlx::query(
        "INSERT INTO record_identities (scope_id, id, node_type) VALUES (?, ?, ?)",
    )
    .bind("other")
    .bind("foreign_record")
    .bind("requirement")
    .execute(pool.pool())
    .await
    .unwrap();
    pool.close().await.unwrap();

    let answer = queries::resolve_record(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        request("foreign_record", NodeType::ALL.to_vec()),
    )
    .await
    .unwrap();

    assert!(matches!(
        answer.result.resolution,
        RecordResolution::Missing
    ));
}

#[tokio::test]
async fn identity_index_rejects_one_id_in_another_scope_and_kind() {
    let (dir, _store, _scope) = seeded_store();
    let layout = crate::layout::ProvenanceLayout::new(root_of(&dir));
    catch_up_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();

    let error = sqlx::query(
        "INSERT INTO record_identities (scope_id, id, node_type) VALUES (?, ?, ?)",
    )
    .bind("other")
    .bind("req_overtime")
    .bind("rule")
    .execute(pool.pool())
    .await
    .unwrap_err();

    assert!(error
        .as_database_error()
        .is_some_and(sqlx::error::DatabaseError::is_unique_violation));
    pool.close().await.unwrap();
}

#[tokio::test]
async fn hidden_duplicate_kind_does_not_change_the_visible_resolution() {
    let (dir, _store, scope) = seeded_store();
    let layout = crate::layout::ProvenanceLayout::new(root_of(&dir));
    catch_up_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    // Simulate a damaged projection so the resolver's visibility rule is tested.
    sqlx::query("DROP INDEX idx_record_identities_id")
        .execute(pool.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO record_identities (scope_id, id, node_type) VALUES (?, ?, ?)",
    )
    .bind(scope.as_str())
    .bind("req_overtime")
    .bind("rule")
    .execute(pool.pool())
    .await
    .unwrap();
    pool.close().await.unwrap();

    let answer = queries::resolve_record(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        request("req_overtime", vec![NodeType::Requirement]),
    )
    .await
    .unwrap();

    let RecordResolution::Found(node) = answer.result.resolution else {
        panic!("one authorized record must resolve")
    };
    assert_eq!(node.node_type(), NodeType::Requirement);
}

#[tokio::test]
async fn identity_resolution_reports_an_ambiguous_visible_index() {
    let (dir, _store, scope) = seeded_store();
    let layout = crate::layout::ProvenanceLayout::new(root_of(&dir));
    catch_up_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    // Simulate a damaged projection so the resolver's ambiguity rule is tested.
    sqlx::query("DROP INDEX idx_record_identities_id")
        .execute(pool.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO record_identities (scope_id, id, node_type) VALUES (?, ?, ?)",
    )
    .bind(scope.as_str())
    .bind("req_overtime")
    .bind("rule")
    .execute(pool.pool())
    .await
    .unwrap();
    pool.close().await.unwrap();

    let answer = queries::resolve_record(
        Some(root_of(&dir)),
        &scope,
        ReadPolicy::default(),
        request("req_overtime", NodeType::ALL.to_vec()),
    )
    .await
    .unwrap();

    assert!(matches!(
        answer.result.resolution,
        RecordResolution::Ambiguous
    ));
}
