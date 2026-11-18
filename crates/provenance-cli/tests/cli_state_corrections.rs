//! A refusal for a state file that does not parse names the correction of
//! the line and then the two procedures, each with what it removes. The state of most
//! tests has a Requirement line whose `refines` is a string and not a list, as
//! in a line of an older form. A get over a bad Requirement line answers at the
//! stored serial, and its freshness warning is the text of the refusal.
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;

use crate::support;
use support::state_faults::{
    committed, cut_manifest, damage, older_manifest, passes, run, HINT, REQUIREMENTS, WRITE,
};

const MESSAGE: &str = "invalid type: string \"req_parent\", expected a sequence";

/// The freshness warning of a read over the Requirement line that does not parse.
fn warning() -> String {
    format!("{REQUIREMENTS} line 1: {MESSAGE}; {HINT}")
}

/// The refusal object of the Requirement line that does not parse.
fn refusal() -> Value {
    json!({"error": {"kind": "invalid_state_file", "file": REQUIREMENTS, "line": 1,
        "message": MESSAGE, "hint": HINT}, "meta": {}})
}

/// The committed repository after a change of the `req_child` line, so that
/// its `refines` is a string.
fn unparsable() -> tempfile::TempDir {
    let directory = committed();
    damage(directory.path());
    directory
}

/// A fault of a manifest, which gives the line and the parse message of the
/// refusal.
type ManifestFault = fn(&Path) -> (u64, String);

/// The two faults of a manifest.
const MANIFEST_FAULTS: [ManifestFault; 2] = [older_manifest, cut_manifest];

/// The refusal text of a manifest that does not parse at `line`.
fn manifest_cause((line, message): &(u64, String)) -> String {
    format!("manifest.json line {line}: {message}; {HINT}")
}

/// The refusal object of a manifest that does not parse at `line`.
fn manifest_refusal((line, message): &(u64, String)) -> Value {
    json!({"error": {"kind": "invalid_state_file", "file": "manifest.json", "line": line,
        "message": message, "hint": HINT}, "meta": {}})
}

/// A JSON get gives the record at the stored serial with the warning.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_json_get_names_the_corrections() {
    let directory = committed();
    let repo = directory.path();
    let clean = passes(repo, &["req_child", "get", "--format", "json"]);
    let mut expected: Value = serde_json::from_str(&clean).unwrap();
    damage(repo);

    let (code, stdout, stderr) = run(repo, &["req_child", "get", "--format", "json"]);

    expected["record_metadata"]["freshness_error"] = json!(warning());
    expected["record_metadata"]["stamp"]["policy"] = json!("catch_up_failed");
    let stdout: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!((code, stdout, stderr), (Some(0), expected, String::new()));
}

/// A text get gives the record at the stored serial and the warning below it.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_text_get_names_the_corrections() {
    let directory = committed();
    let repo = directory.path();
    let clean = passes(repo, &["req_child", "get"]);
    damage(repo);

    let answer = run(repo, &["req_child", "get"]);

    let stdout = format!("{clean}\nwarning: record freshness: {}\n", warning());
    assert_eq!(answer, (Some(0), stdout, String::new()));
}

/// A JSON write refuses with the refusal object.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_json_write_names_the_corrections() {
    let directory = unparsable();
    let args = [&WRITE[..], &["--format", "json"]].concat();

    let answer = run(directory.path(), &args);

    assert_eq!(answer, (Some(1), String::new(), format!("{}\n", refusal())));
}

/// A text write refuses with the refusal object after `Error: `.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_text_write_names_the_corrections() {
    let directory = unparsable();
    let answer = run(directory.path(), &WRITE);

    let stderr = format!("Error: {}\n", refusal());
    assert_eq!(answer, (Some(1), String::new(), stderr));
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn init_names_the_corrections_and_changes_no_file() {
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

/// `check` gives the cause with the hint in each section that the cause stops,
/// and its error line gives the cause with the hint.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn check_names_the_corrections() {
    let directory = unparsable();

    let (code, stdout, stderr) = run(directory.path(), &["check"]);

    let cause = warning();
    let sections = format!(
        "graph: findings\n  - {cause}\nstatements: unavailable\n  - {cause}\nbindings: passed\n"
    );
    assert_eq!((code, stdout), (Some(1), sections));
    assert!(stderr.starts_with(&format!("Error: {cause}")), "{stderr}");
}

