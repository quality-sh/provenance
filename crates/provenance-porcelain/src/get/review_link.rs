//! The review link of one record: where the running review host shows it.

use super::PortFuture;
use provenance_core::NodeType;
use serde::{Deserialize, Serialize};

/// The review link of a record, or the text that tells why there is no link.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewLink {
    /// The link to the record in the running review host.
    pub review_url: Option<String>,
    /// When no review host runs: the text that tells how to start one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Finds the review link of one record in the review host of the bound repository and
/// scope.
pub trait ReviewLinkPort: Send + Sync {
    fn link<'a>(&'a self, kind: NodeType, id: &'a str) -> PortFuture<'a, ReviewLink>;
}
