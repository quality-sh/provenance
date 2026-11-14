//! The server tests of provenance-mcp, compiled as one test binary.
//!
//! Each module is a test file in `tests/`. Each test binary links the full
//! dependency tree, so a small number of binaries keeps the coverage build
//! small. A support module that more than one file uses is declared here
//! once, and those files import it.

#[allow(dead_code)]
#[path = "../support"]
mod support {
    #[cfg(feature = "test-fixture")]
    #[path = "../../../provenance-transport/tests/support/api_fixture.rs"]
    pub mod api_fixture;
    #[cfg(feature = "test-fixture")]
    pub mod api_session;
    #[path = "../../../provenance-transport/tests/support/check_port.rs"]
    pub mod check_port;
    #[cfg(feature = "test-fixture")]
    pub mod fixture_host;
    #[cfg(feature = "test-fixture")]
    #[path = "../../../provenance-transport/tests/support/gate.rs"]
    pub mod gate;
    #[cfg(feature = "test-fixture")]
    pub mod mcp;
    #[cfg(feature = "test-fixture")]
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
    #[cfg(feature = "test-fixture")]
    #[path = "../../../provenance-transport/tests/support/resource_http.rs"]
    pub mod resource_http;
    #[cfg(feature = "test-fixture")]
    pub mod schema_check;
}
#[path = "../claim_holder.rs"]
mod claim_holder;
#[path = "../host_invocation.rs"]
mod host_invocation;
#[path = "../mcp_action_fields.rs"]
mod mcp_action_fields;
#[path = "../mcp_change_results.rs"]
mod mcp_change_results;
#[path = "../mcp_error_envelopes.rs"]
mod mcp_error_envelopes;
#[path = "../mcp_guidance.rs"]
mod mcp_guidance;
#[path = "../mcp_plumbing_names.rs"]
mod mcp_plumbing_names;
#[path = "../mcp_read_tools_do_not_write.rs"]
mod mcp_read_tools_do_not_write;
#[path = "../mcp_schema_dialect.rs"]
mod mcp_schema_dialect;
#[path = "../mcp_tool_schema_size.rs"]
mod mcp_tool_schema_size;
#[path = "../mcp_unknown_field_names_fields.rs"]
mod mcp_unknown_field_names_fields;
#[path = "../mcp_value_refusals.rs"]
mod mcp_value_refusals;
#[path = "../mcp_wire_characterization.rs"]
mod mcp_wire_characterization;
#[path = "../porcelain_api.rs"]
mod porcelain_api;
#[path = "../porcelain_api_discovery.rs"]
mod porcelain_api_discovery;
#[path = "../porcelain_authoring.rs"]
mod porcelain_authoring;
#[path = "../porcelain_authorization.rs"]
mod porcelain_authorization;
#[path = "../porcelain_bindings.rs"]
mod porcelain_bindings;
#[path = "../porcelain_check_statement.rs"]
mod porcelain_check_statement;
#[path = "../porcelain_contracts.rs"]
mod porcelain_contracts;
#[path = "../porcelain_discussion.rs"]
mod porcelain_discussion;
#[path = "../porcelain_schema_variants.rs"]
mod porcelain_schema_variants;
#[path = "../porcelain_search.rs"]
mod porcelain_search;
#[path = "../query_response_budgets.rs"]
mod query_response_budgets;
#[path = "../refusal_characterization.rs"]
mod refusal_characterization;
#[path = "../refusal_guidance.rs"]
mod refusal_guidance;
#[path = "../resources.rs"]
mod resources;
#[path = "../resources_patches.rs"]
mod resources_patches;
#[path = "../review_write_transports.rs"]
mod review_write_transports;
#[path = "../surface_names.rs"]
mod surface_names;
#[path = "../typed_plan_refusals.rs"]
mod typed_plan_refusals;
#[path = "../wire.rs"]
mod wire;
