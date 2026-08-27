use super::super::*;
use super::fixtures::*;
use crate::canonical_digest::{canonical_bytes, digest};
use crate::graph_reference::{graph_digest, projection::load_projection};
use crate::{cache::projection_families, state_store::StateStore};

const CONTRIBUTION_LINE: &str = r#"{"schema_version":1,"scope_id":"default","id":"contrib_new","target":{"artifact_type":"requirement","artifact_id":"req_schads_overtime"},"participant_slot":"reviewer","stance":"support","strongest_finding":"finding","evidence_references":[],"material_claims":[],"risks":[],"objections":[],"challenges":[],"suggested_artifact_changes":[],"unsupported_recommendations":[],"uncertainty":{"level":"low","rationale":"none"},"open_questions":[]}"#;

#[test]
fn same_canonical_state_digests_identically() {
    let (_left_dir, left, _scope) = seeded_layout();
    let (_right_dir, right, _scope) = seeded_layout();

    let left_digest = projection_digest(&left).unwrap();
    let right_digest = projection_digest(&right).unwrap();

    assert_eq!(left_digest, right_digest);
    assert!(left_digest.starts_with("sha256:"));
}

#[test]
fn changing_one_family_record_changes_projection_digest() {
    let (_dir, layout, _scope) = seeded_layout();
    let before = projection_digest(&layout).unwrap();

    let domain_shard =
        crate::shards::domains_path(&layout, &provenance_core::ScopeId::new("default").unwrap());
    let edited = std::fs::read_to_string(&domain_shard)
        .unwrap()
        .replace("Payroll", "Renumeration");
    std::fs::write(&domain_shard, edited).unwrap();

    let after = projection_digest(&layout).unwrap();

    assert_ne!(before, after);
}

#[test]
fn ideation_change_moves_projection_digest_but_not_graph_digest() {
    let (_dir, layout, scope) = seeded_layout();
    let before = projection_digest(&layout).unwrap();
    let graph_before =
        graph_digest(&load_projection(layout.root(), scope.as_str()).unwrap()).unwrap();

    let ideation = crate::shards::contributions_path(&layout, &scope);
    std::fs::create_dir_all(ideation.parent().unwrap()).unwrap();
    let mut lines = std::fs::read_to_string(&ideation).unwrap_or_default();
    lines.push_str(CONTRIBUTION_LINE);
    lines.push('\n');
    std::fs::write(&ideation, lines).unwrap();

    let after = projection_digest(&layout).unwrap();
    let graph_after =
        graph_digest(&load_projection(layout.root(), scope.as_str()).unwrap()).unwrap();

    assert_ne!(
        before, after,
        "ideation content must move the projection digest"
    );
    assert_eq!(
        graph_before, graph_after,
        "ideation content must not move the graph digest"
    );
}

#[test]
fn digest_assembler_is_table_driven() {
    let (_dir, layout, _scope) = seeded_layout();
    let store = StateStore::new(layout.clone());
    let manifest = store.manifest().unwrap();

    let mut families_json = Vec::new();
    for family in projection_families::PROJECTION_FAMILIES {
        let mut records =
            projection_families::family_records(family, &store, &manifest.scopes).unwrap();
        records.sort_by(|left, right| {
            let key = |value: &serde_json::Value| {
                (
                    value
                        .get("scope_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    value
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                )
            };
            key(left).cmp(&key(right))
        });
        families_json.push(serde_json::json!({
            "family": family.name,
            "records": records,
        }));
    }
    let walked =
        digest(&canonical_bytes(&serde_json::json!({ "families": families_json })).unwrap());

    let assembled = projection_digest(&layout).unwrap();

    assert_eq!(assembled, walked);
}
