use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn cli_domains_roundtrip_materialize_and_export() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let import_repo = dir.path().join("imported").to_string_lossy().to_string();
    let export_path = dir.path().join("export.json").to_string_lossy().to_string();
    let import_export_path = dir
        .path()
        .join("import-export.json")
        .to_string_lossy()
        .to_string();

    init(&repo);
    create_domain(&repo);
    create_requirement(&repo);
    create_rule(&repo);
    verify_materialized_lists(&repo);

    // Creating a Requirement enrolls it in the review journal, and a
    // review-bearing scope refuses a lossy export until export carries
    // journal history.
    provenance(&[
        "export",
        "--repo",
        &repo,
        "--scope",
        "default",
        "--format",
        "json",
        "--output",
        &export_path,
    ])
    .failure()
    .stderr(contains(
        "review-bearing scopes require lossless import/export support",
    ));

    // A scope without a Requirement holds no journal history and still
    // exports and roundtrips.
    let plain_repo = dir.path().join("plain").to_string_lossy().to_string();
    init(&plain_repo);
    create_domain(&plain_repo);
    export_scope(&plain_repo, &export_path);
    assert_export_contains_domain_records(&export_path);
    import_export_roundtrip(&import_repo, &export_path, &import_export_path);
}

fn provenance(args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .assert()
}

fn init(repo: &str) {
    provenance(&[
        "init",
        "--path",
        repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ])
    .success();
}

fn create_domain(repo: &str) {
    provenance(&[
        "domains",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "domain_payroll",
        "--name",
        "Payroll",
        "--description",
        "Payroll compliance",
        "--color",
        "#3b82f6",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains("domain_payroll"))
    .stdout(contains(r##""color": "#3b82f6""##));
}

fn create_requirement(repo: &str) {
    provenance(&[
        "requirements",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "req_overtime",
        "--statement",
        "Overtime must be traceable",
        "--status",
        "discovery",
        "--domain-id",
        "domain_payroll",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains(r#""domain_id": "domain_payroll""#));
}

fn create_rule(repo: &str) {
    provenance(&[
        "rules",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "rule_overtime",
        "--requirement-id",
        "req_overtime",
        "--statement",
        "Pay overtime after threshold",
        "--status",
        "active",
        "--severity",
        "high",
        "--format",
        "json",
    ])
    .success();
}

fn verify_materialized_lists(repo: &str) {
    provenance(&["materialize", "--repo", repo, "--format", "json"])
        .success()
        .stdout(contains(r#""records_loaded""#));
    provenance(&[
        "domains", "list", "--repo", repo, "--scope", "default", "--format", "json",
    ])
    .success()
    .stdout(contains("domain_payroll"));
    provenance(&[
        "rules", "list", "--repo", repo, "--scope", "default", "--format", "json",
    ])
    .success()
    .stdout(contains("rule_overtime"));
}

fn export_scope(repo: &str, export_path: &str) {
    provenance(&[
        "export",
        "--repo",
        repo,
        "--scope",
        "default",
        "--format",
        "json",
        "--output",
        export_path,
    ])
    .success();
}

fn assert_export_contains_domain_records(export_path: &str) {
    let exported = std::fs::read_to_string(export_path).unwrap();
    assert!(exported.contains(r#""domains""#));
    assert!(exported.contains("domain_payroll"));
    assert!(!exported.contains(r#""services""#));
    assert!(!exported.contains(r#""service_bindings""#));
}

fn import_export_roundtrip(import_repo: &str, export_path: &str, import_export_path: &str) {
    init(import_repo);
    provenance(&[
        "import",
        "--repo",
        import_repo,
        "--scope",
        "default",
        "--input",
        export_path,
        "--format",
        "json",
    ])
    .success();
    export_scope(import_repo, import_export_path);

    let imported_export = std::fs::read_to_string(import_export_path).unwrap();
    assert!(imported_export.contains("domain_payroll"));
}
