//! Pins JSON-line preservation in the git merge driver's write: a merged
//! shard is written from the stored lines, so records outside the merge's
//! targets keep their stored JSON spelling. Line terminators are normalized
//! to `\n`.

use assert_cmd::Command;
use std::path::{Path, PathBuf};

struct Sides {
    base: PathBuf,
    ours: PathBuf,
    theirs: PathBuf,
    output: PathBuf,
}

impl Sides {
    fn write(directory: &Path, base: &str, ours: &str, theirs: &str) -> Self {
        let sides = Self {
            base: directory.join("base.jsonl"),
            ours: directory.join("ours.jsonl"),
            theirs: directory.join("theirs.jsonl"),
            output: directory.join("merged.jsonl"),
        };
        std::fs::write(&sides.base, base).unwrap();
        std::fs::write(&sides.ours, ours).unwrap();
        std::fs::write(&sides.theirs, theirs).unwrap();
        sides
    }

    fn merge(&self, shard_path: &str) -> Command {
        let mut command = Command::cargo_bin("provenance").unwrap();
        command.args([
            "merge-jsonl",
            self.base.to_str().unwrap(),
            self.ours.to_str().unwrap(),
            self.theirs.to_str().unwrap(),
            "--output",
            self.output.to_str().unwrap(),
            "--path",
            shard_path,
            "--format",
            "json",
        ]);
        command
    }
}

const RULES_SHARD: &str = ".provenance/state/scopes/default/rules/rule.jsonl";

/// A stored rule line this build would encode differently: the keys sit in
/// stored order with stored spacing, not in canonical field order.
fn stored_spelling(requirement: &str) -> String {
    format!(
        "{{\"id\":\"rule_shared\",\"requirement_ids\":[\"{requirement}\"],\
         \"severity\":\"high\",  \"status\":\"active\",\
         \"statement\":\"The rule_shared rule shall hold.\",\
         \"scope_id\":\"default\",\"schema_version\":2}}\n"
    )
}

fn added_rule() -> String {
    "{\"schema_version\":2,\"scope_id\":\"default\",\"id\":\"rule_added\",\
      \"statement\":\"The rule_added rule shall hold.\",\"status\":\"active\",\
      \"severity\":\"high\",\"requirement_ids\":[\"req_theirs\"]}\n"
        .to_string()
}

#[test]
fn an_untouched_row_keeps_its_stored_json_line_through_the_merge() {
    let directory = tempfile::tempdir().unwrap();
    let stored = stored_spelling("req_base");
    let sides = Sides::write(
        directory.path(),
        &stored,
        &stored,
        &(stored.clone() + &added_rule()),
    );

    sides.merge(RULES_SHARD).assert().success();

    let merged = std::fs::read_to_string(&sides.output).unwrap();
    assert!(
        merged.contains(&stored),
        "the untouched JSON line must land as stored: {merged}"
    );
}

#[test]
fn an_adopted_row_lands_as_the_side_that_moved_it_stored_it() {
    let directory = tempfile::tempdir().unwrap();
    let sides = Sides::write(
        directory.path(),
        &stored_spelling("req_base"),
        &stored_spelling("req_base"),
        // Theirs moved the shared rule to a new requirement and stored the
        // result in their own spelling, which differs from canonical form.
        &stored_spelling("req_theirs"),
    );

    sides.merge(RULES_SHARD).assert().success();

    let merged = std::fs::read_to_string(&sides.output).unwrap();
    assert_eq!(
        merged,
        stored_spelling("req_theirs"),
        "the adopted row must land exactly as theirs stored it: {merged}"
    );
}
