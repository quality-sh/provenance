use assert_cmd::Command;
use provenance_macros::verifies;
use sha2::Digest;
use std::path::{Path, PathBuf};

fn rule(id: &str, requirement_ids: &str) -> String {
    format!(
        "{{\"scope_id\":\"default\",\"id\":\"{id}\",\
         \"statement\":\"The {id} rule shall hold.\",\"status\":\"active\",\
         \"severity\":\"high\",\"requirement_ids\":[{requirement_ids}]}}\n"
    )
}

fn valid_rule(id: &str, requirement: &str) -> String {
    rule(id, &format!("\"{requirement}\""))
}

/// A rule no writer would accept: a rule needs one requirement.
fn bare_rule(id: &str) -> String {
    rule(id, "")
}

struct Sides {
    base: PathBuf,
    ours: PathBuf,
    theirs: PathBuf,
    output: PathBuf,
}

impl Sides {
    fn write(directory: &Path, base: &str, ours: &str, theirs: &str) -> Self {
        let sides = Self {
            base: directory.join("base.jsonl"),
            ours: directory.join("ours.jsonl"),
            theirs: directory.join("theirs.jsonl"),
            output: directory.join("merged.jsonl"),
        };
        std::fs::write(&sides.base, base).unwrap();
        std::fs::write(&sides.ours, ours).unwrap();
        std::fs::write(&sides.theirs, theirs).unwrap();
        sides
    }

    fn merge(&self, shard_path: &str) -> Command {
        let mut command = Command::cargo_bin("provenance").unwrap();
        command.args([
            "merge-jsonl",
            self.base.to_str().unwrap(),
            self.ours.to_str().unwrap(),
            self.theirs.to_str().unwrap(),
            "--output",
            self.output.to_str().unwrap(),
            "--path",
            shard_path,
            "--format",
            "json",
        ]);
        command
    }
}

const RULES_SHARD: &str = ".provenance/state/scopes/default/rules/rule.jsonl";

#[test]
fn merge_writes_a_shard_whose_rules_all_carry_a_requirement() {
    let directory = tempfile::tempdir().unwrap();
    let sides = Sides::write(
        directory.path(),
        "",
        &valid_rule("rule_ours", "req_ours"),
        &valid_rule("rule_theirs", "req_theirs"),
    );

    sides.merge(RULES_SHARD).assert().success();

    let merged = std::fs::read_to_string(&sides.output).unwrap();
    assert!(merged.contains("rule_ours"), "{merged}");
    assert!(merged.contains("rule_theirs"), "{merged}");
}

#[test]
fn merge_refuses_to_write_a_rule_with_no_requirement() {
    let directory = tempfile::tempdir().unwrap();
    let sides = Sides::write(
        directory.path(),
        "",
        &bare_rule("rule_bare"),
        &valid_rule("rule_theirs", "req_theirs"),
    );

    let failure = sides.merge(RULES_SHARD).assert().failure();
    let stderr = String::from_utf8(failure.get_output().stderr.clone()).unwrap();

    assert!(
        stderr.contains("rule_bare") && stderr.contains("a rule needs one requirement"),
        "the failure must name the offending rule: {stderr}"
    );
    assert!(
        !sides.output.exists(),
        "an invalid shard must not be written"
    );
}

#[test]
fn merge_leaves_families_without_typed_validation_alone() {
    // Records under a path that names no recognized family merge through
    // unchecked.
    let directory = tempfile::tempdir().unwrap();
    let sides = Sides::write(directory.path(), "", &bare_rule("rule_bare"), "");

    sides
        .merge(".provenance/state/notes/notes.jsonl")
        .assert()
        .success();
    assert!(sides.output.exists());
}

#[test]
fn a_conflicting_merge_fails_so_git_falls_back_to_a_conflict() {
    let directory = tempfile::tempdir().unwrap();
    let sides = Sides::write(
        directory.path(),
        &valid_rule("rule_shared", "req_base"),
        &valid_rule("rule_shared", "req_ours"),
        &valid_rule("rule_shared", "req_theirs"),
    );

    let failure = sides.merge(RULES_SHARD).assert().failure();
    let output = failure.get_output();
    let stdout = String::from_utf8(output.stdout.clone()).unwrap();
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();

    assert!(stdout.contains("divergent_edit"), "{stdout}");
    assert!(stderr.contains("rule_shared"), "{stderr}");
}

