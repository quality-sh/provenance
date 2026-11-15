//! A refusal for a state file that does not parse names the two steps that
//! correct the state, and each step gives a state that Provenance can read.
//! The state of each test is the state of a Requirement line whose `refines`
//! is a string and not a list.

use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;

use crate::support;
use support::review_paths::provenance;

const HINT: &str = "get .provenance/state from a branch or a commit where Provenance can read \
                    it, or remove all records and settings of the repository: remove \
                    .provenance/state and run provenance init";

const REQUIREMENTS: &str = "scopes/default/requirements/req.jsonl";

const MESSAGE: &str = "invalid type: string \"req_parent\", expected a sequence";

/// The refusal object of the Requirement line that does not parse.
fn refusal() -> Value {
    json!({"error": {"kind": "invalid_state_file", "file": REQUIREMENTS, "line": 1,
        "message": MESSAGE, "hint": HINT}, "meta": {}})
}

/// The exit code, the standard output and the standard error of a command.
fn run(repo: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let output = provenance()
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// The standard output of a command that succeeds.
fn passes(repo: &Path, args: &[&str]) -> String {
    let (code, stdout, stderr) = run(repo, args);
    assert_eq!(code, Some(0), "{args:?}: {stderr}");
    stdout
}

/// Creates the Requirement that `args` names, with the flags that follow it.
fn create(repo: &Path, args: &[&str]) {
    let statement = ["--statement", "The system keeps review feedback available."];
    let flags = ["create", "--type", "requirement"];
    passes(repo, &[&args[..1], &flags, &statement, &args[1..]].concat());
}

fn git(repo: &Path, args: &[&str]) {
    let output = provenance_store::git_process::git()
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A Git repository with a commit of `req_parent` and `req_child`, where
/// `req_child` refines `req_parent`.
fn committed() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    git(repo, &["init", "--quiet"]);
    support::review_paths::init_scope(repo, "default", ".");
    create(repo, &["req_parent"]);
    create(repo, &["req_child", "--refines", "req_parent"]);
    git(repo, &["add", "--all"]);
    let identity = ["-c", "user.name=Test", "-c", "user.email=test@example.com"];
    git(repo, &[&identity[..], &["commit", "--quiet", "-m", "records"]].concat());
    directory
}

/// The committed repository after a change of the `req_child` line, so that
/// its `refines` is a string.
fn unparsable() -> tempfile::TempDir {
    let directory = committed();
    let path = directory.path().join(".provenance/state").join(REQUIREMENTS);
    let text = std::fs::read_to_string(&path).unwrap();
    let changed = text.replace(r#""refines":["req_parent"]"#, r#""refines":"req_parent""#);
    assert_ne!(changed, text, "{text}");
    assert!(changed.starts_with(r#"{"scope_id":"default","id":"req_child""#), "{changed}");
    std::fs::write(&path, changed).unwrap();
    directory
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_json_get_names_the_two_ways_out() {
    let directory = unparsable();

    let answer = run(directory.path(), &["req_child", "get", "--format", "json"]);

    assert_eq!(answer, (Some(1), String::new(), format!("{}\n", refusal())));
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_text_get_names_the_two_ways_out() {
    let directory = unparsable();

    let answer = run(directory.path(), &["req_child", "get"]);

    assert_eq!(
        answer,
        (Some(1), String::new(), format!("Error: {}\n", refusal()))
    );
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn init_names_the_two_ways_out_and_changes_no_file() {
    let directory = unparsable();
    let state = directory.path().join(".provenance/state");
    let before = support::state_files::files_below(&state);

    let answer = run(directory.path(), &["init"]);

    let stderr = format!(
        "Error: the planned Provenance state is not valid\n\nCaused by:\n    \
         {REQUIREMENTS} line 1: {MESSAGE}; {HINT}\n"
    );
    assert_eq!(answer, (Some(1), String::new(), stderr));
    assert_eq!(support::state_files::files_below(&state), before);
}

/// The MCP get of the record serves the record of the read cache and gives
/// the refusal of the state as its freshness warning.
#[tokio::test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
async fn an_mcp_get_names_the_two_ways_out() {
    let directory = unparsable();
    let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(["mcp", "--repo", directory.path().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let io = tokio::io::join(child.stdout.take().unwrap(), child.stdin.take().unwrap());
    let client = provenance_mcp::Client::connect(io).await.unwrap();

    let answer = client.call("get", json!({"target": "req_child"})).await;
    client.cancel().await.unwrap();
    child.wait().await.unwrap();

    let warning = format!("{REQUIREMENTS} line 1: {MESSAGE}; {HINT}");
    assert_eq!(answer["isError"], json!(false), "{answer}");
    let metadata = &answer["structuredContent"]["record_metadata"];
    assert_eq!(metadata["freshness_error"], json!(warning), "{answer}");
    let text = answer["content"][0]["text"].as_str().unwrap();
    assert!(
        text.ends_with(&format!("\n\nwarning: record freshness: {warning}")),
        "{text}"
    );
}

/// The second way: after the removal of the state directory, init gives a
/// state that reads, and that state holds none of the records.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn init_after_the_removal_of_the_state_gives_a_state_that_reads() {
    let directory = unparsable();
    let repo = directory.path();

    std::fs::remove_dir_all(repo.join(".provenance/state")).unwrap();
    passes(repo, &["init"]);

    let (code, stdout, stderr) = run(repo, &["req_child", "get", "--format", "json"]);
    assert_eq!(code, Some(1), "{stdout}");
    let refused: Value = serde_json::from_str(&stderr).unwrap();
    assert_eq!(refused["error"]["kind"], "resource_not_found", "{stderr}");
    create(repo, &["req_new"]);
    let read = passes(repo, &["req_new", "get", "--format", "json"]);
    let read: Value = serde_json::from_str(&read).unwrap();
    assert_eq!(read["record"]["id"], "req_new", "{read}");
}

/// The first way: the state directory of a commit where the state reads
/// gives a state that reads, with the records of that commit.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn the_state_of_a_commit_where_it_reads_gives_a_state_that_reads() {
    let directory = unparsable();
    let repo = directory.path();

    git(repo, &["checkout", "HEAD", "--", ".provenance/state"]);

    let read = passes(repo, &["req_child", "get", "--format", "json"]);
    let read: Value = serde_json::from_str(&read).unwrap();
    let refines = &read["record"]["value"]["refines"];
    assert_eq!(refines, &json!(["req_parent"]), "{read}");
}

/// A line of a different record family that is not JSON gives the same two
/// ways out.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_bad_line_of_a_different_family_names_the_same_ways_out() {
    let directory = committed();
    let repo = directory.path();
    let rules = repo.join(".provenance/state/scopes/default/rules/rule.jsonl");
    std::fs::create_dir_all(rules.parent().unwrap()).unwrap();
    std::fs::write(&rules, "{\n").unwrap();

    let answer = run(repo, &["req_child", "get", "--format", "json"]);

    let refusal = json!({"error": {"kind": "invalid_state_file",
        "file": "scopes/default/rules/rule.jsonl", "line": 1,
        "message": "EOF while parsing an object", "hint": HINT}, "meta": {}});
    assert_eq!(answer, (Some(1), String::new(), format!("{refusal}\n")));
}

/// A dictionary reference that does not parse gives the same two ways out.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_dictionary_reference_that_does_not_parse_names_the_same_ways_out() {
    let directory = committed();
    let repo = directory.path();
    std::fs::write(repo.join(".provenance/state/dictionary.json"), "").unwrap();

    let answer = run(repo, &["init"]);

    let stderr = format!("Error: dictionary.json line 1: EOF while parsing a value; {HINT}\n");
    assert_eq!(answer, (Some(1), String::new(), stderr));
}
