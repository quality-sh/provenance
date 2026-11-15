//! The contract router and the read operations port, in one test binary.
//!
//! - `characterization`: the answers of the operations that the review page
//!   calls, over a folder, against a checked transcript.
//! - `any_storage`: the same calls over memory storage, through the port,
//!   give the answers of the folder.
//! - `failures`: the failures through the port, the work slot and the
//!   preparation before it, and the cursor of a second page.
//! - `answers`: the check of each answer that a port gives.
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
