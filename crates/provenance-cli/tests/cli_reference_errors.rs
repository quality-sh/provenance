//! The refusal text of the relation commands: it names the id that was
//! not found and the flag that named it, and a clear names the relation
//! it searched.

use assert_cmd::Command;
use predicates::str::contains;
use std::path::Path;

fn provenance(repo: &Path, args: &[&str]) -> Command {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command
        .args(args)
        .args(["--repo", repo.to_str().unwrap(), "--scope", "default"]);
    command
}

fn init(repo: &Path) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
}

fn requirement(repo: &Path, id: &str) {
    provenance(
        repo,
        &[
            "requirements",
            "create",
            "--id",
            id,
            "--statement",
            "The refund window is counted in days",
        ],
    )
    .assert()
    .success();
}

/// One record of every kind a relation command can address beside a
/// requirement, all named after the refund.
fn seed(repo: &Path) {
    provenance(
        repo,
        &[
            "sources",
            "create",
            "--id",
            "source_policy",
            "--name",
            "Refund policy",
        ],
    )
    .assert()
    .success();
    provenance(
        repo,
        &[
            "resolutions",
            "create",
            "--id",
            "res_refund",
            "--title",
            "Refund",
            "--requirement-id",
            "req_refund_amount",
            "--position",
            "Refund in full",
            "--rationale",
            "The policy says so",
        ],
    )
    .assert()
    .success();
    provenance(
        repo,
        &[
            "rules",
            "create",
            "--id",
            "rule_refund",
            "--requirement-id",
            "req_refund_amount",
            "--statement",
            "Refunds are paid within 30 days",
            "--source-document",
            "docs/refunds.md",
            "--source-section",
            "4.2",
        ],
    )
    .assert()
    .success();
    provenance(
        repo,
        &[
            "topics",
            "create",
            "--id",
            "topic_refund",
            "--requirement-id",
            "req_refund_amount",
            "--title",
            "Refunds",
        ],
    )
    .assert()
    .success();
    provenance(
        repo,
        &[
            "questions",
            "create",
            "--id",
            "question_refund",
            "--topic-id",
            "topic_refund",
            "--question",
            "Which window?",
            "--method",
            "grill",
        ],
    )
    .assert()
    .success();
}

#[test]
fn a_refusal_names_the_id_and_the_flag_it_came_from() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    init(repo);
    requirement(repo, "req_refund_amount");

    provenance(
        repo,
        &[
            "requirements",
            "supersedes",
            "add",
            "--requirement-id",
            "req_ghost",
            "--target-id",
            "req_refund_amount",
        ],
    )
    .assert()
    .failure()
    .stderr(contains(
        "requirement req_ghost does not exist (--requirement-id)",
    ));

    provenance(
        repo,
        &[
            "requirements",
            "supersedes",
            "add",
            "--requirement-id",
            "req_refund_amount",
            "--target-id",
            "req_nope",
        ],
    )
    .assert()
    .failure()
    .stderr(contains(
        "requirement req_nope does not exist (--target-id)",
    ));
}

#[test]
fn every_relation_family_names_its_owner_flag() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    init(repo);
    requirement(repo, "req_refund_amount");
    seed(repo);
    let families: &[(&[&str], &str)] = &[
        (
            &[
                "rules",
                "resolution",
                "add",
                "--rule-id",
                "rule_ghost",
                "--target-id",
                "res_refund",
            ],
            "rule rule_ghost does not exist (--rule-id)",
        ),
        (
            &[
                "resolutions",
                "supersedes",
                "add",
                "--resolution-id",
                "res_ghost",
                "--target-id",
                "res_refund",
            ],
            "resolution res_ghost does not exist (--resolution-id)",
        ),
        (
            &[
                "sources",
                "supersedes",
                "add",
                "--source-id",
                "source_ghost",
                "--target-id",
                "source_policy",
            ],
            "source source_ghost does not exist (--source-id)",
        ),
        (
            &[
                "questions",
                "contradicts",
                "set",
                "--id",
                "question_ghost",
                "--target-id",
                "req_refund_amount",
            ],
            "question question_ghost does not exist (--id)",
        ),
    ];

    for (args, expected) in families {
        provenance(repo, args)
            .assert()
            .failure()
            .stderr(contains(*expected));
    }
}

#[test]
fn a_clear_refusal_names_the_relation_it_searched() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    init(repo);
    requirement(repo, "req_refund_amount");
    requirement(repo, "req_refund_window");
    provenance(
        repo,
        &[
            "requirements",
            "depends-on",
            "add",
            "--requirement-id",
            "req_refund_amount",
            "--target-id",
            "req_refund_window",
        ],
    )
    .assert()
    .success();

    provenance(
        repo,
        &[
            "requirements",
            "supersedes",
            "clear",
            "--requirement-id",
            "req_refund_amount",
            "--target-id",
            "req_refund_window",
        ],
    )
    .assert()
    .failure()
    .stderr(contains(
        "requirement req_refund_amount does not name requirement req_refund_window under supersedes",
    ));

    provenance(
        repo,
        &[
            "requirements",
            "supersedes",
            "add",
            "--requirement-id",
            "req_refund_amount",
            "--target-id",
            "req_refund_window",
        ],
    )
    .assert()
    .success();
    provenance(
        repo,
        &[
            "requirements",
            "supersedes",
            "clear",
            "--requirement-id",
            "req_refund_amount",
            "--target-id",
            "req_refund_window",
        ],
    )
    .assert()
    .success();
}
