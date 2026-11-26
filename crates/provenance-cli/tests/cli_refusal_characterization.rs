//! The exit code and the standard error lines of each refused record command,
//! in text and in JSON format. A test with no Rule is a characterization: its
//! rows pin the kind, the text and the exit code of each refusal.

use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use serde_json::{json, Value};

use crate::support;
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

/// A new link to an archived Requirement is `invalid_update` with the reason.
#[test]
fn a_new_link_to_an_archived_parent_keeps_its_reason() {
    let (_directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_old");
    create_requirement(&repo, "req_child");
    // The archive is a status request: the reviewer accepts it.
    support::record_review::archive_requirement(
        &repo,
        "req_old",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );
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

/// The exit code, the standard output and the standard error of a get of
/// `req_a` with `flags`.
fn get(repo: &str, flags: &[&str]) -> (Option<i32>, String, String) {
    let output = provenance()
        .args(["req_a", "get", "--repo", repo])
        .args(flags)
        .output()
        .unwrap();
    let text = |bytes: Vec<u8>| String::from_utf8(bytes).unwrap();
    (
        output.status.code(),
        text(output.stdout),
        text(output.stderr),
    )
}

/// A line that is not a record in the Requirement shard. A get gives the
/// record at the stored serial with the freshness warning, and a create
/// refuses with the line.
#[test]
fn a_bad_state_line_gives_the_stored_record_and_refuses_a_create() {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    let json = ["--format", "json"];
    let (_, text, _) = get(&repo, &[]);
    let (_, record, _) = get(&repo, &json);
    let path = directory.path().join(REQUIREMENTS);
    let mut state = std::fs::read_to_string(&path).unwrap();
    state.push_str("{\"not\":\"a record\"}\n");
    std::fs::write(&path, state).unwrap();

    let warning = format!(
        "scopes/default/requirements/req.jsonl line 2: missing field `scope_id`; \
         {STATE_CORRECTIONS}"
    );
    let text = format!("{text}\nwarning: record freshness: {warning}\n");
    assert_eq!(get(&repo, &[]), (Some(0), text, String::new()));
    let mut expected: Value = serde_json::from_str(&record).unwrap();
    expected["record_metadata"]["freshness_error"] = json!(warning);
    expected["record_metadata"]["stamp"]["policy"] = json!("catch_up_failed");
    let (code, record, stderr) = get(&repo, &json);
    let record: Value = serde_json::from_str(&record).unwrap();
    assert_eq!((code, record, stderr), (Some(0), expected, String::new()));

    let mut rows = Vec::new();
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
                "command": "req_b create",
                "format": "text",
                "stderr": [
                    refusal_line(
                        "Error: ",
                        "scopes/default/requirements/req.jsonl",
                        2,
                        "missing field `scope_id`",
                        STATE_CORRECTIONS,
                    )
                ],
                "stdout_empty": true
            },
            {
                "code": 1,
                "command": "req_b create",
                "format": "json",
                "stderr": [
                    refusal_line(
                        "",
                        "scopes/default/requirements/req.jsonl",
                        2,
                        "missing field `scope_id`",
                        STATE_CORRECTIONS,
                    )
                ],
                "stdout_empty": true
            }
        ]),
    );
}

/// `api` create with `source_refs` names the field, the accepted fields
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
                    "{\"error\":{\"field\":\"request.source_refs\",\"kind\":\"invalid_input\",\"message\":\"create-requirement does not take source_refs; accepted fields: `actor`, `id`, `statement`, `description`, `status`, `refines`, `depends_on`, `supersedes`, `spawned_by`, `origin_thread`, `origin_message`, `origin`; to cite a Source, update the record with the Source in relationships.cites\",\"reason\":\"unknown_field\"},\"meta\":{}}"
                ],
                "stdout_empty": true
            }
        ]),
    );
}

/// The `api` read of the record of a Withdrawal that names no review
/// submission gives the file and the line of the Withdrawal.
#[test]
#[provenance_macros::verifies("rule_inconsistent_records_refusal_names_records", examples)]
fn a_read_names_the_line_of_a_withdrawal_that_names_no_submission() {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    let path = directory
        .path()
        .join(".provenance/state/scopes/default/ideation/withdrawals.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let withdrawal = json!({"scope_id":"default","id":"withdrawal_lost",
        "proposal_id":"proposal_missing","actor":{"identity_type":"human","id":"reviewer"},"reason":null});
    std::fs::write(&path, format!("{withdrawal}\n")).unwrap();
    assert_rows(
        &[answer(&repo, &["api", "requirements/req_a"], "json")],
        &json!([{
            "code": 1,
            "command": "api requirements/req_a",
            "format": "json",
            "stderr": [
                refusal_line(
                    "",
                    "scopes/default/ideation/withdrawals.jsonl",
                    1,
                    "withdrawal withdrawal_lost names no review submission: proposal_missing",
                    "To repair, remove ideation/withdrawals.jsonl line 1, then commit.",
                )
            ],
            "stdout_empty": true
        }]),
    );
}

/// A standard error line of an `invalid_state_file` refusal of `file` at
/// `line`, with `hint` as the step that corrects the state.
fn refusal_line(prefix: &str, file: &str, line: u64, message: &str, hint: &str) -> String {
    let refusal = json!({"error": {"kind": "invalid_state_file", "file": file, "line": line,
        "message": message, "hint": hint}, "meta": {}});
    format!("{prefix}{refusal}")
}
