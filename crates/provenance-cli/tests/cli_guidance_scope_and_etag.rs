//! The managed AGENTS.md lines and the bundled skills name commands that run
//! as written: a non-default scope travels on every review command, and a
//! Topic or Question update carries the etag that the guidance tells the
//! agent to read first.

use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use serde_json::Value;
use std::path::Path;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init(repo: &Path, scope: &str) {
    provenance()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            scope,
            "--path-prefix",
            ".",
            "--disposition-actor-id",
            "reviewer",
        ])
        .assert()
        .success();
}

fn json(command: &mut Command) -> Value {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// A requirement, a topic under it, and one question under the topic.
fn shaping_records(repo: &str, scope: &str) {
    json(provenance().args([
        "requirements",
        "create",
        "--repo",
        repo,
        "--scope",
        scope,
        "--id",
        "req_a",
        "--statement",
        "The system records topics.",
        "--format",
        "json",
    ]));
    json(provenance().args([
        "topics",
        "create",
        "--repo",
        repo,
        "--scope",
        scope,
        "--id",
        "topic_open",
        "--requirement-id",
        "req_a",
        "--title",
        "Open",
        "--format",
        "json",
    ]));
    json(provenance().args([
        "questions",
        "create",
        "--repo",
        repo,
        "--scope",
        scope,
        "--id",
        "q_one",
        "--topic-id",
        "topic_open",
        "--question",
        "Why?",
        "--method",
        "research",
        "--format",
        "json",
    ]));
}

#[test]
#[verifies("rule_init_guidance_selected_scope", examples)]
fn managed_guidance_carries_the_selected_scope_on_every_review_command() {
    let directory = tempfile::tempdir().unwrap();
    init(directory.path(), "app");
    let agents = std::fs::read_to_string(directory.path().join("AGENTS.md")).unwrap();

    for line in [
        "provenance <record-id> --scope app --review-link`",
        "provenance <record-id> --scope app --review-link --no-open --format json`",
        "provenance <record-id> --scope app --view review`",
        "provenance <requirement-id> --scope app --view review --follow --format json`",
        "provenance questions <question_id> get --scope app --format json`",
        "provenance topics <topic_id> get --scope app --format json`",
        "provenance questions <question_id> update --scope app --if-match <etag> --stdin --format json`",
        "provenance topics <topic_id> update --scope app --if-match <etag> --stdin --format json`",
    ] {
        assert!(agents.contains(line), "missing {line}\n{agents}");
    }
}

#[test]
fn managed_guidance_starts_the_review_host_in_the_background() {
    let directory = tempfile::tempdir().unwrap();
    init(directory.path(), "default");
    let agents = std::fs::read_to_string(directory.path().join("AGENTS.md")).unwrap();

    assert!(agents.contains(
        "provenance review --repo . --repository-id local --scope default --no-open \
         > .provenance/cache/review-host.log 2>&1 &`"
    ));
    assert!(agents.contains("stop the host with `kill <pid>`"));
    assert!(!agents.contains("--no-open`."), "{agents}");
}

#[test]
fn the_documented_archive_commands_run_as_written() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    init(directory.path(), "app");
    shaping_records(repo, "app");
    let commit = "a".repeat(40);
    let body = format!(r#"{{"status":"archived","archived_in_commit":{{"commit":"{commit}"}}}}"#);

    for (family, id) in [("questions", "q_one"), ("topics", "topic_open")] {
        let read = json(provenance().args([
            family, id, "get", "--repo", repo, "--scope", "app", "--format", "json",
        ]));
        let etag = read["data"]["edit"]["etag"].as_str().unwrap().to_owned();
        provenance()
            .args([
                family,
                id,
                "update",
                "--repo",
                repo,
                "--scope",
                "app",
                "--if-match",
                &etag,
                "--stdin",
                "--format",
                "json",
            ])
            .write_stdin(body.clone())
            .assert()
            .success();
        let after = json(provenance().args([
            family, id, "get", "--repo", repo, "--scope", "app", "--format", "json",
        ]));
        assert_eq!(after["data"]["status"], "archived", "{family} {id}");
    }
}

#[test]
fn a_question_update_without_the_header_reports_it_as_required() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    init(directory.path(), "default");
    shaping_records(repo, "default");

    provenance()
        .args([
            "questions",
            "q_one",
            "update",
            "--repo",
            repo,
            "--status",
            "blocked_on_human",
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(r#""field":"If-Match""#))
        .stderr(predicate::str::contains(r#""reason":"required""#));
}

#[test]
fn a_record_read_shows_the_edit_etag() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    init(directory.path(), "default");
    shaping_records(repo, "default");

    for (family, id) in [("requirements", "req_a"), ("topics", "topic_open")] {
        let expected =
            json(provenance().args([family, id, "get", "--repo", repo, "--format", "json"]));
        let etag = expected["data"]["edit"]["etag"].as_str().unwrap();
        let read = json(provenance().args([id, "--repo", repo, "--format", "json"]));
        assert_eq!(read["edit"]["etag"].as_str(), Some(etag), "{id}");
        let read = json(provenance().args([id, "get", "--repo", repo, "--format", "json"]));
        assert_eq!(read["edit"]["etag"].as_str(), Some(etag), "{id}");
    }
}

#[test]
fn the_fork_tournament_skill_passes_the_question_etag() {
    let directory = tempfile::tempdir().unwrap();
    init(directory.path(), "default");
    let skill = std::fs::read_to_string(
        directory
            .path()
            .join(".agents/skills/provenance-fork-tournament/SKILL.md"),
    )
    .unwrap();

    assert!(skill.contains("provenance questions <question_id> get --scope <scope> --format json"));
    assert!(skill.contains("--if-match <etag> \\\n      --status blocked_on_human"));
    assert!(!skill.contains("update --scope <scope> \\\n      --status blocked_on_human"));
}
