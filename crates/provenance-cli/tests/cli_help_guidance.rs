//! Implementation-aid tests, without a Rule binding. `rule_create_help_accepted_fields`
//! states which fields a help lists, and no active Rule states that each root command
//! has a description or that the help of one record type names the update that cites a
//! Source (beads provenance-8h5u and provenance-9v5u). The full text of the root, create
//! and update help is in the `surface_names` pins.

use crate::support;
use support::help_text::help;

#[test]
fn root_help_gives_each_command_a_description() {
    let text = help(&["--help"]);
    let rows = text
        .lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    assert!(rows.len() > 10, "{text}");
    let bare = rows
        .iter()
        .filter(|row| row.split_whitespace().count() < 2)
        .collect::<Vec<_>>();
    assert!(
        bare.is_empty(),
        "rows with no description: {bare:?}\n{text}"
    );
}

#[test]
fn requirement_create_help_says_to_cite_a_source_with_update() {
    let text = help(&["record_x", "create", "--type", "requirement", "--help"]);
    assert!(
        text.contains("update the Requirement after create"),
        "{text}"
    );
}
