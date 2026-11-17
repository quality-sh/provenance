//! A row over the record budget that a path with no write check stored, such
//! as an older build, a pull or a hand edit, does not block a write of another
//! record of its file. A write of that row lands only when the row fits.
use provenance_macros::verifies;
use serde_json::json;
use std::path::Path;

use crate::support::record_budget::{
    api, create_rule, patch, refusal, repository, rule_row_base, success, too_large, write,
};
use crate::support::state_files::state_files;

/// The bytes of the description that `store_long_rule` stores.
const LONG: usize = 70_000;

/// Creates the Rule `id`, then stores a description of `LONG` bytes in its
/// row of the state file, as a path with no write check can.
fn store_long_rule(repo: &Path, id: &str) {
    success(&create_rule(repo, id, "long row"));
    let shard = repo.join(".provenance/state/scopes/default/rules/rule.jsonl");
    let stored = std::fs::read_to_string(&shard).unwrap();
    let line = stored
        .lines()
        .find(|line| line.contains(&format!("\"id\":\"{id}\"")))
        .unwrap();
    let long = line.replace(
        "\"description\":\"long row\"",
        &format!("\"description\":\"{}\"", "x".repeat(LONG)),
    );
    assert_ne!(long, line);
    std::fs::write(&shard, stored.replace(line, &long)).unwrap();
}

/// The description of the Rule `id` from its member read.
fn description(repo: &Path, id: &str) -> String {
    let read = success(&api(repo, &format!("rules/{id}"), None));
    read["data"]["description"].as_str().unwrap().to_owned()
}

#[test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
fn an_update_of_another_rule_lands_next_to_one_long_row() {
    let directory = repository();
    let repo = directory.path();
    store_long_rule(repo, "rule_long");

    success(&write(
        repo,
        &["rule_one", "update"],
        &json!({"description": "The new rule of the data"}),
    ));

    assert_eq!(description(repo, "rule_one"), "The new rule of the data");
    assert_eq!(description(repo, "rule_long"), "x".repeat(LONG));
}

#[test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
fn updates_land_next_to_two_long_rows_and_one_long_row_can_be_made_short() {
    let directory = repository();
    let repo = directory.path();
    store_long_rule(repo, "rule_long");
    store_long_rule(repo, "rule_wide");

    success(&write(
        repo,
        &["rule_one", "update"],
        &json!({"description": "The new rule of the data"}),
    ));
    success(&patch(
        repo,
        "rules/rule_long",
        &json!({"description": "A short rule"}),
    ));

    assert_eq!(description(repo, "rule_one"), "The new rule of the data");
    assert_eq!(description(repo, "rule_long"), "A short rule");
    assert_eq!(description(repo, "rule_wide"), "x".repeat(LONG));
}

#[test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
fn an_update_of_a_long_row_that_stays_long_is_refused_and_names_the_field() {
    let directory = repository();
    let repo = directory.path();
    let base = rule_row_base(repo, "rule_long");
    store_long_rule(repo, "rule_long");
    let before = state_files(repo);

    let refused = refusal(&patch(
        repo,
        "rules/rule_long",
        &json!({"description": "y".repeat(LONG)}),
    ));
    let unchanged = state_files(repo);
    success(&patch(
        repo,
        "rules/rule_long",
        &json!({"description": "A short rule"}),
    ));

    assert_eq!(refused, too_large("rule_long", "description", base + LONG));
    assert_eq!(unchanged, before);
    assert_eq!(description(repo, "rule_long"), "A short rule");
}
