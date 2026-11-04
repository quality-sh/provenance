//! A removed CLI command exits 1 and names the command that replaces it.

use predicates::prelude::*;
use provenance_macros::verifies;

mod support {
    pub mod review_paths;
}
use support::review_paths::{init, provenance};

/// The removed command, with arguments, and the command that replaces it.
const REMOVED: &[(&[&str], &str)] = &[
    (&["gaps"], "provenance check --gaps"),
    (&["health", "--scope", "default"], "provenance check --gaps"),
    (&["orphans", "--format", "json"], "provenance check --gaps"),
];

#[test]
#[verifies("rule_cli_removed_command_names_replacement", examples)]
fn a_removed_command_names_its_replacement() {
    let (_directory, repo) = init();
    for (args, replacement) in REMOVED {
        provenance()
            .args(*args)
            .args(["--repo", &repo])
            .assert()
            .code(1)
            .stdout(predicate::str::is_empty())
            .stderr(predicate::str::contains(format!(
                "`provenance {}` was removed. Use `{replacement}`",
                args[0]
            )));
    }
}
