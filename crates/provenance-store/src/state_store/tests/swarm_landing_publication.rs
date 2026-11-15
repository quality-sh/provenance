//! A swarm landing writes each record to the state file of its kind and
//! publishes the files in one write. A failure or a stop of the process at
//! each probe of the publication leaves the complete state before the
//! landing or the complete state after it.

use crate::cache::tests::fixtures::{open, seeded_layout};
use crate::layout::ProvenanceLayout;
use crate::state_store::IdeationLandingBatch;
use crate::stopped_child::{copy_state, run_as_child, state_tree, stop_in_child, CRASH_PHASES};
use crate::test_probes;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::ScopeId;
use provenance_macros::verifies;
use serde_json::json;
use std::collections::BTreeMap;

const CHILD: &str = "state_store::tests::swarm_landing_publication::landing_child";

/// The files of the ideation directory that a landing of [`run`] writes.
const LANDED: [&str; 2] = [
    "scopes/default/ideation/contributions.jsonl",
    "scopes/default/ideation/synthesis_packets.jsonl",
];

/// A run with one contribution and one synthesis packet.
fn run() -> IdeationLandingBatch {
    let target = json!({"artifact_type": "requirement", "artifact_id": "req_schads_overtime"});
    serde_json::from_value(json!({
        "contributions": [{
            "scope_id": "default", "id": "contribution_a", "target": target,
            "participant_slot": "reviewer", "stance": "support",
            "strongest_finding": "Observed", "evidence_references": [],
            "material_claims": [{"claim_id": "claim_a", "statement": "Observed",
                "evidence_type": "source", "evidence_reference_ids": []}],
            "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
            "unsupported_recommendations": [],
            "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
        }],
        "synthesis_packets": [{
            "scope_id": "default", "id": "packet_a", "target": target, "summary": "Adjudicated",
            "consensus": [], "contested_claims": [], "minority_objections": [],
            "evidence_gaps": [], "unsupported_speculation": [], "open_questions": [],
            "suggested_artifacts": [], "required_human_decisions": []
        }]
    }))
    .unwrap()
}

fn land(root: &Utf8Path) -> anyhow::Result<()> {
    let repository = open(&ProvenanceLayout::new(root.to_owned()));
    repository
        .store()
        .land_ideation_batch(&ScopeId::new("default").unwrap(), run(), false)
}

/// A repository, its state tree, and the state tree after the landing.
struct Fixture {
    _dir: tempfile::TempDir,
    _twin: tempfile::TempDir,
    root: Utf8PathBuf,
    old: BTreeMap<Utf8PathBuf, Vec<u8>>,
    new: BTreeMap<Utf8PathBuf, Vec<u8>>,
}

fn fixture() -> Fixture {
    let (dir, layout, _scope) = seeded_layout();
    let root = layout.root().to_owned();
    let old = state_tree(&root);
    let twin = tempfile::tempdir().unwrap();
    let twin_root = Utf8PathBuf::from_path_buf(twin.path().to_path_buf()).unwrap();
    copy_state(&root, &twin_root);
    land(&twin_root).unwrap();
    let new = state_tree(&twin_root);
    Fixture {
        _dir: dir,
        _twin: twin,
        root,
        old,
        new,
    }
}

/// After the next section entry, the state tree is the complete old or the
/// complete new tree, and the read of the store agrees.
fn assert_reopened(fixture: &Fixture, landed: bool, phase: &str) {
    let repository = open(&ProvenanceLayout::new(fixture.root.clone()));
    let scope = ScopeId::new("default").unwrap();
    let contributions = repository.store().list_contributions(&scope).unwrap();
    let packets = repository.store().list_synthesis_packets(&scope).unwrap();
    assert_eq!(
        (contributions.len(), packets.len()),
        if landed { (1, 1) } else { (0, 0) },
        "{phase}: the read holds each record of the landing or none"
    );
    let expected = if landed { &fixture.new } else { &fixture.old };
    assert!(
        state_tree(&fixture.root) == *expected,
        "{phase}: the state is the complete {} state",
        if landed { "new" } else { "old" }
    );
}

#[test]
#[verifies("rule_swarm_run_records_in_kind_files", examples)]
fn a_landing_writes_the_file_of_each_kind_of_its_records() {
    let fixture = fixture();
    let changed: Vec<_> = fixture
        .new
        .iter()
        .filter(|(path, bytes)| fixture.old.get(*path) != Some(*bytes))
        .map(|(path, _)| path.as_str())
        .collect();
    assert_eq!(changed, LANDED);
}

#[test]
fn landing_child() {
    let phases = CRASH_PHASES.map(|(label, _)| label);
    run_as_child(&phases, |root| land(root).unwrap());
}

#[test]
#[verifies("rule_swarm_run_records_in_kind_files", examples)]
fn a_stop_at_each_probe_of_the_publication_leaves_all_of_the_landing_or_none() {
    for (phase, landed) in CRASH_PHASES {
        let fixture = fixture();
        stop_in_child(CHILD, &fixture.root, phase);
        assert_reopened(&fixture, landed, phase);
    }
}

#[test]
#[verifies("rule_swarm_run_records_in_kind_files", examples)]
fn a_failure_at_each_probe_of_the_publication_leaves_all_of_the_landing_or_none() {
    for (phase, _) in CRASH_PHASES {
        let fixture = fixture();
        test_probes::crash_at(phase);
        let result = land(&fixture.root);
        test_probes::disarm(phase);
        assert!(result.is_err(), "{phase}: the probe error reaches the caller");
        assert_reopened(&fixture, phase == "state_published", phase);
    }
}
