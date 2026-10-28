use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::path::Path;

#[path = "export_fixture_support/mod.rs"]
mod export_fixture_support;

fn provenance(repo: &Path) -> Command {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.args(["--quiet"]);
    command.current_dir(repo);
    command
}

fn git(repo: &Path, args: &[&str]) {
    if args.first() == Some(&"add") {
        export_fixture_support::make_default_scope_portable(repo);
    }
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn committed_store() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), &["init", "-q"]);
    git(
        temp.path(),
        &["config", "user.name", "Graph Reference Test"],
    );
    git(
        temp.path(),
        &["config", "user.email", "graph-reference@example.invalid"],
    );
    provenance(temp.path())
        .args(["init", "--path", ".", "--scope", "default"])
        .assert()
        .success();
    git(temp.path(), &["add", ".provenance/state"]);
    git(temp.path(), &["commit", "-qm", "canonical graph"]);
    temp
}

fn issue(repo: &Path, extra: &[&str]) -> Value {
    let output = provenance(repo)
        .args([
            "graph-reference",
            "issue",
            "--repo",
            ".",
            "--scope",
            "default",
        ])
        .args(extra)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn write_reference(repo: &Path, reference: &Value) -> std::path::PathBuf {
    let path = repo.join("reference.json");
    std::fs::write(&path, serde_json::to_vec(reference).unwrap()).unwrap();
    path
}

#[path = "cli_graph_references/export.rs"]
mod export;
#[path = "cli_graph_references/issuance.rs"]
mod issuance;
#[path = "cli_graph_references/pinned_records.rs"]
mod pinned_records;
#[path = "cli_graph_references/schema.rs"]
mod schema;
