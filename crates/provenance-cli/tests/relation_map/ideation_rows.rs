//! Rows 25 to 34: the ideation records, each a record of an action with its
//! own identity and lifecycle.

use super::model::{Act, Clear, Read, Row};
use serde_json::json;

const EVIDENCE: &str = r#"[{"reference_id":"evidence_code_line","evidence_type":"artifact","summary":"Existing payroll check","file_path":"src/payroll/overtime.rs","line":42}]"#;
const CLAIMS: &str = r#"[{"claim_id":"claim_threshold","statement":"Overtime starts after the award threshold.","evidence_type":"artifact","evidence_reference_ids":["evidence_code_line"],"confidence":0.87}]"#;
const CONSENSUS: &str = r#"[{"statement":"The requirement needs a source reference.","supporting_participant_slots":["reviewer"],"evidence_reference_ids":["evidence_code_line"]}]"#;
const ARTIFACTS: &str = r#"[{"proposal_id":"proposal_a","proposal_key":"req-a-traceability","proposal_type":"requirement_candidate","summary":"Clarify source traceability.","origin_participant_slots":["reviewer"]}]"#;
const DECISIONS: &str = r#"[{"decision_key":"decide_scope","prompt":"Confirm the governing award.","blocks_promotion":true}]"#;

const fn output(pointer: &'static str, expect: serde_json::Value) -> Read {
    Read::Output {
        pointer,
        expect,
        capture: &[],
    }
}

const fn row(
    number: u8,
    relation: &'static str,
    owner: &'static str,
    act: Act,
    read: Read,
    clear: Clear,
) -> Row {
    Row {
        number,
        relation,
        owner,
        act,
        read,
        clear,
    }
}

#[rustfmt::skip]
pub fn ideation_rows() -> Vec<Row> {
    vec![
        row(25, "target", "contribution",
            Act::Cli(&["contributions", "create", "--id", "contrib_a", "--target-type", "requirement", "--target-id", "req_a",
                "--participant-slot", "reviewer", "--stance", "support", "--strongest-finding", "The requirement is supported.",
                "--evidence-json", EVIDENCE, "--claims-json", CLAIMS, "--uncertainty-level", "medium", "--uncertainty-rationale", "Overrides were not reviewed."]),
            output("/target/artifact_id", json!("req_a")), Clear::None),
        row(26, "target", "synthesis packet",
            Act::Cli(&["synthesis-packets", "create", "--id", "synth_a", "--target-type", "requirement", "--target-id", "req_a",
                "--summary", "Participants agree.", "--consensus-json", CONSENSUS, "--suggested-artifacts-json", ARTIFACTS,
                "--required-human-decisions-json", DECISIONS]),
            output("/target/artifact_id", json!("req_a")), Clear::None),
        row(27, "target", "proposal",
            Act::Cli(&["proposals", "create", "--id", "proposal_a", "--proposal-key", "req-a-traceability", "--proposal-type", "requirement_candidate",
                "--title", "Clarify traceability", "--summary", "Add source-backed language.", "--target-type", "requirement", "--target-id", "req_a",
                "--source-id", "source_award", "--evidence-json", EVIDENCE, "--supporting-claim-id", "claim_threshold"]),
            output("/traceability/target/artifact_id", json!("req_a")), Clear::Immutable),
        row(28, "source_ids", "proposal",
            Act::Derived("the same proposals create names its sources"),
            Read::OutputOf { row: 27, pointer: "/traceability/source_ids/0", expect: json!("source_award") }, Clear::Immutable),
        row(31, "proposal_id", "assertion",
            Act::Cli(&["proposals", "assert", "--id", "assertion_a", "--proposal-id", "proposal_a", "--synthesis-packet-id", "synth_a",
                "--supporting-claim-id", "claim_threshold", "--resolve-human-gate", "--decision-key", "decide_scope"]),
            output("/proposal_id", json!("proposal_a")), Clear::Immutable),
        row(32, "synthesis_packet_id", "assertion",
            Act::Derived("the same proposals assert names its packet"),
            Read::OutputOf { row: 31, pointer: "/synthesis_packet_id", expect: json!("synth_a") }, Clear::Immutable),
        row(29, "builds_on", "proposal",
            Act::Cli(&["proposals", "create", "--id", "proposal_b", "--proposal-key", "req-a-follow-up", "--proposal-type", "requirement_candidate",
                "--title", "Follow the assertion", "--summary", "Builds on the first proposal.", "--target-type", "requirement", "--target-id", "req_a",
                "--builds-on", "assertion_a"]),
            output("/builds_on/0", json!("assertion_a")), Clear::Immutable),
        row(30, "duplicate_of / superseded_by", "proposal",
            Act::Excluded("no authoring path: validate_proposal_intrinsic refuses the fields on modern rows, and ADR 0001 routes them through a disposition this cut does not build"),
            Read::None, Clear::None),
        row(33, "proposal_id", "disposition",
            Act::Cli(&["dispositions", "create", "--id", "disposition_a", "--proposal-id", "proposal_a", "--decision", "accepted",
                "--rationale", "Confirmed.", "--actor-id", "ben", "--actor-type", "human",
                "--canonical-artifact-type", "requirement", "--canonical-artifact-id", "req_a"]),
            output("/proposal_id", json!("proposal_a")), Clear::Immutable),
        row(34, "canonical_artifact", "disposition",
            Act::Derived("the same dispositions create names the artifact it produced"),
            Read::OutputOf { row: 33, pointer: "/canonical_artifact/artifact_id", expect: json!("req_a") }, Clear::Immutable),
    ]
}
