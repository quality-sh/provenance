use crate::support::merge_driver::{cli, commit, git, git_ok, json, repository};
use provenance_macros::verifies;
use serde_json::Value;
use std::path::{Path, PathBuf};

const LANDINGS: &str = ".provenance/state/scopes/default/ideation/landings.jsonl";

/// The read of each family that a landing batch holds.
const FAMILIES: [&str; 5] = [
    "contributions",
    "synthesis-packets",
    "proposals",
    "assertions",
    "dispositions",
];

/// A repository with the Requirement that the landed records target, and a
/// directory of its own for the swarm runs that the test lands.
struct Landing {
    repo: tempfile::TempDir,
    runs: tempfile::TempDir,
}

impl Landing {
    fn repo(&self) -> &Path {
        self.repo.path()
    }
}

fn landing_repository() -> Landing {
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

fn landings(repo: &Path) -> PathBuf {
    repo.join(LANDINGS)
}

/// The rows of the landings file, each as the file stores it.
fn landing_lines(repo: &Path) -> Vec<String> {
    std::fs::read_to_string(landings(repo))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// A contribution to `req_a` with one claim `claim_<id>`.
fn contribution(id: &str, finding: &str) -> Value {
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

/// Lands the swarm run `run` with these contributions and this merge output,
/// and returns the batch row that the landing adds.
fn land_run(
    landing: &Landing,
    run: &str,
    contributions: &[Value],
    merge: Option<Value>,
) -> String {
    let repo = landing.repo();
    let run = landing.runs.path().join(format!("run_{run}"));
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
    landing_lines(repo).pop().unwrap()
}

/// Lands a swarm run whose one contribution is `contrib_<name>`.
fn land(landing: &Landing, name: &str) -> String {
    let id = format!("contrib_{name}");
    land_run(landing, name, &[contribution(&id, "The requirement holds.")], None)
}

/// Checks out a new branch at `into`, merges `from` into it, and returns the
/// output of the merge.
fn merge_into(repo: &Path, into: &str, from: &str) -> std::process::Output {
    git_ok(repo, &["checkout", "-B", &format!("into_{into}"), into]);
    let output = git(repo, &["merge", from, "-m", "merge"]);
    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    output
}

/// The read of each family of the scope, after a check of the scope passes.
fn read_families(repo: &Path) -> Vec<Value> {
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
fn contribution_ids(reads: &[Value]) -> Vec<&str> {
    reads[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect()
}

/// The driver run again on the three stages that Git keeps for the landings
/// file after a conflict: its exit code and the merge outcome it prints.
fn driver_outcome(landing: &Landing) -> (Option<i32>, Value) {
    let repo = landing.repo();
    let stages = landing.runs.path().join("stages");
    std::fs::create_dir_all(&stages).unwrap();
    let mut sides = Vec::new();
    for stage in 1..=3 {
        let path = stages.join(format!("{stage}.jsonl"));
        let shown = git(repo, &["show", &format!(":{stage}:{LANDINGS}")]);
        assert!(shown.status.success());
        std::fs::write(&path, shown.stdout).unwrap();
        sides.push(path.to_str().unwrap().to_owned());
    }
    let output = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .arg("merge-jsonl")
        .args(&sides)
        .args(["--path", LANDINGS, "--format", "json"])
        .output()
        .unwrap();
    let outcome = serde_json::from_slice(&output.stdout).unwrap();
    (output.status.code(), outcome)
}

#[test]
#[verifies("rule_landing_batch_merge", examples)]
fn the_merge_keeps_the_different_batches_of_each_side() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base = land(&landing, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let theirs = land(&landing, "theirs");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    let ours = land(&landing, "ours");
    commit(repo, "ours");

    assert_eq!(merge_into(repo, "main", "theirs").status.code(), Some(0));
    assert_eq!(landing_lines(repo), [base, ours, theirs]);
    assert_eq!(
        contribution_ids(&read_families(repo)),
        ["contrib_base", "contrib_ours", "contrib_theirs"]
    );
}

#[test]
#[verifies("rule_landing_batch_merge", examples)]
fn the_merge_keeps_a_batch_that_the_two_sides_add_one_time() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base = land(&landing, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let same = land(&landing, "same");
    let theirs = land(&landing, "theirs");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    assert_eq!(land(&landing, "same"), same);
    commit(repo, "ours");

    assert_eq!(merge_into(repo, "main", "theirs").status.code(), Some(0));
    assert_eq!(landing_lines(repo), [base, same, theirs]);
    assert_eq!(
        contribution_ids(&read_families(repo)),
        ["contrib_base", "contrib_same", "contrib_theirs"]
    );
}

#[test]
#[verifies("rule_landing_batch_merge", examples)]
#[verifies("rule_record_merge", examples)]
fn the_merge_removes_a_batch_that_one_side_removes_and_the_other_keeps() {
    let landing = landing_repository();
    let repo = landing.repo();
    let kept = land(&landing, "kept");
    let removed = land(&landing, "removed");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let theirs = land(&landing, "theirs");
    assert_eq!(landing_lines(repo), [kept.clone(), removed, theirs.clone()]);
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    // The product does not remove a batch, so this side removes it by hand.
    std::fs::write(landings(repo), format!("{kept}\n")).unwrap();
    commit(repo, "ours");

    assert_eq!(merge_into(repo, "main", "theirs").status.code(), Some(0));
    assert_eq!(landing_lines(repo), [kept, theirs]);
    assert_eq!(
        contribution_ids(&read_families(repo)),
        ["contrib_kept", "contrib_theirs"]
    );
}

#[test]
#[verifies("rule_landing_batch_merge", examples)]
#[verifies("rule_record_merge", examples)]
fn a_record_that_two_batches_hold_with_equal_content_merges_to_one_read_in_each_direction() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base = land(&landing, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let same = contribution("contrib_same", "Equal on the two sides.");
    let theirs = land_run(
        &landing,
        "theirs",
        &[same.clone(), contribution("contrib_theirs", "Of theirs.")],
        None,
    );
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "-b", "ours", "main"]);
    let changed = contribution("contrib_base", "Changed by ours only.");
    let ours = land_run(
        &landing,
        "ours",
        &[changed, same, contribution("contrib_ours", "Of ours.")],
        None,
    );
    commit(repo, "ours");

    assert_eq!(merge_into(repo, "ours", "theirs").status.code(), Some(0));
    assert_eq!(landing_lines(repo), [base.clone(), ours.clone(), theirs.clone()]);
    let into_ours = read_families(repo);
    assert_eq!(merge_into(repo, "theirs", "ours").status.code(), Some(0));
    assert_eq!(landing_lines(repo), [base, theirs, ours]);
    assert_eq!(read_families(repo), into_ours);
    assert_eq!(
        contribution_ids(&into_ours),
        ["contrib_base", "contrib_ours", "contrib_same", "contrib_theirs"]
    );
    assert_eq!(into_ours[0][0]["strongest_finding"], "Changed by ours only.");
}

#[test]
#[verifies("rule_record_merge", examples)]
fn two_batches_that_change_one_contribution_differently_conflict_and_keep_ours() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base_record = contribution("contrib_x", "Finding of the base.");
    let base = land_run(&landing, "base", std::slice::from_ref(&base_record), None);
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let their_record = contribution("contrib_x", "Finding of theirs.");
    let theirs = land_run(&landing, "theirs", std::slice::from_ref(&their_record), None);
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "-b", "ours", "main"]);
    let our_record = contribution("contrib_x", "Finding of ours.");
    let ours = land_run(&landing, "ours", std::slice::from_ref(&our_record), None);
    commit(repo, "ours");

    for (into, from, kept, mine, other) in [
        ("ours", "theirs", &ours, &our_record, &their_record),
        ("theirs", "ours", &theirs, &their_record, &our_record),
    ] {
        let merge = merge_into(repo, into, from);
        assert_eq!(merge.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&merge.stderr);
        assert!(
            stderr.contains("merge left 1 conflicting record(s): contributions/contrib_x"),
            "{stderr}"
        );
        let (code, outcome) = driver_outcome(&landing);
        assert_eq!(code, Some(1));
        assert_eq!(outcome["status"], "conflicted");
        assert_eq!(
            outcome["conflicts"],
            serde_json::json!([{
                "kind": "divergent_edit",
                "record_id": "contributions/contrib_x",
                "base": base_record,
                "ours": mine,
                "theirs": other,
            }])
        );
        assert_eq!(landing_lines(repo), [base.clone(), kept.clone()]);
        let read = json(repo, &["api", "contributions"]);
        assert_eq!(read["data"]["items"][0]["strongest_finding"], mine["strongest_finding"]);
        git_ok(repo, &["merge", "--abort"]);
    }
}