/// An implementation aid with no Rule binding, because no Rule states that the
/// error line of `check` gives each distinct cause one time. It keeps the
/// error line of one bad line to one copy of the hint.
#[test]
fn check_gives_the_hint_one_time_in_its_error_line() {
    let directory = unparsable();

    let (_, _, stderr) = run(directory.path(), &["check"]);

    assert_eq!(stderr, format!("Error: {}\n", warning()));
}

/// The MCP get of the record serves the record of the read cache and gives
/// the refusal of the state as its freshness warning.
#[tokio::test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
async fn an_mcp_get_names_the_corrections() {
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

    let warning = warning();
    assert_eq!(answer["isError"], json!(false), "{answer}");
    let metadata = &answer["structuredContent"]["record_metadata"];
    assert_eq!(metadata["freshness_error"], json!(warning), "{answer}");
    let text = answer["content"][0]["text"].as_str().unwrap();
    assert!(
        text.ends_with(&format!("\nwarning: record freshness: {warning}")),
        "{text}"
    );
}

/// A line of a different record family that is not a record gives the same
/// corrections.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_bad_line_of_a_different_family_names_the_same_corrections() {
    let directory = committed();
    let repo = directory.path();
    let file = "scopes/default/ideation/proposal_cards.jsonl";
    let path = repo.join(".provenance/state").join(file);
    let mut text = std::fs::read_to_string(&path).unwrap();
    let line = text.lines().count() + 1;
    text.push_str("{\"scope_id\":\"default\"}\n");
    std::fs::write(&path, text).unwrap();

    let answer = run(repo, &[&WRITE[..], &["--format", "json"]].concat());

    let refusal = json!({"error": {"kind": "invalid_state_file", "file": file, "line": line,
        "message": "missing field `id`", "hint": HINT}, "meta": {}});
    assert_eq!(answer, (Some(1), String::new(), format!("{refusal}\n")));
}

/// A JSON write and a JSON get over a manifest that does not parse refuse with
/// the refusal object of the manifest.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_json_command_over_a_bad_manifest_names_the_corrections() {
    for fault in MANIFEST_FAULTS {
        let directory = committed();
        let repo = directory.path();
        let located = fault(repo);
        let stderr = format!("{}\n", manifest_refusal(&located));

        let write = run(repo, &[&WRITE[..], &["--format", "json"]].concat());
        let get = run(repo, &["req_child", "get", "--format", "json"]);

        assert_eq!(write, (Some(1), String::new(), stderr.clone()));
        assert_eq!(get, (Some(1), String::new(), stderr));
    }
}

/// A text get and an init over a manifest that does not parse refuse with the
/// refusal text of the manifest, and the init changes no file.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_text_get_and_init_over_a_bad_manifest_name_the_corrections() {
    for fault in MANIFEST_FAULTS {
        let directory = committed();
        let repo = directory.path();
        let located = fault(repo);
        let state = repo.join(".provenance/state");
        let before = support::state_files::files_below(&state);
        let stderr = format!("Error: {}\n", manifest_cause(&located));

        let get = run(repo, &["req_child", "get"]);
        let init = run(repo, &["init"]);

        assert_eq!(get, (Some(1), String::new(), stderr.clone()));
        assert_eq!(init, (Some(1), String::new(), stderr));
        assert_eq!(support::state_files::files_below(&state), before);
    }
}

/// `check` over a manifest that does not parse gives the refusal text of the
/// manifest in each section that it stops and in its error line.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn check_over_a_bad_manifest_names_the_corrections() {
    for fault in MANIFEST_FAULTS {
        let directory = committed();
        let located = fault(directory.path());

        let (code, stdout, stderr) = run(directory.path(), &["check"]);

        let cause = manifest_cause(&located);
        let sections = format!(
            "graph: findings\n  - {cause}\nstatements: unavailable\n  - {cause}\n\
             bindings: unavailable\n  - {cause}\n"
        );
        assert_eq!((code, stdout), (Some(1), sections));
        assert!(stderr.starts_with(&format!("Error: {cause}")), "{stderr}");
    }
}
