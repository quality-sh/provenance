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
        "export function startWorkflow(): void {}\nexport class WorkflowRunner {}\n",
    )
    .unwrap();
    directory
}

fn spec(implementation_file: &str, statement: &str) -> Value {
    spec_with_symbol(implementation_file, "startWorkflow", statement)
}

fn spec_with_symbol(implementation_file: &str, symbol: &str, statement: &str) -> Value {
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "workflow-runtime",
        "declared_by": "spec://typescript/workflow-runtime",
        "requirements": [{
            "key": "workflows",
            "statement": "Accepted workflows execute"
        }, {
            "key": "operations",
            "statement": "Workflow operations are callable"
        }],
        "rules": [{
            "key": "start",
            "requirements": ["workflows", "operations"],
            "statement": statement,
            "implementation": {
                "file": implementation_file,
                "symbol": symbol
            }
        }]
    })
}

#[test]
fn apply_materializes_an_exported_class_as_the_canonical_binding() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let target = directory.path().join("src/runtime.ts");

    let applied = sdk(
        repo,
        "apply",
        &spec_with_symbol(
            target.to_str().unwrap(),
            "WorkflowRunner",
            "Accepted workflows run",
        ),
    )
    .unwrap();

    assert_eq!(
        applied["implementation_bindings"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        applied["implementation_bindings"][0]["file"],
        "src/runtime.ts"
    );
    assert_eq!(
        applied["implementation_bindings"][0]["symbol"],
        "WorkflowRunner"
    );
}

fn sdk(repo: &str, command: &str, input: &Value) -> Result<Value, String> {
    let output = provenance()
        .args([
            "sdk", command, "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(serde_json::from_slice(&output.stdout).unwrap())
}

#[test]
fn apply_materializes_one_idempotent_binding_for_a_shared_rule() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let target = directory.path().join("src/runtime.ts");
    let input = spec(target.to_str().unwrap(), "Accepted workflows start");

    let first = sdk(repo, "apply", &input).unwrap();
    let second = sdk(repo, "apply", &input).unwrap();

    assert_eq!(
        first["implementation_bindings"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        first["implementation_bindings"],
        second["implementation_bindings"]
    );
    let binding = &first["implementation_bindings"][0];
    assert_eq!(binding["file"], "src/runtime.ts");
    assert_eq!(binding["symbol"], "startWorkflow");
    let rule_id = first["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule")
        .unwrap()["id"]
        .clone();
    assert_eq!(binding["rule_id"], rule_id);

    let records = read_jsonl(
        &directory
            .path()
            .join(".provenance/state/scopes/default/implementations/binding.jsonl"),
    );
    assert_eq!(records.len(), 1);
    assert_eq!(records[0], *binding);
}

#[test]
fn rust_rejects_missing_and_outside_repository_targets() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let missing = directory.path().join("src/missing.ts");
    let outside = tempfile::NamedTempFile::new().unwrap();

    let missing_error = sdk(
        repo,
        "apply",
        &spec(missing.to_str().unwrap(), "Accepted workflows start"),
    )
    .unwrap_err();
    assert!(missing_error.contains("does not exist"), "{missing_error}");

    let outside_error = sdk(
        repo,
        "apply",
        &spec(outside.path().to_str().unwrap(), "Accepted workflows start"),
    )
    .unwrap_err();
    assert!(
        outside_error.contains("outside repository"),
        "{outside_error}"
    );
}

#[test]
fn semantic_plan_reports_the_typed_implementation() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let target = directory.path().join("src/runtime.ts");
    let planned = sdk(
        repo,
        "plan",
        &spec(target.to_str().unwrap(), "Accepted workflows start"),
    )
    .unwrap();

    assert_eq!(
        planned["affected_rules"][0]["implementations"],
        json!([{
            "file": "src/runtime.ts",
            "symbol": "startWorkflow"
        }])
    );

    sdk(
        repo,
        "apply",
        &spec(target.to_str().unwrap(), "Accepted workflows start"),
    )
    .unwrap();
    fs::write(
        directory.path().join("src/alternate.ts"),
        "export function startWorkflow(): void {}\n",
    )
    .unwrap();
    let alternate = directory.path().join("src/alternate.ts");
    let moved = sdk(
        repo,
        "plan",
        &spec(alternate.to_str().unwrap(), "Accepted workflows start"),
    )
    .unwrap();
    assert_eq!(
        moved["affected_rules"][0]["implementations"],
        json!([{
            "file": "src/alternate.ts",
            "symbol": "startWorkflow"
        }])
    );
}

#[test]
fn canonical_binding_and_matching_scanner_site_are_one_implementation() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let target = directory.path().join("src/runtime.ts");
    let applied = sdk(
        repo,
        "apply",
        &spec(target.to_str().unwrap(), "Accepted workflows start"),
    )
    .unwrap();
    let rule_id = applied["implementation_bindings"][0]["rule_id"]
        .as_str()
        .unwrap();

    let initial_coverage = coverage(repo);
    assert!(
        !initial_coverage.contains("has no implementation"),
        "{initial_coverage}"
    );

    fs::write(
        directory.path().join("src/runtime.ts"),
        format!("// @provenance rule: {rule_id}\nexport function startWorkflow(): void {{}}\n"),
    )
    .unwrap();
    let matching = coverage(repo);
    assert!(
        !matching.contains("more than one primary implementation binding"),
        "{matching}"
    );
}

#[test]
fn canonical_binding_conflicts_with_a_different_scanner_implementation() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let target = directory.path().join("src/runtime.ts");
    let applied = sdk(
        repo,
        "apply",
        &spec(target.to_str().unwrap(), "Accepted workflows start"),
    )
    .unwrap();
    let rule_id = applied["implementation_bindings"][0]["rule_id"]
        .as_str()
        .unwrap();
    fs::write(
        directory.path().join("src/other.ts"),
        format!("// @provenance rule: {rule_id}\nexport function otherStart(): void {{}}\n"),
    )
    .unwrap();

    let duplicate = coverage(repo);

    assert!(
        duplicate.contains("more than one primary implementation binding"),
        "{duplicate}"
    );
}

fn coverage(repo: &str) -> String {
    let output = provenance()
        .args([
            "coverage",
            "scan",
            "--repo",
            repo,
            "--path",
            repo,
            "--scope",
            "default",
            "--validate-rules",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

fn read_jsonl(path: &std::path::Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
