use camino::Utf8Path;
use provenance_store::jsonl::write_jsonl_atomic;
use serde_json::json;

#[test]
fn unintegrated_atomic_writer_cannot_replace_an_enrolled_requirement() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let path = root.join(".provenance/state/scopes/default/requirements/req.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let before = json!({"schema_version":3,"scope_id":"default","id":"req_a","statement":"A"});
    std::fs::write(&path, format!("{before}\n")).unwrap();
    let result = write_jsonl_atomic(
        &path,
        &[json!({"schema_version":2,"scope_id":"default","id":"req_a","statement":"B"})],
    );
    assert!(
        result.is_err(),
        "an enrolled row must refuse an unintegrated writer"
    );
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        format!("{before}\n")
    );
}

#[test]
fn review_schema_is_readable_only_for_requirements_and_manifest() {
    use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":3,"scopes":[],"disposition_actor_ids":[]}"#,
    )
    .unwrap();
    assert!(StateStore::new(layout).manifest().is_ok());
    assert!(provenance_core::ensure_supported_schema_version(
        "proposal",
        provenance_core::SchemaVersion(3)
    )
    .is_err());
}

#[test]
#[ignore = "requires PROVENANCE_LEGACY_BIN from a version-2 build"]
fn installed_legacy_writer_refuses_the_enrolled_shard() {
    #[path = "review_support/mod.rs"]
    mod support;
    let (temp, store) = support::fixture();
    store
        .save_requirement(support::save(&store, "enroll", json!({})))
        .unwrap();
    let shard = temp
        .path()
        .join(".provenance/state/scopes/default/requirements/req.jsonl");
    let before = std::fs::read(&shard).unwrap();
    let binary = std::env::var("PROVENANCE_LEGACY_BIN").unwrap();
    let output = std::process::Command::new(binary)
        .args([
            "requirements",
            "fog",
            "set",
            "--repo",
            temp.path().to_str().unwrap(),
            "--scope",
            "default",
            "--requirement-id",
            "req_a",
            "--text",
            "old writer",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("schema_version"), "{error}");
    assert_eq!(std::fs::read(shard).unwrap(), before);
    eprintln!("legacy writer refusal: {error}");
}

#[test]
fn duplicate_enrolled_identity_cannot_hide_an_extra_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let path = root.join(".provenance/state/scopes/default/requirements/req.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let before = json!({"schema_version":3,"id":"req_a","statement":"A"});
    std::fs::write(&path, format!("{before}\n")).unwrap();
    let mut duplicate = before.clone();
    duplicate["statement"] = json!("B");
    assert!(write_jsonl_atomic(&path, &[before, duplicate]).is_err());
}
