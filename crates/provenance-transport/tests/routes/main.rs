//! The routes tests of provenance-transport, compiled as one test binary.
//!
//! Each module is a test file in `tests/`. Each test binary links the full
//! dependency tree, so a small number of binaries keeps the coverage build
//! small. A support module that more than one file uses is declared here
//! once, and those files import it.

#[cfg(feature = "test-fixture")]
#[path = "../support"]
mod support {
    pub mod api_fixture;
    #[path = "../support/asserted_proposal.rs"]
    pub mod asserted_proposal;
    pub mod catch_up_steps;
    pub mod ideation;
    pub mod records;
    pub mod resource_http;
    pub mod transcript;
}
#[path = "../caller_refusal_kinds.rs"]
mod caller_refusal_kinds;
#[path = "../catch_up_characterization.rs"]
mod catch_up_characterization;
#[path = "../claim_holder.rs"]
mod claim_holder;
#[path = "../close_topic_body.rs"]
mod close_topic_body;
#[path = "../decision_actor_refusal.rs"]
mod decision_actor_refusal;
#[path = "../disagreeing_discussions.rs"]
mod disagreeing_discussions;
#[path = "../disagreeing_records.rs"]
mod disagreeing_records;
#[path = "../discussion_parent_routes.rs"]
mod discussion_parent_routes;
#[path = "../discussion_write_refusals.rs"]
mod discussion_write_refusals;
#[path = "../document_review_characterization.rs"]
mod document_review_characterization;
#[path = "../document_visibility.rs"]
mod document_visibility;
#[path = "../fallback_auth.rs"]
mod fallback_auth;
#[path = "../file_access.rs"]
mod file_access;
#[path = "../history_evidence.rs"]
mod history_evidence;
#[path = "../history_pages.rs"]
mod history_pages;
#[path = "../host_body_deadline.rs"]
mod host_body_deadline;
#[path = "../ideation_ids.rs"]
mod ideation_ids;
#[path = "../multiple_parents.rs"]
mod multiple_parents;
#[path = "../patch_contract.rs"]
mod patch_contract;
#[path = "../porcelain_api.rs"]
mod porcelain_api;
#[path = "../porcelain_api_discovery.rs"]
mod porcelain_api_discovery;
#[path = "../porcelain_route_inventory.rs"]
mod porcelain_route_inventory;
#[path = "../porcelain_target_registration.rs"]
mod porcelain_target_registration;
#[path = "../query_response_budgets.rs"]
mod query_response_budgets;
#[path = "../refusal_characterization.rs"]
mod refusal_characterization;
#[path = "../request_body_decode.rs"]
mod request_body_decode;
#[path = "../resources.rs"]
mod resources;
#[path = "../resources_patches.rs"]
mod resources_patches;
#[path = "../review_access_refusal.rs"]
mod review_access_refusal;
#[path = "../review_read_shard_errors.rs"]
mod review_read_shard_errors;
#[path = "../review_revisions.rs"]
mod review_revisions;
#[path = "../rework.rs"]
mod rework;
#[path = "../round2.rs"]
mod round2;
#[path = "../rule_patch_submissions.rs"]
mod rule_patch_submissions;
#[path = "../search_discovery.rs"]
mod search_discovery;
#[path = "../target_config.rs"]
mod target_config;
#[path = "../typed_spec_discussion_deletion.rs"]
mod typed_spec_discussion_deletion;
#[path = "../typed_spec_plan_refusals.rs"]
mod typed_spec_plan_refusals;
#[path = "../verification_runs_cache_lines.rs"]
mod verification_runs_cache_lines;
#[path = "../wire.rs"]
mod wire;
