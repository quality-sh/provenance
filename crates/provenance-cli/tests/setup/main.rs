//! The setup tests of provenance-cli, compiled as one test binary.
//!
//! Each module is a test file in `tests/`. Each test binary links the full
//! dependency tree, so a small number of binaries keeps the coverage build
//! small. A support module that more than one file uses is declared here
//! once, and those files import it.

#[allow(dead_code)]
#[path = "../support"]
mod support {
    pub mod help_text;
    pub mod review_paths;
    pub mod state_files;
    pub mod surface;
}
#[path = "../bounded_process_report.rs"]
mod bounded_process_report;
#[path = "../bundled_guidance_commands.rs"]
mod bundled_guidance_commands;
#[path = "../cargo_dependency_overrides.rs"]
mod cargo_dependency_overrides;
#[path = "../cargo_init.rs"]
mod cargo_init;
#[path = "../cargo_shim.rs"]
mod cargo_shim;
#[path = "../cli_action_help.rs"]
mod cli_action_help;
#[path = "../cli_audit_guidance.rs"]
mod cli_audit_guidance;
#[path = "../cli_docs.rs"]
mod cli_docs;
#[path = "../cli_dogfood.rs"]
mod cli_dogfood;
#[path = "../cli_dogfood_absent.rs"]
mod cli_dogfood_absent;
#[path = "../cli_dogfood_regressions.rs"]
mod cli_dogfood_regressions;
#[path = "../cli_dogfood_triage.rs"]
mod cli_dogfood_triage;
#[path = "../cli_end_to_end.rs"]
mod cli_end_to_end;
#[path = "../cli_guidance_scope_and_etag.rs"]
mod cli_guidance_scope_and_etag;
#[path = "../cli_help.rs"]
mod cli_help;
#[path = "../cli_help_guidance.rs"]
mod cli_help_guidance;
#[path = "../cli_init.rs"]
mod cli_init;
#[path = "../cli_init_long_path.rs"]
mod cli_init_long_path;
#[path = "../cli_init_review_guidance.rs"]
mod cli_init_review_guidance;
#[path = "../cli_init_reviewer.rs"]
mod cli_init_reviewer;
#[path = "../cli_init_scope_guidance.rs"]
mod cli_init_scope_guidance;
#[path = "../cli_init_skill_guidance.rs"]
mod cli_init_skill_guidance;
#[path = "../cli_init_state.rs"]
mod cli_init_state;
#[path = "../cli_init_typescript_commands.rs"]
mod cli_init_typescript_commands;
#[path = "../cli_init_typescript_missing_package.rs"]
mod cli_init_typescript_missing_package;
#[allow(dead_code)]
#[path = "../cli_init_typescript/support.rs"]
mod cli_init_typescript_support;
#[path = "../cli_prime_guidance.rs"]
mod cli_prime_guidance;
#[path = "../cli_removed_commands.rs"]
mod cli_removed_commands;
#[path = "../cli_removed_fields.rs"]
mod cli_removed_fields;
#[path = "../cli_repo_context_help.rs"]
mod cli_repo_context_help;
#[path = "../cli_routing.rs"]
mod cli_routing;
#[path = "../cli_sdk_lifecycle.rs"]
mod cli_sdk_lifecycle;
#[path = "../command_surface.rs"]
mod command_surface;
#[allow(dead_code)]
#[path = "../cli_dictionary/support.rs"]
mod dictionary_support;
#[path = "../parser_foundation.rs"]
mod parser_foundation;
#[path = "../surface_names.rs"]
mod surface_names;
