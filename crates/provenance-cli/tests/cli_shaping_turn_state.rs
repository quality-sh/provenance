#[path = "shaping_support/fixtures.rs"]
mod fixtures;
#[path = "shaping_support/provenance.rs"]
mod provenance;

use fixtures::{create_source_and_requirement, create_topic, init};
use predicates::str::contains;
use provenance::{provenance, provenance_stdin};

// Characterization evidence: each command starts a new process. This test
// checks stored recovery data, not whether a shaping agent follows its method.
#[test]
fn recovery_reads_survive_process_exit_and_claim_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    init(&repo);
    create_source_and_requirement(&repo);
    create_topic(&repo);
    assert_cmd::Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(&repo)
        .args(["skills", "install"])
        .assert()
        .success();

    for (id, method) in [("question_ask", "ask"), ("question_research", "research")] {
        provenance(&[
            id,
            "create",
            "--type",
            "question",
            "--repo",
            &repo,
            "--topic-id",
            "topic_overtime",
            "--question",
            "Which threshold applies?",
            "--resolution-method",
            method,
        ])
        .success();
        provenance(&[id, "claim", "--repo", &repo, "--actor", "agent-one"]).success();
    }
    provenance(&[
        "topic_overtime",
        "claim",
        "--repo",
        &repo,
        "--actor",
        "agent-one",
    ])
    .success();

    for conflict in [false, true] {
        if conflict {
            for id in ["topic_overtime", "question_ask"] {
                provenance(&[id, "claim", "--repo", &repo, "--actor", "agent-two"]).failure();
            }
        }
        let topic = recovery_record(&repo, "topics", "topic_overtime");
        assert_eq!(topic["id"], "topic_overtime");
        assert_eq!(topic["claimed_by"], "agent-one");
        for (id, method) in [("question_ask", "ask"), ("question_research", "research")] {
            let question = recovery_record(&repo, "questions", id);
            assert_eq!(question["id"], id);
            assert_eq!(question["topic_id"], "topic_overtime");
            assert_eq!(question["claimed_by"], "agent-one");
            assert_eq!(question["resolution_method"], method);
        }
    }
}

fn recovery_record(repo: &str, collection: &str, id: &str) -> serde_json::Value {
    let path = format!("{collection}/{id}");
    let output = provenance(&["api", &path, "--repo", repo, "--format", "json"])
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    value["data"].clone()
}

#[test]
fn cli_shaping_actions_and_requirement_patch_share_the_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    init(&repo);
    create_source_and_requirement(&repo);
    create_topic(&repo);

    provenance(&[
        "question_threshold",
        "create",
        "--type",
        "question",
        "--repo",
        &repo,
        "--topic-id",
        "topic_overtime",
        "--question",
        "Which threshold applies?",
        "--resolution-method",
        "research",
    ])
    .success();

    provenance(&[
        "question_threshold",
        "claim",
        "--repo",
        &repo,
        "--actor",
        "agent-one",
    ])
    .success()
    .stdout(contains(r#""claimed_by": "agent-one""#));
    provenance(&[
        "question_threshold",
        "answer",
        "--repo",
        &repo,
        "--answer",
        "Use the SCHADS overtime threshold.",
    ])
    .success()
    .stdout(contains(r#""status": "answered""#));

    provenance(&[
        "topic_overtime",
        "claim",
        "--repo",
        &repo,
        "--actor",
        "agent-one",
    ])
    .success();
    provenance_stdin(
        &[
            "api",
            "topics/topic_overtime/close",
            "--method",
            "post",
            "--input",
            "-",
            "--repo",
            &repo,
        ],
        "{}",
    )
    .success()
    .stdout(contains(r#""status": "closed""#));

    let store = provenance_store::repository::Repository::open(&repo, "default")
        .unwrap()
        .store();
    let etag = store
        .requirement_edit_state(
            &provenance_core::ScopeId::new("default").unwrap(),
            &provenance_core::StableId::new("req_overtime").unwrap(),
        )
        .unwrap()
        .etag;
    provenance_stdin(
        &[
            "req_overtime",
            "update",
            "--repo",
            &repo,
            "--if-match",
            &etag,
            "--stdin",
        ],
        r#"{"description":"something about sleepovers"}"#,
    )
    .success()
    .stdout(contains("something about sleepovers"));
}
