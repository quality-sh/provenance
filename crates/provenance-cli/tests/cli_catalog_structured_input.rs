use assert_cmd::Command;
use predicates::str::contains;
use serde_json::{json, Value};

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
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
    (directory, repo)
}

fn output(command: &mut Command) -> Value {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn create_source(repo: &str, id: &str) {
    provenance()
        .args([
            "sources", "create", "--repo", repo, "--id", id, "--name", id,
        ])
        .assert()
        .success();
}

fn create_requirement(repo: &str, id: &str) -> Value {
    output(provenance().args([
        "requirements",
        "create",
        "--repo",
        repo,
        "--id",
        id,
        "--statement",
        &format!("The system records {id}."),
    ]))
}

#[test]
fn repeated_array_flags_equal_the_canonical_json_request() {
    let (_directory, repo) = init();
    create_source(&repo, "source_old_a");
    create_source(&repo, "source_old_b");

    let flags = output(provenance().args([
        "sources",
        "create",
        "--repo",
        &repo,
        "--id",
        "source_flags",
        "--name",
        "Flags",
        "--supersedes",
        "source_old_a",
        "--supersedes",
        "source_old_b",
    ]));
    let stdin = output(
        provenance()
            .args(["sources", "create", "--repo", &repo, "--stdin"])
            .write_stdin(
                json!({
                    "id": "source_stdin",
                    "name": "Stdin",
                    "source_type": "policy",
                    "supersedes": ["source_old_a", "source_old_b"]
                })
                .to_string(),
            ),
    );

    assert_eq!(flags["data"]["supersedes"], stdin["data"]["supersedes"]);
    assert_eq!(
        flags["data"]["supersedes"],
        json!(["source_old_a", "source_old_b"])
    );
}

#[test]
fn canonical_json_flags_express_empty_arrays_and_nested_deltas() {
    let (_directory, repo) = init();
    let target_a = create_requirement(&repo, "req_target_a");
    create_requirement(&repo, "req_target_b");
    assert_eq!(target_a["data"]["depends_on"], json!([]));

    let owner = output(
        provenance()
            .args([
                "requirements",
                "create",
                "--repo",
                &repo,
                "--stdin",
                "--description",
                "Created from mixed input",
                "--depends-on-json",
                "[]",
            ])
            .write_stdin(
                json!({
                    "id": "req_owner",
                    "statement": "The system records the first statement."
                })
                .to_string(),
            ),
    );
    assert_eq!(owner["data"]["description"], "Created from mixed input");
    assert_eq!(owner["data"]["depends_on"], json!([]));

    let with_delta = output(provenance().args([
        "requirements",
        "update",
        "req_owner",
        "--repo",
        &repo,
        "--if-match",
        owner["data"]["edit"]["etag"].as_str().unwrap(),
        "--statement",
        "The system records the changed statement.",
        "--relationships-json",
        r#"{"depends_on":{"add":["req_target_a"]}}"#,
    ]));
    assert_eq!(
        with_delta["data"]["statement"],
        "The system records the changed statement."
    );
    assert_eq!(with_delta["data"]["depends_on"], json!(["req_target_a"]));

    let full_set = output(provenance().args([
        "requirements",
        "update",
        "req_owner",
        "--repo",
        &repo,
        "--if-match",
        with_delta["data"]["edit"]["etag"].as_str().unwrap(),
        "--relationships-json",
        r#"{"depends_on":["req_target_b"]}"#,
    ]));
    assert_eq!(full_set["data"]["depends_on"], json!(["req_target_b"]));

    let removed = output(provenance().args([
        "requirements",
        "update",
        "req_owner",
        "--repo",
        &repo,
        "--if-match",
        full_set["data"]["edit"]["etag"].as_str().unwrap(),
        "--relationships-json",
        r#"{"depends_on":{"remove":["req_target_b"]}}"#,
    ]));
    assert_eq!(removed["data"]["depends_on"], json!([]));
}

#[test]
fn scalar_strings_keep_json_like_and_option_like_text() {
    let (_directory, repo) = init();
    let source = output(provenance().args([
        "sources",
        "create",
        "--repo",
        &repo,
        "--id",
        "source_literals",
        "--name",
        "[Draft] policy",
        "--url",
        "null",
        "--reference",
        "--help",
    ]));

    assert_eq!(source["data"]["name"], "[Draft] policy");
    assert_eq!(source["data"]["url"], "null");
    assert_eq!(source["data"]["reference"], "--help");
}

#[test]
fn json_null_is_distinct_from_the_scalar_string_null() {
    let (_directory, repo) = init();
    let created = output(provenance().args([
        "requirements",
        "create",
        "--repo",
        &repo,
        "--id",
        "req_null",
        "--statement",
        "The system records nullable input.",
        "--description",
        "Initial",
    ]));
    let literal = output(provenance().args([
        "requirements",
        "update",
        "req_null",
        "--repo",
        &repo,
        "--if-match",
        created["data"]["edit"]["etag"].as_str().unwrap(),
        "--description",
        "null",
    ]));
    assert_eq!(literal["data"]["description"], "null");

    let cleared = output(provenance().args([
        "requirements",
        "update",
        "req_null",
        "--repo",
        &repo,
        "--if-match",
        literal["data"]["edit"]["etag"].as_str().unwrap(),
        "--description-json",
        "null",
    ]));
    assert!(cleared["data"]["description"].is_null());
}

#[test]
fn repeated_alias_items_accumulate_but_competing_spellings_fail() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_alias_a");
    create_requirement(&repo, "req_alias_b");

    let rule = output(provenance().args([
        "rules",
        "create",
        "--repo",
        &repo,
        "--id",
        "rule_alias",
        "--statement",
        "The system records alias input.",
        "--requirement-id",
        "req_alias_a",
        "--requirement-id",
        "req_alias_b",
    ]));
    assert_eq!(
        rule["data"]["requirement_ids"],
        json!(["req_alias_a", "req_alias_b"])
    );

    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            &repo,
            "--id",
            "rule_collision",
            "--statement",
            "The system rejects competing spellings.",
            "--requirement-id",
            "req_alias_a",
            "--requirement-ids",
            "req_alias_b",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "--requirement-ids conflicts with --requirement-id for requirement_ids",
        ));
}

