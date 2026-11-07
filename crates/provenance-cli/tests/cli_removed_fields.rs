//! Implementation aid, bound to no Rule: records that the store holds read
//! through `get` whether or not their saved lines still carry the fields that
//! Provenance no longer has. The record reader ignores a field that its
//! record type does not hold, so a line written before the fields were
//! removed keeps reading, and the read does not print the old fields.

use assert_cmd::Command;
use serde_json::{json, Value};
use std::path::Path;

const REMOVED: [&str; 8] = [
    "effective_date",
    "review_date",
    "domain_id",
    "fog",
    "enforcement",
    "confidence",
    "review_on",
    "color",
];

fn provenance(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .args(["--repo", repo.to_str().unwrap()])
        .output()
        .unwrap()
}

fn write_shard(repo: &Path, shard: &str, records: &[Value]) {
    let path = repo.join(".provenance/state/scopes/default").join(shard);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let lines: String = records.iter().map(|record| format!("{record}\n")).collect();
    std::fs::write(path, lines).unwrap();
}

fn get(repo: &Path, id: &str) -> String {
    let output = provenance(repo, &[id, "get", "--format", "json"]);
    assert!(
        output.status.success(),
        "{id}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn records_with_and_without_the_removed_fields_read_through_get() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
        ])
        .assert()
        .success();
    write_shard(
        repo,
        "sources/source.jsonl",
        &[
            json!({"scope_id":"default","id":"source_new","name":"New policy",
                "source_type":"policy"}),
            json!({"scope_id":"default","id":"source_old","name":"Old policy",
                "source_type":"policy","effective_date":1_714_521_600_000_i64,
                "review_date":1_717_200_000_000_i64}),
        ],
    );
    write_shard(
        repo,
        "requirements/req.jsonl",
        &[
            json!({"scope_id":"default","id":"req_new","statement":"The system keeps records.",
                "status":"active"}),
            json!({"scope_id":"default","id":"req_old","statement":"The system stores records.",
                "status":"active","domain_id":"domain_payroll","fog":"Something about storage"}),
        ],
    );
    write_shard(
        repo,
        "resolutions/res.jsonl",
        &[
            json!({"scope_id":"default","id":"res_new","title":"New decision",
                "position":"Keep records.","rationale":"They are needed.","status":"proposed",
                "inputs":[],"requirement_ids":["req_new"]}),
            json!({"scope_id":"default","id":"res_old","title":"Old decision",
                "position":"Store records.","rationale":"They are needed.","status":"proposed",
                "inputs":[],"requirement_ids":["req_old"],"enforcement":"specification",
                "confidence":0.9,"review_on":"2027-01-01"}),
        ],
    );

    for id in [
        "source_new",
        "source_old",
        "req_new",
        "req_old",
        "res_new",
        "res_old",
    ] {
        let text = get(repo, id);
        assert!(text.contains(&format!("\"{id}\"")), "{id}: {text}");
        for field in REMOVED {
            assert!(
                !text.contains(&format!("\"{field}\"")),
                "{id} prints {field}: {text}"
            );
        }
    }
}
