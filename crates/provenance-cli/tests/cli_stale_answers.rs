//! Characterization of the stale query answers: the same sites, the same
//! order and the same refusals, from a new process and from one process
//! that answers many times, through the CLI, the store query, HTTP and MCP.

#[path = "cli_stale_answers/answers.rs"]
mod answers;
#[path = "cli_stale_answers/cases.rs"]
mod cases;
#[path = "cli_stale_answers/hosts.rs"]
mod hosts;
#[path = "cli_stale_answers/refusals.rs"]
mod refusals;
#[path = "review_host_support/mod.rs"]
mod review_host_support;
#[path = "support/review_paths.rs"]
mod review_paths;
#[path = "support/stale_repo.rs"]
mod stale_repo;
#[path = "cli_stale_answers/support.rs"]
mod support;
