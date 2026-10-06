#[allow(dead_code)]
mod review_support;

use provenance_store::review::SaveRequirement;
use review_support::*;
use serde_json::json;

/// Implementation aid: the owner requires saves to preserve stored format fields.
#[test]
fn review_save_preserves_the_stored_schema_marker() {
    let (_temp, store) = fixture();
    store
        .apply_typed_spec(
            &scope(),
            serde_json::from_value(json!({
                "schema_version":2, "spec":"markers", "declared_by":"spec://markers",
                "requirements":[{"key":"base", "id":"req_marker",
                    "statement":"The system stores records."}]
            }))
            .unwrap(),
        )
        .unwrap();
    let id = provenance_core::StableId::new("req_marker").unwrap();
    let before = store
        .list_requirements(&scope())
        .unwrap()
        .into_iter()
        .find(|record| record.id == id)
        .unwrap();
    let edit = store.requirement_edit_state(&scope(), &id).unwrap();
    let input: SaveRequirement = serde_json::from_value(json!({
        "actor":"ben", "expected_etag":edit.etag,
        "update":{"scope_id":"default", "id":id, "declared_by":"spec://markers",
            "description":"The records have names."},
        "relationships":null
    }))
    .unwrap();
    store.save_requirement(input).unwrap();
    let after = store
        .list_requirements(&scope())
        .unwrap()
        .into_iter()
        .find(|record| record.id == id)
        .unwrap();
    assert_eq!(after.schema_version, before.schema_version);
}

/// Implementation aid: the regenerated client has no snapshot field.
#[test]
fn edit_state_omits_snapshot() {
    let (_temp, store) = fixture();
    let state = store.requirement_edit_state(&scope(), &id()).unwrap();
    let value = serde_json::to_value(state).unwrap();
    assert!(value.get("snapshot").is_none());
}

/// Implementation aid: retained stored markers cannot change review edit state.
#[test]
fn review_edit_state_ignores_retained_format_markers() {
    let (temp, store) = fixture();
    let before = store.requirement_edit_state(&scope(), &id()).unwrap();
    let layout = provenance_store::layout::ProvenanceLayout::new(
        camino::Utf8Path::from_path(temp.path()).unwrap(),
    );
    let path = provenance_store::shards::requirements_path(&layout, &scope());
    let mut stored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for marker in [2, 3] {
        stored["schema_version"] = json!(marker);
        std::fs::write(&path, format!("{stored}\n")).unwrap();
        assert_eq!(
            store.requirement_edit_state(&scope(), &id()).unwrap(),
            before
        );
    }
}
