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
    /// The document read did not catch up the document. For a state line
    /// that does not parse, `file`, `line` and `message` are those of
    /// `invalid_state_file`. A cause with no file has no `file` and no `line`,
    /// and `message` is the cause of the catch-up error.
    #[error("catch-up failed: {message}")]
    DocumentCatchUpFailed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<u64>,
        message: String,
    },
    #[error("repository file access denied")]
    FileAccessDenied,
    #[error("secure repository file access unavailable")]
    FileUnavailable,
    #[error("Git capability unavailable")]
    GitUnavailable,
    #[error("Git revision does not name a commit")]
    GitRevisionNotFound,
    #[error("the addressed resource does not exist")]
    ResourceNotFound,
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
    #[error("the cache close did not finish in time; retry the read")]
    CacheCloseTimedOut { stage: CacheCloseStage },
    #[error("{file} line {line}: {message}")]
    InvalidStateFile {
        file: String,
        line: u64,
        message: String,
    },
    /// A read failure of no known kind. `cause` is the display chain of the
    /// native error. A path below a state directory in it is relative to that
    /// directory.
    #[error("read failed: {cause}")]
    ReadFailed { cause: String },
}

/// What a cache close was waiting for when its deadline passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CacheCloseStage {
    /// Another close held the per-database close lock.
    CloseLock,
    /// The pool still held a connection.
    Teardown,
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
