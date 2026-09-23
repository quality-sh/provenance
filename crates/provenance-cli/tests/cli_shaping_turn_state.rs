#[path = "shaping_support/fixtures.rs"]
mod fixtures;
#[path = "shaping_support/provenance.rs"]
mod provenance;

use fixtures::{create_source_and_requirement, create_topic, init};
use predicates::str::contains;
use provenance::{provenance, provenance_stdin};

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
