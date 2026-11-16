//! The graph check shows a finding for a clone where git sends the state
//! files to the `provenance-jsonl` merge driver and has no command for it.

use provenance_macros::verifies;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

const MISSING_DRIVER: &str = "Git sends .provenance/state files to the provenance-jsonl merge \
     driver, but this clone has no command for it. Run provenance init to set it.";

/// Runs `command` with an empty global git configuration and no system
/// configuration below `root`.
fn isolated(root: &Path, mut command: Command) -> Output {
    command
        .env("GIT_CONFIG_GLOBAL", root.join("gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.test")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.test")
        .env("PROVENANCE_STE100_ASSET_DIR", root.join("assets"))
        .env("PROVENANCE_STE100_INDEX_DIR", root.join("indexes"))
        .env(
            "PROVENANCE_TEST_STE100_ASSET_URL",
            env!("PROVENANCE_TEST_STE100_ASSET_URL"),
        );
    let output = command.output().unwrap();
    assert!(output.status.success(), "{command:?}: {output:?}");
    output
}

fn git(root: &Path, folder: &Path, args: &[&str]) {
    let mut command = provenance_store::git_process::git();
    command.current_dir(folder).args(args);
    isolated(root, command);
}

fn provenance(root: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
    command.args(args);
    isolated(root, command)
}

fn init(root: &Path, repo: &Path) {
    let repo = repo.to_str().unwrap();
    provenance(root, &["init", "--path", repo, "--scope", "default"]);
}

/// The findings of the graph category; the run exits 0.
fn graph_findings(root: &Path, repo: &Path) -> Value {
    let repo = repo.to_str().unwrap();
    let output = provenance(
        root,
        &["check", "--repo", repo, "--graph", "--format", "json"],
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["categories"][0]["category"], "graph");
    report["categories"][0]["findings"].clone()
}

/// A committed repository after init, and a clone of it with no local git
/// configuration.
fn origin_and_clone() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(root.join("gitconfig"), "").unwrap();
    let origin = root.join("origin");
    std::fs::create_dir(&origin).unwrap();
    git(root, &origin, &["init", "--quiet"]);
    init(root, &origin);
    git(root, &origin, &["add", "."]);
    git(root, &origin, &["commit", "--quiet", "-m", "init"]);
    // The skill links of init are symbolic links, which a clone on Windows
    // keeps only with core.symlinks.
    let clone_args = [
        "-c",
        "core.symlinks=true",
        "clone",
        "--quiet",
        "origin",
        "clone",
    ];
    git(root, root, &clone_args);
    let clone = root.join("clone");
    (directory, clone)
}

#[test]
#[verifies("rule_check_reports_missing_merge_driver", examples)]
fn a_clone_without_the_driver_gets_a_finding_until_init() {
    let (directory, clone) = origin_and_clone();
    let root = directory.path();

    assert_eq!(
        graph_findings(root, &clone),
        serde_json::json!([{ "message": MISSING_DRIVER }])
    );

    init(root, &clone);

    assert_eq!(graph_findings(root, &clone), serde_json::json!([]));
}

#[test]
#[verifies("rule_check_reports_missing_merge_driver", examples)]
fn a_repository_without_the_line_gets_no_finding() {
    let (directory, clone) = origin_and_clone();
    let root = directory.path();
    git(root, &clone, &["rm", "--quiet", ".gitattributes"]);

    assert_eq!(graph_findings(root, &clone), serde_json::json!([]));
}
