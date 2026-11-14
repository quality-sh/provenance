//! The records tests of provenance-cli, compiled as one test binary.
//!
//! Each module is a test file in `tests/`. Each test binary links the full
//! dependency tree, so a small number of binaries keeps the coverage build
//! small. A support module that more than one file uses is declared here
//! once, and those files import it.

#[path = "../support"]
mod support {
    pub mod merge_driver;
    pub mod record_review;
    pub mod review_paths;
}
#[path = "../cli_approval_evidence.rs"]
mod cli_approval_evidence;
#[path = "../cli_archive_commit_plain_flag.rs"]
mod cli_archive_commit_plain_flag;
#[path = "../cli_cache_completion_deadline.rs"]
mod cli_cache_completion_deadline;
#[path = "../cli_closed_value_refusal.rs"]
mod cli_closed_value_refusal;
#[path = "../cli_collection_words.rs"]
mod cli_collection_words;
#[path = "../cli_content_change_submission.rs"]
mod cli_content_change_submission;
#[path = "../cli_dictionary.rs"]
mod cli_dictionary;
#[path = "../cli_dictionary_diagnostics.rs"]
mod cli_dictionary_diagnostics;
#[path = "../cli_disposition_schema.rs"]
mod cli_disposition_schema;
#[path = "../cli_enrichment.rs"]
mod cli_enrichment;
#[path = "../cli_export_stray_state.rs"]
mod cli_export_stray_state;
#[path = "../cli_graph_state.rs"]
mod cli_graph_state;
#[path = "../cli_id_charset_conformance.rs"]
mod cli_id_charset_conformance;
#[path = "../cli_import_exact_replacement.rs"]
mod cli_import_exact_replacement;
#[path = "../cli_import_record_count.rs"]
mod cli_import_record_count;
#[path = "../cli_import_threads.rs"]
mod cli_import_threads;
#[path = "../cli_import_unknown_fields.rs"]
mod cli_import_unknown_fields;
#[path = "../cli_input_boundaries.rs"]
mod cli_input_boundaries;
#[path = "../cli_json_refusals.rs"]
mod cli_json_refusals;
#[path = "../cli_manifest_diagnostics.rs"]
mod cli_manifest_diagnostics;
#[path = "../cli_merge_discussions.rs"]
mod cli_merge_discussions;
#[path = "../cli_merge_jsonl.rs"]
mod cli_merge_jsonl;
#[path = "../cli_merge_jsonl_preservation.rs"]
mod cli_merge_jsonl_preservation;
#[path = "../cli_merge_jsonl_refusals.rs"]
mod cli_merge_jsonl_refusals;
#[path = "../cli_merge_landing_batches.rs"]
mod cli_merge_landing_batches;
#[path = "../cli_multiple_parents.rs"]
mod cli_multiple_parents;
#[path = "../cli_origin.rs"]
mod cli_origin;
#[path = "../cli_planning_first_rules.rs"]
mod cli_planning_first_rules;
#[path = "../cli_read_freshness.rs"]
mod cli_read_freshness;
#[path = "../cli_record_format.rs"]
mod cli_record_format;
#[path = "../cli_record_updates.rs"]
mod cli_record_updates;
#[path = "../cli_refusal_characterization.rs"]
mod cli_refusal_characterization;
#[path = "../cli_refusal_discussions_word.rs"]
mod cli_refusal_discussions_word;
#[path = "../cli_refusal_guidance.rs"]
mod cli_refusal_guidance;
#[path = "../cli_refusal_next_step.rs"]
mod cli_refusal_next_step;
#[path = "../cli_registration.rs"]
mod cli_registration;
#[path = "../cli_resolution_decision_guard.rs"]
mod cli_resolution_decision_guard;
#[path = "../cli_resolution_refusal_guidance.rs"]
mod cli_resolution_refusal_guidance;
#[path = "../cli_schema_validate.rs"]
mod cli_schema_validate;
#[path = "../cli_shaping_archival.rs"]
mod cli_shaping_archival;
#[path = "../cli_shaping_guidance.rs"]
mod cli_shaping_guidance;
#[path = "../cli_shaping_question_guidance.rs"]
mod cli_shaping_question_guidance;
#[path = "../cli_shaping_question_updates.rs"]
mod cli_shaping_question_updates;
#[path = "../cli_shaping_recovery.rs"]
mod cli_shaping_recovery;
#[path = "../cli_shaping_roundtrip.rs"]
mod cli_shaping_roundtrip;
#[path = "../cli_shaping_turn_state.rs"]
mod cli_shaping_turn_state;
#[path = "../cli_shard_parse_errors.rs"]
mod cli_shard_parse_errors;
#[path = "../cli_source_requirement.rs"]
mod cli_source_requirement;
#[path = "../cli_statement_write_gate.rs"]
mod cli_statement_write_gate;
#[path = "../cli_ste_onboarding_concurrency.rs"]
mod cli_ste_onboarding_concurrency;
#[path = "../cli_ste_statement_paths.rs"]
mod cli_ste_statement_paths;
#[path = "../cli_swarm_backtrace_land.rs"]
mod cli_swarm_backtrace_land;
#[path = "../cli_time_range.rs"]
mod cli_time_range;
#[path = "../cli_typed_deletion_mixed_ids.rs"]
mod cli_typed_deletion_mixed_ids;
#[path = "../cli_typed_deletion_month_shards.rs"]
mod cli_typed_deletion_month_shards;
#[path = "../cli_typed_deletion_origins.rs"]
mod cli_typed_deletion_origins;
#[path = "../cli_unclassified_cause.rs"]
mod cli_unclassified_cause;
#[path = "../cli_unparsable_state.rs"]
mod cli_unparsable_state;
#[path = "../cli_unreadable_row.rs"]
mod cli_unreadable_row;
#[path = "../cli_update_guards.rs"]
mod cli_update_guards;
#[allow(dead_code)]
#[path = "../cli_dictionary/support.rs"]
mod dictionary_support;
#[path = "../export_fixture_support/mod.rs"]
mod export_fixture_support;
#[path = "../shaping_support/fixtures.rs"]
mod fixtures;
#[path = "../shaping_support/provenance.rs"]
mod provenance;
#[path = "../requirement_archive.rs"]
mod requirement_archive;
#[path = "../requirement_archive_merge.rs"]
mod requirement_archive_merge;
#[path = "../review_host_support/mod.rs"]
mod review_host_support;
#[path = "../support/thread_record.rs"]
mod thread_record;
