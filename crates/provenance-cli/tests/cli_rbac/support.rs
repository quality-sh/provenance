//! Fixtures shared by the CLI RBAC suites: binary access, manifest
//! installation, and the seeded live proposal.

#![allow(dead_code)]

use assert_cmd::prelude::*;
use std::process::Command;

pub const MISSING_CLAIM: &str =
    "rbac: no actor claim supplied for a mutating operation on an rbac-managed repository";

pub const AMBIGUOUS: &str = "ambiguous manifest: disposition_actor_ids and rbac.assignments are both present; move disposition actors into rbac assignments and remove disposition_actor_ids";

pub const RATIFICATION: &str =
    "rbac: disposition actor reviewer needs an assignment with identity_type human to end a live proposal";

pub fn provenance() -> Command {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.env("PROVENANCE_SKIP_ONBOARDING", "1");
    command
}

pub fn init_repo(directory: &std::path::Path) {
    provenance()
        .args([
            "init",
            "--path",
            directory.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
}

pub fn manifest_path(directory: &std::path::Path) -> std::path::PathBuf {
    directory.join(".provenance/state/manifest.json")
}

/// Replaces the manifest wholesale; the JSON must be a complete manifest.
pub fn install_manifest(directory: &std::path::Path, body: &str) {
    std::fs::write(manifest_path(directory), body).unwrap();
}

pub fn grants(assignments: &str) -> String {
    format!(
        r#"{{
        "schema_version": 1,
        "scopes": [{{"id": "default", "path_prefix": "."}}],
        "disposition_actor_ids": [],
        "rbac": {assignments}
    }}"#
    )
}

pub fn reviewer_human(capabilities: &str) -> String {
    reviewer_human_scoped(capabilities, "\"default\"")
}

pub fn reviewer_human_scoped(capabilities: &str, scopes: &str) -> String {
    format!(
        r#"{{"assignments": [{{"actor_id": "reviewer", "identity_type": "human",
            "capabilities": [{capabilities}], "scopes": [{scopes}]}}]}}"#
    )
}

/// Seeds a scope holding a live (asserted) proposal by writing canonical
/// shards directly, so disposition tests can drive the write path alone.
pub fn seed_live_proposal(directory: &std::path::Path) {
    let scopes = directory.join(".provenance/state/scopes/default");
    for dir in ["ideation", "requirements", "sources"] {
        std::fs::create_dir_all(scopes.join(dir)).unwrap();
    }
    std::fs::write(
        scopes.join("requirements/req.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"req_overtime","declared_by":null,"declaration_address":null,"retired":false,"statement":"Overtime must be traceable","description":null,"fog":null,"status":"active","domain_id":null,"source_refs":[],"origin_thread":null,"origin_message":null}
"#,
    )
    .unwrap();
    std::fs::write(
        scopes.join("sources/source.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"source_policy","declared_by":null,"declaration_address":null,"retired":false,"name":"Policy","source_type":"policy","url":null,"reference":null,"commit_pin":null,"effective_date":null,"review_date":null,"superseded_by":null,"origin_thread":null,"origin_message":null}
"#,
    )
    .unwrap();
    std::fs::write(
        scopes.join("ideation/contributions.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"contribution_overtime","target":{"artifact_type":"requirement","artifact_id":"req_overtime"},"participant_slot":"reviewer","stance":"support","strongest_finding":"Observed","evidence_references":[{"reference_id":"evidence_overtime","evidence_type":"source","summary":"Pinned"}],"material_claims":[{"claim_id":"claim_overtime","statement":"Observed","evidence_type":"source","evidence_reference_ids":["evidence_overtime"],"confidence":null}],"risks":[],"objections":[],"challenges":[],"suggested_artifact_changes":[],"unsupported_recommendations":[],"uncertainty":{"level":"low","rationale":"Direct"},"open_questions":[]}
"#,
    )
    .unwrap();
    std::fs::write(
        scopes.join("ideation/synthesis_packets.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"synthesis_overtime","target":{"artifact_type":"requirement","artifact_id":"req_overtime"},"summary":"Adjudicated","consensus":[],"contested_claims":[],"minority_objections":[],"evidence_gaps":[],"unsupported_speculation":[],"open_questions":[],"suggested_artifacts":[{"proposal_id":"proposal_overtime","proposal_key":"overtime","proposal_type":"requirement_candidate","summary":"Candidate","origin_participant_slots":["reviewer"]}],"required_human_decisions":[]}
"#,
    )
    .unwrap();
    std::fs::write(
        scopes.join("ideation/proposal_cards.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"proposal_overtime","proposal_key":"overtime","proposal_type":"requirement_candidate","title":"Overtime","summary":"Candidate","confidence":null,"traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_overtime"},"source_ids":[],"evidence_references":[],"supporting_claim_ids":["claim_overtime"]},"promotion_state":"proposed","builds_on":[],"duplicate_of":null,"superseded_by":null}
"#,
    )
    .unwrap();
    std::fs::write(
        scopes.join("ideation/assertions.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"assertion_overtime","proposal_id":"proposal_overtime","synthesis_packet_id":"synthesis_overtime","supporting_claim_ids":["claim_overtime"]}
"#,
    )
    .unwrap();
}

pub fn create_source_assert(
    directory: &std::path::Path,
    actor: Option<&str>,
) -> assert_cmd::assert::Assert {
    let mut command = provenance();
    command.args([
        "sources",
        "create",
        "--repo",
        directory.to_str().unwrap(),
        "--scope",
        "default",
        "--id",
        "source_added",
        "--name",
        "Added",
    ]);
    if let Some(actor) = actor {
        command.arg("--actor-id").arg(actor);
    }
    command.assert()
}

pub fn dispositions_create(
    directory: &std::path::Path,
    id: &str,
    actor_type: &str,
) -> assert_cmd::assert::Assert {
    provenance()
        .current_dir(directory)
        .args(["--actor-id", "reviewer"])
        .args([
            "dispositions",
            "create",
            "--scope",
            "default",
            "--id",
            id,
            "--proposal-id",
            "proposal_overtime",
            "--decision",
            "accepted",
            "--rationale",
            "Reviewed",
            "--actor-type",
            actor_type,
            "--actor-id",
            "reviewer",
        ])
        .assert()
}
