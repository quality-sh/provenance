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

    let RecordResolution::Found(node) = answer.result else {
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

    assert!(matches!(answer.result, RecordResolution::Missing));
    assert_eq!(answer.stamp.attested, ["record_identities"]);
}

#[tokio::test]
async fn identity_resolution_reports_an_ambiguous_visible_index() {
    let (dir, _store, scope) = seeded_store();
    let layout = crate::layout::ProvenanceLayout::new(root_of(&dir));
    catch_up_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
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

    assert!(matches!(answer.result, RecordResolution::Ambiguous));
}
