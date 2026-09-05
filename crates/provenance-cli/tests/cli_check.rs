use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use std::path::Path;

#[test]
fn check_rejects_dangling_artifact_links() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let state = dir.path().join(".provenance/state");
    write_jsonl(
        &state.join("scopes/default/requirements/req.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"req_existing","statement":"Existing requirement","status":"active"}).to_string(),
    );
    write_jsonl(
        &state.join("scopes/default/topics/topic.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"topic_existing","requirement_id":"req_existing","title":"Existing topic","status":"open","links":[{"target_type":"rule","target_id":"rule_missing"}]}).to_string(),
    );

    provenance(dir.path())
        .failure()
        .stderr(contains("dangling reference"))
        .stderr(contains("topic topic_existing"))
        .stderr(contains("link rule rule_missing"));
}

#[test]
fn check_registers_every_scope_record_before_validating_references() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let state = dir.path().join(".provenance/state");
    std::fs::write(
        state.join("manifest.json"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scopes":[{"id":"frontend","path_prefix":"."},{"id":"platform","path_prefix":"services/platform"}]}).to_string(),
    )
    .unwrap();
    write_jsonl(
        &state.join("scopes/platform/requirements/req.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"platform","id":"req_platform","domain_id":"domain_platform","statement":"Platform requirement","status":"active"}).to_string(),
    );
    write_jsonl(
        &state.join("scopes/platform/domains/domain.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"platform","id":"domain_platform","name":"Platform domain"}).to_string(),
    );

    provenance(dir.path())
        .success()
        .stdout(contains(r#""status": "ok""#));
}

#[test]
fn check_rejects_record_whose_embedded_scope_differs_from_directory_scope() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let state = dir.path().join(".provenance/state");
    std::fs::write(
        state.join("manifest.json"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scopes":[{"id":"frontend","path_prefix":"."},{"id":"platform","path_prefix":"services/platform"}]}).to_string(),
    )
    .unwrap();
    write_jsonl(
        &state.join("scopes/frontend/requirements/req.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"frontend","id":"req_frontend","domain_id":"domain_misfiled","statement":"Frontend requirement","status":"active"}).to_string(),
    );
    write_jsonl(
        &state.join("scopes/platform/domains/domain.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"frontend","id":"domain_misfiled","name":"Misfiled domain"}).to_string(),
    );

    provenance(dir.path())
        .failure()
        .stderr(contains("scope ownership finding(s):"))
        .stderr(contains(
            "domain domain_misfiled loaded from scope platform has embedded scope_id frontend",
        ));
}

#[test]
fn check_rejects_dangling_disposition_proposal_id() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let state = dir.path().join(".provenance/state");
    write_jsonl(
        &state.join("scopes/default/ideation/dispositions.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"disposition_missing_proposal","proposal_id":"proposal_missing","decision":"accepted","rationale":"Looks good.","actor":{"identity_type":"human","id":"ben"}}).to_string(),
    );

    provenance(dir.path())
        .failure()
        .stderr(contains("proposal does not exist"));
}

#[test]
fn check_rejects_missing_wrong_kind_and_wrong_scope_canonical_artifacts() {
    for case in ["missing", "wrong_kind", "wrong_scope"] {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path());
        let state = dir.path().join(".provenance/state");
        std::fs::write(
            state.join("manifest.json"),
            if case == "wrong_scope" {
                serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scopes":[{"id":"default","path_prefix":"."},{"id":"other","path_prefix":"other"}],"disposition_actor_ids":["reviewer"]}).to_string()
            } else {
                serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scopes":[{"id":"default","path_prefix":"."}],"disposition_actor_ids":["reviewer"]}).to_string()
            },
        )
        .unwrap();
        write_jsonl(
            &state.join("scopes/default/sources/source.jsonl"),
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"source_anchor","name":"Anchor","source_type":"document"}).to_string(),
        );
        let (artifact_type, artifact_id) = match case {
            "missing" => ("requirement", "req_missing"),
            "wrong_kind" => {
                write_jsonl(
                    &state.join("scopes/default/requirements/req.jsonl"),
                    serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"artifact_collision","statement":"Collision","status":"active"}).to_string(),
                );
                ("source", "artifact_collision")
            }
            "wrong_scope" => {
                write_jsonl(
                    &state.join("scopes/other/requirements/req.jsonl"),
                    serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"other","id":"req_other","statement":"Other","status":"active"}).to_string(),
                );
                ("requirement", "req_other")
            }
            _ => unreachable!(),
        };
        write_jsonl(
            &state.join("scopes/default/ideation/proposal_cards.jsonl"),
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"proposal_a","proposal_key":"a","proposal_type":"source_gap","title":"A","summary":"A","traceability":{"target":{"artifact_type":"source","artifact_id":"source_anchor"},"source_ids":[],"evidence_references":[],"supporting_claim_ids":[]},"promotion_state":"proposed"}).to_string(),
        );
        write_jsonl(
            &state.join("scopes/default/ideation/dispositions.jsonl"),
            format!(
                r#"{{"schema_version":{version},"scope_id":"default","id":"disposition_a","proposal_id":"proposal_a","decision":"rejected","rationale":"Reviewed","actor":{{"identity_type":"human","id":"reviewer"}},"canonical_artifact":{{"artifact_type":"{artifact_type}","artifact_id":"{artifact_id}"}}}}"#,
                version = SUPPORTED_SCHEMA_VERSION.0
            ),
        );

        provenance(dir.path())
            .failure()
            .stderr(contains("canonical artifact does not exist"));
    }
}

