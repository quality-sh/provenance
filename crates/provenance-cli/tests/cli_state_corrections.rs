//! A refusal for a state file that does not parse names the two procedures
//! that correct the state and what each procedure removes, and each procedure,
//! done as the refusal says, gives a state that Provenance can read. The state
//! of most tests has a Requirement line whose `refines` is a string and not a
//! list, as in a line of an older form. The tests of the two procedures also
//! use a line that a person cut, and a manifest of each of these two forms. A
//! get over a bad Requirement line answers at the stored serial, and its
//! freshness warning is the text of the refusal.
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;

use crate::support;
use support::review_paths::provenance;
use support::stale_repo::{commit, git, init_repo};

/// The hint of a record file that does not parse. The other tests compare
/// with the public constant; this file pins the text that a person reads.
const HINT: &str = "To correct this fault, use one of these two procedures. \
    Procedure 1: get .provenance/state from a commit where Provenance can read it. \
    This can remove records and changes that are not in that commit. \
    Procedure 2: remove .provenance/state and then do the command provenance init. \
    This removes all records, the scopes, the disposition actors and the technical names of \
    the dictionary.";

const REQUIREMENTS: &str = "scopes/default/requirements/req.jsonl";

const MESSAGE: &str = "invalid type: string \"req_parent\", expected a sequence";

/// A write that reads the Requirement file.
const WRITE: [&str; 6] = [
    "req_new",
    "create",
    "--type",
    "requirement",
    "--statement",
    "The system keeps a record.",
];

/// The freshness warning of a read over the Requirement line that does not parse.
fn warning() -> String {
    format!("{REQUIREMENTS} line 1: {MESSAGE}; {HINT}")
}

/// The refusal object of the Requirement line that does not parse.
fn refusal() -> Value {
    json!({"error": {"kind": "invalid_state_file", "file": REQUIREMENTS, "line": 1,
        "message": MESSAGE, "hint": HINT}, "meta": {}})
}

