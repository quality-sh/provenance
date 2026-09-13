use super::SaveRequirement;
use crate::{layout::ProvenanceLayout, state_store::StateStore, test_probes};
use camino::Utf8Path;
use provenance_core::{ScopeId, StableId};
use serde_json::json;

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}
fn id() -> StableId {
    StableId::new("req_a").unwrap()
}
fn open(root: &Utf8Path) -> StateStore {
    StateStore::new(ProvenanceLayout::new(root))
}
fn input(store: &StateStore, request: &str) -> SaveRequirement {
    serde_json::from_value(json!({"request_id":request,"actor":"ben", "expected_etag":store.requirement_edit_state(&scope(), &id()).unwrap().etag,
        "update":{"scope_id":"default","id":"req_a","description":request},"relationships":null})).unwrap()
}
fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let store = open(root);
    store.create_requirement(serde_json::from_value(json!({"scope_id":"default","id":"req_a","statement":"The system stores records.","status":"discovery","depends_on":[],"supersedes":[]})).unwrap()).unwrap();
    store.save_requirement(input(&store, "baseline")).unwrap();
    temp
}

#[test]
fn crash_child() {
    let Ok(root) = std::env::var("PROVENANCE_REVIEW_CRASH_ROOT") else {
        return;
    };
    let phase = std::env::var("PROVENANCE_REVIEW_CRASH_PHASE").unwrap();
    let phase = match phase.as_str() {
        "state_prepared" => "state_prepared",
        "state_marker_prepared" => "state_marker_prepared",
        "state_backup_created" => "state_backup_created",
        "state_installed" => "state_installed",
        "state_published" => "state_published",
        _ => panic!("unknown crash phase"),
    };
    let store = open(Utf8Path::new(&root));
    let input = input(&store, "crash_request");
    test_probes::arm(phase, || std::process::exit(86));
    store.save_requirement(input).unwrap();
    panic!("crash phase was not reached");
}

#[test]
fn process_crashes_reopen_as_complete_old_or_new_state() {
    for (phase, committed) in [
        ("state_prepared", false),
        ("state_marker_prepared", false),
        ("state_backup_created", false),
        ("state_installed", true),
        ("state_published", true),
    ] {
        let temp = fixture();
        let root = Utf8Path::from_path(temp.path()).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "review::recovery_tests::crash_child",
                "--nocapture",
            ])
            .env("PROVENANCE_REVIEW_CRASH_ROOT", root.as_str())
            .env("PROVENANCE_REVIEW_CRASH_PHASE", phase)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86), "{phase}");
        let store = open(root);
        let receipt = store
            .requirement_save_receipt(
                &scope(),
                &id(),
                &StableId::new("crash_request").unwrap(),
                "ben",
                None,
            )
            .unwrap();
        assert_eq!(receipt.is_some(), committed, "{phase}");
        let record = store.list_requirements(&scope()).unwrap().remove(0);
        assert_eq!(
            record.description.as_deref(),
            Some(if committed {
                "crash_request"
            } else {
                "baseline"
            }),
            "{phase}"
        );
        store.validated_review_entries(&scope()).unwrap();
        assert!(!ProvenanceLayout::new(root)
            .publication_marker_path()
            .exists());
    }
}

#[test]
fn failed_rollback_retains_recovery_material() {
    let temp = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    assert_failed_rollback(root);
}

#[cfg(any(unix, windows))]
#[test]
fn failed_rollback_recovers_through_a_symlinked_repository_parent() {
    let temp = fixture();
    let aliases = tempfile::tempdir().unwrap();
    let physical = Utf8Path::from_path(temp.path()).unwrap();
    let alias = Utf8Path::from_path(aliases.path()).unwrap().join("parent");
    #[cfg(unix)]
    std::os::unix::fs::symlink(physical.parent().unwrap(), &alias).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(physical.parent().unwrap(), &alias).unwrap();
    assert_failed_rollback(&alias.join(physical.file_name().unwrap()));
}

fn assert_failed_rollback(root: &Utf8Path) {
    let store = open(root);
    let input = input(&store, "failed");
    test_probes::crash_at("state_before_install");
    test_probes::crash_at("state_before_rollback");
    let outcome = store.save_requirement(input);
    test_probes::disarm("state_before_install");
    test_probes::disarm("state_before_rollback");
    assert!(outcome.is_err());
    assert!(ProvenanceLayout::new(root)
        .publication_marker_path()
        .exists());
    let store = open(root);
    assert!(store
        .requirement_save_receipt(
            &scope(),
            &id(),
            &StableId::new("failed").unwrap(),
            "ben",
            None
        )
        .unwrap()
        .is_none());
    assert_eq!(
        store.list_requirements(&scope()).unwrap()[0]
            .description
            .as_deref(),
        Some("baseline")
    );
}

#[test]
fn a_lost_result_is_uncertain_until_the_receipt_is_read_after_recovery() {
    let temp = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let store = open(root);
    let input = input(&store, "lost_result");
    test_probes::crash_at("state_published");
    let error = store.save_requirement(input).unwrap_err();
    test_probes::disarm("state_published");
    assert!(matches!(
        crate::write_error::WriteError(error).safe(),
        crate::write_error::WriteFailure::UncertainWrite
    ));
    let receipt = open(root)
        .requirement_save_receipt(
            &scope(),
            &id(),
            &StableId::new("lost_result").unwrap(),
            "ben",
            None,
        )
        .unwrap();
    assert!(receipt.is_some());
}

#[test]
fn a_relationship_only_review_save_updates_the_record_stamp() {
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    };
    git(&["init", "--quiet"]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.test",
        "commit",
        "--allow-empty",
        "-m",
        "First",
    ]);
    let store = open(root);
    for id in ["req_a", "req_b"] {
        store
            .create_requirement(
                serde_json::from_value(json!({
                    "scope_id":"default", "id":id, "statement":"The system stores records.",
                    "status":"active", "depends_on":[], "supersedes":[]
                }))
                .unwrap(),
            )
            .unwrap();
    }
    store.save_requirement(input(&store, "baseline")).unwrap();
    let before = store.requirement(&scope(), &id()).unwrap();
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.test",
        "commit",
        "--allow-empty",
        "-m",
        "Second",
    ]);
    let second = git(&["rev-parse", "HEAD"]);
    let save: SaveRequirement = serde_json::from_value(json!({
        "request_id":"relationships", "actor":"ben",
        "expected_etag":store.requirement_edit_state(&scope(), &id()).unwrap().etag,
        "update":{"scope_id":"default","id":"req_a"},
        "relationships":{"refines":null,"depends_on":["req_b"],"supersedes":[],"spawned_by":null,"source_refs":[]}
    })).unwrap();
    store.save_requirement(save).unwrap();
    let after = store.requirement(&scope(), &id()).unwrap();

    assert_eq!(after.created, before.created);
    assert_eq!(after.updated.unwrap().commit, second);
}
