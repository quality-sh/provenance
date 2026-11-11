//! The refusal of `merge-jsonl` when a side holds rows that it cannot merge.

use provenance_macros::verifies;
use serde_json::json;

mod support {
    pub mod review_paths;
}
use support::review_paths::provenance;

fn rule(id: &str) -> String {
    json!({"scope_id":"default", "id":id, "statement":"The rule shall hold.",
        "status":"active", "severity":"high", "requirement_ids":["req_a"]})
    .to_string()
}

/// The exit code, whether standard output is empty, and the lines of
/// standard error of `merge-jsonl` on the base, ours and theirs contents.
fn merge(base: &str, ours: &str, theirs: &str) -> (Option<i32>, bool, Vec<String>) {
    let directory = tempfile::tempdir().unwrap();
    let sides = [
        ("base.jsonl", base),
        ("ours.jsonl", ours),
        ("theirs.jsonl", theirs),
    ]
    .map(|(name, contents)| {
        let path = directory.path().join(name);
        std::fs::write(&path, contents).unwrap();
        path.to_str().unwrap().to_owned()
    });
    let output = provenance()
        .arg("merge-jsonl")
        .args(&sides)
        .args([
            "--path",
            ".provenance/state/scopes/default/rules/rule.jsonl",
        ])
        .args(["--format", "json"])
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    (
        output.status.code(),
        output.stdout.is_empty(),
        stderr.lines().map(str::to_owned).collect(),
    )
}

/// A merge side that holds one record id on two rows gives the shard, the
/// side, the line of each row and the id, before the merge starts. A blank
/// line counts as a line.
#[test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
fn merge_jsonl_names_the_side_and_the_lines_of_a_duplicate_id() {
    let ours = format!("{}\n\n{}\n", rule("rule_a"), rule("rule_a"));
    assert_eq!(
        merge("", &ours, &format!("{}\n", rule("rule_b"))),
        (
            Some(1),
            true,
            vec![
                "Error: scopes/default/rules/rule.jsonl line 3: ours holds record id rule_a \
                 at line 1 and at line 3"
                    .to_owned()
            ]
        )
    );
}

/// The sides are checked in the order base, ours, theirs, and the rows of
/// each side in their order. A row with no string id on base comes before
/// an id on two rows of ours, so its refusal wins.
///
/// No Rule states which refusal wins; this test pins the order.
#[test]
fn a_row_with_no_string_id_before_a_duplicate_id_gives_its_own_refusal() {
    let ours = format!("{}\n{}\n", rule("rule_a"), rule("rule_a"));
    assert_eq!(
        merge("{\"x\":1}\n", &ours, ""),
        (
            Some(1),
            true,
            vec!["Error: record is missing string id".to_owned()]
        )
    );
}