/// The exit code, the standard output and the standard error of a command.
fn run(repo: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let output = provenance().current_dir(repo).args(args).output().unwrap();
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

/// A Git repository with a commit of `req_parent` and `req_child`, where
/// `req_child` refines `req_parent`.
fn committed() -> tempfile::TempDir {
    let directory = init_repo();
    let repo = directory.path();
    create(repo, &["req_parent"]);
    create(repo, &["req_child", "--refines", "req_parent"]);
    commit(repo, "records");
    directory
}

/// The committed repository after a change of the `req_child` line, so that
/// its `refines` is a string.
fn unparsable() -> tempfile::TempDir {
    let directory = committed();
    damage(directory.path());
    directory
}

/// Changes the `req_child` line, so that its `refines` is a string, as in a
/// line of an older form.
fn damage(repo: &Path) {
    let path = repo.join(".provenance/state").join(REQUIREMENTS);
    let text = std::fs::read_to_string(&path).unwrap();
    let changed = text.replace(r#""refines":["req_parent"]"#, r#""refines":"req_parent""#);
    assert_ne!(changed, text, "{text}");
    let first = r#"{"scope_id":"default","id":"req_child""#;
    assert!(changed.starts_with(first), "{changed}");
    std::fs::write(&path, changed).unwrap();
}

/// Removes the last character of the `req_child` line, as a person who
/// stopped an edit of the line.
fn cut(repo: &Path) {
    let path = repo.join(".provenance/state").join(REQUIREMENTS);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].contains(r#""id":"req_child""#), "{text}");
    lines[0] = &lines[0][..lines[0].len() - 1];
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
}

/// Changes the manifest, so that its disposition actors are one string and
/// not a list, as in a manifest of an older form. It gives the line and the
/// parse message of the refusal.
fn older_manifest(repo: &Path) -> (u64, String) {
    let path = repo.join(".provenance/state/manifest.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["disposition_actor_ids"] = json!("reviewer");
    let text = serde_json::to_string_pretty(&manifest).unwrap();
    assert!(text.starts_with("{\n  \"disposition_actor_ids\": \"reviewer\","), "{text}");
    std::fs::write(&path, text + "\n").unwrap();
    let message = "invalid type: string \"reviewer\", expected a sequence";
    (2, message.to_owned())
}

/// Removes the last character of the manifest, as a person who stopped an
/// edit of the file. It gives the line and the parse message of the refusal.
fn cut_manifest(repo: &Path) -> (u64, String) {
    let path = repo.join(".provenance/state/manifest.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.trim_end();
    assert!(text.ends_with('}'), "{text}");
    let cut = &text[..text.len() - 1];
    std::fs::write(&path, cut).unwrap();
    let line = u64::try_from(cut.matches('\n').count() + 1).unwrap();
    (line, "EOF while parsing an object".to_owned())
}

/// A fault of a manifest, which gives the line and the parse message of the
/// refusal.
type ManifestFault = fn(&Path) -> (u64, String);

/// The two faults of a manifest.
const MANIFEST_FAULTS: [ManifestFault; 2] = [older_manifest, cut_manifest];

/// The faults of the state: a `req_child` line of an older form, a
/// `req_child` line that a person cut, and the two faults of a manifest.
const FAULTS: [fn(&Path); 4] = [
    damage,
    cut,
    |repo| {
        older_manifest(repo);
    },
    |repo| {
        cut_manifest(repo);
    },
];

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
fn a_json_get_names_the_two_ways_out() {
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
fn a_text_get_names_the_two_ways_out() {
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
fn a_json_write_names_the_two_ways_out() {
    let directory = unparsable();
    let args = [&WRITE[..], &["--format", "json"]].concat();

    let answer = run(directory.path(), &args);

    assert_eq!(answer, (Some(1), String::new(), format!("{}\n", refusal())));
}

/// A text write refuses with the refusal object after `Error: `.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_text_write_names_the_two_ways_out() {
    let directory = unparsable();
    let answer = run(directory.path(), &WRITE);

    let stderr = format!("Error: {}\n", refusal());
    assert_eq!(answer, (Some(1), String::new(), stderr));
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

/// `check` gives the cause with the hint in each section that the cause stops,
/// and its error line gives the cause with the hint.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn check_names_the_two_procedures() {
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

/// The hint of the refusal of a write.
fn hint(repo: &Path) -> Value {
    let (code, _, stderr) = run(repo, &[&WRITE[..], &["--format", "json"]].concat());
    assert_eq!(code, Some(1), "{stderr}");
    serde_json::from_str::<Value>(&stderr).unwrap()["error"]["hint"].take()
}

/// The JSON get of `id`, which passes with no freshness warning.
fn read(repo: &Path, id: &str) -> Value {
    let read = passes(repo, &[id, "get", "--format", "json"]);
    let read: Value = serde_json::from_str(&read).unwrap();
    let warning = &read["record_metadata"]["freshness_error"];
    assert_eq!(warning, &Value::Null, "{read}");
    read
}

/// Writes a dictionary reference with a technical name to the state. The CLI
/// has no command that adds a technical name.
fn add_technical_name(repo: &Path) {
    let dictionary = crate::dictionary_support::imported_dictionary();
    crate::dictionary_support::write_reference_with_technical_names(
        repo,
        dictionary,
        &["Provenance"],
    );
}

/// The JSON file at `file` below the state directory.
fn state_json(repo: &Path, file: &str) -> Value {
    let path = repo.join(".provenance/state").join(file);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// The technical names of the dictionary reference of the state, when it has
/// a reference.
fn technical_names(repo: &Path) -> Option<Value> {
    let path = repo.join(".provenance/state/dictionary.json");
    let bytes = std::fs::read(path).ok()?;
    let reference: Value = serde_json::from_slice(&bytes).unwrap();
    reference.get("technical_names").cloned()
}

/// Procedure 2, done as the hint says: remove .provenance/state and then do
/// the command provenance init. After them, the state reads, and it holds none
/// of the records, scopes, disposition actors and technical names that it held.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn init_after_the_removal_of_the_state_gives_a_state_that_reads() {
    for fault in FAULTS {
        let directory = committed();
        let repo = directory.path();
        std::fs::create_dir(repo.join("other")).unwrap();
        support::review_paths::init_scope(repo, "other", "other");
        fault(repo);
        assert_eq!(hint(repo), json!(HINT));
        add_technical_name(repo);
        assert_eq!(technical_names(repo), Some(json!(["Provenance"])));

        std::fs::remove_dir_all(repo.join(".provenance/state")).unwrap();
        passes(repo, &["init"]);

        let manifest = state_json(repo, "manifest.json");
        let scopes = json!([{"id": "default", "path_prefix": "."}]);
        assert_eq!(manifest["scopes"], scopes, "{manifest}");
        let actors = manifest["disposition_actor_ids"].as_array().unwrap();
        assert!(!actors.contains(&json!("reviewer")), "{manifest}");
        assert_eq!(technical_names(repo), None);
        let (code, stdout, stderr) = run(repo, &["req_child", "get", "--format", "json"]);
        assert_eq!(code, Some(1), "{stdout}");
        let refused: Value = serde_json::from_str(&stderr).unwrap();
        assert_eq!(refused["error"]["kind"], "resource_not_found", "{stderr}");
        create(repo, &["req_new"]);
        let read = read(repo, "req_new");
        assert_eq!(read["record"]["id"], "req_new", "{read}");
    }
}

/// Procedure 1, done as the hint says: get .provenance/state from a commit
/// where Provenance can read it. The state then reads and holds the records of
/// that commit, and a record that is not in that commit is removed.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn the_state_of_a_commit_where_it_reads_gives_a_state_that_reads() {
    for fault in FAULTS {
        let directory = committed();
        let repo = directory.path();
        create(repo, &["req_extra"]);
        fault(repo);
        assert_eq!(hint(repo), json!(HINT));

        git(repo, &["checkout", "HEAD", "--", ".provenance/state"]);

        let read = read(repo, "req_child");
        let refines = &read["record"]["value"]["refines"];
        assert_eq!(refines, &json!(["req_parent"]), "{read}");
        let (code, _, stderr) = run(repo, &["req_extra", "get", "--format", "json"]);
        assert_eq!(code, Some(1), "{stderr}");
        let refused: Value = serde_json::from_str(&stderr).unwrap();
        assert_eq!(refused["error"]["kind"], "resource_not_found", "{stderr}");
    }
}

/// A line of a different record family that is not a record gives the same
/// two ways out.
#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_bad_line_of_a_different_family_names_the_same_ways_out() {
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
fn a_json_command_over_a_bad_manifest_names_the_two_procedures() {
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
fn a_text_get_and_init_over_a_bad_manifest_name_the_two_procedures() {
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
fn check_over_a_bad_manifest_names_the_two_procedures() {
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
