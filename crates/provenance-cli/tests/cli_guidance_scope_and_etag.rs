//! The managed AGENTS.md lines and the bundled skills name commands that run
//! as written: a non-default scope travels on every review command, and a
//! Topic or Question update carries the etag that the guidance tells the
//! agent to read first.

use predicates::prelude::*;
use provenance_macros::verifies;
use std::path::Path;

mod support {
    pub mod review_paths;
}
use support::review_paths::{init_scope, json_output as json, provenance};

fn init(repo: &Path, scope: &str) {
    init_scope(repo, scope, ".");
}

/// A requirement, a topic under it, and one question under the topic.
fn shaping_records(repo: &str, scope: &str) {
    json(provenance().args([
        "req_a",
        "create",
        "--type",
        "requirement",
        "--repo",
        repo,
        "--scope",
        scope,
        "--statement",
        "The system records topics.",
        "--format",
        "json",
    ]));
    json(provenance().args([
        "topic_open",
        "create",
        "--type",
        "topic",
        "--repo",
        repo,
        "--scope",
        scope,
        "--requirement-id",
        "req_a",
        "--title",
        "Open",
        "--format",
        "json",
    ]));
    json(provenance().args([
        "q_one",
        "create",
        "--type",
        "question",
        "--repo",
        repo,
        "--scope",
        scope,
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
        "provenance review --repo . --repository-id local --scope app --detach --no-open`",
        "provenance review --repo . --repository-id local --scope app --detach`",
        "provenance <record-id> --scope app --review-link --format json`",
        "provenance <record-id> --scope app --review-link --no-open --format json`",
        "provenance <record-id> --scope app --view review`",
        "provenance <requirement-id> --scope app --view review --follow --format json`",
        "provenance <question_id> --scope app --view review --format json`",
        "provenance <topic_id> --scope app --view review --format json`",
        "provenance api questions/<question_id> --method patch --header \"If-Match: <etag>\" --input - --scope app --format json`",
        "provenance api topics/<topic_id> --method patch --header \"If-Match: <etag>\" --input - --scope app --format json`",
    ] {
        assert!(agents.contains(line), "missing {line}\n{agents}");
    }
}

#[test]
#[verifies("rule_init_guidance_background_review_host", examples)]
fn managed_guidance_runs_one_shared_review_host_that_sessions_reuse() {
    let directory = tempfile::tempdir().unwrap();
    init(directory.path(), "default");
    let agents = std::fs::read_to_string(directory.path().join("AGENTS.md")).unwrap();
    let words = agents.split_whitespace().collect::<Vec<_>>().join(" ");

    for text in [
        "start or reuse the review host",
        "--detach --no-open`",
        "One host serves the repository and scope for all sessions.",
        "It keeps running after the command returns, and later sessions reuse it, so leave it running.",
    ] {
        assert!(words.contains(text), "missing {text}\n{agents}");
    }
    for text in ["`kill", "`pid`", "stop the host"] {
        assert!(!words.contains(text), "{text}\n{agents}");
    }
    assert!(!words.contains("nohup"), "{agents}");
}

#[test]
#[verifies("rule_init_guidance_selected_scope", examples)]
fn the_documented_archive_commands_run_as_written() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    init(directory.path(), "app");
    shaping_records(repo, "app");
    let commit = "a".repeat(40);
    let body = format!(r#"{{"status":"archived","archived_in_commit":{{"commit":"{commit}"}}}}"#);

    for (family, id) in [("questions", "q_one"), ("topics", "topic_open")] {
        let read = json(provenance().args([
            id, "--repo", repo, "--scope", "app", "--view", "review", "--format", "json",
        ]));
        let etag = read["review"]["edit"]["etag"].as_str().unwrap().to_owned();
        let path = format!("{family}/{id}");
        provenance()
            .args([
                "api",
                &path,
                "--method",
                "patch",
                "--header",
                &format!("If-Match: {etag}"),
                "--input",
                "-",
                "--repo",
                repo,
                "--scope",
                "app",
                "--format",
                "json",
            ])
            .write_stdin(body.clone())
            .assert()
            .success();
        let after = json(provenance().args([
            "api", &path, "--repo", repo, "--scope", "app", "--format", "json",
        ]));
        assert_eq!(after["data"]["status"], "archived", "{family} {id}");
    }
}

#[test]
#[verifies("rule_missing_if_match_is_required", examples)]
fn a_question_update_without_the_header_reports_it_as_required() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    init(directory.path(), "default");
    shaping_records(repo, "default");

    provenance()
        .args([
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
#[verifies("rule_porcelain_get_returns_record", examples)]
fn a_record_read_shows_the_edit_etag() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    init(directory.path(), "default");
    shaping_records(repo, "default");

    for (family, id) in [("requirements", "req_a"), ("topics", "topic_open")] {
        let expected = json(provenance().args([
            "api",
            &format!("{family}/{id}"),
            "--repo",
            repo,
            "--format",
            "json",
        ]));
        let etag = expected["data"]["edit"]["etag"].as_str().unwrap();
        let read = json(provenance().args([id, "--repo", repo, "--format", "json"]));
        assert_eq!(read["edit"]["etag"].as_str(), Some(etag), "{id}");
        let read = json(provenance().args([id, "get", "--repo", repo, "--format", "json"]));
        assert_eq!(read["edit"]["etag"].as_str(), Some(etag), "{id}");
    }
}

// Implementation aid: keeps the bundled skill's Question update in step with the If-Match guard.
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
    let words = skill.split_whitespace().collect::<Vec<_>>().join(" ");

    assert!(words.contains("provenance <question_id> --view review --scope <scope> --format json"));
    assert!(
        words.contains("update --scope <scope> \\ --if-match <etag> \\ --status blocked_on_human")
    );
    assert!(!words.contains("update --scope <scope> \\ --status blocked_on_human"));
}
