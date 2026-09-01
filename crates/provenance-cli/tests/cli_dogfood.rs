#![cfg(feature = "dogfood")]

use assert_cmd::Command;
use std::path::Path;

fn note_cmd(spool_dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("provenance").unwrap();
    cmd.env("PROVENANCE_DOGFOOD_DIR", spool_dir)
        .env_remove("PROVENANCE_SESSION_ID")
        .env_remove("WORKFLOWD_SESSION_ID")
        .env_remove("CLAUDE_SESSION_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("OPENCODE_SESSION_ID");
    cmd
}

fn read_spool(spool_dir: &Path) -> Vec<serde_json::Value> {
    let raw = std::fs::read_to_string(spool_dir.join("notes.jsonl")).unwrap();
    raw.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn note_appends_structured_jsonl_line() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().join("work");
    std::fs::create_dir_all(&cwd).unwrap();

    note_cmd(temp.path())
        .env("PROVENANCE_SESSION_ID", "sess-123")
        .current_dir(&cwd)
        .args([
            "dogfood",
            "note",
            "--surface",
            "prime",
            "--category",
            "friction",
            "--severity",
            "annoyance",
            "--detail",
            "Had to run three follow-up queries to see full rule statements.",
            "prime output truncates rule statements",
        ])
        .assert()
        .success();

    let notes = read_spool(temp.path());
    assert_eq!(notes.len(), 1);
    let note = &notes[0];
    assert_eq!(note["surface"], "prime");
    assert_eq!(note["category"], "friction");
    assert_eq!(note["severity"], "annoyance");
    assert_eq!(note["summary"], "prime output truncates rule statements");
    assert_eq!(
        note["detail"],
        "Had to run three follow-up queries to see full rule statements."
    );
    assert_eq!(note["session_id"], "sess-123");
    assert!(note["ts_ms"].as_i64().unwrap() > 0);
    assert_eq!(note["provenance_version"], env!("CARGO_PKG_VERSION"));
    assert!(note["host"].as_str().is_some());
    // cwd is not a git repo: repo context is null, capture still succeeds
    assert!(note["repo"].is_null());
    assert!(note["branch"].is_null());
    assert!(note["commit"].is_null());
}

#[test]
fn note_captures_claude_code_session_id() {
    let temp = tempfile::tempdir().unwrap();

    note_cmd(temp.path())
        .env("CLAUDE_CODE_SESSION_ID", "cc-sess-9")
        .args([
            "dogfood",
            "note",
            "--surface",
            "general",
            "--category",
            "friction",
            "--severity",
            "annoyance",
            "from claude code",
        ])
        .assert()
        .success();

    let notes = read_spool(temp.path());
    assert_eq!(notes[0]["session_id"], "cc-sess-9");
}

#[test]
fn note_without_session_env_records_null_session() {
    let temp = tempfile::tempdir().unwrap();

    note_cmd(temp.path())
        .args([
            "dogfood",
            "note",
            "--surface",
            "general",
            "--category",
            "idea",
            "--severity",
            "annoyance",
            "an idea",
        ])
        .assert()
        .success();

    let notes = read_spool(temp.path());
    assert!(notes[0]["session_id"].is_null());
}

#[test]
fn note_accepts_any_known_subcommand_as_surface() {
    let temp = tempfile::tempdir().unwrap();

    note_cmd(temp.path())
        .args([
            "dogfood",
            "note",
            "--surface",
            "coverage",
            "--category",
            "missing",
            "--severity",
            "workaround",
            "coverage scan has no way to ignore vendored code",
        ])
        .assert()
        .success();
}

#[test]
fn note_rejects_unknown_surface_and_lists_valid_ones() {
    let temp = tempfile::tempdir().unwrap();

    note_cmd(temp.path())
        .args([
            "dogfood",
            "note",
            "--surface",
            "not-a-real-surface",
            "--category",
            "friction",
            "--severity",
            "annoyance",
            "whatever",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("prime"))
        .stderr(predicates::str::contains("general"));

    assert!(!temp.path().join("notes.jsonl").exists());
}

#[test]
fn list_outputs_all_notes_as_json() {
    let temp = tempfile::tempdir().unwrap();

    for summary in ["first", "second"] {
        note_cmd(temp.path())
            .args([
                "dogfood",
                "note",
                "--surface",
                "general",
                "--category",
                "confusion",
                "--severity",
                "annoyance",
                summary,
            ])
            .assert()
            .success();
    }

    let output = note_cmd(temp.path())
        .args(["dogfood", "list", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let notes: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let notes = notes.as_array().unwrap();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0]["summary"], "first");
    assert_eq!(notes[1]["summary"], "second");
}

#[test]
fn list_with_empty_spool_outputs_empty_array() {
    let temp = tempfile::tempdir().unwrap();

    let output = note_cmd(temp.path())
        .args(["dogfood", "list", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let notes: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(notes.as_array().unwrap().len(), 0);
}

#[test]
fn report_groups_counts_by_surface_category_severity() {
    let temp = tempfile::tempdir().unwrap();

    for (surface, category, severity, summary) in [
        ("prime", "friction", "annoyance", "a"),
        ("prime", "friction", "annoyance", "b"),
        ("coverage", "missing", "blocked", "c"),
    ] {
        note_cmd(temp.path())
            .args([
                "dogfood",
                "note",
                "--surface",
                surface,
                "--category",
                category,
                "--severity",
                severity,
                summary,
            ])
            .assert()
            .success();
    }

    let output = note_cmd(temp.path())
        .args(["dogfood", "report", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["total"], 3);
    let counts = report["counts"].as_array().unwrap();
    let prime = counts
        .iter()
        .find(|c| c["surface"] == "prime")
        .expect("prime bucket");
    assert_eq!(prime["category"], "friction");
    assert_eq!(prime["severity"], "annoyance");
    assert_eq!(prime["count"], 2);
    let coverage = counts
        .iter()
        .find(|c| c["surface"] == "coverage")
        .expect("coverage bucket");
    assert_eq!(coverage["count"], 1);
}

#[test]
fn report_enriches_notes_from_contract_file() {
    let temp = tempfile::tempdir().unwrap();

    note_cmd(temp.path())
        .env("PROVENANCE_SESSION_ID", "sess-abc")
        .args([
            "dogfood",
            "note",
            "--surface",
            "prime",
            "--category",
            "friction",
            "--severity",
            "blocked",
            "enriched note",
        ])
        .assert()
        .success();
    note_cmd(temp.path())
        .args([
            "dogfood",
            "note",
            "--surface",
            "general",
            "--category",
            "idea",
            "--severity",
            "annoyance",
            "unenriched note",
        ])
        .assert()
        .success();

    let enrichment = serde_json::json!({
        "contract": "provenance-dogfood-enrichment/v1",
        "sessions": {
            "sess-abc": {
                "harness": "opencode",
                "harness_version": "1.2.3",
                "model": "claude-opus-5",
                "machine": "mint",
                "agent": "build",
                "repository": "BNasraoui/provenance"
            }
        }
    });
    let enrich_path = temp.path().join("sessions.json");
    std::fs::write(&enrich_path, serde_json::to_vec(&enrichment).unwrap()).unwrap();

    let output = note_cmd(temp.path())
        .args([
            "dogfood",
            "report",
            "--enrich",
            enrich_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let notes = report["notes"].as_array().unwrap();
    let enriched = notes
        .iter()
        .find(|n| n["summary"] == "enriched note")
        .unwrap();
    assert_eq!(enriched["session"]["harness"], "opencode");
    assert_eq!(enriched["session"]["model"], "claude-opus-5");
    assert_eq!(enriched["session"]["machine"], "mint");
    let unenriched = notes
        .iter()
        .find(|n| n["summary"] == "unenriched note")
        .unwrap();
    assert!(unenriched["session"].is_null());
}

#[test]
fn report_rejects_unknown_enrichment_contract() {
    let temp = tempfile::tempdir().unwrap();

    let enrichment = serde_json::json!({
        "contract": "something-else/v9",
        "sessions": {}
    });
    let enrich_path = temp.path().join("sessions.json");
    std::fs::write(&enrich_path, serde_json::to_vec(&enrichment).unwrap()).unwrap();

    note_cmd(temp.path())
        .args([
            "dogfood",
            "report",
            "--enrich",
            enrich_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "provenance-dogfood-enrichment/v1",
        ));
}
