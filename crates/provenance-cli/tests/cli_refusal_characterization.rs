//! Characterization: the exit code and the standard error lines of each
//! refused record command on the base commit, in text and in JSON format.
//! Later changes to the store refusals keep these rows unchanged.

use serde_json::{json, Value};

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, json_output, provenance};

const REQUIREMENTS: &str = ".provenance/state/scopes/default/requirements/req.jsonl";

/// The exit code and the standard error lines, with the repository path
/// replaced by `<repo>`. Text format is the default, so it adds no flag.
fn answer(repo: &str, args: &[&str], format: &str) -> Value {
    let mut command = provenance();
    command.args(args).args(["--repo", repo]);
    if format == "json" {
        command.args(["--format", "json"]);
    }
    let output = command.output().unwrap();
    let stderr = String::from_utf8(output.stderr)
        .unwrap()
        .replace(repo, "<repo>");
    json!({
        "command": args[..2].join(" "),
        "format": format,
        "code": output.status.code(),
        "stdout_empty": output.stdout.is_empty(),
        "stderr": stderr.lines().collect::<Vec<_>>(),
    })
}

fn both(rows: &mut Vec<Value>, repo: &str, args: &[&str]) {
    for format in ["text", "json"] {
        rows.push(answer(repo, args, format));
    }
}

fn etag(repo: &str, id: &str) -> String {
    let read =
        json_output(provenance().args(["api", &format!("requirements/{id}"), "--repo", repo]));
    read["data"]["edit"]["etag"].as_str().unwrap().to_owned()
}

fn assert_rows(rows: &[Value], expected: &Value) {
    assert_eq!(
        &Value::Array(rows.to_vec()),
        expected,
        "actual rows:\n{}",
        serde_json::to_string_pretty(rows).unwrap()
    );
}

