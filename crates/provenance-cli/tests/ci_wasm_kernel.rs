//! The CI wiring for the kernel crates: one job checks each crate that the
//! crate dependency policy names as a kernel crate, for the policy target and
//! with default features, and the required `CI OK` job needs that job. The
//! same job runs the memory adapter tests of the store without `host`, lints
//! every store target without `host` and runs the store library unit tests.

use provenance_macros::verifies;
use std::{fs, path::PathBuf};

fn workspace_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate sits two levels below the workspace root")
        .to_path_buf()
}

/// The workflow text with LF line ends. A Windows checkout can convert the
/// file to CRLF, and the job and filter markers below expect LF.
fn workflow() -> String {
    fs::read_to_string(workspace_root().join(".github/workflows/ci.yml"))
        .unwrap()
        .replace("\r\n", "\n")
}

/// The text of one top-level job, from its key to the next job key.
fn job(workflow: &str, name: &str) -> String {
    let start = workflow
        .split_once(&format!("\n  {name}:\n"))
        .unwrap_or_else(|| panic!("the {name} job exists"))
        .1;
    let end = start
        .match_indices("\n  ")
        .find(|(index, _)| {
            let rest = &start[index + 3..];
            !rest.starts_with(' ')
                && rest
                    .split_once(':')
                    .is_some_and(|(key, _)| !key.contains(' '))
        })
        .map_or(start.len(), |(index, _)| index);
    start[..end].to_string()
}

/// The text of one step of `job`, from its name to the next step.
fn step(job: &str, name: &str) -> String {
    let start = job
        .split_once(&format!("- name: {name}\n"))
        .unwrap_or_else(|| panic!("the {name} step exists:\n{job}"))
        .1;
    let step = start
        .split_once("\n      - ")
        .map_or(start, |(step, _)| step);
    // The last step of a job ends at the blank line before the next job.
    step.split_once("\n\n")
        .map_or(step, |(step, _)| step)
        .to_string()
}

#[test]
#[verifies("rule_kernel_crates_build_for_wasm", conformance)]
fn wasm_kernel_job_checks_each_policy_kernel_crate_for_the_policy_target() {
    let job = job(&workflow(), "wasm-kernel");

    for read in [
        "jq -r '.kernel.target' .config/crate-dependencies.json",
        "jq -r '.kernel.crates | join(\" \")' .config/crate-dependencies.json",
        "rustup target add \"$TARGET\"",
        "for crate in $CRATES; do",
        "cargo check --locked -p \"$crate\" --target \"$TARGET\"",
    ] {
        assert!(
            job.contains(read),
            "the wasm-kernel job runs `{read}`:\n{job}"
        );
    }
    assert!(
        !job.contains("--features") && !job.contains("--all-features"),
        "the wasm-kernel job checks the default features only:\n{job}"
    );
    let check = step(
        &job,
        "Check each kernel crate for the target with default features",
    );
    assert!(
        !check.contains("-p provenance-"),
        "the wasm-kernel job takes its crates from the policy, not from a list of its own:\n{check}"
    );
}

/// Implementation aid: no proposed Rule covers the CI wiring of the memory
/// adapter tests. The tests that this step runs carry the evidence of
/// `rule_engine_runs_on_any_storage`; this test shows that CI runs them
/// without `host`.
#[test]
fn wasm_kernel_job_runs_the_memory_adapter_tests_without_host() {
    let job = job(&workflow(), "wasm-kernel");
    let test = step(&job, "Test the store on the memory adapter without host");
    let command = test.split_whitespace().collect::<Vec<_>>().join(" ");

    for part in [
        "cargo test --locked -p provenance-store",
        "--test memory_repository",
        "--test memory_repository_stage_reads",
    ] {
        assert!(command.contains(part), "the step runs `{part}`:\n{test}");
    }
    assert!(
        !command.contains("--features") && !command.contains("--target"),
        "the step runs the native build with default features:\n{test}"
    );
}

/// Implementation aid: no proposed Rule covers the CI wiring of the store
/// targets without `host`. This test shows that CI lints every store target
/// and runs the library unit tests with default features.
#[test]
fn wasm_kernel_job_builds_every_store_target_without_host() {
    let job = job(&workflow(), "wasm-kernel");
    for (name, command) in [
        (
            "Lint all store targets without host",
            "cargo clippy --locked -p provenance-store --all-targets -- -D warnings",
        ),
        (
            "Test the store library without host",
            "cargo test --locked -p provenance-store --lib",
        ),
    ] {
        let step = step(&job, name);
        let run = step.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(
            run,
            format!("run: {command}"),
            "the step runs only `{command}`, the native build with default features:\n{step}"
        );
    }
}

#[test]
#[verifies("rule_kernel_crates_build_for_wasm", conformance)]
fn ci_ok_needs_the_wasm_kernel_job() {
    let workflow = workflow();
    let ci_ok = job(&workflow, "ci-ok");
    let needs = ci_ok
        .split_once("needs:")
        .expect("CI OK lists the jobs it needs")
        .1
        .split_once("if:")
        .expect("the needs list ends at the condition")
        .0;

    assert!(
        needs.lines().any(|line| line.trim() == "- wasm-kernel"),
        "CI OK needs the wasm-kernel job:\n{needs}"
    );
    assert!(
        ci_ok.contains("if: always()"),
        "CI OK runs when a needed job fails"
    );
}

#[test]
#[verifies("rule_kernel_crates_build_for_wasm", conformance)]
fn a_policy_change_runs_the_wasm_kernel_job() {
    let workflow = workflow();
    let rust_filter = workflow
        .split_once("\n            rust:\n")
        .expect("the changes job has a rust filter")
        .1
        .lines()
        .take_while(|line| line.starts_with("              "))
        .collect::<Vec<_>>()
        .join("\n");
    let job = job(&workflow, "wasm-kernel");

    assert!(
        rust_filter.contains("\".config/crate-dependencies.json\""),
        "a change to the crate dependency policy selects the rust jobs:\n{rust_filter}"
    );
    assert!(
        job.contains("needs.changes.outputs.rust == 'true'"),
        "{job}"
    );
}
