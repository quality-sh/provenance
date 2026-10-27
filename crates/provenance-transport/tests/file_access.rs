#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_macros::verifies;
use provenance_store::write_error::{WriteError, WriteFailure};
use serde_json::json;
use support::records::Repository;
use support::resource_http::{call, host};

#[cfg(unix)]
#[tokio::test]
#[verifies("rule_verification_file_access_before_publication", examples)]
async fn verification_refuses_a_symlink_outside_the_repository_before_publication() {
    let repo = Repository::new("The evidence is readable.");
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("outside.rs"), "OUTSIDE_SENTINEL").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("outside.rs"),
        repo.dir.path().join("link.rs"),
    )
    .unwrap();
    let scope = provenance_core::ScopeId::new("default").unwrap();
    let host = host(&repo, true);
    let (status, failure) = call(
        &host,
        "POST",
        "/verification-runs/begin-verification",
        Some(json!({"data":{"rule":"rule_shared","key":"check","method":"examples","declared_by":"fixture","file":"link.rs"}})),
    )
    .await;
    assert_eq!(status, 403, "{failure}");
    assert_eq!(failure["error"]["kind"], "file_access_denied");
    assert!(!repo.layout.verification_runs_path(&scope).exists());

    // HTTP rejects these path forms before file access. The native API accepts them.
    for file in [
        repo.dir.path().join("link.rs").to_str().unwrap().to_owned(),
        "../outside.rs".to_owned(),
    ] {
        let input = serde_json::from_value(json!({"rule":"rule_shared","key":"check","method":"examples","declared_by":"fixture","file":file})).unwrap();
        let error = WriteError::from(
            provenance_store::operations::begin_verification(
                Some(repo.dir.path().to_str().unwrap().into()),
                scope.clone(),
                input,
            )
            .unwrap_err(),
        );
        assert!(matches!(error.safe(), WriteFailure::FileAccessDenied));
        assert_eq!(error.status(), 403);
        assert!(!repo.layout.verification_runs_path(&scope).exists());
    }
}

#[test]
#[verifies("rule_verification_run_path_is_relative", examples)]
fn an_absolute_evidence_path_is_stored_relative_to_the_repository() {
    let repo = Repository::new("The evidence is readable.");
    let scope = provenance_core::ScopeId::new("default").unwrap();
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

#[tokio::test]
#[verifies("rule_verification_file_access_before_publication", examples)]
async fn missing_verification_file_refuses_before_any_run_is_written() {
    let repo = Repository::new("The evidence is readable.");
    let scope = provenance_core::ScopeId::new("default").unwrap();
    let host = host(&repo, true);
    let (status, failure) = call(
        &host,
        "POST",
        "/verification-runs/begin-verification",
        Some(json!({"data":{"rule":"rule_shared","key":"check","method":"examples","declared_by":"fixture","file":"missing.rs"}})),
    )
    .await;
    assert_eq!(status, 503, "{failure}");
    assert_eq!(failure["error"]["kind"], "file_unavailable");
    assert!(!repo.layout.verification_runs_path(&scope).exists());
}