#[test]
fn conflicting_body_sources_are_rejected_without_overwrite() {
    let (_directory, repo) = init();
    provenance()
        .args([
            "sources", "create", "--repo", &repo, "--id", "source_duplicate", "--name",
            "First", "--name", "Second",
        ])
        .assert()
        .failure()
        .stderr(contains("body field name is assigned more than once"));

    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_competing",
            "--name",
            "Competing",
            "--supersedes",
            "source_old",
            "--supersedes-json",
            "[]",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "--supersedes-json conflicts with --supersedes for supersedes",
        ));

    provenance()
        .args([
            "sources", "create", "--repo", &repo, "--stdin", "--name", "Flag name",
        ])
        .write_stdin(r#"{"id":"source_stdin_collision","name":"Stdin name"}"#)
        .assert()
        .failure()
        .stderr(contains("stdin field name conflicts with --name"));
}

#[test]
fn malformed_or_schema_invalid_json_flags_are_rejected() {
    let (_directory, repo) = init();
    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_malformed",
            "--name",
            "Malformed",
            "--supersedes-json",
            "[",
        ])
        .assert()
        .failure()
        .stderr(contains("invalid JSON value for --supersedes-json"));

    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_wrong_shape",
            "--name",
            "Wrong shape",
            "--supersedes-json",
            r#"{"not":"an array"}"#,
        ])
        .assert()
        .failure()
        .stderr(contains("invalid value for --supersedes-json"));
}

#[test]
fn existing_header_preconditions_remain_required() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_precondition");

    provenance()
        .args([
            "requirements",
            "update",
            "req_precondition",
            "--repo",
            &repo,
            "--description",
            "No precondition",
        ])
        .assert()
        .failure()
        .stderr(contains("expected_etag"));
}
