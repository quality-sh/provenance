//! The stale answers for the cases that shared and kept scans can break: a
//! deleted file, files changed then reverted across two ranges, one blob
//! at two paths, and the evidence query with a base.

use crate::support::{
    apply_rules, cli_api, cli_stale, commit, function, init_repo, write, Goldens,
};
use provenance_core::protocol::{StaleQuery, SDK_PROTOCOL_VERSION};
use provenance_macros::verifies;
use provenance_store::operations::queries;
use provenance_store::operations::read_policy::ReadPolicy;
use provenance_store::repository::Repository;
use serde_json::Value;
use std::path::Path;

/// Three commits. The second deletes `src/deleted.rs`, changes
/// `src/flip.rs` and `src/two.rs`, and adds `src/three.rs` with the blob
/// of `src/one.rs`. Before the second commit, `src/one.rs` and
/// `src/two.rs` hold one blob. The third commit puts back the tree of the
/// first.
pub struct Cases {
    directory: tempfile::TempDir,
    pub commits: [String; 3],
}

const NAMES: [&str; 3] = ["FIRST", "SECOND", "THIRD"];

/// The ranges of the cases, by the index of their commits.
pub const RANGES: [(usize, usize); 4] = [(0, 1), (1, 2), (0, 2), (2, 1)];

impl Cases {
    pub fn path(&self) -> &Path {
        self.directory.path()
    }

    pub fn normalized(&self, value: &Value) -> Value {
        let mut text = value.to_string();
        for (commit, name) in self.commits.iter().zip(NAMES) {
            text = text.replace(commit.as_str(), name);
        }
        serde_json::from_str(&text).unwrap()
    }

    pub fn range_name(&self, (base, head): (usize, usize)) -> String {
        format!(
            "{}_{}",
            NAMES[base].to_lowercase(),
            NAMES[head].to_lowercase()
        )
    }

    /// The stale `data` of a new CLI process for the range.
    pub fn cli_answer(&self, (base, head): (usize, usize)) -> Value {
        let (code, stdout, stderr) = cli_stale(
            self.path(),
            &[
                ("base", self.commits[base].as_str()),
                ("head", self.commits[head].as_str()),
            ],
        );
        assert_eq!(code, 0, "{stderr}");
        stdout["data"].clone()
    }

    /// The evidence `data` of a new CLI process for the Rule and range.
    pub fn cli_evidence(&self, rule: &str, (base, head): (usize, usize)) -> Value {
        let (code, stdout, stderr) = cli_api(
            self.path(),
            &format!("rules/{rule}/evidence"),
            &[
                ("base", self.commits[base].as_str()),
                ("head", self.commits[head].as_str()),
            ],
        );
        assert_eq!(code, 0, "{stderr}");
        stdout["data"].clone()
    }
}

pub fn cases() -> Cases {
    let directory = init_repo();
    let repo = directory.path();
    let twin = function("rule_twin_paths", "twin", "1");
    write(
        repo,
        "src/deleted.rs",
        function("rule_deleted", "deleted", "1"),
    );
    write(repo, "src/flip.rs", function("rule_flip", "flip", "1"));
    write(repo, "src/one.rs", &twin);
    write(repo, "src/two.rs", &twin);
    apply_rules(repo, &["rule_deleted", "rule_flip", "rule_twin_paths"]);
    let first = commit(repo, "first");
    std::fs::remove_file(repo.join("src/deleted.rs")).unwrap();
    write(repo, "src/flip.rs", function("rule_flip", "flip", "2"));
    write(repo, "src/two.rs", function("rule_twin_paths", "twin", "2"));
    write(repo, "src/three.rs", &twin);
    let second = commit(repo, "second");
    std::fs::remove_file(repo.join("src/three.rs")).unwrap();
    crate::support::git(repo, &["checkout", &first, "--", "."]);
    let third = commit(repo, "third");
    assert_eq!(
        crate::support::git(repo, &["rev-parse", &format!("{first}^{{tree}}")]),
        crate::support::git(repo, &["rev-parse", &format!("{third}^{{tree}}")]),
    );
    Cases {
        directory,
        commits: [first, second, third],
    }
}

/// Each range of the cases from a new CLI process gives its recorded
/// answer, and so does the evidence query with a base.
#[test]
#[verifies("rule_gate_reports_without_review", examples)]
fn each_case_gives_the_recorded_answer() {
    let cases = cases();
    let mut goldens = Goldens::default();
    for range in RANGES {
        goldens.check(
            &format!("cases_{}", cases.range_name(range)),
            &cases.normalized(&cases.cli_answer(range)),
        );
    }
    for (rule, range) in [("rule_flip", (0, 1)), ("rule_twin_paths", (1, 2))] {
        let evidence = cases.cli_evidence(rule, range);
        goldens.check(
            &format!("cases_evidence_{rule}_{}", cases.range_name(range)),
            &cases.normalized(&evidence["stale"]),
        );
    }
    goldens.finish();
}

/// One process that answers the ranges in a mixed order, so that a range
/// follows a range that held the same blobs or the same tree, gives the
/// answer of a new process each time.
#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_gate_reports_without_review", examples)]
async fn one_process_gives_the_answers_of_new_processes() {
    let cases = cases();
    let repository = Repository::open(cases.path(), "default").unwrap();
    let expected = RANGES.map(|range| cases.cli_answer(range));
    for index in [0, 1, 2, 3, 1, 0, 2, 0] {
        let (base, head) = RANGES[index];
        let answer = queries::stale(
            &repository,
            ReadPolicy::default(),
            StaleQuery {
                protocol_version: Some(SDK_PROTOCOL_VERSION),
                base: cases.commits[base].clone(),
                head: Some(cases.commits[head].clone()),
                rules: Vec::new(),
                limit: 50,
            },
        )
        .await
        .unwrap();
        let answer = serde_json::to_value(answer.result).unwrap();
        assert_eq!(answer["sites"], expected[index]["items"], "range {index}");
        assert_eq!(
            answer["summary"], expected[index]["summary"],
            "range {index}"
        );
        assert_eq!(answer["files_changed"], expected[index]["files_changed"]);
    }
}
