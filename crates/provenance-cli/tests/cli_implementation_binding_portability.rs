use assert_cmd::Command;
use predicates::str::contains;
use serde_json::{json, Value};
use std::path::Path;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init(repo: &Path) {
    provenance()
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
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "req_workflows",
            "--statement",
            "Accepted workflows start",
        ])
        .assert()
        .success();
}

fn create_rule(repo: &Path, id: &str) {
    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            id,
            "--requirement-id",
            "req_workflows",
            "--statement",
            "Accepted workflows start",
        ])
        .assert()
        .success();
}

fn binding(id: &str, rule_id: &str) -> Value {
    json!({
        "schema_version": 1,
        "scope_id": "default",
        "id": id,
        "rule_id": rule_id,
        "declared_by": "spec://typescript/workflows",
        "file": "src/runtime.ts",
        "symbol": "startWorkflow"
    })
}

fn write_implementation_bindings(repo: &Path, bindings: &[Value]) {
    let path = repo.join(".provenance/state/scopes/default/implementations/binding.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut contents = String::new();
    for record in bindings {
        contents.push_str(&serde_json::to_string(record).unwrap());
        contents.push('\n');
    }
    std::fs::write(path, contents).unwrap();
}

fn export(repo: &Path, output: &Path) -> Value {
    provenance()
        .args([
            "export",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap()
}

#[test]
fn scope_export_import_round_trip_preserves_implementation_bindings() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    let target = directory.path().join("target");
    let exported_path = directory.path().join("scope.json");
    let reexported_path = directory.path().join("round-trip.json");
    init(&source);
    create_rule(&source, "rule_start");
    let mut retired = binding("implementation_binding_start", "rule_start");
    retired["retired"] = json!(true);
    write_implementation_bindings(&source, &[retired.clone()]);

    let exported = export(&source, &exported_path);
    assert_eq!(exported["implementation_bindings"], json!([retired]));

    init(&target);
    provenance()
        .args([
            "import",
            "--repo",
            target.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            exported_path.to_str().unwrap(),
        ])
        .assert()
        .success();
    provenance()
        .args(["check", "--repo", target.to_str().unwrap()])
        .assert()
        .success();

    let reexported = export(&target, &reexported_path);
    assert_eq!(
        reexported["implementation_bindings"],
        exported["implementation_bindings"]
    );
}

#[test]
fn scope_import_defaults_an_omitted_implementation_binding_family_to_empty() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source");
    let target = directory.path().join("target");
    let exported_path = directory.path().join("scope.json");
    init(&source);
    init(&target);
    let mut exported = export(&source, &exported_path);
    assert_eq!(exported["implementation_bindings"], json!([]));
    exported
        .as_object_mut()
        .unwrap()
        .remove("implementation_bindings");
    std::fs::write(&exported_path, serde_json::to_vec(&exported).unwrap()).unwrap();

    provenance()
        .args([
            "import",
            "--repo",
            target.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            exported_path.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn check_rejects_invalid_implementation_binding_rows() {
    let cases = [
        (
            "unsupported schema",
            vec![{
                let mut record = binding("implementation_binding_start", "rule_start");
                record["schema_version"] = json!(2);
                record
            }],
            "schema_version 2",
        ),
        (
            "wrong scope",
            vec![{
                let mut record = binding("implementation_binding_start", "rule_start");
                record["scope_id"] = json!("other");
                record
            }],
            "implementation binding implementation_binding_start loaded from scope default",
        ),
        (
            "unknown rule",
            vec![binding("implementation_binding_missing", "rule_missing")],
            "implementation binding implementation_binding_missing has dangling reference: rule_id rule rule_missing",
        ),
        (
            "duplicate primary",
            vec![
                binding("implementation_binding_first", "rule_start"),
                binding("implementation_binding_second", "rule_start"),
            ],
            "more than one canonical primary implementation binding for rule rule_start",
        ),
        (
            "absolute file",
            vec![{
                let mut record = binding("implementation_binding_start", "rule_start");
                record["file"] = json!("/tmp/runtime.ts");
                record
            }],
            "implementation binding implementation_binding_start file must be repository-relative",
        ),
        (
            "empty symbol",
            vec![{
                let mut record = binding("implementation_binding_start", "rule_start");
                record["symbol"] = json!(" ");
                record
            }],
            "implementation binding implementation_binding_start symbol must not be empty",
        ),
        (
            "parent file",
            vec![{
                let mut record = binding("implementation_binding_start", "rule_start");
                record["file"] = json!("../runtime.ts");
                record
            }],
            "implementation binding implementation_binding_start file must be repository-relative",
        ),
        (
            "platform-specific separator",
            vec![{
                let mut record = binding("implementation_binding_start", "rule_start");
                record["file"] = json!(r"src\runtime.ts");
                record
            }],
            "implementation binding implementation_binding_start file must be repository-relative",
        ),
        (
            "empty owner",
            vec![{
                let mut record = binding("implementation_binding_start", "rule_start");
                record["declared_by"] = json!(" ");
                record
            }],
            "implementation binding implementation_binding_start declared_by must not be empty",
        ),
    ];

    for (_name, records, expected) in cases {
        let directory = tempfile::tempdir().unwrap();
        init(directory.path());
        create_rule(directory.path(), "rule_start");
        write_implementation_bindings(directory.path(), &records);
        provenance()
            .args(["check", "--repo", directory.path().to_str().unwrap()])
            .assert()
            .failure()
            .stderr(contains(expected));
    }
}

#[test]
fn check_rejects_a_platform_specific_verification_path() {
    let directory = tempfile::tempdir().unwrap();
    init(directory.path());
    create_rule(directory.path(), "rule_start");
    let path = directory
        .path()
        .join(".provenance/state/scopes/default/verifications/binding.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let record = json!({
        "schema_version": 1,
        "scope_id": "default",
        "id": "verification_binding_start",
        "rule_id": "rule_start",
        "key": "start",
        "method": "examples",
        "declared_by": "test://typescript",
        "file": r"tests\runtime.test.ts",
        "symbol": "workflow starts"
    });
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string(&record).unwrap()),
    )
    .unwrap();

    provenance()
        .args(["check", "--repo", directory.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains(
            "verification binding verification_binding_start file must be repository-relative",
        ));
}
