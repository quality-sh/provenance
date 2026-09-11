//! The CI wiring for the lifecycle binding policy: a whole-repository scan
//! runs for code, graph-only, and configuration changes, and its findings
//! become visible annotations and a job summary.

use provenance_macros::verifies;
use std::fs;

fn workflow() -> String {
    fs::read_to_string(workspace_root().join(".github/workflows/ci.yml")).unwrap()
}

fn workspace_root() -> std::path::PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest)
        .ancestors()
        .nth(2)
        .expect("crate sits two levels below the workspace root")
        .to_path_buf()
}

fn rule_coverage_job(workflow: &str) -> String {
    let start = workflow
        .split_once("  rule-coverage:")
        .expect("the rule-coverage job exists")
        .1;
    let end = start
        .split_once("\n  rust-sdk:")
        .expect("rule-coverage is followed by the rust-sdk job");
    end.0.to_string()
}

#[test]
fn graph_only_changes_trigger_the_rule_coverage_job() {
    let workflow = workflow();
    let filters = workflow
        .split_once("filters: |")
        .expect("the changes job declares path filters")
        .1;
    assert!(
        filters.contains(".provenance/**"),
        "graph state and settings changes must trigger the coverage check: {filters}"
    );
    let changes = workflow
        .split_once("\n  changes:")
        .expect("changes job")
        .1
        .split_once("\n  generate-operations:")
        .expect("changes job ends at the next job")
        .0;
    assert!(
        changes.contains("policy: ${{ steps.filter.outputs.policy }}"),
        "the policy result must be an output of the changes job: {changes}"
    );
}

#[test]
fn the_rule_coverage_job_runs_for_policy_changes() {
    let job = rule_coverage_job(&workflow());
    let condition = job
        .split_once("if:")
        .expect("rule-coverage declares a condition")
        .1
        .split_once("runs-on:")
        .expect("the condition ends at runs-on")
        .0;
    assert!(
        condition.contains("needs.changes.outputs.policy == 'true'"),
        "graph-only pull requests must run the coverage check: {condition}"
    );
}

#[test]
#[verifies("rule_binding_finding_uses_configured_severity", examples)]
fn the_job_scans_the_whole_repository_under_the_configured_policy() {
    let job = rule_coverage_job(&workflow());
    let full_scan = job
        .lines()
        .find(|line| {
            line.contains("coverage scan")
                && line.contains("--path .")
                && line.contains("--validate-rules")
        })
        .expect("the job must run a whole-repository scan");
    assert!(
        !full_scan.contains("--strict"),
        "the whole-repository scan follows the configured policy, not a blanket strict flag: {full_scan}"
    );
    assert!(
        job.contains("--path crates") && job.contains("--path packages/create-provenance"),
        "the partial marker scans stay in place: {job}"
    );
}

#[test]
fn binding_findings_become_annotations_and_a_job_summary() {
    let job = rule_coverage_job(&workflow());
    let reporting = job
        .split_once("Rule binding findings")
        .expect("a reporting step names the findings")
        .1;
    assert!(
        reporting.contains("::warning"),
        "the step must emit visible GitHub annotations: {reporting}"
    );
    assert!(
        reporting.contains("binding_finding"),
        "the step reads the findings the policy governs: {reporting}"
    );
    assert!(
        reporting.contains("GITHUB_STEP_SUMMARY"),
        "the step must write a job summary a human sees: {reporting}"
    );
    assert!(
        job.contains("if: always()"),
        "findings are reported even when the configured policy fails the scan: {job}"
    );
    assert!(
        job.contains("provenance-coverage-report.json"),
        "the machine-readable report feeds the reporting step: {job}"
    );
}
