use provenance_macros::ProjectionRow;
use serde::{Deserialize, Serialize};

use crate::model::ids::{ScopeId, StableId};

/// The withdrawal of one review submission from review. It keeps the
/// submission, its feedback, and its decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ProjectionRow)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[table("withdrawals")]
pub struct Withdrawal {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub proposal_id: StableId,
    pub actor: String,
    pub reason: Option<String>,
}