#[test]
fn check_rejects_duplicate_evidence_record_ids() {
    let cases = [
        (
            "contributions.jsonl",
            [
    serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"contribution_a","target":{"artifact_type":"source","artifact_id":"source_a"},"participant_slot":"reviewer","stance":"support","strongest_finding":"First","evidence_references":[],"material_claims":[],"risks":[],"objections":[],"challenges":[],"suggested_artifact_changes":[],"unsupported_recommendations":[],"uncertainty":{"level":"low","rationale":"Direct"},"open_questions":[]}).to_string(),
    serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"contribution_a","target":{"artifact_type":"source","artifact_id":"source_a"},"participant_slot":"reviewer","stance":"support","strongest_finding":"Divergent","evidence_references":[],"material_claims":[],"risks":[],"objections":[],"challenges":[],"suggested_artifact_changes":[],"unsupported_recommendations":[],"uncertainty":{"level":"low","rationale":"Direct"},"open_questions":[]}).to_string(),
].join("\n"),
            "duplicate immutable contribution id contribution_a",
        ),
        (
            "synthesis_packets.jsonl",
            [
    serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"synthesis_a","target":{"artifact_type":"source","artifact_id":"source_a"},"summary":"First","consensus":[],"contested_claims":[],"minority_objections":[],"evidence_gaps":[],"unsupported_speculation":[],"open_questions":[],"suggested_artifacts":[],"required_human_decisions":[]}).to_string(),
    serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"synthesis_a","target":{"artifact_type":"source","artifact_id":"source_a"},"summary":"Divergent","consensus":[],"contested_claims":[],"minority_objections":[],"evidence_gaps":[],"unsupported_speculation":[],"open_questions":[],"suggested_artifacts":[],"required_human_decisions":[]}).to_string(),
].join("\n"),
            "duplicate immutable synthesis packet id synthesis_a",
        ),
    ];

    for (file, records, expected) in cases {
        let dir = tempfile::tempdir().unwrap();
        init(dir.path());
        write_jsonl(
            &dir.path()
                .join(".provenance/state/scopes/default/ideation")
                .join(file),
            records,
        );

        provenance(dir.path()).failure().stderr(contains(expected));
    }
}

#[test]
fn check_rejects_dangling_origin_thread_and_message_references() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let state = dir.path().join(".provenance/state");
    write_jsonl(
        &state.join("scopes/default/sources/source.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"source_policy","name":"Policy","source_type":"policy","origin_thread":"thread_missing","origin_message":"message_missing"}).to_string(),
    );

    provenance(dir.path())
        .failure()
        .stderr(contains("source source_policy"))
        .stderr(contains("origin_thread thread thread_missing"))
        .stderr(contains("origin_message message message_missing"));
}

#[test]
fn check_accepts_origin_message_in_non_default_month_shard() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let state = dir.path().join(".provenance/state");
    write_jsonl(
        &state.join("scopes/default/sources/source.jsonl"),
        [
    serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"source_july","name":"July policy","source_type":"policy","origin_thread":"thread_source_july","origin_message":"msg_july"}).to_string(),
    serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"source_august","name":"August policy","source_type":"policy","origin_thread":"thread_source_august","origin_message":"msg_august"}).to_string(),
].join("\n"),
    );
    write_jsonl(
        &state.join("scopes/default/threads/threads.jsonl"),
        [
    serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"thread_source_july","parent":{"node_type":"source","node_id":"source_july"},"status":"active","created_at":1}).to_string(),
    serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"thread_source_august","parent":{"node_type":"source","node_id":"source_august"},"status":"active","created_at":2}).to_string(),
].join("\n"),
    );
    write_jsonl(
        &state.join("scopes/default/threads/2026-07.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"msg_july","thread_id":"thread_source_july","role":"user","body":"July policy discussion","created_at":1}).to_string(),
    );
    write_jsonl(
        &state.join("scopes/default/threads/2026-08.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"msg_august","thread_id":"thread_source_august","role":"user","body":"August policy discussion","created_at":2}).to_string(),
    );

    provenance(dir.path())
        .success()
        .stdout(contains(r#""status": "ok""#));
}

#[test]
fn check_reports_scope_directory_absent_from_manifest() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let state = dir.path().join(".provenance/state");
    write_jsonl(
        &state.join("scopes/unlisted/requirements/req.jsonl"),
        r#"{"corrupt":"record"}"#,
    );

    provenance(dir.path())
        .failure()
        .stderr(contains("scope directory finding(s):"))
        .stderr(contains("scope directory unlisted is absent from manifest"))
        .stderr(predicates::str::contains("dangling reference(s):").not());
}

#[test]
#[cfg(unix)]
fn check_rejects_symlinked_cache_without_writing_to_target() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let cache = dir.path().join(".provenance/cache");
    if cache.exists() {
        std::fs::remove_dir_all(&cache).unwrap();
    }
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, &cache).unwrap();

    provenance(dir.path()).failure();

    assert!(std::fs::read_dir(outside).unwrap().next().is_none());
}

fn init(repo: &Path) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
}

fn provenance(repo: &Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--repo",
            repo.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
}

fn write_jsonl(path: &Path, record: impl AsRef<str>) {
    let record = record.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("{record}\n")).unwrap();
}
