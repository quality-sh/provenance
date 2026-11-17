//! A Disposition that names a record that the scope does not have is a fault
//! of the stored records: when one branch stores a Disposition of a
//! Requirement and a second branch deletes that Requirement, a merge of the
//! two gives this state. Each read of the state refuses it at the line of the
//! Disposition.

use super::plan_apply::{document, route, run, Answer, KEPT};
use super::porcelain_authoring_support::initialized_repo;
use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use provenance_macros::verifies;
use serde_json::{json, Value};

const PROPOSALS: &str = "scopes/default/ideation/proposal_cards.jsonl";
const DISPOSITIONS: &str = "scopes/default/ideation/dispositions.jsonl";

/// Stores a proposal and its Disposition, which names the Requirement
/// `req_missing` that the scope does not have.
fn store_disposition_of_missing_requirement(repo: &str) {
    let source = [
        "source_anchor",
        "create",
        "--type",
        "source",
        "--repo",
        repo,
        "--name",
        "Anchor",
        "--source-type",
        "document",
        "--format",
        "json",
    ];
    assert_eq!(run(&source, None).code, Some(0));
    let proposal = json!({
        "scope_id": "default", "id": "proposal_a", "proposal_key": "a",
        "proposal_type": "source_gap", "title": "A", "summary": "A",
        "traceability": {
            "target": {"artifact_type": "source", "artifact_id": "source_anchor"},
            "source_ids": [], "evidence_references": [], "supporting_claim_ids": []
        },
        "promotion_state": "proposed"
    });
    let disposition = json!({
        "scope_id": "default", "id": "disposition_a", "proposal_id": "proposal_a",
        "decision": "rejected", "rationale": "Reviewed",
        "actor": {"identity_type": "human", "id": "reviewer"},
        "canonical_artifact": {"artifact_type": "requirement", "artifact_id": "req_missing"}
    });
    let state = std::path::Path::new(repo).join(".provenance/state");
    std::fs::create_dir_all(state.join(DISPOSITIONS).parent().unwrap()).unwrap();
    std::fs::write(state.join(PROPOSALS), format!("{proposal}\n")).unwrap();
    std::fs::write(state.join(DISPOSITIONS), format!("{disposition}\n")).unwrap();
}

/// The refusal that `answer` gives: the error object of the JSON form, else
/// the text. `check` and `materialize` give the text form.
fn refusal(answer: &Answer) -> Value {
    assert_eq!(answer.code, Some(1), "{}", answer.stdout);
    let text = answer
        .stderr
        .strip_prefix("Error: ")
        .unwrap_or(&answer.stderr);
    serde_json::from_str::<Value>(text)
        .map_or_else(|_| json!(answer.stderr), |value| value["error"].clone())
}

#[test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
fn each_read_refuses_a_stored_disposition_of_a_missing_record_at_its_line() {
    let (_directory, repo) = initialized_repo(Some("reviewer"));
    store_disposition_of_missing_requirement(&repo);
    let kept_only = document(&[KEPT]);

    let answers = [
        (
            "check",
            run(&["check", "--repo", &repo, "--format", "json"], None),
        ),
        (
            "materialize",
            run(&["materialize", "--repo", &repo, "--format", "json"], None),
        ),
        ("plan", route(&repo, "authoring-plans", &kept_only, true)),
        ("apply", route(&repo, "authoring-changes", &kept_only, true)),
    ];

    let message = "disposition disposition_a names requirement req_missing, \
                   which the scope does not have";
    let fault = json!({"kind": "invalid_state_file", "file": DISPOSITIONS, "line": 1,
        "message": message, "hint": STATE_CORRECTIONS});
    assert_eq!(
        answers.map(|(surface, answer)| (surface, refusal(&answer))),
        [
            ("check", fault.clone()),
            ("materialize", fault.clone()),
            ("plan", fault.clone()),
            ("apply", fault),
        ]
    );
}
