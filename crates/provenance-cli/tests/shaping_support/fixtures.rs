use predicates::str::contains;

use crate::provenance::provenance;

pub fn init(repo: &str) {
    provenance(&[
        "init",
        "--path",
        repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ])
    .success();
}

pub fn create_source_and_requirement(repo: &str) {
    // The requirement seeds through the import path, which keeps the record
    // plain: the shaping roundtrip pins topic, question, and thread flows, and
    // a CLI-created Requirement would enroll its scope into the review
    // journal, whose lossless export is a later phase.
    let seed = std::path::Path::new(repo).join("seed.json");
    std::fs::write(
        &seed,
        serde_json::json!({
            "scope": "default",
            "sources": [],
            "requirements": [{
                "schema_version": 2,
                "scope_id": "default",
                "id": "req_overtime",
                "statement": "Overtime must follow SCHADS thresholds",
                "status": "discovery"
            }],
            "resolutions": [],
            "rules": [],
            "threads": [],
            "messages": []
        })
        .to_string(),
    )
    .unwrap();
    provenance(&[
        "import",
        "--repo",
        repo,
        "--scope",
        "default",
        "--input",
        seed.to_str().unwrap(),
        "--format",
        "json",
    ])
    .success();

    provenance(&[
        "sources",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "source_schads",
        "--name",
        "SCHADS Award",
        "--format",
        "json",
    ])
    .success();
}

pub fn create_topic(repo: &str) {
    provenance(&[
        "topics",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "topic_overtime",
        "--requirement-id",
        "req_overtime",
        "--title",
        "Overtime eligibility",
        "--status",
        "open",
        "--links-json",
        r#"[{"target_type":"source","target_id":"source_schads"}]"#,
        "--format",
        "json",
    ])
    .success()
    .stdout(contains("topic_overtime"))
    .stdout(contains(r#""status": "open""#));
}
