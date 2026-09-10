//! Safe read refusals retain revision facts and logical unit names.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReadFailure {
    #[error("invalid cursor; restart the query")]
    CursorInvalid,
    #[error("projection revision changed; restart the query")]
    CursorRevisionChanged,
    #[error("page read work exceeds the engine budget")]
    PageBudgetExceeded,
    #[error("canonical record exceeds the page record budget")]
    PageRecordTooLarge,
    #[error("document root is missing")]
    DocumentRootMissing,
    #[error("document root is retired")]
    DocumentRootRetired,
    #[error("catch-up failed; restart the document read")]
    DocumentCatchUpFailed,
    #[error("repository file access denied")]
    FileAccessDenied,
    #[error("secure repository file access unavailable")]
    FileUnavailable,
    #[error("Git capability unavailable")]
    GitUnavailable,
    #[error("Git revision does not name a commit")]
    GitRevisionNotFound,
    #[error("no projection; run provenance materialize")]
    NoProjection,
    #[error("projection differs from canonical state")]
    Stale {
        serial: i64,
        digest: String,
        instance_id: String,
        moved: Vec<MovedUnit>,
    },
    #[error("cannot hash canonical unit {unit}")]
    UnitUnreadable { unit: String },
    #[error("projection schema needs materialization")]
    SchemaBehind,
    #[error("projection migration needs materialization")]
    HalfMigrated,
    #[error("read failed")]
    ReadFailed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct MovedUnit {
    pub unit: String,
    pub stored: String,
    pub live: String,
}

/// The stage that failed is known even when its lower-level error is not public.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FreshnessCause {
    CatchUpFailed,
}
