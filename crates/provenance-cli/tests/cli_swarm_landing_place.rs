//! A swarm landing gives each record the place in the file of its type that
//! a direct write of that record gives. The rows of the existing run `z`
//! sort after the ids of the records that the test lands.

use crate::support::landing_runs::{
    api_write, contribution, ideation_files, land_run, landing_repository, proposal_run, Landing,
};
use crate::support::merge_driver::{commit, git_ok};
use provenance_macros::verifies;
use serde_json::Value;

/// A repository with a first commit on `main`.
fn base() -> Landing {
    let landing = landing_repository();
    commit(landing.repo(), "base");
    landing
}

/// Lands the run `name`: the contribution `contrib_<name>` and the proposal
/// `proposal_<name>`, which `assertion_<name>` asserts.
fn land_asserted_run(landing: &Landing, name: &str) {
    let claim = format!("claim_contrib_{name}");
    land_run(
        landing,
        name,
        &[contribution(&format!("contrib_{name}"), "Observed.")],
        Some(proposal_run(name, "Candidate", &claim)),
    );
}

/// The synthesis packet `packet_a`, which suggests no proposal.
fn packet_a() -> Value {
    serde_json::json!({
        "scope_id": "default", "id": "packet_a",
        "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
        "summary": "Adjudicated", "consensus": [], "contested_claims": [],
        "minority_objections": [], "evidence_gaps": [], "unsupported_speculation": [],
        "open_questions": [], "suggested_artifacts": [], "required_human_decisions": []
    })
}

/// The proposal `proposal_a`, which no synthesis packet suggests.
fn proposal_a() -> Value {
    serde_json::json!({
        "scope_id": "default", "id": "proposal_a", "proposal_key": "a",
        "proposal_type": "requirement_candidate", "title": "A", "summary": "Candidate",
        "traceability": {
            "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
            "source_ids": [], "evidence_references": [], "supporting_claim_ids": []
        },
        "builds_on": [], "promotion_state": "proposed"
    })
}

#[test]
#[verifies("rule_swarm_run_records_in_kind_files", examples)]
fn a_landing_places_each_record_where_its_direct_write_places_it() {
    let landing = base();
    let repo = landing.repo();
    land_asserted_run(&landing, "z");
    commit(repo, "z");
    git_ok(repo, &["checkout", "-b", "direct"]);
    for (path, record) in [
        ("/contributions", contribution("contrib_a", "A.")),
        ("/synthesis-packets", packet_a()),
        ("/proposals", proposal_a()),
    ] {
        api_write(&landing, "post", path, &record, &["scope_id"]);
    }
    commit(repo, "direct");
    let direct = ideation_files(repo);

    git_ok(repo, &["checkout", "-b", "landed", "main"]);
    let merge = serde_json::json!({"synthesis_packet": packet_a(), "proposals": [proposal_a()]});
    land_run(
        &landing,
        "a",
        &[contribution("contrib_a", "A.")],
        Some(merge),
    );
    commit(repo, "landed");
    assert!(
        ideation_files(repo) == direct,
        "the landing and the direct writes give the same files"
    );
}

/// No public surface writes an assertion that a synthesis packet qualifies
/// outside a landing, so the assertion `assertion_p`, which sorts before
/// `assertion_z`, is compared with the file of the other order of the two
/// landings.
#[test]
#[verifies("rule_swarm_run_records_in_kind_files", examples)]
fn the_files_of_two_landings_do_not_depend_on_the_order_of_the_landings() {
    let landing = base();
    let repo = landing.repo();
    let mut files = Vec::new();
    for (branch, runs) in [("z_then_p", ["z", "p"]), ("p_then_z", ["p", "z"])] {
        git_ok(repo, &["checkout", "-b", branch, "main"]);
        for run in runs {
            land_asserted_run(&landing, run);
        }
        commit(repo, branch);
        files.push(ideation_files(repo));
    }
    assert!(
        files[0] == files[1],
        "the two orders of the landings give the same files"
    );
}
