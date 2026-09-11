#[test]
fn enrolled_requirements_keep_review_history_out_of_the_pinned_graph() {
    let temp = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let layout = crate::layout::ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let store = crate::state_store::StateStore::new(layout);
    let scope = provenance_core::ScopeId::new("default").unwrap();
    let id = provenance_core::StableId::new("req_a").unwrap();
    store.create_requirement(serde_json::from_value(serde_json::json!({"scope_id":"default","id":"req_a","statement":"The system stores records.","status":"active","depends_on":[],"supersedes":[]})).unwrap()).unwrap();
    let save = |request: &str| {
        store
            .save_requirement(
                serde_json::from_value(serde_json::json!({"request_id":request,"actor":"ben",
            "expected_etag":store.requirement_edit_state(&scope,&id).unwrap().etag,
            "update":{"scope_id":"default","id":"req_a"},"relationships":null}))
                .unwrap(),
            )
            .unwrap();
    };
    save("enroll");
    let before = super::super::load_projection(root, "default").unwrap();
    save("noop");
    assert_eq!(
        before,
        super::super::load_projection(root, "default").unwrap()
    );
}
