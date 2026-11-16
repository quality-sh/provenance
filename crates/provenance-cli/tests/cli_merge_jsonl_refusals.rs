//! The refusal of `merge-jsonl` when a side holds rows that it cannot merge.

use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use provenance_macros::verifies;
use serde_json::json;

use crate::support;
use support::review_paths::provenance;

const SHARD: &str = ".provenance/state/scopes/default/rules/rule.jsonl";

fn rule(id: &str) -> String {
    json!({"scope_id":"default", "id":id, "statement":"The rule shall hold.",
        "status":"active", "severity":"high", "requirement_ids":["req_a"]})
    .to_string()
}

/// The answer of `merge-jsonl` in the form that Git runs it: the result goes
/// to the file of ours.
#[derive(Debug, PartialEq, Eq)]
struct Answer {
    code: Option<i32>,
    stdout: String,
    stderr: Vec<String>,
    /// The file of ours after the command.
    ours: String,
}

/// The answer of `merge-jsonl` on the base, ours and theirs contents, for the
/// Rule shard.
fn merge(base: impl AsRef<[u8]>, ours: &str, theirs: impl AsRef<[u8]>) -> Answer {
    let directory = tempfile::tempdir().unwrap();
    let [base, ours_path, theirs] = [
        ("base.jsonl", base.as_ref()),
        ("ours.jsonl", ours.as_bytes()),
        ("theirs.jsonl", theirs.as_ref()),
    ]
    .map(|(name, contents)| {
        let path = directory.path().join(name);
        std::fs::write(&path, contents).unwrap();
        path.to_str().unwrap().to_owned()
    });
    let output = provenance()
        .arg("merge-jsonl")
        .args([&base, &ours_path, &theirs])
        .args(["--output", &ours_path])
        .args(["--path", SHARD])
        .args(["--format", "json"])
        .output()
        .unwrap();
    Answer {
        code: output.status.code(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect(),
        ours: std::fs::read_to_string(&ours_path).unwrap(),
    }
}

/// The answer of a refusal at `line` of the Rule shard with `message`, when
/// the file of ours holds `ours`.
fn refusal(line: usize, message: &str, ours: String) -> Answer {
    Answer {
        code: Some(1),
        stdout: String::new(),
        stderr: vec![format!(
            "Error: scopes/default/rules/rule.jsonl line {line}: {message}; {STATE_CORRECTIONS}"
        )],
        ours,
    }
}

/// A merge side that holds one record id on two rows gives the shard, the
/// side, the line of each row and the id, before the merge starts. A blank
/// line counts as a line.
#[test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
fn merge_jsonl_names_the_side_and_the_lines_of_a_duplicate_id() {
    let ours = format!("{}\n\n{}\n", rule("rule_a"), rule("rule_a"));
    assert_eq!(
        merge("", &ours, format!("{}\n", rule("rule_b"))),
        refusal(
            3,
            "ours holds record id rule_a at line 1 and at line 3",
            ours
        )
    );
}

/// The sides are read in the order base, ours, theirs, and the lines of each
/// side in their order, before any id is compared. A line of base that a read
/// of the Rule file refuses comes before an id on two rows of ours, so its
/// refusal wins.
///
/// No Rule states which refusal wins; this test pins the order.
#[test]
fn a_line_of_base_that_does_not_read_wins_over_a_duplicate_id_of_ours() {
    let ours = format!("{}\n{}\n", rule("rule_a"), rule("rule_a"));
    assert_eq!(
        merge("{\"x\":1}\n", &ours, ""),
        refusal(1, "base: missing field `scope_id`", ours)
    );
}

/// The exit code and the standard error of `merge-jsonl` with no `--path`,
/// run in `working_directory`, when the input `ours.jsonl` in `directory`
/// holds the one line `ours`.
fn merge_with_no_path(
    directory: &std::path::Path,
    working_directory: &std::path::Path,
    ours: &str,
) -> (Option<i32>, String) {
    std::fs::create_dir_all(directory).unwrap();
    let sides = [
        ("base.jsonl", ""),
        ("ours.jsonl", ours),
        ("theirs.jsonl", ""),
    ]
    .map(|(name, contents)| {
        let path = directory.join(name);
        std::fs::write(&path, format!("{contents}\n")).unwrap();
        path.to_str().unwrap().to_owned()
    });
    let output = provenance()
        .current_dir(working_directory)
        .arg("merge-jsonl")
        .args(&sides)
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// `merge_with_no_path` when the line of ours holds a member `retired`.
fn merge_with_a_retired_row_in(
    directory: &std::path::Path,
    working_directory: &std::path::Path,
) -> (Option<i32>, String) {
    let retired = rule("rule_a").replacen('{', "{\"retired\":true,", 1);
    merge_with_no_path(directory, working_directory, &retired)
}

/// The refusal of `ours.jsonl` line 1 at `file`.
fn retired_row_refusal(file: &str) -> (Option<i32>, String) {
    (
        Some(1),
        format!(
            "Error: {file} line 1: ours: record rule_a contains legacy field `retired`; canonical \
             JSONL requires record-deletion migration 026 before this build can read it\n"
        ),
    )
}

/// An input below the repository of the working directory is named by its
/// path below the repository root. The paths are canonical, as the working
/// directory of a process is.
#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_merge_input_below_a_repository_is_named_by_its_path_in_the_repository() {
    let (directory, _repo) = support::review_paths::init();
    let root = dunce::canonicalize(directory.path()).unwrap();

    let answer = merge_with_a_retired_row_in(&root.join("fixtures/merge"), &root);

    assert_eq!(answer, retired_row_refusal("fixtures/merge/ours.jsonl"));
}

/// An input outside the repository of the working directory is named by its
/// file name only, also when a different checkout holds the input.
#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_merge_input_outside_the_repository_is_named_by_its_file_name() {
    let (directory, _repo) = support::review_paths::init();
    let root = dunce::canonicalize(directory.path()).unwrap();
    let other = tempfile::tempdir().unwrap();
    let checkout = dunce::canonicalize(other.path()).unwrap();
    std::fs::create_dir(checkout.join(".git")).unwrap();

    let answer = merge_with_a_retired_row_in(&checkout.join("inputs"), &root);

    assert_eq!(answer, retired_row_refusal("ours.jsonl"));
}

