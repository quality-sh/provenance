//! The notify request of the get review view: a client thread that gets the review
//! changes of one record.

use super::PortFuture;
use serde::{Deserialize, Serialize};

/// The refusal message when no review host serves the repository and scope.
pub const NO_REVIEW_HOST: &str = "no review host is running for this repository and scope";

/// The client that gets review changes: `codex` registers a thread, `off` ends the
/// registration of that thread.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotifyClient {
    Codex,
    Off,
}

/// One notify request for a client thread.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Notify {
    pub client: NotifyClient,
    /// The Codex thread UUID.
    #[schemars(length(min = 1))]
    pub thread: String,
}

/// The result of one notify request. `accepted` is true when Codex accepted the full
/// document message into the thread queue. It does not show that the agent read it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub client: NotifyClient,
    pub thread: String,
    pub registered: bool,
    pub accepted: bool,
    #[serde(default)]
    pub removed: bool,
}

/// Sends a notify request to the review host of the repository and scope.
pub trait NotifyPort: Send + Sync {
    fn register<'a>(
        &'a self,
        root: &'a str,
        limit: usize,
        notify: &'a Notify,
    ) -> PortFuture<'a, Registration>;
}
