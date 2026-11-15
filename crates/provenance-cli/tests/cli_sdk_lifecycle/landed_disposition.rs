//! A Disposition that names a record that the scope does not have is a fault
//! of the stored records. A landing of an ideation batch stores Dispositions
//! too: when one branch lands a Disposition of a Requirement and a second
//! branch deletes that Requirement, a merge of the two gives this state. Each
//! read of the state refuses it at the line of the landing.

use super::plan_apply::{document, route, run, Answer, KEPT};
use super::porcelain_authoring_support::initialized_repo;
use provenance_macros::verifies;
use serde_json::{json, Value};

const LANDINGS: &str = "scopes/default/ideation/landings.jsonl";

/// Writes one landing whose Disposition names the Requirement `req_missing`,
/// which the scope does not have.
fn land_disposition_of_missing_requirement(repo: &str) {
    let source = [
        "source_anchor", "create", "--type", "source", "--repo", repo, "--name", "Anchor",
        "--source-type", "document", "--format", "json",
    ];
    assert_eq!(run(&source, None).code, Some(0));
    let batch = json!({
        "contributions": [], "synthesis_packets": [], "assertions": [],
        "proposals": [{
            "scope_id": "default", "id": "proposal_a", "proposal_key": "a",
            "proposal_type": "source_gap", "title": "A", "summary": "A",
            "traceability": {
                "target": {"artifact_type": "source", "artifact_id": "source_anchor"},
                "source_ids": [], "evidence_references": [], "supporting_claim_ids": []
            },
            "promotion_state": "proposed"
        }],
        "dispositions": [{
            "scope_id": "default", "id": "disposition_a", "proposal_id": "proposal_a",
            "decision": "rejected", "rationale": "Reviewed",
            "actor": {"identity_type": "human", "id": "reviewer"},
            "canonical_artifact": {"artifact_type": "requirement", "artifact_id": "req_missing"}
        }]
    });
    let path = std::path::Path::new(repo).join(".provenance/state").join(LANDINGS);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("{batch}\n")).unwrap();
}

fn error(answer: &Answer) -> Value {
    assert_eq!(answer.code, Some(1), "{}", answer.stdout);
    let text = answer.stderr.strip_prefix("Error: ").unwrap_or(&answer.stderr);
    serde_json::from_str::<Value>(text).unwrap_or_else(|_| panic!("{}", answer.stderr))["error"]
        .clone()
}

#[test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
fn each_read_refuses_a_landed_disposition_of_a_missing_record_at_its_line() {
    let (_directory, repo) = initialized_repo(Some("reviewer"));
    land_disposition_of_missing_requirement(&repo);
    let kept_only = document(&[KEPT]);

    let check = run(&["check", "--repo", &repo, "--format", "json"], None);
    let materialize = run(&["materialize", "--repo", &repo, "--format", "json"], None);
    let plan = route(&repo, "authoring-plans", &kept_only, true);
    let apply = route(&repo, "authoring-changes", &kept_only, true);

    let fault = json!({
        "kind": "invalid_state_file", "file": LANDINGS, "line": 1,
        "message": "disposition disposition_a names requirement req_missing, \
                    which the scope does not have"
    });
    for (surface, answer) in [
        ("check", check),
        ("materialize", materialize),
        ("plan", plan),
        ("apply", apply),
    ] {
        assert_eq!(error(&answer), fault, "{surface}");
    }
}
