use crate::support::merge_driver::{cli, commit, git, git_ok, json, repository};
use provenance_macros::verifies;
use std::path::{Path, PathBuf};

/// A repository with the Requirement that the landed contributions target.
fn landing_repository() -> tempfile::TempDir {
    let dir = repository();
    json(
        dir.path(),
        &[
            "req_a",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The system is safe.",
        ],
    );
    dir
}

fn landings(repo: &Path) -> PathBuf {
    repo.join(".provenance/state/scopes/default/ideation/landings.jsonl")
}

/// The rows of the landings file, each as the file stores it.
fn landing_lines(repo: &Path) -> Vec<String> {
    std::fs::read_to_string(landings(repo))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Lands a swarm run whose one contribution is `contrib_<name>`, and
/// returns the batch row that the landing adds.
fn land(repo: &Path, name: &str) -> String {
    let run = repo.parent().unwrap().join(format!("run_{name}"));
    let extractors = run.join("extractors");
    std::fs::create_dir_all(&extractors).unwrap();
    let contribution = serde_json::json!({
        "contribution": {
            "scope_id": "default",
            "id": format!("contrib_{name}"),
            "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
            "participant_slot": format!("slot_{name}"),
            "stance": "support",
            "strongest_finding": "The requirement holds.",
            "evidence_references": [],
            "material_claims": [],
            "risks": [],
            "objections": [],
            "challenges": [],
            "suggested_artifact_changes": [],
            "unsupported_recommendations": [],
            "uncertainty": {"level": "low", "rationale": "Direct."},
            "open_questions": []
        }
    });
    std::fs::write(extractors.join("one.json"), contribution.to_string()).unwrap();
    json(
        repo,
        &[
            "swarm-backtrace",
            "land",
            "--run-dir",
            run.to_str().unwrap(),
        ],
    );
    landing_lines(repo).pop().unwrap()
}

/// Merges the branch `theirs` into the checked-out branch, and returns the
/// exit code of the merge.
fn merge_theirs(repo: &Path) -> Option<i32> {
    let output = git(repo, &["merge", "theirs", "-m", "merge"]);
    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    output.status.code()
}

/// The contribution ids that a CLI read of the scope gives, after a check of
/// the scope passes.
fn read_contribution_ids(repo: &Path) -> Vec<String> {
    let check = cli(repo, &["check"]);
    assert_eq!(
        check.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&check.stdout)
    );
    json(repo, &["api", "contributions"])["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
#[verifies("rule_landing_batch_merge", examples)]
fn the_merge_keeps_the_different_batches_of_each_side() {
    let dir = landing_repository();
    let repo = dir.path();
    let base = land(repo, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let theirs = land(repo, "theirs");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    let ours = land(repo, "ours");
    commit(repo, "ours");

    assert_eq!(merge_theirs(repo), Some(0));
    assert_eq!(landing_lines(repo), [base, ours, theirs]);
    assert_eq!(
        read_contribution_ids(repo),
        ["contrib_base", "contrib_ours", "contrib_theirs"]
    );
}

#[test]
#[verifies("rule_landing_batch_merge", examples)]
fn the_merge_keeps_a_batch_that_the_two_sides_add_one_time() {
    let dir = landing_repository();
    let repo = dir.path();
    let base = land(repo, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let same = land(repo, "same");
    let theirs = land(repo, "theirs");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    assert_eq!(land(repo, "same"), same);
    commit(repo, "ours");

    assert_eq!(merge_theirs(repo), Some(0));
    assert_eq!(landing_lines(repo), [base, same, theirs]);
    assert_eq!(
        read_contribution_ids(repo),
        ["contrib_base", "contrib_same", "contrib_theirs"]
    );
}

#[test]
#[verifies("rule_landing_batch_merge", examples)]
fn the_merge_keeps_a_batch_that_one_side_adds() {
    let dir = landing_repository();
    let repo = dir.path();
    let base = land(repo, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let theirs = land(repo, "theirs");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    json(
        repo,
        &[
            "req_b",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The system is fast.",
        ],
    );
    commit(repo, "ours");

    assert_eq!(merge_theirs(repo), Some(0));
    assert_eq!(landing_lines(repo), [base, theirs]);
    assert_eq!(
        read_contribution_ids(repo),
        ["contrib_base", "contrib_theirs"]
    );
}

#[test]
#[verifies("rule_landing_batch_merge", examples)]
#[verifies("rule_record_merge", examples)]
fn the_merge_removes_a_batch_that_one_side_removes_and_the_other_keeps() {
    let dir = landing_repository();
    let repo = dir.path();
    let kept = land(repo, "kept");
    let removed = land(repo, "removed");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let theirs = land(repo, "theirs");
    assert_eq!(landing_lines(repo), [kept.clone(), removed, theirs.clone()]);
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    // The product does not remove a batch, so this side removes it by hand.
    std::fs::write(landings(repo), format!("{kept}\n")).unwrap();
    commit(repo, "ours");

    assert_eq!(merge_theirs(repo), Some(0));
    assert_eq!(landing_lines(repo), [kept, theirs]);
    assert_eq!(
        read_contribution_ids(repo),
        ["contrib_kept", "contrib_theirs"]
    );
}
