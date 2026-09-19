#![cfg(all(feature = "test-fixture", unix))]
#[allow(dead_code)]
#[path = "support/records.rs"]
mod records;
use records::Repository;
use serde_json::json;

#[test]
fn native_verification_checks_relative_and_absolute_files_before_publication() {
    let repo = Repository::new("The evidence is readable.");
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("outside.rs"), "OUTSIDE_SENTINEL").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("outside.rs"),
        repo.dir.path().join("link.rs"),
    )
    .unwrap();
    let scope = provenance_core::ScopeId::new("default").unwrap();
    for file in [
        "link.rs".to_owned(),
        repo.dir.path().join("link.rs").to_str().unwrap().to_owned(),
        "../outside.rs".to_owned(),
    ] {
        let input = serde_json::from_value(json!({"rule":"rule_shared","key":"check","method":"examples","declared_by":"fixture","file":file})).unwrap();
        let error = provenance_store::operations::begin_verification(
            Some(repo.dir.path().to_str().unwrap().into()),
            scope.clone(),
            input,
        )
        .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<provenance_store::operations::files::FileAccessRefusal>(),
            Some(provenance_store::operations::files::FileAccessRefusal::Denied)
        ));
        assert!(!repo.layout.verification_runs_path(&scope).exists());
    }
    std::fs::write(repo.dir.path().join("valid.rs"), "fn check() {}\n").unwrap();
    let input = serde_json::from_value(json!({"rule":"rule_shared","key":"check","method":"examples","declared_by":"fixture","file":repo.dir.path().join("valid.rs")})).unwrap();
    let run = provenance_store::operations::begin_verification(
        Some(repo.dir.path().to_str().unwrap().into()),
        scope,
        input,
    )
    .unwrap();
    assert_eq!(run.file.unwrap(), "valid.rs");
}

#[test]
fn missing_native_verification_file_refuses_before_any_run_is_written() {
    let repo = Repository::new("The evidence is readable.");
    let scope = provenance_core::ScopeId::new("default").unwrap();
    let input = serde_json::from_value(json!({"rule":"rule_shared","key":"check","method":"examples","declared_by":"fixture","file":"missing.rs"})).unwrap();
    let error = provenance_store::operations::begin_verification(
        Some(repo.dir.path().to_str().unwrap().into()),
        scope.clone(),
        input,
    )
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<provenance_store::operations::files::FileAccessRefusal>(),
        Some(provenance_store::operations::files::FileAccessRefusal::Missing)
    ));
    assert!(!repo.layout.verification_runs_path(&scope).exists());
}
