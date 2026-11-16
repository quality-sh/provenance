//! The refusal of `merge-jsonl` when an input line holds one member two times.
//!
//! No active Rule states the refusal of a merge input line at fault, so these
//! tests have no Rule binding. They pin that the merge driver refuses such a
//! line as each other reader of a state file does, before it writes a result.

use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
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
    stderr: String,
    /// The file of ours after the command.
    ours: String,
}

fn merge(base: &str, ours: &str, theirs: &str) -> Answer {
    let directory = tempfile::tempdir().unwrap();
    let [base, ours_path, theirs] = [
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
        .args([&base, &ours_path, &theirs])
        .args(["--output", &ours_path])
        .args(["--path", SHARD])
        .output()
        .unwrap();
    Answer {
        code: output.status.code(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
        ours: std::fs::read_to_string(&ours_path).unwrap(),
    }
}

/// A line of theirs that holds `id` two times refuses the merge at that line
/// with the message of each other reader, and the file of ours does not
/// change.
#[test]
fn a_line_of_theirs_with_id_two_times_refuses_and_writes_no_result() {
    let base = format!("{}\n", rule("rule_a"));
    let ours = format!("{}\n", rule("rule_a"));
    let repeated = rule("rule_b").replacen('{', "{\"id\":\"rule_shadow\",", 1);
    let theirs = format!("{}\n{repeated}\n", rule("rule_a"));

    assert_eq!(
        merge(&base, &ours, &theirs),
        Answer {
            code: Some(1),
            stdout: String::new(),
            stderr: format!(
                "Error: scopes/default/rules/rule.jsonl line 2: duplicate field `id`; \
                 {STATE_CORRECTIONS}\n"
            ),
            ours,
        }
    );
}

/// A line of base that holds a member that a Rule does not own two times
/// refuses the merge at that line, and the file of ours does not change.
#[test]
fn a_line_of_base_with_an_unknown_member_two_times_refuses_and_writes_no_result() {
    let repeated = rule("rule_a").replacen('}', ",\"zz\":1,\"zz\":2}", 1);
    let ours = format!("{}\n", rule("rule_a"));
    let theirs = format!("{}\n{}\n", rule("rule_a"), rule("rule_b"));

    assert_eq!(
        merge(&format!("{repeated}\n"), &ours, &theirs),
        Answer {
            code: Some(1),
            stdout: String::new(),
            stderr: format!(
                "Error: scopes/default/rules/rule.jsonl line 1: duplicate field `zz`; \
                 {STATE_CORRECTIONS}\n"
            ),
            ours,
        }
    );
}

/// A syntax fault on a line wins over a later line that holds a member two
/// times, because the lines are read in their order.
#[test]
fn a_syntax_fault_on_an_earlier_line_wins_over_a_repeated_member() {
    let repeated = rule("rule_b").replacen('{', "{\"id\":\"rule_shadow\",", 1);
    let ours = format!("{}\n", rule("rule_a"));

    let answer = merge("", &ours, &format!("{{\"id\"\n{repeated}\n"));

    assert_eq!(
        answer,
        Answer {
            code: Some(1),
            stdout: String::new(),
            stderr: format!(
                "Error: scopes/default/rules/rule.jsonl line 1: EOF while parsing an object; \
                 {STATE_CORRECTIONS}\n"
            ),
            ours,
        }
    );
}
