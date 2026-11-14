//! The engine tests of provenance-store, compiled as one test binary.
//!
//! Each module is a test file in `tests/`. Each test binary links the full
//! dependency tree, so a small number of binaries keeps the coverage build
//! small. A support module that more than one file uses is declared here
//! once, and those files import it.

#[path = "../begin_verification_answers.rs"]
mod begin_verification_answers;
#[path = "../begin_verification_local_disk.rs"]
mod begin_verification_local_disk;
#[path = "../begin_verification_refusals.rs"]
mod begin_verification_refusals;
#[path = "../begin_verification_run_ids.rs"]
mod begin_verification_run_ids;
#[path = "../begin_verification_section.rs"]
mod begin_verification_section;
#[path = "../begin_verification_write_set.rs"]
mod begin_verification_write_set;
#[path = "../catch_up_lock.rs"]
mod catch_up_lock;
#[path = "../changed_statement_analysis.rs"]
mod changed_statement_analysis;
#[path = "../coverage_binding_settings.rs"]
mod coverage_binding_settings;
#[path = "../discussion_support.rs"]
mod discussion_support;
#[path = "../history_git_failures.rs"]
mod history_git_failures;
#[path = "../history_versions_contract.rs"]
mod history_versions_contract;
#[path = "../manifest_scope_ids.rs"]
mod manifest_scope_ids;
#[path = "../support/memory.rs"]
mod memory;
#[path = "../multiple_parents.rs"]
mod multiple_parents;
#[path = "../one_write_path.rs"]
mod one_write_path;
#[path = "../operation_schema.rs"]
mod operation_schema;
#[path = "../query_handles.rs"]
mod query_handles;
#[path = "../read_settings.rs"]
mod read_settings;
#[path = "../record_save_files.rs"]
mod record_save_files;
#[path = "../record_updates.rs"]
mod record_updates;
#[path = "../refusals_declare_a_kind.rs"]
mod refusals_declare_a_kind;
#[path = "../relation_deltas.rs"]
mod relation_deltas;
#[path = "../repository_open.rs"]
mod repository_open;
#[path = "../staged_view.rs"]
mod staged_view;
#[path = "../state_view.rs"]
mod state_view;
#[path = "../stray_state_entries.rs"]
mod stray_state_entries;
