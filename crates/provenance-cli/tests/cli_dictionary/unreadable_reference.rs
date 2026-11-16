//! The answers of the dictionary commands when the dictionary reference path
//! holds an entry that the process cannot read as a file: a directory, a file
//! with no read permission, or a link with no target.
//!
//! Such a reference is a reference that cannot load. The statement check, a
//! statement write, MCP `check` and `dictionary status` warn that Rule 1.1 did
//! not run, as `rule_ste_dictionary_index_missing_warning` states. The strict
//! check fails, as `rule_ste_strict_dictionary_index_gate` states. The problem
//! gives the file below the state directory, the system error and the repair.
//! Only a path with no entry is no reference.

use super::support::{
    create_requirement, unloaded_check, unloaded_status, unloaded_strict_check, ReferenceProject,
};
use provenance_macros::verifies;
use provenance_mcp::Client;
use serde_json::{json, Value};
use std::io::ErrorKind;
use std::process::Stdio;

/// The repair that each answer gives for a reference that cannot be read.
const UNREADABLE_REPAIR: &str = "make the path a file that this user can read, or get the file \
     again from git, and then do the command again";

/// A statement that breaks Rule 8.1, which needs no dictionary: it has a semicolon.
const REFUSED_STATEMENT: &str = "The pump stops; the valve opens.";

/// The kind of the error of a read of a directory: Unix systems refuse it
/// as a directory, and Windows refuses access to it.
#[cfg(unix)]
const DIRECTORY_READ: ErrorKind = ErrorKind::IsADirectory;
#[cfg(windows)]
const DIRECTORY_READ: ErrorKind = ErrorKind::PermissionDenied;

/// The problem of a reference whose read fails with `system`.
fn problem(system: &std::io::Error) -> String {
    format!("dictionary.json: cannot read the file: {system}; {UNREADABLE_REPAIR}")
}

/// A project with a directory at the reference path, and the problem of its
/// reference.
fn directory_reference() -> (ReferenceProject, String) {
    let project = ReferenceProject::empty();
    std::fs::create_dir(project.reference()).unwrap();
    let system = std::fs::read(project.reference()).unwrap_err();
    assert_eq!(system.kind(), DIRECTORY_READ);
    (project, problem(&system))
}

/// A project whose reference file has no read permission, and the problem
/// of its reference. A process of the root user reads the file, so the
/// test that calls it with its `name` stops with a `SKIPPED` line.
#[cfg(unix)]
fn reference_with_no_read_permission(name: &str) -> Option<(ReferenceProject, String)> {
    use std::os::unix::fs::PermissionsExt;
    let project = ReferenceProject::with_reference("{}");
    let mode = std::fs::Permissions::from_mode(0o000);
    std::fs::set_permissions(project.reference(), mode).unwrap();
    let Err(system) = std::fs::read(project.reference()) else {
        eprintln!("SKIPPED {name}: this user reads a file with mode 000");
        return None;
    };
    assert_eq!(system.kind(), ErrorKind::PermissionDenied);
    Some((project, problem(&system)))
}

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_statement_check_with_a_directory_at_the_reference_path_warns_that_rule_1_1_did_not_run() {
    let (project, problem) = directory_reference();

    assert_eq!(project.statement_check(&[]), unloaded_check(&problem));
}

#[test]
#[verifies("rule_ste_strict_dictionary_index_gate", examples)]
fn a_strict_statement_check_with_a_directory_at_the_reference_path_fails() {
    let (project, problem) = directory_reference();

    assert_eq!(
        project.statement_check(&["--strict"]),
        unloaded_strict_check(&problem)
    );
}

#[cfg(unix)]
#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_statement_check_with_no_read_permission_on_the_reference_warns_that_rule_1_1_did_not_run() {
    let name = "a_statement_check_with_no_read_permission_on_the_reference_warns_that_rule_1_1_did_not_run";
    let Some((project, problem)) = reference_with_no_read_permission(name) else {
        return;
    };

    assert_eq!(project.statement_check(&[]), unloaded_check(&problem));
}

#[cfg(unix)]
#[test]
#[verifies("rule_ste_strict_dictionary_index_gate", examples)]
fn a_strict_statement_check_with_no_read_permission_on_the_reference_fails() {
    let name = "a_strict_statement_check_with_no_read_permission_on_the_reference_fails";
    let Some((project, problem)) = reference_with_no_read_permission(name) else {
        return;
    };

    assert_eq!(
        project.statement_check(&["--strict"]),
        unloaded_strict_check(&problem)
    );
}

