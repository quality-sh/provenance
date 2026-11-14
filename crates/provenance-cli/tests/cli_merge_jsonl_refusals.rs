//! The refusal of `merge-jsonl` when a side holds rows that it cannot merge.

use provenance_macros::verifies;
use serde_json::json;

use crate::support;
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

/// The standard error of `merge-jsonl` with no `--path` when the input
/// `ours.jsonl` in `directory` holds a row that does not parse as a record.
fn merge_with_a_retired_row_in(directory: &std::path::Path) -> (Option<i32>, String) {
    std::fs::create_dir_all(directory).unwrap();
    let retired = rule("rule_a").replacen('{', "{\"retired\":true,", 1);
    let sides = [
        ("base.jsonl", ""),
        ("ours.jsonl", retired.as_str()),
        ("theirs.jsonl", ""),
    ]
    .map(|(name, contents)| {
        let path = directory.join(name);
        std::fs::write(&path, format!("{contents}\n")).unwrap();
        path.to_str().unwrap().to_owned()
    });
    let output = provenance().arg("merge-jsonl").args(&sides).output().unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// The refusal of `ours.jsonl` line 1 at `file`.
fn retired_row_refusal(file: &str) -> (Option<i32>, String) {
    (
        Some(1),
        format!(
            "Error: {file} line 1: record rule_a contains legacy field `retired`; canonical \
             JSONL requires record-deletion migration 026 before this build can read it\n"
        ),
    )
}

/// An input below a repository is named by its path below the repository root.
#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_merge_input_below_a_repository_is_named_by_its_path_in_the_repository() {
    let (directory, _repo) = support::review_paths::init();

    let answer = merge_with_a_retired_row_in(&directory.path().join("fixtures/merge"));

    assert_eq!(answer, retired_row_refusal("fixtures/merge/ours.jsonl"));
}

/// An input that no repository holds is named by its file name only.
#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_merge_input_outside_a_repository_is_named_by_its_file_name() {
    let directory = tempfile::tempdir().unwrap();

    let answer = merge_with_a_retired_row_in(directory.path());

    assert_eq!(answer, retired_row_refusal("ours.jsonl"));
}