#[test]
fn each_refused_record_command_keeps_its_exit_code_and_lines() {
    let (_directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    let mut rows = Vec::new();
    let statement = "The system keeps review feedback available.";
    both(
        &mut rows,
        &repo,
        &[
            "req_a",
            "create",
            "--type",
            "requirement",
            "--statement",
            statement,
        ],
    );
    both(&mut rows, &repo, &["req_missing", "get"]);
    both(
        &mut rows,
        &repo,
        &[
            "req_b",
            "create",
            "--type",
            "requirement",
            "--statement",
            statement,
            "--depends-on-json",
            r#"["req_missing"]"#,
        ],
    );
    assert_rows(
        &rows,
        &json!([
            {
                "code": 1,
                "command": "req_a create",
                "format": "text",
                "stderr": [
                    "Error: {\"error\":{\"kind\":\"already_exists\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            },
            {
                "code": 1,
                "command": "req_a create",
                "format": "json",
                "stderr": [
                    "{\"error\":{\"kind\":\"already_exists\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            },
            {
                "code": 1,
                "command": "req_missing get",
                "format": "text",
                "stderr": [
                    "Error: record does not exist"
                ],
                "stdout_empty": true
            },
            {
                "code": 1,
                "command": "req_missing get",
                "format": "json",
                "stderr": [
                    "{\"error\":{\"kind\":\"resource_not_found\",\"message\":\"record does not exist\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            },
            {
                "code": 1,
                "command": "req_b create",
                "format": "text",
                "stderr": [
                    "Error: {\"error\":{\"kind\":\"missing_reference\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            },
            {
                "code": 1,
                "command": "req_b create",
                "format": "json",
                "stderr": [
                    "{\"error\":{\"kind\":\"missing_reference\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            }
        ]),
    );
}

/// memu: a new link to an archived Requirement is `invalid_update` with the reason.
#[test]
fn a_new_link_to_an_archived_parent_keeps_its_reason() {
    let (_directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_old");
    create_requirement(&repo, "req_child");
    let guard = etag(&repo, "req_old");
    json_output(provenance().args([
        "req_old",
        "update",
        "--if-match",
        &guard,
        "--repo",
        &repo,
        "--actor",
        "author",
        "--status",
        "archived",
        "--archived-in-commit-json",
        r#"{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        "--format",
        "json",
    ]));
    let guard = etag(&repo, "req_child");
    let mut rows = Vec::new();
    both(
        &mut rows,
        &repo,
        &[
            "req_child",
            "update",
            "--if-match",
            &guard,
            "--actor",
            "author",
            "--relationships-json",
            r#"{"refines":["req_old"]}"#,
        ],
    );
    assert_rows(
        &rows,
        &json!([
            {
                "code": 1,
                "command": "req_child update",
                "format": "text",
                "stderr": [
                    "Error: {\"error\":{\"kind\":\"invalid_update\",\"message\":\"Requirement req_old is archived; a new link cannot point to it\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            },
            {
                "code": 1,
                "command": "req_child update",
                "format": "json",
                "stderr": [
                    "{\"error\":{\"kind\":\"invalid_update\",\"message\":\"Requirement req_old is archived; a new link cannot point to it\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            }
        ]),
    );
}

/// A line that is not a record in the Requirement shard.
#[test]
fn a_bad_state_line_keeps_its_exit_code_and_lines() {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    let path = directory.path().join(REQUIREMENTS);
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("{\"not\":\"a record\"}\n");
    std::fs::write(&path, text).unwrap();
    let mut rows = Vec::new();
    both(&mut rows, &repo, &["req_a", "get"]);
    both(
        &mut rows,
        &repo,
        &[
            "req_b",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The system keeps review feedback available.",
        ],
    );
    assert_rows(
        &rows,
        &json!([
            {
                "code": 1,
                "command": "req_a get",
                "format": "text",
                "stderr": [
                    "Error: {\"error\":{\"file\":\"scopes/default/requirements/req.jsonl\",\"kind\":\"invalid_state_file\",\"line\":2,\"message\":\"missing field `scope_id`\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            },
            {
                "code": 1,
                "command": "req_a get",
                "format": "json",
                "stderr": [
                    "{\"error\":{\"file\":\"scopes/default/requirements/req.jsonl\",\"kind\":\"invalid_state_file\",\"line\":2,\"message\":\"missing field `scope_id`\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            },
            {
                "code": 1,
                "command": "req_b create",
                "format": "text",
                "stderr": [
                    "Error: {\"error\":{\"file\":\"scopes/default/requirements/req.jsonl\",\"kind\":\"invalid_state_file\",\"line\":2,\"message\":\"missing field `scope_id`\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            },
            {
                "code": 1,
                "command": "req_b create",
                "format": "json",
                "stderr": [
                    "{\"error\":{\"file\":\"scopes/default/requirements/req.jsonl\",\"kind\":\"invalid_state_file\",\"line\":2,\"message\":\"missing field `scope_id`\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            }
        ]),
    );
}

/// e92s: `api` create with `source_refs` names the field, the accepted fields
/// and how to cite a Source.
#[test]
#[provenance_macros::verifies("rule_unknown_request_field_names_accepted_fields", examples)]
fn api_create_with_source_refs_names_the_unknown_field() {
    let (_directory, repo) = support::review_paths::init();
    let mut rows = Vec::new();
    for format in ["text", "json"] {
        let mut command = provenance();
        command.args([
            "api",
            "requirements",
            "--method",
            "post",
            "--input",
            "-",
            "--repo",
            &repo,
        ]);
        if format == "json" {
            command.args(["--format", "json"]);
        }
        let output = command
            .write_stdin(
                json!({"actor":"a", "id":"req_b", "statement":"The system stores each record.",
                    "status":"discovery", "depends_on":[], "supersedes":[], "source_refs":[]})
                .to_string(),
            )
            .output()
            .unwrap();
        rows.push(json!({
            "command": "api requirements",
            "format": format,
            "code": output.status.code(),
            "stdout_empty": output.stdout.is_empty(),
            "stderr": String::from_utf8(output.stderr).unwrap().lines().collect::<Vec<_>>(),
        }));
    }
    assert_rows(
        &rows,
        &json!([
            {
                "code": 1,
                "command": "api requirements",
                "format": "text",
                "stderr": [
                    "Error: {\"error\":{\"field\":\"request.source_refs\",\"kind\":\"invalid_input\",\"message\":\"create-requirement does not take source_refs; accepted fields: `actor`, `id`, `statement`, `description`, `status`, `refines`, `depends_on`, `supersedes`, `spawned_by`, `origin_thread`, `origin_message`, `origin`; to cite a Source, update the record with the Source in relationships.cites\",\"reason\":\"unknown_field\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            },
            {
                "code": 1,
                "command": "api requirements",
                "format": "json",
                "stderr": [
                    "Error: {\"error\":{\"field\":\"request.source_refs\",\"kind\":\"invalid_input\",\"message\":\"create-requirement does not take source_refs; accepted fields: `actor`, `id`, `statement`, `description`, `status`, `refines`, `depends_on`, `supersedes`, `spawned_by`, `origin_thread`, `origin_message`, `origin`; to cite a Source, update the record with the Source in relationships.cites\",\"reason\":\"unknown_field\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            }
        ]),
    );
}

/// 8yzq: `get` on a Withdrawal that names no review submission gives the
/// file and the line of the Withdrawal.
#[test]
#[provenance_macros::verifies("rule_inconsistent_records_refusal_names_records", examples)]
fn get_names_the_line_of_a_withdrawal_that_names_no_submission() {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    let path = directory
        .path()
        .join(".provenance/state/scopes/default/ideation/withdrawals.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let withdrawal = json!({"scope_id":"default","id":"withdrawal_lost",
        "proposal_id":"proposal_missing","actor":"reviewer","reason":null});
    std::fs::write(&path, format!("{withdrawal}\n")).unwrap();
    assert_rows(
        &[answer(&repo, &["req_a", "get"], "json")],
        &json!([{
            "code": 1,
            "command": "req_a get",
            "format": "json",
            "stderr": [
                "{\"error\":{\"file\":\"scopes/default/ideation/withdrawals.jsonl\",\"kind\":\"invalid_state_file\",\"line\":1,\"message\":\"withdrawal withdrawal_lost names no review submission: proposal_missing\"},\"meta\":{}}"
            ],
            "stdout_empty": true
        }]),
    );
}
