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
    let output = help(&["requirements", "req_example", "update", "--help"]);
    let description = catalog::definitions()
        .iter()
        .find(|definition| definition.operation_id == "updateRequirement")
        .expect("registered Requirement update")
        .description;

    assert!(output.contains(description));
    assert!(output.contains("provenance requirements <id> update"));
    assert!(output.contains("--statement <string>"));
    assert!(output.contains("--status <"));
    assert!(output.contains("--relationships-json <json>"));
    assert!(output.contains("--actor <string>"));
    assert!(output.contains("default: \"cli\""));
    assert!(output.contains("--clear-fields-json <json>"));
    assert!(output.contains("default: []"));
    assert!(output.contains("--if-match <string>"));
    assert!(output.contains("required"));
    assert!(output.contains("--idempotency-key <string>"));
    assert!(output.contains("generated if omitted"));
    assert!(output.contains("--stdin"));
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
    let output = help(&["requirements", "req_example", "neighbors", "--help"]);

    assert!(output.contains("--direction <in|out|both>"));
    assert!(output.contains("--limit <integer>"));
    assert!(output.contains("--max-depth <integer>"));
    assert!(!output.contains("--text"));
    assert!(!output.contains("--base"));
    assert!(!output.contains("--head"));
}

#[test]
fn operation_help_is_specific_to_the_selected_registration() {
    let create = help(&["requirements", "create", "--help"]);
    let update = help(&["requirements", "req_example", "update", "--help"]);

    assert!(create.contains("--id <string>"));
    assert!(create.contains("--depends-on <string> (repeatable)"));
    assert!(!create.contains("--if-match"));
    assert!(!create.contains("--relationships-json"));
    assert!(!update.contains("--id <string>"));
}
