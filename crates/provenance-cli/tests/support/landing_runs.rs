//! A repository where Git merges the state files with the JSONL merge
//! driver, and the commands that land swarm runs in it and read the result.

use super::merge_driver::{cli, git, git_ok, json, repository};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The directory of the ideation state files of the default scope.
const IDEATION: &str = ".provenance/state/scopes/default/ideation";

/// The read of each family that a swarm run writes.
const FAMILIES: [&str; 5] = [
    "contributions",
    "synthesis-packets",
    "proposals",
    "assertions",
    "dispositions",
];

/// A repository with the Requirement that the landed records target, and a
/// directory of its own for the swarm runs that the test lands.
pub struct Landing {
    repo: tempfile::TempDir,
    runs: tempfile::TempDir,
}

impl Landing {
    pub fn repo(&self) -> &Path {
        self.repo.path()
    }

    /// The directory of the swarm run `run`.
    pub fn run_dir(&self, run: &str) -> PathBuf {
        self.runs.path().join(format!("run_{run}"))
    }
}

pub fn landing_repository() -> Landing {
    let dir = repository();
    json(
        dir.path(),
        &[
            "req_a",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The system is safe.",
        ],
    );
    Landing {
        repo: dir,
        runs: tempfile::tempdir().unwrap(),
    }
}

/// The bytes of each file of the ideation directory, by file name.
pub fn ideation_files(repo: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(repo.join(IDEATION))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            (name, std::fs::read(entry.path()).unwrap())
        })
        .collect()
}

/// A contribution to `req_a` with one claim `claim_<id>`.
pub fn contribution(id: &str, finding: &str) -> Value {
    serde_json::json!({
        "scope_id": "default",
        "id": id,
        "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
        "participant_slot": "slot_a",
        "stance": "support",
        "strongest_finding": finding,
        "evidence_references": [{"reference_id": format!("evidence_{id}"),
            "evidence_type": "source", "summary": "Pinned"}],
        "material_claims": [{"claim_id": format!("claim_{id}"), "statement": finding,
            "evidence_type": "source", "evidence_reference_ids": [format!("evidence_{id}")]}],
        "risks": [],
        "objections": [],
        "challenges": [],
        "suggested_artifact_changes": [],
        "unsupported_recommendations": [],
        "uncertainty": {"level": "low", "rationale": "Direct."},
        "open_questions": []
    })
}

/// The merge output of a run that suggests, lands and asserts `proposal_p`,
/// with `title`, on the claim `claim`.
pub fn proposal_run(title: &str, claim: &str) -> Value {
    let target = serde_json::json!({"artifact_type": "requirement", "artifact_id": "req_a"});
    serde_json::json!({
        "synthesis_packet": {"scope_id": "default", "id": "packet_p", "target": target,
            "summary": "Adjudicated", "consensus": [], "contested_claims": [],
            "minority_objections": [], "evidence_gaps": [], "unsupported_speculation": [],
            "open_questions": [],
            "suggested_artifacts": [{"proposal_id": "proposal_p", "proposal_key": "p",
                "proposal_type": "requirement_candidate", "summary": "Candidate",
                "origin_participant_slots": ["slot_a"]}],
            "required_human_decisions": []},
        "assertions": [{"scope_id": "default", "id": "assertion_p", "proposal_id": "proposal_p",
            "synthesis_packet_id": "packet_p", "supporting_claim_ids": [claim]}],
        "proposals": [{"scope_id": "default", "id": "proposal_p", "proposal_key": "p",
            "proposal_type": "requirement_candidate", "title": title, "summary": "Candidate",
            "traceability": {"target": target, "source_ids": [], "evidence_references": [],
                "supporting_claim_ids": [claim]},
            "promotion_state": "proposed"}]
    })
}

/// Lands the swarm run `run` with these contributions and this merge output.
pub fn land_run(landing: &Landing, run: &str, contributions: &[Value], merge: Option<Value>) {
    let repo = landing.repo();
    let run = landing.run_dir(run);
    let extractors = run.join("extractors");
    std::fs::create_dir_all(&extractors).unwrap();
    std::fs::write(
        extractors.join("one.json"),
        Value::from(contributions.to_vec()).to_string(),
    )
    .unwrap();
    if let Some(merge) = merge {
        std::fs::write(run.join("merge.json"), merge.to_string()).unwrap();
    }
    json(
        repo,
        &[
            "swarm-backtrace",
            "land",
            "--replace",
            "--run-dir",
            run.to_str().unwrap(),
        ],
    );
}

/// Lands a swarm run whose one contribution is `contrib_<name>`.
pub fn land(landing: &Landing, name: &str) {
    let id = format!("contrib_{name}");
    land_run(
        landing,
        name,
        &[contribution(&id, "The requirement holds.")],
        None,
    )
}

/// Checks out a new branch at `into`, merges `from` into it, and returns the
/// output of the merge.
pub fn merge_into(repo: &Path, into: &str, from: &str) -> std::process::Output {
    git_ok(repo, &["checkout", "-B", &format!("into_{into}"), into]);
    let output = git(repo, &["merge", from, "-m", "merge"]);
    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    output
}

/// The read of each family of the scope, after a check of the scope passes.
pub fn read_families(repo: &Path) -> Vec<Value> {
    let check = cli(repo, &["check"]);
    assert_eq!(
        check.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&check.stdout)
    );
    FAMILIES
        .iter()
        .map(|family| json(repo, &["api", family])["data"]["items"].clone())
        .collect()
}

/// The contribution ids of a read of the scope.
pub fn contribution_ids(reads: &[Value]) -> Vec<&str> {
    reads[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect()
}

/// The record with `id` in the read of one family.
pub fn read_record<'a>(read: &'a Value, id: &str) -> &'a Value {
    read.as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == id)
        .unwrap_or_else(|| panic!("no record {id} in {read}"))
}
