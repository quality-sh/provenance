use provenance_store::review_changes::ScopeChanges;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(super) enum Frame {
    Ready {
        epoch: String,
        sequence: u64,
    },
    Changed {
        epoch: String,
        sequence: u64,
        scopes: Vec<ScopeChanges>,
    },
    Reset {
        epoch: String,
        sequence: u64,
        reason: String,
    },
}
