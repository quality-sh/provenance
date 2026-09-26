use assert_cmd::Command;
use provenance_store::operations::catalog;

fn help(arguments: &[&str]) -> String {
    let empty = tempfile::tempdir().expect("create empty working directory");
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .current_dir(empty.path())
        .args(arguments)
        .output()
        .expect("run provenance help");
    assert!(
        output.status.success(),
        "help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("help output is UTF-8")
}

/// One rendered operation block: a usage line through the next usage line.
fn block(output: &str, usage: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.trim() == usage)
        .unwrap_or_else(|| panic!("help lists no {usage} usage: {output}"));
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with("provenance "))
        .map_or(lines.len(), |offset| start + 1 + offset);
    lines[start..end].join("\n")
}

#[test]
fn immutable_collection_help_lists_only_registered_operations() {
    let output = help(&["verification-bindings", "--help"]);

    assert!(output.contains("provenance verification-bindings list"));
    assert!(output.contains("provenance verification-bindings <id> get"));
    assert!(output.contains("--rule <string>"));
    assert!(!output.contains("verification-bindings create"));
    assert!(!output.contains("verification-bindings <id> update"));
    assert!(!output.contains("verification-bindings <id> trace"));
}

#[test]
fn guarded_requirement_update_help_describes_real_inputs_and_controls() {
    let output = help(&["requirements", "--help"]);
    let update = block(&output, "provenance requirements <id> update");
    let description = catalog::definitions()
        .iter()
        .find(|definition| definition.operation_id == "updateRequirement")
        .expect("registered Requirement update")
        .description;

    assert!(update.contains(description), "{update}");
    assert!(update.contains("--statement <string>"), "{update}");
    assert!(update.contains("--status <"), "{update}");
    assert!(update.contains("--relationships-json <json>"), "{update}");
    assert!(update.contains("--actor <string>"), "{update}");
    assert!(update.contains("default: \"cli\""), "{update}");
    assert!(update.contains("--if-match <string>"), "{update}");
    assert!(update.contains("required"), "{update}");
    assert!(update.contains("--idempotency-key <string>"), "{update}");
    assert!(update.contains("generated if omitted"), "{update}");
    assert!(update.contains("--stdin"), "{update}");
}

#[test]
fn nested_resources_use_parent_owned_registered_addresses() {
    let output = help(&["proposals", "--help"]);

    assert!(output.contains("provenance proposals <id> assertions get"));
    assert!(output.contains("provenance proposals <id> assertions create"));
    assert!(output.contains("provenance proposals <id> assertions <fact_id> get"));
    assert!(output.contains("provenance proposals <id> dispositions get"));
    assert!(!output.contains("provenance assertions create"));
}

#[test]
fn query_help_shows_only_the_selected_query_options() {
    let output = help(&["requirements", "--help"]);
    let neighbors = block(&output, "provenance requirements <id> neighbors");
    let trace = block(&output, "provenance requirements <id> trace");

    assert!(
        neighbors.contains("--direction <out|in|both>"),
        "{neighbors}"
    );
    assert!(neighbors.contains("--limit <integer>"), "{neighbors}");
    assert!(!neighbors.contains("--max-depth"), "{neighbors}");
    assert!(!neighbors.contains("--text"), "{neighbors}");
    assert!(!neighbors.contains("--base"), "{neighbors}");
    assert!(!neighbors.contains("--head"), "{neighbors}");
    assert!(trace.contains("--max-depth <integer>"), "{trace}");
}

#[test]
fn operation_help_is_specific_to_the_selected_registration() {
    let output = help(&["requirements", "--help"]);
    let create = block(&output, "provenance requirements create");
    let update = block(&output, "provenance requirements <id> update");

    assert!(create.contains("--id <string>"), "{create}");
    assert!(
        create.contains("--depends-on <string> (repeatable)"),
        "{create}"
    );
    assert!(!create.contains("--if-match"), "{create}");
    assert!(!create.contains("--relationships-json"), "{create}");
    assert!(!update.contains("--id <string>"), "{update}");
}

