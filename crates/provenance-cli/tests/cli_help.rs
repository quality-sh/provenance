use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

#[test]
fn top_level_help_keeps_product_commands_and_retires_the_sdk_dispatcher() {
    provenance().arg("--help").assert().success().stdout(
        contains("docs")
            .and(contains("graph"))
            .and(contains("traceability"))
            .and(contains("coverage"))
            .and(contains("review"))
            .and(contains("sdk").not()),
    );
}

#[test]
fn product_and_catalog_help_are_available() {
    provenance().args(["docs", "--help"]).assert().success();
    provenance()
        .args(["requirements", "--help"])
        .assert()
        .success()
        .stdout(contains("requirements list"));
}
