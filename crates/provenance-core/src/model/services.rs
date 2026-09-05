use provenance_macros::ProjectionRow;
use serde::{Deserialize, Serialize};

use super::ids::{SchemaVersion, ScopeId, StableId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ProjectionRow)]
#[table("domains")]
pub struct Domain {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}
