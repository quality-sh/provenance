use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init_repo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    directory
}

fn output(command: &mut Command) -> Value {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn info_reports_the_language_neutral_engine_contract() {
    let directory = init_repo();
    let root = std::fs::canonicalize(directory.path()).unwrap();
    let info = output(provenance().args([
        "sdk",
        "info",
        "--repo",
        directory.path().to_str().unwrap(),
        "--format",
        "json",
    ]));

    assert_eq!(info["engine_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(info["protocol_version"], 6);
    assert_eq!(info["state_schema_version"], 1);
    assert_eq!(info["repository"], root.to_str().unwrap());
}

#[test]
#[verifies("rule_sdk_project_discovery", examples)]
fn sdk_commands_discover_the_nearest_initialized_project() {
    let directory = init_repo();
    let nested = directory.path().join("packages/app/src");
    std::fs::create_dir_all(&nested).unwrap();
    let spec = json!({
        "schema_version": 1,
        "spec": "nested",
        "declared_by": "spec://typescript/nested",
        "requirements": [{
            "key": "runs",
            "statement": "Nested SDK calls use their enclosing project"
        }]
    });

    let applied = output(
        provenance()
            .current_dir(&nested)
            .args(["sdk", "apply", "--scope", "default", "--format", "json"])
            .write_stdin(serde_json::to_vec(&spec).unwrap()),
    );

    assert_eq!(applied["created"], 1);
    let exported = output(provenance().args([
        "export",
        "--repo",
        directory.path().to_str().unwrap(),
        "--scope",
        "default",
        "--format",
        "json",
    ]));
    assert_eq!(
        exported["requirements"][0]["id"],
        applied["resources"][0]["id"]
    );
}

#[test]
#[verifies("rule_sdk_project_discovery", examples)]
fn info_falls_back_to_the_git_worktree_root_before_initialization() {
    let directory = tempfile::tempdir().unwrap();
    std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(directory.path())
        .status()
        .unwrap();
    let nested = directory.path().join("packages/app");
    std::fs::create_dir_all(&nested).unwrap();

    let info = output(
        provenance()
            .current_dir(&nested)
            .args(["sdk", "info", "--format", "json"]),
    );

    assert_eq!(
        info["repository"],
        std::fs::canonicalize(directory.path())
            .unwrap()
            .to_str()
            .unwrap()
    );
}

#[test]
fn an_explicit_repository_wins_over_enclosing_project_discovery() {
    let directory = init_repo();
    let nested = directory.path().join("isolated");
    std::fs::create_dir_all(&nested).unwrap();

    let info = output(provenance().args([
        "sdk",
        "info",
        "--repo",
        nested.to_str().unwrap(),
        "--format",
        "json",
    ]));

    assert_eq!(
        info["repository"],
        std::fs::canonicalize(nested).unwrap().to_str().unwrap()
    );
}
