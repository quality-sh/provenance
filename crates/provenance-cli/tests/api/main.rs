//! The api tests of provenance-cli, compiled as one test binary.
//!
//! Each module is a test file in `tests/`. Each test binary links the full
//! dependency tree, so a small number of binaries keeps the coverage build
//! small. A support module that more than one file uses is declared here
//! once, and those files import it.

#[path = "../support/bounded_process.rs"]
mod bounded_process;
#[path = "../cli_api_catalog_bounds.rs"]
mod cli_api_catalog_bounds;
#[path = "../cli_api_changed_state.rs"]
mod cli_api_changed_state;
#[path = "../cli_catalog.rs"]
mod cli_catalog;
#[path = "../cli_catalog_structured_input.rs"]
mod cli_catalog_structured_input;
#[path = "../cli_catch_up_characterization.rs"]
mod cli_catch_up_characterization;
#[path = "../cli_get_edit_etag.rs"]
mod cli_get_edit_etag;
#[path = "../cli_get_trace.rs"]
mod cli_get_trace;
#[path = "../cli_mcp_api_catalog.rs"]
mod cli_mcp_api_catalog;
#[path = "../cli_mcp_read_parity.rs"]
mod cli_mcp_read_parity;
#[path = "../cli_mcp_record_actions.rs"]
mod cli_mcp_record_actions;
#[path = "../cli_mcp_server.rs"]
mod cli_mcp_server;
#[path = "../cli_mcp_shared_commands.rs"]
mod cli_mcp_shared_commands;
#[path = "../porcelain_api.rs"]
mod porcelain_api;
#[path = "../porcelain_authoring.rs"]
mod porcelain_authoring;
#[path = "../porcelain_authoring_support/mod.rs"]
mod porcelain_authoring_support;
#[path = "../porcelain_bindings.rs"]
mod porcelain_bindings;
#[path = "../porcelain_discussion.rs"]
mod porcelain_discussion;
#[path = "../porcelain_dispatch.rs"]
mod porcelain_dispatch;
#[path = "../porcelain_search.rs"]
mod porcelain_search;
#[path = "../porcelain_target_routing.rs"]
mod porcelain_target_routing;
#[path = "../search_lifecycle.rs"]
mod search_lifecycle;
#[path = "../support/search_repository.rs"]
mod search_repository;
#[path = "../search_text_first_page.rs"]
mod search_text_first_page;
#[path = "../search_text_pages.rs"]
mod search_text_pages;
