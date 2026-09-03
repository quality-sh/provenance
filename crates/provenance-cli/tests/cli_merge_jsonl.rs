use assert_cmd::Command;
use std::path::{Path, PathBuf};

fn rule(id: &str, requirement_ids: &str) -> String {
    format!(
        "{{\"schema_version\":1,\"scope_id\":\"default\",\"id\":\"{id}\",\
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
    let output = std::process::Command::new("git")
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

    let merge = std::process::Command::new("git")
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
