//! The contract router and the read operations port, in one test binary.
//!
//! - `characterization`: the answers of the operations that the review page
//!   calls, over a folder, against a checked transcript.
//! - `any_storage`: the same calls over memory storage, through the port,
//!   give the answers of the folder; and so do the 18 history routes over
//!   the history contract's memory adapter of a folder's Git history.
//! - `failures`: the failures through the port, the work slot and the
//!   preparation before it, and the cursor of a second page.
//! - `answers`: the check of each answer that a port gives.
//! - `long_history`: the entry route past the first page of 200 versions
//!   over memory storage gives the answers over a folder's Git history.
//! - `history_pages`: a client that follows the cursor of the history route
//!   comes to the end when one version is larger than the page bytes.
//!
//! Implementation aids: no Rule in the graph covers the port. The plan
//! proposes `rule_contract_router_serves_any_storage` for the owner.
#![cfg(feature = "test-fixture")]

#[path = "router_port/answers.rs"]
mod answers;
#[path = "router_port/any_storage.rs"]
mod any_storage;
#[path = "router_port/characterization.rs"]
mod characterization;
#[path = "router_port/failures.rs"]
mod failures;
#[path = "router_port/history_pages.rs"]
mod history_pages;
#[path = "router_port/long_history.rs"]
mod long_history;

/// One test of this binary at a time. `any_storage` sets the home, the
/// temporary and the working directory of the process, and each other test
/// makes temporary directories. A runner that gives each test its own
/// process, as `cargo nextest` does, never waits here.
static ONE_AT_A_TIME: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(tokio::sync::Mutex::default);

/// Waits until no other test of this binary runs.
async fn one_at_a_time() -> tokio::sync::MutexGuard<'static, ()> {
    ONE_AT_A_TIME.lock().await
}

mod support {
    #[path = "../../../provenance-store/tests/support/history_cases.rs"]
    pub mod history_cases;
    #[path = "../../../provenance-store/tests/support/history_fixture.rs"]
    pub mod history_fixture;
    #[path = "../../../provenance-store/tests/support/history_folder.rs"]
    pub mod history_folder;
    #[path = "../../../provenance-store/tests/support/history_long.rs"]
    pub mod history_long;
    #[path = "../support/history_routes.rs"]
    pub mod history_routes;
    #[path = "../support/memory_host.rs"]
    pub mod memory_host;
    #[path = "../support/names.rs"]
    pub mod names;
    #[path = "../support/records.rs"]
    pub mod records;
    #[path = "../support/router_answer.rs"]
    pub mod router_answer;
    #[path = "../support/site_operations.rs"]
    pub mod site_operations;
    #[path = "../support/views.rs"]
    pub mod views;
}
