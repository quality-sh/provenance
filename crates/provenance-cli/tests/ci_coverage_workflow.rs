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
    job_block(workflow, "  rule-coverage:", "\n  rust-sdk:")
}

fn job_block(workflow: &str, start_marker: &str, end_marker: &str) -> String {
    let start = workflow
        .split_once(start_marker)
        .unwrap_or_else(|| panic!("the {start_marker} job exists"))
        .1;
    start
        .split_once(end_marker)
        .unwrap_or_else(|| panic!("{start_marker} is followed by {end_marker}"))
        .0
        .to_string()
}

fn condition_between(block: &str, end_marker: &str) -> String {
    block
        .split_once("if:")
        .unwrap_or_else(|| panic!("the job declares a condition"))
        .1
        .split_once(end_marker)
        .unwrap_or_else(|| panic!("the condition ends at {end_marker}"))
        .0
        .to_string()
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
    let condition = condition_between(&job, "runs-on:");
    assert!(
        condition.contains("needs.changes.outputs.policy == 'true'"),
        "graph-only pull requests must run the coverage check: {condition}"
    );
}

/// A policy-only pull request skips the rust and sdk jobs, so every job in
/// the chain rule-coverage hangs from must run for the same change set:
/// generate-operations builds the operation source review-assets composes
/// from, review-assets composes the review assets the CLI build restores,
/// and build-cli-linux produces the CLI artifact rule-coverage downloads.
#[test]
fn a_policy_only_change_builds_the_artifacts_rule_coverage_downloads() {
    let workflow = workflow();
    let generate = job_block(&workflow, "  generate-operations:", "\n  review-assets:");
    let generate_condition = condition_between(&generate, "runs-on:");
    assert!(
        generate_condition.contains("needs.changes.outputs.policy == 'true'"),
        "review-assets composes from the operation-generated artifact, so its source must build too: {generate_condition}"
    );
    let review = job_block(&workflow, "\n  review-assets:", "\n  # One build per OS.");
    let review_condition = condition_between(&review, "uses:");
    assert!(
        review_condition.contains("needs.changes.outputs.policy == 'true'"),
        "the CLI build restores the composed review assets, so their job must run too: {review_condition}"
    );
    let build = job_block(&workflow, "  build-cli-linux:", "\n  build-cli-platforms:");
    let build_condition = condition_between(&build, "runs-on:");
    assert!(
        build_condition.contains("needs.changes.outputs.policy == 'true'"),
        "the linux CLI build must produce the artifact whenever rule-coverage can run: {build_condition}"
    );
    let coverage = rule_coverage_job(&workflow);
    assert!(
        coverage.contains("needs: [changes, build-cli-linux]"),
        "rule-coverage must wait for the builder it downloads from: {coverage}"
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

/// GitHub renders at most ten warning annotations per step, and the
/// repository holds more findings than that. The step stops at the cap and
/// the tail annotation names where the full list lives.
#[test]
fn annotations_stop_at_the_github_cap_and_name_where_the_rest_live() {
    let job = rule_coverage_job(&workflow());
    let reporting = job
        .split_once("Rule binding findings")
        .expect("a reporting step names the findings")
        .1;
    assert!(
        reporting.contains(".[:10]"),
        "annotations must stop at the ten GitHub renders per step: {reporting}"
    );
    assert!(
        reporting.contains("more Rule binding finding"),
        "the tail must say how many findings the summary and artifact hold: {reporting}"
    );
}

#[test]
fn pull_requests_render_the_comparison_report_without_failing_the_job() {
    let job = rule_coverage_job(&workflow());
    assert!(
        job.contains("fetch-depth: 0"),
        "the report builder must have the pull request base commit: {job}"
    );

    let report = job
        .split_once("Build and render the pull request report")
        .expect("the job builds and renders a pull request report")
        .1;
    assert!(
        report.contains("if: github.event_name == 'pull_request'"),
        "the comparison report must run only for pull requests: {report}"
    );
    assert!(
        report.contains("continue-on-error: true"),
        "the new report must warn without failing the job: {report}"
    );
    assert!(
        report.contains("BASE_SHA: ${{ github.event.pull_request.base.sha }}")
            && report.contains("HEAD_SHA: ${{ github.event.pull_request.head.sha }}")
            && report.contains("REPOSITORY: ${{ github.repository }}"),
        "event values must enter the script through its environment: {report}"
    );
    assert!(
        report.contains("git fetch --no-tags origin \"$HEAD_SHA\"")
            && report.contains("git merge-base \"$BASE_SHA\" \"$HEAD_SHA\""),
        "the report must compare from the fetched pull request merge base: {report}"
    );
    assert!(
        report.contains("report build")
            && report.contains("--base \"$REPORT_BASE\"")
            && report.contains("--head \"$HEAD_SHA\"")
            && report.contains("--repository \"$REPOSITORY\""),
        "the envelope must compare the merge base with the pull request head: {report}"
    );
    assert!(
        !report
            .split_once("run: |")
            .expect("the report step has a shell script")
            .1
            .contains("${{"),
        "event expressions must not be inlined in the shell script: {report}"
    );
    assert!(
        report.contains("$RUNNER_TEMP/provenance-report-envelope.json")
            && report.contains("$RUNNER_TEMP/provenance-coverage-report.json"),
        "report inputs and outputs must not dirty the repository: {report}"
    );
    assert!(
        report.contains("report render") && report.contains("GITHUB_STEP_SUMMARY"),
        "the rendered report must appear in the job summary: {report}"
    );
    assert!(
        report.contains("select(.comparison == \"new\")") && report.contains("::warning"),
        "each new finding must produce a workflow annotation: {report}"
    );
}
