use camino::Utf8Path;
use provenance_core::{ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout, review::SaveRequirement, state_store::StateStore,
};
use serde_json::{json, Value};

pub fn fixture() -> (tempfile::TempDir, StateStore) {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let store = StateStore::new(layout);
    store.create_requirement(serde_json::from_value(json!({"scope_id":"default","id":"req_a","statement":"The system stores records.","status":"discovery","depends_on":[],"supersedes":[]})).unwrap()).unwrap();
    (temp, store)
}

pub fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}
pub fn id() -> StableId {
    StableId::new("req_a").unwrap()
}

pub fn save(store: &StateStore, request: &str, fields: Value) -> SaveRequirement {
    let mut update = json!({"scope_id":"default", "id":"req_a"});
    let Value::Object(fields) = fields else {
        panic!("update fields must be an object")
    };
    update.as_object_mut().unwrap().extend(fields);
    serde_json::from_value(json!({
        "request_id":request,"actor":"ben", "expected_etag":store.requirement_edit_state(&scope(), &id()).unwrap().etag,
        "update":update,"relationships":null
    })).unwrap()
}
