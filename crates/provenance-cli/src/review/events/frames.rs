use provenance_store::review_changes::ScopeChanges;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(in crate::review) enum Frame {
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

impl Frame {
    pub fn position(&self) -> (&str, u64) {
        match self {
            Self::Ready { epoch, sequence }
            | Self::Changed {
                epoch, sequence, ..
            }
            | Self::Reset {
                epoch, sequence, ..
            } => (epoch, *sequence),
        }
    }
}
