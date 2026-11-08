use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use provenance_macros::verifies;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

#[test]
fn top_level_help_keeps_product_commands_and_retires_the_sdk_dispatcher() {
    provenance().arg("--help").assert().success().stdout(
        contains("graph-reference")
            .not()
            .and(contains("\n  docs ").not())
            .and(contains("provenance <id> trace"))
            .and(contains("coverage"))
            .and(contains("review"))
            .and(contains("sdk").not()),
    );
}

#[test]
fn root_help_names_api_and_no_collection_command() {
    provenance().arg("--help").assert().success().stdout(
        contains("provenance api [PATH]")
            .and(contains("provenance api --filter <text>"))
            .and(contains("<collection>").not())
            .and(contains("requirements list").not()),
    );
}

/// `provenance help [command]` prints the same help as `--help` for the root, a word
/// command, and a target action.
#[test]
#[verifies("rule_cli_help_command_shows_help", examples)]
fn the_help_command_prints_the_help_of_its_command() {
    let help = |args: &[&str]| {
        let output = provenance().args(args).output().unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap()
    };
    assert_eq!(help(&["help"]), help(&["--help"]));
    assert_eq!(help(&["help", "check"]), help(&["check", "--help"]));
    assert_eq!(
        help(&["help", "req_a", "trace"]),
        help(&["req_a", "trace", "--help"])
    );
}

/// The root help does not list merge-jsonl, which Git runs as a merge driver.
#[test]
#[verifies("rule_cli_help_hides_program_commands", examples)]
fn root_help_does_not_list_merge_jsonl() {
    provenance()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("merge-jsonl").not());
    provenance()
        .args(["merge-jsonl", "--help"])
        .assert()
        .success();
}