/// The merge output of a run that suggests, lands and asserts `proposal_p`,
/// with `title`, on the claim of `contrib_p`.
fn proposal_run(title: &str) -> Value {
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
            "synthesis_packet_id": "packet_p", "supporting_claim_ids": ["claim_contrib_p"]}],
        "proposals": [{"scope_id": "default", "id": "proposal_p", "proposal_key": "p",
            "proposal_type": "requirement_candidate", "title": title, "summary": "Candidate",
            "traceability": {"target": target, "source_ids": [], "evidence_references": [],
                "supporting_claim_ids": ["claim_contrib_p"]},
            "promotion_state": "proposed"}]
    })
}

#[test]
#[verifies("rule_record_merge", examples)]
fn two_batches_that_add_one_proposal_differently_conflict_and_keep_ours() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base = land(&landing, "base");
    commit(repo, "base");
    let evidence = [contribution("contrib_p", "The claim of the proposal.")];
    git_ok(repo, &["checkout", "-b", "theirs"]);
    land_run(&landing, "theirs", &evidence, Some(proposal_run("Of theirs")));
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    let ours = land_run(&landing, "ours", &evidence, Some(proposal_run("Of ours")));
    commit(repo, "ours");

    let merge = merge_into(repo, "main", "theirs");
    assert_eq!(merge.status.code(), Some(1));
    let (code, outcome) = driver_outcome(&landing);
    assert_eq!(code, Some(1));
    let conflicts = outcome["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1, "{outcome}");
    assert_eq!(conflicts[0]["kind"], "add_add");
    assert_eq!(conflicts[0]["record_id"], "proposals/proposal_p");
    assert_eq!(conflicts[0]["base"], Value::Null);
    assert_eq!(conflicts[0]["ours"]["title"], "Of ours");
    assert_eq!(conflicts[0]["theirs"]["title"], "Of theirs");
    assert_eq!(landing_lines(repo), [base, ours]);
    let read = json(repo, &["api", "proposals"]);
    assert_eq!(read["data"]["items"][0]["title"], "Of ours", "{read}");
}
