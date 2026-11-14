//! The checks tests of provenance-cli, compiled as one test binary.
//!
//! Each module is a test file in `tests/`. Each test binary links the full
//! dependency tree, so a small number of binaries keeps the coverage build
//! small. A support module that more than one file uses is declared here
//! once, and those files import it.

#[path = "../support"]
mod support {
    pub mod review_paths;
}
#[path = "../review_host_support/mod.rs"]
mod review_host_support;
#[path = "../support/stale_repo.rs"]
mod stale_repo;
use support::review_paths;
#[path = "../cli_binding_policy.rs"]
mod cli_binding_policy;
#[path = "../cli_check.rs"]
mod cli_check;
#[path = "../cli_check_gaps.rs"]
mod cli_check_gaps;
#[path = "../cli_check_porcelain.rs"]
mod cli_check_porcelain;
#[path = "../cli_check_threads.rs"]
mod cli_check_threads;
#[path = "../cli_comment_binding.rs"]
mod cli_comment_binding;
#[path = "../cli_coverage.rs"]
mod cli_coverage;
#[path = "../cli_coverage_anchors.rs"]
mod cli_coverage_anchors;
#[path = "../cli_coverage_parse_warnings.rs"]
mod cli_coverage_parse_warnings;
#[path = "../cli_coverage_regex.rs"]
mod cli_coverage_regex;
#[path = "../cli_coverage_scopes.rs"]
mod cli_coverage_scopes;
#[path = "../cli_dangling_targets.rs"]
mod cli_dangling_targets;
#[path = "../cli_implementation_binding_portability.rs"]
mod cli_implementation_binding_portability;
#[path = "../cli_reports.rs"]
mod cli_reports;
#[path = "../cli_rule_evidence_sites.rs"]
mod cli_rule_evidence_sites;
#[path = "../cli_rules.rs"]
mod cli_rules;
#[path = "../cli_stale_answers.rs"]
mod cli_stale_answers;
#[path = "../cli_stale_typed_implementations.rs"]
mod cli_stale_typed_implementations;
#[path = "../cli_stale_typed_verifications.rs"]
mod cli_stale_typed_verifications;
#[path = "../report_build.rs"]
mod report_build;
#[path = "../report_render_cli.rs"]
mod report_render_cli;
#[path = "../report_source_integrity.rs"]
mod report_source_integrity;
