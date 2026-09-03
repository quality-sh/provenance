//! The relation map (gist 04, 39 rows by authoring act) as data: every row
//! is authored on a fresh repository, read back, and cleared where a clear
//! exists. A row whose act is derived is read back only; a row with no
//! authoring path is excluded by name with its reason in the data.

#[path = "relation_map/ideation_rows.rs"]
mod ideation_rows;
#[path = "relation_map/model.rs"]
mod model;
#[path = "relation_map/rows.rs"]
mod rows;
#[path = "relation_map/sdk_rows.rs"]
mod sdk_rows;

use assert_cmd::Command;
use model::{Act, Row, Runner};

fn all_rows() -> Vec<Row> {
    let mut rows = rows::graph_rows();
    rows.extend(rows::shaping_rows());
    rows.extend(rows::collaboration_rows());
    rows.extend(ideation_rows::ideation_rows());
    rows.extend(sdk_rows::sdk_rows());
    rows
}

/// The records every row acts on: one domain, two sources, three
/// requirements, two resolutions, one rule, one topic, one question, and
/// one boundary.
#[rustfmt::skip]
const SETUP: &[&[&str]] = &[
    &["domains", "create", "--id", "domain_payroll", "--name", "Payroll"],
    &["sources", "create", "--id", "source_award", "--name", "Award"],
    &["sources", "create", "--id", "source_award_2019", "--name", "Award 2019"],
    &["requirements", "create", "--id", "req_a", "--statement", "Overtime is paid", "--domain-id", "domain_payroll"],
    &["requirements", "create", "--id", "req_b", "--statement", "Rates are known"],
    &["requirements", "create", "--id", "req_c", "--statement", "Thresholds are set"],
    &["resolutions", "create", "--id", "res_a", "--title", "Threshold", "--requirement-id", "req_a",
        "--position", "Use the award", "--rationale", "It is the source"],
    &["resolutions", "create", "--id", "res_b", "--title", "Threshold again", "--requirement-id", "req_a",
        "--position", "Use the newer award", "--rationale", "It replaced the old one"],
    &["rules", "create", "--id", "rule_a", "--requirement-id", "req_a", "--statement", "Pay overtime after the threshold",
        "--source-document", "docs/award.md", "--source-section", "4.2"],
    &["topics", "create", "--id", "topic_t", "--requirement-id", "req_a", "--title", "Rates",
        "--links-json", r#"[{"target_type":"rule","target_id":"rule_a"}]"#],
    &["questions", "create", "--id", "question_q", "--topic-id", "topic_t", "--question", "Which threshold?", "--method", "grill"],
    &["boundaries", "create", "--id", "boundary_b", "--requirement-id", "req_a", "--statement", "No back pay",
        "--source-id", "source_award", "--source-clause", "4.3"],
];

fn setup(runner: &Runner) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            &runner.repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
            "--disposition-actor-id",
            "ben",
        ])
        .assert()
        .success();
    std::fs::write(
        std::path::Path::new(&runner.repo).join("share-links.ts"),
        "export function createShareLink() {}\n",
    )
    .unwrap();
    for command in SETUP {
        runner.cli(command);
    }
}

#[test]
fn the_thirty_nine_rows_are_named_once_each() {
    let mut numbers: Vec<u8> = all_rows().iter().map(|row| row.number).collect();
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers, (1..=39).collect::<Vec<u8>>());
    let excluded: Vec<u8> = all_rows()
        .iter()
        .filter(|row| matches!(row.act, Act::Excluded(_)))
        .map(|row| row.number)
        .collect();
    assert_eq!(excluded, [30]);
}

#[test]
fn every_row_authors_reads_back_and_clears_where_named() {
    let directory = tempfile::tempdir().unwrap();
    let mut runner = Runner::new(directory.path().to_str().unwrap().to_string());
    setup(&runner);
    for row in all_rows() {
        runner.run(&row);
    }
}