/// A link with no target gives "not found" to a read, but the path holds the
/// link, so the reference is there and cannot be read.
#[cfg(unix)]
#[test]
#[verifies("rule_ste_strict_dictionary_index_gate", examples)]
fn a_strict_statement_check_with_a_link_with_no_target_at_the_reference_path_fails() {
    let project = ReferenceProject::empty();
    std::os::unix::fs::symlink("absent.json", project.reference()).unwrap();
    let system = std::fs::read(project.reference()).unwrap_err();
    assert_eq!(system.kind(), ErrorKind::NotFound);

    assert_eq!(
        project.statement_check(&["--strict"]),
        unloaded_strict_check(&problem(&system))
    );
}

/// The strict check of a path with no entry passes: the project uses no
/// dictionary. `missing_index::a_project_without_a_reference_gets_no_warning`
/// gives the check that is not strict and the write.
#[test]
#[verifies("rule_ste_strict_dictionary_index_gate", examples)]
fn a_strict_statement_check_with_no_entry_at_the_reference_path_passes() {
    let project = ReferenceProject::empty();
    let passed = json!({"categories": [{
        "category": "statements",
        "status": "passed",
        "findings": [],
    }]});

    assert_eq!(
        project.statement_check(&["--strict"]),
        (Some(0), passed, String::new())
    );
}

/// Implementation aid with no Rule: no Rule states the `load_problem` of the
/// dictionary status. The test records the answer with its repair step.
#[test]
fn the_status_of_a_directory_at_the_reference_path_names_the_read_error() {
    let (project, problem) = directory_reference();

    assert_eq!(project.status(), unloaded_status(&project, &problem));
}

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_statement_write_with_a_directory_at_the_reference_path_warns_that_rule_1_1_did_not_run() {
    let (project, problem) = directory_reference();
    let warning = format!("ASD-STE100 Rule 1.1 did not run: {problem}");

    let text = project.run(&[
        "req_text",
        "create",
        "--type",
        "requirement",
        "--repo",
        project.repo(),
        "--scope",
        "default",
        "--statement",
        "The pump stops.",
    ]);
    let refused = create_requirement(
        &project.repo,
        &project.index_directory,
        "req_refused",
        REFUSED_STATEMENT,
    );

    assert_eq!(
        (text.status.code(), String::from_utf8_lossy(&text.stderr)),
        (Some(0), format!("warning: {warning}\n").into())
    );
    let refusal: Value = serde_json::from_slice(&refused.stderr).unwrap();
    assert_eq!(
        (
            refused.status.code(),
            &refusal["error"]["kind"],
            &refusal["meta"]["warnings"]
        ),
        (Some(1), &json!("statement_invalid"), &json!([warning]))
    );
}

/// The `check` answer of the MCP server of `project` for `arguments`: the
/// structured content and the text.
async fn mcp_check(project: &ReferenceProject, arguments: Value) -> (Value, Value) {
    let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .env("PROVENANCE_STE100_INDEX_DIR", &project.index_directory)
        .args(["mcp", "--repo", project.repo()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let client = Client::connect(tokio::io::join(stdout, stdin))
        .await
        .unwrap();
    let answer = client.call("check", arguments).await;
    client.cancel().await.unwrap();
    assert!(child.wait().await.unwrap().success());
    assert_ne!(answer["isError"], json!(true), "{answer}");
    (
        answer["structuredContent"].clone(),
        answer["content"][0]["text"].clone(),
    )
}

#[tokio::test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
async fn an_mcp_check_with_a_directory_at_the_reference_path_warns_and_its_strict_check_is_unavailable(
) {
    let (project, problem) = directory_reference();
    let statement = "The pump stops.";

    let warned = mcp_check(&project, json!({"statement": statement})).await;
    let strict = mcp_check(&project, json!({"statement": statement, "strict": true})).await;

    let warning = format!("ASD-STE100 Rule 1.1 did not run: {problem}");
    let warned_check = json!({"categories": [{
        "category": "statements",
        "status": "findings",
        "findings": [{"message": warning}],
    }]});
    let strict_check = json!({"categories": [{
        "category": "statements",
        "status": "unavailable",
        "findings": [],
        "unavailable_reason": problem,
    }]});
    assert_eq!(
        [warned, strict],
        [
            (
                warned_check,
                json!(format!("statements: findings\n  - {warning}"))
            ),
            (
                strict_check,
                json!(format!("statements: unavailable\n  - {problem}"))
            ),
        ]
    );
}