fn git(repository: &Path, arguments: &[&str]) -> std::process::Output {
    let output = provenance_store::git_process::git()
        .current_dir(repository)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn commit_shard(repository: &Path, contents: &str, message: &str) {
    let shard = repository.join(RULES_SHARD);
    std::fs::create_dir_all(shard.parent().unwrap()).unwrap();
    std::fs::write(&shard, contents).unwrap();
    git(repository, &["add", "."]);
    git(repository, &["commit", "-m", message]);
}

/// The documented one-time setup, run for real: the driver command from
/// `docs/cli.md` with an absolute binary path, and the repository
/// `.gitattributes` line that points the state shards at it.
#[test]
fn the_documented_driver_configuration_merges_two_branches() {
    let directory = tempfile::tempdir().unwrap();
    let repository = directory.path();
    git(repository, &["init", "--initial-branch", "main"]);
    git(repository, &["config", "user.email", "test@example.test"]);
    git(repository, &["config", "user.name", "Test"]);
    git(
        repository,
        &[
            "config",
            "merge.provenance-jsonl.name",
            "Provenance canonical JSONL merge",
        ],
    );
    let binary = assert_cmd::cargo::cargo_bin("provenance");
    git(
        repository,
        &[
            "config",
            "merge.provenance-jsonl.driver",
            &format!(
                "'{}' merge-jsonl %O %A %B --output %A --path %P",
                // Git runs the driver through sh, which eats backslashes;
                // Windows accepts forward slashes in exec paths.
                binary.to_str().unwrap().replace('\\', "/")
            ),
        ],
    );
    std::fs::write(
        repository.join(".gitattributes"),
        ".provenance/state/**/*.jsonl merge=provenance-jsonl\n",
    )
    .unwrap();

    commit_shard(repository, &valid_rule("rule_base", "req_base"), "base");
    git(repository, &["checkout", "-b", "theirs"]);
    commit_shard(
        repository,
        &(valid_rule("rule_base", "req_base") + &valid_rule("rule_theirs", "req_theirs")),
        "theirs",
    );
    git(repository, &["checkout", "main"]);
    commit_shard(
        repository,
        &(valid_rule("rule_base", "req_base") + &valid_rule("rule_ours", "req_ours")),
        "ours",
    );

    git(repository, &["merge", "theirs", "-m", "merge"]);

    let merged = std::fs::read_to_string(repository.join(RULES_SHARD)).unwrap();
    assert!(merged.contains("rule_ours"), "{merged}");
    assert!(merged.contains("rule_theirs"), "{merged}");
    assert_eq!(merged.lines().count(), 3, "{merged}");
}

/// The same setup, but one branch carries a rule with no requirement:
/// git must be told the merge failed rather than committing the invalid shard.
#[test]
fn the_documented_driver_configuration_refuses_an_invalid_merge() {
    let directory = tempfile::tempdir().unwrap();
    let repository = directory.path();
    git(repository, &["init", "--initial-branch", "main"]);
    git(repository, &["config", "user.email", "test@example.test"]);
    git(repository, &["config", "user.name", "Test"]);
    let binary = assert_cmd::cargo::cargo_bin("provenance");
    git(
        repository,
        &[
            "config",
            "merge.provenance-jsonl.driver",
            &format!(
                "'{}' merge-jsonl %O %A %B --output %A --path %P",
                // Git runs the driver through sh, which eats backslashes;
                // Windows accepts forward slashes in exec paths.
                binary.to_str().unwrap().replace('\\', "/")
            ),
        ],
    );
    std::fs::write(
        repository.join(".gitattributes"),
        ".provenance/state/**/*.jsonl merge=provenance-jsonl\n",
    )
    .unwrap();

    commit_shard(repository, &valid_rule("rule_base", "req_base"), "base");
    git(repository, &["checkout", "-b", "theirs"]);
    commit_shard(
        repository,
        &(valid_rule("rule_base", "req_base") + &bare_rule("rule_bare")),
        "theirs",
    );
    git(repository, &["checkout", "main"]);
    commit_shard(
        repository,
        &(valid_rule("rule_base", "req_base") + &valid_rule("rule_ours", "req_ours")),
        "ours",
    );

    let merge = provenance_store::git_process::git()
        .current_dir(repository)
        .args(["merge", "theirs", "-m", "merge"])
        .output()
        .unwrap();

    assert!(
        !merge.status.success(),
        "git must not commit a merge the driver rejected"
    );
    let status = git(repository, &["status", "--porcelain"]);
    let status = String::from_utf8(status.stdout).unwrap();
    assert!(
        status.contains(RULES_SHARD),
        "the shard must be left unmerged: {status}"
    );
}

const REVIEW_SHARD: &str = ".provenance/state/scopes/default/requirements/review.jsonl";

/// One review row as the store writes it. The rows are written by hand: no
/// public command sets the time at which a verification run clears a review,
/// so two branches cannot clear the same review at fixed times through the
/// CLI. The id is the hash that the store computes from the Rule, the
/// Requirement, the field and the new text.
fn review_row(rule_id: &str, requirement_id: &str, after: &str, cleared_at: Option<i64>) -> String {
    let identity = format!("{rule_id}\0{requirement_id}\0statement\0{after}");
    let digest = hex::encode(sha2::Sha256::digest(identity.as_bytes()));
    let id = format!("requirement_review_{}", &digest[..20]);
    let cleared = cleared_at.map_or_else(String::new, |time| {
        format!(",\"cleared_at\":{time},\"cleared_by_run\":\"verification_{time}\"")
    });
    format!(
        "{{\"scope_id\":\"default\",\"id\":\"{id}\",\"rule_id\":\"{rule_id}\",\
         \"requirement_id\":\"{requirement_id}\",\"field\":\"statement\",\
         \"before\":\"The old text.\",\"after\":\"{after}\",\
         \"changed_at\":1800000000000{cleared}}}\n"
    )
}

/// The review that a restatement of `req_a` raised for `rule_a`.
fn first_review(cleared_at: Option<i64>) -> String {
    review_row("rule_a", "req_a", "The requirement shall hold.", cleared_at)
}

/// The review that the same restatement raised for `rule_b`.
fn second_review(cleared_at: Option<i64>) -> String {
    review_row("rule_b", "req_a", "The requirement shall hold.", cleared_at)
}

/// A review that a restatement of `req_c` on our branch raised.
fn new_review() -> String {
    review_row("rule_c", "req_c", "The new requirement shall hold.", None)
}

/// The rows in the order of their ids, which is the order the store writes.
fn in_id_order(rows: &[&str]) -> String {
    let mut rows = rows.to_vec();
    rows.sort_by_key(|row| review_id(row));
    rows.concat()
}

fn review_id(row: &str) -> String {
    serde_json::from_str::<serde_json::Value>(row).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
#[verifies("rule_record_merge", examples)]
fn different_added_review_rows_merge_to_both_rows() {
    let directory = tempfile::tempdir().unwrap();
    let base = first_review(None);
    let ours = in_id_order(&[&base, &new_review()]);
    let theirs = in_id_order(&[&base, &second_review(None)]);
    let sides = Sides::write(directory.path(), &base, &ours, &theirs);

    sides.merge(REVIEW_SHARD).assert().success();

    assert_eq!(
        std::fs::read_to_string(&sides.output).unwrap(),
        in_id_order(&[&base, &new_review(), &second_review(None)])
    );
}

/// Two branches each run a verification of `rule_a`, so each clears the same
/// review at a different time. Their branch also clears the review of
/// `rule_b`, and our branch adds a new review.
#[test]
#[verifies("rule_record_merge", examples)]
fn one_review_row_cleared_on_both_sides_reports_a_conflict() {
    let directory = tempfile::tempdir().unwrap();
    let ours_cleared = first_review(Some(1_800_000_001_000));
    let theirs_cleared = first_review(Some(1_800_000_002_000));
    let sides = Sides::write(
        directory.path(),
        &in_id_order(&[&first_review(None), &second_review(None)]),
        &in_id_order(&[&ours_cleared, &second_review(None), &new_review()]),
        &in_id_order(&[&theirs_cleared, &second_review(Some(1_800_000_002_000))]),
    );

    let failure = sides.merge(REVIEW_SHARD).assert().code(1);

    let report: serde_json::Value = serde_json::from_slice(&failure.get_output().stdout).unwrap();
    assert_eq!(report["status"], "conflicted");
    assert_eq!(report["conflicts"].as_array().unwrap().len(), 1, "{report}");
    assert_eq!(report["conflicts"][0]["kind"], "divergent_edit");
    assert_eq!(
        report["conflicts"][0]["record_id"],
        review_id(&ours_cleared).as_str()
    );
    // Git keeps the output file: our clear of the conflicted row, their clear
    // of the second row, and our new row, each as its stored line.
    assert_eq!(
        std::fs::read_to_string(&sides.output).unwrap(),
        in_id_order(&[
            &ours_cleared,
            &second_review(Some(1_800_000_002_000)),
            &new_review(),
        ])
    );
}
