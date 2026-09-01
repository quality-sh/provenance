#![cfg(feature = "dogfood")]

use assert_cmd::Command;
use std::path::Path;

fn note_cmd(spool_dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("provenance").unwrap();
    cmd.env("PROVENANCE_DOGFOOD_DIR", spool_dir)
        .env_remove("PROVENANCE_SESSION_ID")
        .env_remove("WORKFLOWD_SESSION_ID")
        .env_remove("CLAUDE_SESSION_ID")
        .env_remove("OPENCODE_SESSION_ID");
    cmd
}

fn read_spool(spool_dir: &Path) -> Vec<serde_json::Value> {
    let raw = std::fs::read_to_string(spool_dir.join("notes.jsonl")).unwrap();
    raw.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

// --- Regression tests from the thermonuclear review (F1, F2, F5, F7, stdin gap) ---

#[test]
fn note_inside_git_repo_records_repo_branch_and_commit() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(&repo)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(status.status.success(), "git {args:?} failed");
    };
    git(&["init", "-b", "feat/spool-x"]);
    git(&["commit", "--allow-empty", "-m", "seed"]);

    note_cmd(temp.path())
        .current_dir(&repo)
        .args([
            "dogfood",
            "note",
            "--surface",
            "prime",
            "--category",
            "friction",
            "--severity",
            "annoyance",
            "in-repo note",
        ])
        .assert()
        .success();

    let notes = read_spool(temp.path());
    let note = &notes[0];
    let repo_field = note["repo"]
        .as_str()
        .expect("repo populated inside a git repo");
    assert!(repo_field.ends_with("repo"), "repo was {repo_field}");
    assert_eq!(
        note["branch"], "feat/spool-x",
        "branch must record the checked-out branch"
    );
    let commit = note["commit"].as_str().expect("commit populated");
    assert_eq!(commit.len(), 40);
    assert!(commit.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn list_and_report_skip_malformed_spool_lines() {
    let temp = tempfile::tempdir().unwrap();

    note_cmd(temp.path())
        .args([
            "dogfood",
            "note",
            "--surface",
            "general",
            "--category",
            "bug",
            "--severity",
            "annoyance",
            "good note",
        ])
        .assert()
        .success();

    // Simulate a torn write followed by more valid notes.
    let spool = temp.path().join("notes.jsonl");
    let mut contents = std::fs::read_to_string(&spool).unwrap();
    contents.push_str("{\"ts_ms\": 12, \"session_id\": TORN GARBA");
    contents.push('\n');
    std::fs::write(&spool, contents).unwrap();
    note_cmd(temp.path())
        .args([
            "dogfood",
            "note",
            "--surface",
            "general",
            "--category",
            "idea",
            "--severity",
            "annoyance",
            "note after garbage",
        ])
        .assert()
        .success();

    let output = note_cmd(temp.path())
        .args(["dogfood", "list", "--format", "json"])
        .assert()
        .success()
        .stderr(predicates::str::contains("skipping"))
        .get_output()
        .stdout
        .clone();
    let notes: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let summaries: Vec<_> = notes
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["summary"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(summaries, ["good note", "note after garbage"]);

    let output = note_cmd(temp.path())
        .args(["dogfood", "report", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["total"], 2);
}

#[test]
fn empty_dogfood_dir_env_falls_back_to_home_spool() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let cwd = temp.path().join("cwd");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();

    let mut cmd = Command::cargo_bin("provenance").unwrap();
    cmd.env("PROVENANCE_DOGFOOD_DIR", "")
        .env("HOME", &home)
        .current_dir(&cwd)
        .args([
            "dogfood",
            "note",
            "--surface",
            "general",
            "--category",
            "friction",
            "--severity",
            "annoyance",
            "empty env override",
        ])
        .assert()
        .success();

    assert!(
        home.join(".provenance/dogfood/notes.jsonl").exists(),
        "empty PROVENANCE_DOGFOOD_DIR must fall back to the home spool"
    );
    assert!(
        !cwd.join("notes.jsonl").exists(),
        "must not write into the current directory"
    );
}

#[test]
fn capture_survives_missing_home() {
    let temp = tempfile::tempdir().unwrap();
    let tmpdir = temp.path().join("tmp");
    std::fs::create_dir_all(&tmpdir).unwrap();

    let mut cmd = Command::cargo_bin("provenance").unwrap();
    cmd.env_remove("PROVENANCE_DOGFOOD_DIR")
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .env_remove("HOMEDRIVE")
        .env_remove("HOMEPATH")
        .env("TMPDIR", &tmpdir)
        .args([
            "dogfood",
            "note",
            "--surface",
            "general",
            "--category",
            "confusion",
            "--severity",
            "annoyance",
            "homeless note",
        ])
        .assert()
        .success();

    assert!(
        tmpdir.join("provenance-dogfood/notes.jsonl").exists(),
        "capture must degrade to the temp-dir spool when HOME is unset"
    );
}

#[test]
fn report_enriches_from_stdin() {
    let temp = tempfile::tempdir().unwrap();

    note_cmd(temp.path())
        .env("PROVENANCE_SESSION_ID", "sess-stdin")
        .args([
            "dogfood",
            "note",
            "--surface",
            "prime",
            "--category",
            "friction",
            "--severity",
            "blocked",
            "stdin enriched",
        ])
        .assert()
        .success();

    let enrichment = serde_json::json!({
        "contract": "provenance-dogfood-enrichment/v1",
        "sessions": { "sess-stdin": { "model": "glm-5.3-flash" } }
    });
    let output = note_cmd(temp.path())
        .args(["dogfood", "report", "--enrich", "-", "--format", "json"])
        .write_stdin(serde_json::to_vec(&enrichment).unwrap())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["notes"][0]["session"]["model"], "glm-5.3-flash");
}
