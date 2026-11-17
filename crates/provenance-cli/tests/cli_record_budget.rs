//! A record whose stored row passes the record budget is refused before it
//! writes, and a record at the budget lands and reads back through get,
//! trace, search and evidence.
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

use crate::support::record_budget::{
    api, cli, create_rule, refusal, repository, rule_row_base, success, too_large, write, BUDGET,
};
use crate::support::state_files::state_files;

#[test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
fn a_rule_with_a_long_description_is_refused_at_create_and_at_update() {
    let directory = repository();
    let repo = directory.path();
    let base = rule_row_base(repo, "rule_long");
    let description = "x".repeat(72_900);
    let before = state_files(repo);

    let created = refusal(&create_rule(repo, "rule_long", &description));
    let updated = refusal(&write(
        repo,
        &["rule_one", "update"],
        &json!({"description": description}),
    ));

    assert_eq!(
        created,
        too_large("rule_long", "description", base + 72_900)
    );
    // The id `rule_one` is one byte shorter than the id `rule_long`.
    assert_eq!(
        updated,
        too_large("rule_one", "description", base - 1 + 72_900)
    );
    assert_eq!(state_files(repo), before);
    success(&cli(repo, &["rule_one", "get"], &[("--format", "json")]));
    success(&api(repo, "rules/rule_one/evidence", None));
}

#[test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
fn a_requirement_with_a_long_description_is_refused() {
    let directory = repository();
    let repo = directory.path();
    let before = state_files(repo);

    let refused = refusal(&write(
        repo,
        &["req_long", "create", "--type", "requirement"],
        &json!({"statement": "The system keeps the data", "description": "x".repeat(70_000)}),
    ));

    let longer = refusal(&write(
        repo,
        &["req_long", "create", "--type", "requirement"],
        &json!({"statement": "The system keeps the data", "description": "x".repeat(70_001)}),
    ));

    let bytes = usize::try_from(refused["bytes"].as_u64().unwrap()).unwrap();
    assert_eq!(refused, too_large("req_long", "description", bytes));
    assert_eq!(longer, too_large("req_long", "description", bytes + 1));
    assert_eq!(state_files(repo), before);
}

#[test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
fn a_rule_at_the_budget_lands_and_reads_back_and_one_byte_more_is_refused() {
    let directory = repository();
    let repo = directory.path();
    let base = rule_row_base(repo, "rule_edge");
    let before = state_files(repo);

    let over = refusal(&create_rule(
        repo,
        "rule_edge",
        &"x".repeat(BUDGET + 1 - base),
    ));
    let unchanged = state_files(repo);
    let description = "x".repeat(BUDGET - base);
    success(&create_rule(repo, "rule_edge", &description));
    let get = success(&cli(repo, &["rule_edge", "get"], &[("--format", "json")]));
    let trace = success(&cli(repo, &["rule_edge", "trace"], &[("--format", "json")]));
    let search = success(&cli(
        repo,
        &["search"],
        &[("--text", "rule_edge"), ("--format", "json")],
    ));
    let evidence = success(&api(repo, "rules/rule_edge/evidence", None));

    assert_eq!(over, too_large("rule_edge", "description", BUDGET + 1));
    assert_eq!(unchanged, before);
    assert_eq!(get["record"]["value"]["description"], description.as_str());
    assert_eq!(
        trace["record"]["value"]["description"],
        description.as_str()
    );
    let found: Vec<&Value> = search["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["id"] == "rule_edge")
        .collect();
    assert_eq!(found.len(), 1, "{search}");
    assert_eq!(found[0]["description"], description.as_str());
    assert_eq!(evidence["data"]["rule_id"], "rule_edge", "{evidence}");
}