#[test]
fn addressed_update_help_excludes_other_operations_inputs() {
    let output = help(&["requirements", "req_example", "update", "--help"]);

    assert!(
        output.contains("provenance requirements <id> update"),
        "{output}"
    );
    assert!(output.contains("--statement <string>"), "{output}");
    assert!(output.contains("--relationships-json <json>"), "{output}");
    assert!(output.contains("--if-match <string>"), "{output}");
    assert!(output.contains("--idempotency-key <string>"), "{output}");
    assert!(!output.contains("--id <string>"), "{output}");
    assert!(!output.contains("--depends-on"), "{output}");
    assert!(!output.contains("--max-depth"), "{output}");
    assert!(!output.contains("--direction"), "{output}");
    assert!(
        !output.contains("provenance requirements create"),
        "{output}"
    );
    assert!(!output.contains("provenance requirements list"), "{output}");
    assert!(!output.contains("Catalog commands for"), "{output}");
}

#[test]
fn addressed_query_help_shows_only_the_selected_query() {
    let neighbors = help(&["requirements", "req_example", "neighbors", "--help"]);
    let trace = help(&["requirements", "req_example", "trace", "--help"]);

    assert!(
        neighbors.contains("--direction <out|in|both>"),
        "{neighbors}"
    );
    assert!(neighbors.contains("--limit <integer>"), "{neighbors}");
    assert!(
        neighbors.contains("--relations <array<string>>"),
        "{neighbors}"
    );
    assert!(!neighbors.contains("--max-depth"), "{neighbors}");
    assert!(trace.contains("--max-depth <integer>"), "{trace}");
    assert!(!trace.contains("--text"), "{trace}");
    assert!(!trace.contains("--base"), "{trace}");
    assert!(!trace.contains("Catalog commands for"), "{trace}");
}

#[test]
fn addressed_create_help_excludes_update_only_controls() {
    let create = help(&["requirements", "create", "--help"]);

    assert!(
        create.contains("provenance requirements create"),
        "{create}"
    );
    assert!(create.contains("--id <string>"), "{create}");
    assert!(
        create.contains("--depends-on <string> (repeatable)"),
        "{create}"
    );
    assert!(!create.contains("--if-match"), "{create}");
    assert!(!create.contains("--relationships-json"), "{create}");
}

#[test]
fn nested_addressed_help_is_specific_to_the_nested_operation() {
    let output = help(&[
        "proposals",
        "prop_example",
        "assertions",
        "create",
        "--help",
    ]);

    assert!(
        output.contains("provenance proposals <id> assertions create"),
        "{output}"
    );
    assert!(
        output.contains("--synthesis-packet-id <string>"),
        "{output}"
    );
    assert!(!output.contains("dispositions"), "{output}");
    assert!(!output.contains("--if-match"), "{output}");
    assert!(!output.contains("Catalog commands for"), "{output}");
}

#[test]
fn addressed_help_honors_global_options_and_stays_repo_free() {
    let output = help(&[
        "--repo",
        ".",
        "requirements",
        "req_example",
        "update",
        "--help",
    ]);
    assert!(output.contains("--if-match <string>"), "{output}");
    let quiet = help(&["requirements", "--quiet", "req_example", "update", "--help"]);
    assert!(quiet.contains("--statement <string>"), "{quiet}");
}

#[test]
fn addressed_help_reports_unknown_addresses_as_usage_errors() {
    let empty = tempfile::tempdir().expect("create empty working directory");
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .current_dir(empty.path())
        .args(["requirements", "bogus", "--help"])
        .output()
        .expect("run provenance help");

    assert!(
        !output.status.success(),
        "unknown addresses must not render help"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not declare"), "{stderr}");
}

#[test]
fn a_declared_flag_value_can_be_the_literal_help_word() {
    let empty = tempfile::tempdir().expect("create empty working directory");
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .current_dir(empty.path())
        .args([
            "requirements",
            "create",
            "--id",
            "req_example",
            "--statement",
            "--help",
        ])
        .output()
        .expect("run provenance create");

    assert!(!output.status.success(), "the guarded write must not run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not initialized"), "{stderr}");
    let stdout = String::from_utf8(output.stdout).expect("create output is UTF-8");
    assert!(
        !stdout.contains("Catalog commands for requirements"),
        "{stdout}"
    );
}

#[test]
fn global_options_work_in_every_help_position() {
    for arguments in [
        vec!["--repo", ".", "requirements", "--help"],
        vec!["requirements", "--quiet", "--help"],
        vec![
            "--scope",
            "default",
            "--format",
            "json",
            "requirements",
            "--help",
        ],
    ] {
        let output = help(&arguments);
        assert!(
            output.contains("Catalog commands for requirements"),
            "{arguments:?}: {output}"
        );
    }
}
