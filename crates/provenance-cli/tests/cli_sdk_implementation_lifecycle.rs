use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use serde_json::{json, Value};
use std::fs;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init_repo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    fs::create_dir(directory.path().join("src")).unwrap();
    fs::write(
        directory.path().join("src/runtime.ts"),
        "export function startWorkflow(): void {}\n",
    )
    .unwrap();
    fs::write(
        directory.path().join("src/alternate.ts"),
        "export function resumeWorkflow(): void {}\n",
    )
    .unwrap();
    directory
}

fn document(repo: &std::path::Path, implementation: Option<(&str, &str)>) -> Value {
    let implementation = implementation.map(|(file, symbol)| {
        json!({
            "file": repo.join(file),
            "symbol": symbol
        })
    });
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "workflow-runtime",
        "declared_by": "spec://typescript/workflow-runtime",
        "requirements": [{
            "key": "workflows",
            "statement": "Accepted workflows execute"
        }],
        "rules": [{
            "key": "start",
            "requirement": "workflows",
            "statement": "Accepted workflows start",
            "implementation": implementation
        }]
    })
}

fn sdk(repo: &std::path::Path, command: &str, input: &Value) -> Result<Value, String> {
    let output = provenance()
        .args([
            "sdk",
            command,
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(serde_json::from_slice(&output.stdout).unwrap())
}

fn stored_bindings(repo: &std::path::Path) -> Vec<Value> {
    fs::read_to_string(repo.join(".provenance/state/scopes/default/implementations/binding.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn rule_change<'a>(result: &'a Value, field: &str) -> &'a Value {
    result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| {
            resource["kind"] == "rule"
                && resource["changes"]
                    .as_array()
                    .is_some_and(|changes| changes.iter().any(|change| change["field"] == field))
        })
        .unwrap()
}

#[test]
fn restoring_an_implementation_recreates_the_same_binding() {
    let directory = init_repo();
    let implemented = document(directory.path(), Some(("src/runtime.ts", "startWorkflow")));
    let first = sdk(directory.path(), "apply", &implemented).unwrap();
    let binding_id = first["implementation_bindings"][0]["id"].clone();
    sdk(directory.path(), "apply", &document(directory.path(), None)).unwrap();

    let planned = sdk(directory.path(), "plan", &implemented).unwrap();
    assert_eq!(planned["updated"], 1);
    assert_eq!(
        rule_change(&planned, "implementation")["changes"][0]["before"],
        Value::Null
    );
    let restored = sdk(directory.path(), "apply", &implemented).unwrap();
    assert_eq!(restored["implementation_bindings"][0]["id"], binding_id);
}

#[test]
fn replacing_an_implementation_updates_the_same_binding() {
    let directory = init_repo();
    let initial = document(directory.path(), Some(("src/runtime.ts", "startWorkflow")));
    let first = sdk(directory.path(), "apply", &initial).unwrap();
    let binding_id = first["implementation_bindings"][0]["id"].clone();
    let replacement = document(
        directory.path(),
        Some(("src/alternate.ts", "resumeWorkflow")),
    );

    let planned = sdk(directory.path(), "plan", &replacement).unwrap();
    assert_eq!(planned["updated"], 1);
    let change = &rule_change(&planned, "implementation")["changes"][0];
    assert_eq!(change["before"]["file"], "src/runtime.ts");
    assert_eq!(change["after"]["file"], "src/alternate.ts");

    let applied = sdk(directory.path(), "apply", &replacement).unwrap();
    assert_eq!(applied["implementation_bindings"][0]["id"], binding_id);
    let stored = stored_bindings(directory.path());
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["file"], "src/alternate.ts");
}

#[test]
fn removing_an_implementation_deletes_its_binding() {
    let directory = init_repo();
    sdk(
        directory.path(),
        "apply",
        &document(directory.path(), Some(("src/runtime.ts", "startWorkflow"))),
    )
    .unwrap();
    let omitted = document(directory.path(), None);
    let plan = sdk(directory.path(), "plan", &omitted).unwrap();
    assert_eq!(plan["updated"], 1);
    assert_eq!(stored_bindings(directory.path()).len(), 1);
    sdk(directory.path(), "apply", &omitted).unwrap();
    assert!(stored_bindings(directory.path()).is_empty());
    assert_eq!(
        sdk(directory.path(), "plan", &omitted).unwrap()["updated"],
        0
    );
}
