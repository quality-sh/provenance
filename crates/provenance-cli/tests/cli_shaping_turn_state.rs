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
    provenance(&["skills", "install", "--repo", &repo]).success();

    for (id, method) in [
        ("question_grill", "grill"),
        ("question_research", "research"),
    ] {
        provenance(&[
            "questions",
            "create",
            "--repo",
            &repo,
            "--id",
            id,
            "--topic-id",
            "topic_overtime",
            "--question",
            "Which threshold applies?",
            "--method",
            method,
        ])
        .success();
        provenance(&[
            "questions",
            id,
            "claim",
            "--repo",
            &repo,
            "--actor",
            "agent-one",
        ])
        .success();
    }
    provenance(&[
        "topics",
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
            for (collection, id) in [
                ("topics", "topic_overtime"),
                ("questions", "question_grill"),
            ] {
                provenance(&[
                    collection,
                    id,
                    "claim",
                    "--repo",
                    &repo,
                    "--actor",
                    "agent-two",
                ])
                .failure();
            }
        }
        let topic = recovery_record(&repo, "topics", "topic_overtime");
        assert_eq!(topic["id"], "topic_overtime");
        assert_eq!(topic["claimed_by"], "agent-one");
        for (id, method) in [
            ("question_grill", "grill"),
            ("question_research", "research"),
        ] {
            let question = recovery_record(&repo, "questions", id);
            assert_eq!(question["id"], id);
            assert_eq!(question["topic_id"], "topic_overtime");
            assert_eq!(question["claimed_by"], "agent-one");
            assert_eq!(question["resolution_method"], method);
        }
    }
}

fn recovery_record(repo: &str, collection: &str, id: &str) -> serde_json::Value {
    let output = provenance(&[collection, id, "get", "--repo", repo, "--format", "json"])
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
        "questions",
        "create",
        "--repo",
        &repo,
        "--id",
        "question_threshold",
        "--topic-id",
        "topic_overtime",
        "--question",
        "Which threshold applies?",
        "--method",
        "research",
    ])
    .success();

    provenance(&[
        "questions",
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
        "questions",
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
        "topics",
        "topic_overtime",
        "claim",
        "--repo",
        &repo,
        "--actor",
        "agent-one",
    ])
    .success();
    provenance(&["topics", "topic_overtime", "close", "--repo", &repo])
        .success()
        .stdout(contains(r#""status": "closed""#));

    let store = provenance_store::state_store::StateStore::new(
        provenance_store::layout::ProvenanceLayout::new(&repo),
    );
    let etag = store
        .requirement_edit_state(
            &provenance_core::ScopeId::new("default").unwrap(),
            &provenance_core::StableId::new("req_overtime").unwrap(),
        )
        .unwrap()
        .etag;
    provenance_stdin(
        &[
            "requirements",
            "req_overtime",
            "update",
            "--repo",
            &repo,
            "--if-match",
            &etag,
            "--stdin",
        ],
        r#"{"fog":"something about sleepovers"}"#,
    )
    .success()
    .stdout(contains("something about sleepovers"));
}