// No Rule states the refusal of a merge input line that holds a member two
// times. The tests below pin that the merge driver refuses each line that a
// read of the file refuses, names the side and the line of that side's input,
// and does not change the file of ours.

/// `rule(id)` with `member` as its first member.
fn rule_with(id: &str, member: &str) -> String {
    rule(id).replacen('{', &format!("{{{member},"), 1)
}

#[test]
fn a_line_of_theirs_with_id_two_times_refuses_and_names_theirs() {
    let ours = format!("{}\n", rule("rule_a"));
    let theirs = format!(
        "{}\n{}\n",
        rule("rule_a"),
        rule_with("rule_b", r#""id":"x""#)
    );

    assert_eq!(
        merge(&ours, &ours, theirs),
        refusal(2, "theirs: duplicate field `id`", ours)
    );
}

#[test]
fn a_line_of_ours_with_an_unknown_member_two_times_refuses_and_names_ours() {
    let ours = format!(
        "{}\n{}\n",
        rule("rule_a"),
        rule_with("rule_b", r#""zz":1,"zz":2"#)
    );
    let base = format!("{}\n", rule("rule_a"));

    assert_eq!(
        merge(&base, &ours, &base),
        refusal(2, "ours: duplicate field `zz`", ours)
    );
}

/// A member two times in a nested object that the record type owns is
/// refused as a read refuses it, although a JSON value keeps the last one.
#[test]
fn a_nested_member_two_times_refuses_as_a_read_does() {
    let ours = format!("{}\n", rule("rule_a"));
    let commit = |digit: &str| digit.repeat(40);
    let updated = format!(
        r#""updated":{{"commit":"{}","commit":"{}","at":"2026-11-01T00:00:00Z"}}"#,
        commit("a"),
        commit("b")
    );
    let theirs = format!("{}\n{}\n", rule("rule_a"), rule_with("rule_b", &updated));

    assert_eq!(
        merge(&ours, &ours, theirs),
        refusal(2, "theirs: duplicate field `commit`", ours)
    );
}

/// A fault that only base holds refuses the merge, also when ours and theirs
/// both hold the record in a form that reads.
#[test]
fn a_fault_only_in_base_refuses_when_both_sides_hold_a_corrected_line() {
    let base = format!("{}\n", rule_with("rule_a", r#""zz":1,"zz":2"#));
    let ours = format!("{}\n", rule("rule_a"));
    let theirs = format!("{}\n{}\n", rule("rule_a"), rule("rule_b"));

    assert_eq!(
        merge(base, &ours, theirs),
        refusal(1, "base: duplicate field `zz`", ours)
    );
}

/// A line that is not UTF-8 gives the file, the side and the line.
#[test]
fn a_line_that_is_not_utf8_refuses_and_names_the_side() {
    let ours = format!("{}\n", rule("rule_a"));
    let mut theirs = ours.clone().into_bytes();
    theirs.extend_from_slice(b"{\"id\":\"\xff\"}\n");

    assert_eq!(
        merge(&ours, &ours, theirs),
        refusal(
            2,
            "theirs: invalid utf-8 sequence of 1 bytes from index 7",
            ours
        )
    );
}

/// A syntax fault gives the file, the line of the input and the parse
/// message, and it wins over a later line that holds a member two times,
/// because the lines are read in their order.
#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_syntax_fault_gives_the_file_the_line_and_the_parse_message() {
    let ours = format!("{}\n", rule("rule_a"));
    let theirs = format!("{{\"id\"\n{}\n", rule_with("rule_b", r#""id":"x""#));

    assert_eq!(
        merge("", &ours, theirs),
        refusal(1, "theirs: EOF while parsing an object", ours)
    );
}

/// With no `--path`, an input is a file of no known record type. A line of
/// that file that holds a top-level member two times is refused, as each line
/// of a state file is, although a JSON value keeps the last one.
#[test]
fn a_member_two_times_in_a_file_of_no_known_type_refuses_and_names_ours() {
    let (directory, _repo) = support::review_paths::init();
    let root = dunce::canonicalize(directory.path()).unwrap();
    let ours = rule_with("rule_a", r#""zz":1,"zz":2"#);

    let answer = merge_with_no_path(&root.join("fixtures/merge"), &root, &ours);

    assert_eq!(
        answer,
        (
            Some(1),
            format!(
                "Error: fixtures/merge/ours.jsonl line 1: ours: duplicate field `zz`; \
                 {STATE_CORRECTIONS}\n"
            )
        )
    );
}
