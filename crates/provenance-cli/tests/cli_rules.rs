//! `rules list` and `rules show`: the read verbs beside `rules create`.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

/// A repository holding two rules: one citing a source document and section,
/// one citing neither and carrying a statement long enough to be cut.
fn repo_with_rules() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().to_str().unwrap().to_string();

    provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "req_overtime",
            "--statement",
            "Overtime is paid",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "rule_overtime",
            "--requirement-id",
            "req_overtime",
            "--statement",
            "Pay overtime after the threshold",
            "--severity",
            "high",
            "--source-document",
            "docs/awards/schads.md",
            "--source-section",
            "Clause 4.2",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "rule_long_statement",
            "--requirement-id",
            "req_overtime",
            "--statement",
            &"w".repeat(140),
            "--status",
            "draft",
            "--severity",
            "low",
        ])
        .assert()
        .success();
    tmp
}

#[test]
fn rules_list_shows_every_rule_with_its_status_severity_and_source_binding() {
    let tmp = repo_with_rules();
    let repo = tmp.path().to_str().unwrap();

    provenance()
        .args([
            "rules", "list", "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(
            contains("rule_overtime")
                .and(contains("rule_long_statement"))
                .and(contains("\"status\": \"active\""))
                .and(contains("\"status\": \"draft\""))
                .and(contains("\"severity\": \"high\""))
                .and(contains("\"severity\": \"low\""))
                .and(contains("Pay overtime after the threshold"))
                .and(contains("docs/awards/schads.md"))
                .and(contains("Clause 4.2")),
        );
}

#[test]
fn rules_list_cuts_a_long_statement() {
    let tmp = repo_with_rules();
    let repo = tmp.path().to_str().unwrap();

    provenance()
        .args([
            "rules", "list", "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(contains("w".repeat(100) + "...").and(contains("w".repeat(140)).not()));
}

#[test]
fn rules_show_prints_the_whole_record() {
    let tmp = repo_with_rules();
    let repo = tmp.path().to_str().unwrap();

    provenance()
        .args([
            "rules",
            "show",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            "rule_long_statement",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(
            contains("w".repeat(140))
                .and(contains("schema_version"))
                .and(contains("rule_overtime").not()),
        );
}

#[test]
fn rules_show_names_the_rule_it_could_not_find() {
    let tmp = repo_with_rules();
    let repo = tmp.path().to_str().unwrap();

    provenance()
        .args([
            "rules",
            "show",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            "rule_absent",
        ])
        .assert()
        .failure()
        .stderr(contains("rule_absent").and(contains("not found")));
}

/// The read verbs land on the `default` scope like `wiki build` and
/// `coverage scan` do, so the common case takes no flag.
#[test]
fn the_read_verbs_fall_back_to_the_default_scope() {
    let tmp = repo_with_rules();
    let repo = tmp.path().to_str().unwrap();

    provenance()
        .args(["rules", "list", "--repo", repo, "--format", "json"])
        .assert()
        .success()
        .stdout(contains("rule_overtime"));
    provenance()
        .args([
            "rules",
            "show",
            "--repo",
            repo,
            "--id",
            "rule_overtime",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(contains("Pay overtime after the threshold"));
}

/// Every flag on all three verbs carries help text. A flag a reader has to
/// guess at is a flag they get wrong.
#[test]
fn every_rules_flag_is_described_in_help() {
    for verb in ["create", "list", "show"] {
        let help = provenance()
            .args(["rules", verb, "--help"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let help = String::from_utf8(help).unwrap();

        let lines = help.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            let Some(rest) = line.trim_start().strip_prefix("--") else {
                continue;
            };
            // clap prints `--flag <VALUE>  Description`, or puts the
            // description on the line below when the flag is wide. Either way
            // an undescribed flag is a flag with nothing after its value.
            let described = rest.split_whitespace().count() > 2
                || lines.get(index + 1).is_some_and(|next| {
                    let next = next.trim();
                    !next.is_empty() && !next.starts_with('-')
                });
            assert!(
                described,
                "undescribed flag in `rules {verb} --help`: {line}"
            );
        }
    }
}
