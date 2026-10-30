use super::seeded_requirement_store;
use crate::state_store::{ArchiveCommitGap, ArchiveCommitGapReason, StateStore};
use camino::Utf8Path;
use provenance_core::ScopeId;
use provenance_macros::verifies;
use std::process::Command;

const UNKNOWN_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn git(repo: &Utf8Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn commit(repo: &Utf8Path, message: &str) -> String {
    git(repo, &["commit", "--quiet", "--allow-empty", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

fn archived_rule(id: &str, commit: &str) -> String {
    serde_json::json!({
        "schema_version": 3,
        "scope_id": "default",
        "id": id,
        "statement": "The fixture rule holds.",
        "status": "archived",
        "archived_in_commit": {"commit": commit},
        "severity": "low",
        "requirement_ids": ["req_overtime"],
    })
    .to_string()
}

fn write_rules(store: &StateStore, rules: &[(&str, &str)]) {
    let path = crate::shards::rules_path(&store.layout, &ScopeId::new("default").unwrap());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let lines: Vec<_> = rules
        .iter()
        .map(|(id, commit)| archived_rule(id, commit))
        .collect();
    std::fs::write(path, lines.join("\n") + "\n").unwrap();
}

/// A base commit on main, an archive commit on a branch, and a commit on
/// main that stands for the squash merge of that branch.
fn squashed_history(repo: &Utf8Path) -> (String, String) {
    git(repo, &["init", "--quiet", "--initial-branch=main"]);
    let base = commit(repo, "Base");
    git(repo, &["checkout", "--quiet", "-b", "feature"]);
    let branch = commit(repo, "Archive on the branch");
    git(repo, &["checkout", "--quiet", "main"]);
    commit(repo, "Squash merge of the branch");
    (base, branch)
}

fn gap(id: &str, commit: &str, reason: ArchiveCommitGapReason) -> ArchiveCommitGap {
    ArchiveCommitGap {
        scope_id: "default".into(),
        record_id: id.into(),
        commit: commit.into(),
        reason,
    }
}

#[test]
#[verifies("rule_archive_commit_in_head_history", examples)]
fn reports_dropped_and_unknown_archive_commits_once_each() {
    let (dir, store, _) = seeded_requirement_store();
    let repo = Utf8Path::from_path(dir.path()).unwrap();
    let (base, branch) = squashed_history(repo);
    write_rules(
        &store,
        &[
            ("rule_kept", &base),
            ("rule_dropped", &branch),
            ("rule_unknown", UNKNOWN_COMMIT),
        ],
    );

    let gaps = store
        .archive_commits_outside_history(repo, &store.manifest().unwrap())
        .unwrap();

    assert_eq!(
        gaps,
        vec![
            gap(
                "rule_dropped",
                &branch,
                ArchiveCommitGapReason::NotAncestorOfHead
            ),
            gap(
                "rule_unknown",
                UNKNOWN_COMMIT,
                ArchiveCommitGapReason::Missing
            ),
        ]
    );
    assert_eq!(
        gaps[0].to_string(),
        format!("rule_dropped: archive commit {branch} is not in the history of HEAD")
    );
}

#[test]
#[verifies("rule_archive_commit_in_head_history", examples)]
fn ancestor_archive_commits_give_no_finding() {
    let (dir, store, _) = seeded_requirement_store();
    let repo = Utf8Path::from_path(dir.path()).unwrap();
    let (base, _) = squashed_history(repo);
    write_rules(&store, &[("rule_kept", &base)]);

    assert_eq!(
        store
            .archive_commits_outside_history(repo, &store.manifest().unwrap())
            .unwrap(),
        Vec::new()
    );
}

#[test]
#[verifies("rule_archive_commit_in_head_history", examples)]
fn skips_a_directory_outside_git_and_a_shallow_clone() {
    let (dir, store, _) = seeded_requirement_store();
    write_rules(&store, &[("rule_unknown", UNKNOWN_COMMIT)]);
    let plain = Utf8Path::from_path(dir.path()).unwrap();
    assert_eq!(
        store
            .archive_commits_outside_history(plain, &store.manifest().unwrap())
            .unwrap(),
        Vec::new()
    );

    let origin = tempfile::tempdir().unwrap();
    let origin_path = Utf8Path::from_path(origin.path()).unwrap();
    squashed_history(origin_path);
    commit(origin_path, "Second commit on main");
    let clones = tempfile::tempdir().unwrap();
    let clones_path = Utf8Path::from_path(clones.path()).unwrap();
    git(
        clones_path,
        &[
            "clone",
            "--quiet",
            "--no-local",
            "--depth",
            "1",
            origin_path.as_str(),
            "shallow",
        ],
    );
    let shallow = clones_path.join("shallow");
    assert_eq!(
        git(&shallow, &["rev-parse", "--is-shallow-repository"]),
        "true"
    );
    assert_eq!(
        store
            .archive_commits_outside_history(&shallow, &store.manifest().unwrap())
            .unwrap(),
        Vec::new()
    );
}
