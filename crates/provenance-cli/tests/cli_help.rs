use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

#[test]
fn top_level_help_keeps_commands_from_each_cli_domain() {
    provenance().arg("--help").assert().success().stdout(
        contains("requirements")
            .and(contains("questions"))
            .and(contains("proposals"))
            .and(contains("docs"))
            .and(contains("sdk")),
    );
}

#[test]
fn nested_help_parses_commands_from_each_cli_domain() {
    for command in [
        &["requirements", "--help"][..],
        &["questions", "--help"][..],
        &["proposals", "--help"][..],
        &["docs", "--help"][..],
        &["sdk", "--help"][..],
    ] {
        provenance().args(command).assert().success();
    }
}

/// Neither list is required: emptying one is allowed, so the help text
/// must not claim a last entry is kept.
#[test]
fn optional_list_clear_help_does_not_claim_a_required_last_entry() {
    for command in [
        &["resolutions", "supersedes", "clear", "--help"][..],
        &["rules", "resolution", "clear", "--help"][..],
    ] {
        provenance()
            .args(command)
            .assert()
            .success()
            .stdout(contains("Remove one record from the list"))
            .stdout(contains("keeps its last requirement").not());
    }
}
