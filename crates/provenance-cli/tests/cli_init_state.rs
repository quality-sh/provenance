#[cfg(target_os = "linux")]
use assert_cmd::prelude::CommandCargoExt;
use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
use std::path::Path;

#[path = "cli_dictionary/support.rs"]
#[allow(dead_code)]
mod dictionary_support;

fn init(root: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let asset_dir = root.join("asset-cache");
    std::fs::create_dir_all(&asset_dir).unwrap();
    std::fs::write(
        asset_dir.join("ASD-STE100_ISSUE9.pdf"),
        dictionary_support::dictionary_pdf(),
    )
    .unwrap();
    let mut command = Command::cargo_bin("provenance").unwrap();
    command
        .current_dir(root)
        .env("PROVENANCE_STE100_ASSET_DIR", asset_dir)
        .env(
            "PROVENANCE_STE100_INDEX_DIR",
            root.join("dictionary-indexes"),
        )
        .arg("init")
        .args(args);
    command.assert()
}

fn manifest(root: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(root.join(".provenance/state/manifest.json")).unwrap())
        .unwrap()
}

#[test]
fn bare_init_defaults_to_current_directory_and_default_scope() {
    let temporary = tempfile::tempdir().unwrap();
    init(temporary.path(), &[]).success().stdout(contains(
        "Have your agent run provenance prime to get acclimated.",
    ));
    let state = manifest(temporary.path());
    assert_eq!(state["scopes"][0]["id"], "default");
    assert!(!temporary.path().join("asset-cache/.provenance").exists());
}

#[test]
fn explicit_path_and_scope_still_select_the_new_graph() {
    let temporary = tempfile::tempdir().unwrap();
    init(temporary.path(), &["--path", "child", "--scope", "review"]).success();
    assert_eq!(
        manifest(&temporary.path().join("child"))["scopes"][0]["id"],
        "review"
    );
    assert!(!temporary
        .path()
        .join(".provenance/state/manifest.json")
        .exists());
}

#[test]
fn bare_rerun_keeps_the_manifest_scope_and_actor_ids() {
    let temporary = tempfile::tempdir().unwrap();
    init(
        temporary.path(),
        &["--scope", "review", "--disposition-actor-id", "reviewer"],
    )
    .success();
    let manifest_path = temporary.path().join(".provenance/state/manifest.json");
    let before = std::fs::read(&manifest_path).unwrap();
    #[cfg(unix)]
    let inode_before = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&manifest_path).unwrap().ino()
    };
    init(temporary.path(), &[])
        .success()
        .stdout(contains("No change."));
    assert_eq!(std::fs::read(&manifest_path).unwrap(), before);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            std::fs::metadata(&manifest_path).unwrap().ino(),
            inode_before
        );
    }
    assert_eq!(
        manifest(temporary.path())["disposition_actor_ids"][0],
        "reviewer"
    );
}

#[test]
fn missing_manifest_gives_init_guidance_before_graph_reads_or_catalog_writes() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    for args in [
        vec!["health"],
        vec!["search", "--text", "probe"],
        vec![
            "sources",
            "create",
            "--id",
            "source_probe",
            "--name",
            "Probe",
            "--source-type",
            "project_artifact",
            "--reference",
            "probe",
        ],
    ] {
        let mut command = Command::cargo_bin("provenance").unwrap();
        command.current_dir(root).args(args);
        command
            .assert()
            .failure()
            .stderr(contains("provenance init"));
        assert!(!root.join(".provenance/state/scopes").exists());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn init_reports_a_failed_summary_after_publishing_state() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let asset_dir = root.join("asset-cache");
    std::fs::create_dir_all(&asset_dir).unwrap();
    std::fs::write(
        asset_dir.join("ASD-STE100_ISSUE9.pdf"),
        dictionary_support::dictionary_pdf(),
    )
    .unwrap();
    let mut command = std::process::Command::cargo_bin("provenance").unwrap();
    command
        .current_dir(root)
        .env("PROVENANCE_STE100_ASSET_DIR", asset_dir)
        .env(
            "PROVENANCE_STE100_INDEX_DIR",
            root.join("dictionary-indexes"),
        )
        .arg("init")
        .stdout(std::fs::File::create("/dev/full").unwrap());

    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No space left on device"));
    assert!(root.join(".provenance/state/manifest.json").is_file());
}
