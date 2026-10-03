use assert_cmd::Command;
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
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
    let store = StateStore::new(ProvenanceLayout::new(&repo));
    store
        .create_source(
            serde_json::from_value(
                json!({
                    "scope_id":"default", "id":"source_review", "name":"Source",
                    "source_type":"policy", "supersedes":[]
                }),
            )
            .unwrap(),
        )
        .unwrap();
    store.create_requirement(serde_json::from_value(json!({"scope_id":"default","id":"req_review","statement":"The system shows the review state.","status":"active","depends_on":[],"supersedes":[]})).unwrap()).unwrap();
    store.create_resolution(serde_json::from_value(json!({"scope_id":"default","id":"resolution_review","title":"Decision","position":"Show the state.","rationale":"Agents need the state.","status":"draft","requirement_ids":["req_review"],"supersedes":[],"inputs":[]})).unwrap()).unwrap();
    store.create_rule(serde_json::from_value(json!({"scope_id":"default","id":"rule_review","statement":"The system shows the review state.","status":"draft","severity":"medium","requirement_ids":["req_review"],"resolution_ids":[]})).unwrap()).unwrap();
    store
        .create_domain(
            serde_json::from_value(
                json!({"scope_id":"default","id":"domain_review","name":"Review"}),
            )
            .unwrap(),
        )
        .unwrap();
    store.create_boundary(serde_json::from_value(json!({"scope_id":"default","id":"boundary_review","requirement_id":"req_review","statement":"Keep the review bounded."})).unwrap()).unwrap();
    store.create_topic(serde_json::from_value(json!({"scope_id":"default","id":"topic_review","requirement_id":"req_review","title":"Review","status":"open","links":[]})).unwrap()).unwrap();
    store.create_question(serde_json::from_value(json!({"scope_id":"default","id":"question_review","topic_id":"topic_review","question":"Is the review complete?","resolution_method":"research","status":"open","links":[]})).unwrap()).unwrap();
    (directory, repo)
}

#[test]
fn review_view_reads_every_review_record_kind() {
    let (_directory, repo) = reviewed_records();
    for (kind, id, supports_discussions) in [
        ("source", "source_review", true),
        ("requirement", "req_review", true),
        ("resolution", "resolution_review", true),
        ("rule", "rule_review", true),
        ("domain", "domain_review", false),
        ("boundary", "boundary_review", false),
        ("topic", "topic_review", true),
        ("question", "question_review", true),
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
        assert!(result["review"]["edit"]["etag"].is_string());
        assert!(result["review"]["decision"].is_object());
        assert_eq!(result["review"]["discussions"]["entries"], json!([]));
        if !supports_discussions {
            assert_eq!(result["review"]["discussions"]["has_more"], false);
            assert_eq!(result["review"]["follow_up_commands"], json!([]));
        }
    }
}
