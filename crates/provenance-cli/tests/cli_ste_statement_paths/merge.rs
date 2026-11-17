use super::support::{diagnostic, error_json, provenance, record, REQUIREMENTS_SHARD, RULES_SHARD};
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

struct Sides {
    base: PathBuf,
    ours: PathBuf,
    theirs: PathBuf,
    output: PathBuf,
}

impl Sides {
    fn new(directory: &Path, base: &str, ours: &str, theirs: &str) -> Self {
        let sides = Self {
            base: directory.join("base.jsonl"),
            ours: directory.join("ours.jsonl"),
            theirs: directory.join("theirs.jsonl"),
            output: directory.join("output.jsonl"),
        };
        std::fs::write(&sides.base, base).unwrap();
        std::fs::write(&sides.ours, ours).unwrap();
        std::fs::write(&sides.theirs, theirs).unwrap();
        sides
    }

    fn run(&self, path: &str) -> std::process::Output {
        provenance()
            .args([
                "merge-jsonl",
                self.base.to_str().unwrap(),
                self.ours.to_str().unwrap(),
                self.theirs.to_str().unwrap(),
                "--output",
                self.output.to_str().unwrap(),
                "--path",
                path,
                "--format",
                "json",
            ])
            .output()
            .unwrap()
    }
}

#[test]
#[verifies("rule_ste_merge_changed_statement_gate", examples)]
fn requirement_and_rule_merges_reject_selected_changed_statements_without_touching_output() {
    for (kind, path) in [("requirement", REQUIREMENTS_SHARD), ("rule", RULES_SHARD)] {
        let directory = tempfile::tempdir().unwrap();
        let base = record("record_a", "Original statement", kind);
        let ours = record("record_a", "Café; changed", kind);
        let sides = Sides::new(directory.path(), &base, &ours, &base);
        std::fs::write(&sides.output, b"sentinel output bytes").unwrap();

        let output = sides.run(path);

        assert!(!output.status.success());
        assert_eq!(
            std::fs::read(&sides.output).unwrap(),
            b"sentinel output bytes"
        );
        let report = error_json(&output);
        assert_eq!(
            report,
            json!({
                "error": {
                    "kind": "statement_invalid",
                    "diagnostics": [diagnostic(kind, "record_a", 5)],
                },
                "meta": {},
            })
        );

        let repeated = sides.run(path);
        assert!(!repeated.status.success());
        assert_eq!(error_json(&repeated), report);
        assert_eq!(
            std::fs::read(&sides.output).unwrap(),
            b"sentinel output bytes"
        );
    }
}

#[test]
#[verifies("rule_ste_merge_changed_statement_gate", examples)]
fn existing_three_way_selection_is_analyzed_after_both_sides_change() {
    let directory = tempfile::tempdir().unwrap();
    let first = record("req_a", "Original statement", "requirement");
    let second = record("req_b", "Second statement", "requirement");
    let base = first.clone() + &second;
    let ours = record("req_a", "Ours; invalid", "requirement") + &second;
    let mut theirs_value: Value = serde_json::from_str(second.trim()).unwrap();
    theirs_value["description"] = json!("Their metadata change");
    let theirs = first + &format!("{}\n", serde_json::to_string(&theirs_value).unwrap());
    let sides = Sides::new(directory.path(), &base, &ours, &theirs);

    let output = sides.run(REQUIREMENTS_SHARD);

    assert!(!output.status.success());
    assert_eq!(
        error_json(&output)["error"]["diagnostics"],
        Value::Array(vec![diagnostic("requirement", "req_a", 4)])
    );
    assert!(!sides.output.exists());
}

#[test]
#[verifies("rule_ste_merge_changed_statement_gate", examples)]
fn unchanged_legacy_invalid_record_does_not_block_an_unrelated_clean_merge() {
    let directory = tempfile::tempdir().unwrap();
    let legacy = record("req_legacy", "Legacy; invalid", "requirement");
    let clean = record("req_clean", "Clean addition", "requirement");
    let sides = Sides::new(
        directory.path(),
        &legacy,
        &legacy,
        &(legacy.clone() + &clean),
    );

    let output = sides.run(REQUIREMENTS_SHARD);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let merged = std::fs::read_to_string(&sides.output).unwrap();
    assert!(merged.contains("req_legacy"));
    assert!(merged.contains("req_clean"));
}

#[test]
#[verifies("rule_ste_merge_changed_statement_gate", examples)]
fn conflicted_changed_statement_reports_both_contracts_without_touching_output() {
    let directory = tempfile::tempdir().unwrap();
    let base = record("req_conflict", "Original statement", "requirement");
    let ours = record("req_conflict", "Ours; invalid", "requirement");
    let theirs = record("req_conflict", "Their clean change", "requirement");
    let sides = Sides::new(directory.path(), &base, &ours, &theirs);
    std::fs::write(&sides.output, b"sentinel output bytes").unwrap();

    let output = sides.run(REQUIREMENTS_SHARD);

    assert!(!output.status.success());
    assert_eq!(
        std::fs::read(&sides.output).unwrap(),
        b"sentinel output bytes"
    );
    let conflict: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(conflict["status"], "conflicted");
    assert_eq!(conflict["conflicts"][0]["record_id"], "req_conflict");
    assert_eq!(
        error_json(&output)["error"]["diagnostics"],
        Value::Array(vec![diagnostic("requirement", "req_conflict", 4)])
    );
}
