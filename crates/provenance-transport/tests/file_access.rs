#![cfg(all(feature = "test-fixture", unix))]
#[allow(dead_code)]
#[path = "support/records.rs"]
mod records;
use records::{call, host, Repository};
use serde_json::json;

#[tokio::test]
async fn selected_source_paths_cannot_read_an_outside_sentinel() {
    let repo = Repository::new("The evidence is readable.");
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("outside.rs"),
        "#[rule(\"rule_shared\")]\nfn OUTSIDE_SENTINEL() {}\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("outside.rs"),
        repo.dir.path().join("link.rs"),
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), repo.dir.path().join("ancestor")).unwrap();
    let host = host(&[("selected", &repo)], &["selected"]);
    for file in [
        "link.rs",
        "ancestor/outside.rs",
        "../outside.rs",
        "/tmp/outside.rs",
        "C:\\outside.rs",
        "src/../outside.rs",
    ] {
        let (status, answer) = call(
            &host,
            "resolve-symbol",
            json!({"context":{"repository":"selected","scope":"default"},"request":{"file":file}}),
        )
        .await;
        assert_eq!(status, 403, "{file}: {answer}");
        assert_eq!(answer["error"]["kind"], "file_access_denied");
        assert!(!answer.to_string().contains("OUTSIDE_SENTINEL"));
    }
    let (status, answer) = call(&host, "impact", json!({"context":{"repository":"selected","scope":"default"},"request":{"id":"rule_shared"}})).await;
    assert_eq!(status, 403, "{answer}");
    assert_eq!(answer["error"]["kind"], "file_access_denied");
    host.shutdown().await;
}

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
