use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn reviewed_records() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    for (collection, body) in [
        (
            "sources",
            json!({"id":"source_review", "name":"Source", "source_type":"policy"}),
        ),
        (
            "requirements",
            json!({"id":"req_review", "statement":"The system stores records."}),
        ),
        (
            "resolutions",
            json!({"id":"resolution_review", "title":"Decision", "position":"Store records.",
                "rationale":"Records are necessary.", "requirement_ids":["req_review"]}),
        ),
        (
            "rules",
            json!({"id":"rule_review", "statement":"The system stores records.",
                "requirement_ids":["req_review"]}),
        ),
        ("domains", json!({"id":"domain_review", "name":"Review"})),
        (
            "boundaries",
            json!({"id":"boundary_review", "requirement_id":"req_review",
                "statement":"Keep the work in the repository."}),
        ),
        (
            "topics",
            json!({"id":"topic_review", "requirement_id":"req_review", "title":"Review"}),
        ),
        (
            "questions",
            json!({"id":"question_review", "topic_id":"topic_review",
                "question":"Is the work complete?", "resolution_method":"research"}),
        ),
    ] {
        provenance()
            .args([collection, "create", "--repo", &repo, "--stdin"])
            .write_stdin(body.to_string())
            .assert()
            .success();
    }
    (directory, repo)
}

#[test]
#[verifies("rule_porcelain_review_returns_current_state", examples)]
fn review_view_reads_every_review_record_kind() {
    let (_directory, repo) = reviewed_records();
    for (kind, id) in [
        ("source", "source_review"),
        ("requirement", "req_review"),
        ("resolution", "resolution_review"),
        ("rule", "rule_review"),
        ("domain", "domain_review"),
        ("boundary", "boundary_review"),
        ("topic", "topic_review"),
        ("question", "question_review"),
    ] {
        let output = provenance()
            .args([
                id, "get", "--view", "review", "--repo", &repo, "--format", "json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{kind}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["record"]["kind"], kind);
        let review = &result["review"];
        assert_eq!(review["decision"]["requirement_id"], id);
        assert_eq!(
            review["decision"]["current_revision"],
            review["edit"]["revision"]
        );
        assert_eq!(review["discussions"]["entries"], json!([]));
        assert_eq!(review["discussions"]["has_more"], false);
        assert!(review["discussions"]["next_cursor"].is_null());
    }
}
