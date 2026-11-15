//! Safe read refusals retain revision facts and logical unit names.
use camino::Utf8Path;
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
    #[error("{file} line {line}: {message}{}", hint_detail(hint.as_deref()))]
    InvalidStateFile {
        file: String,
        line: u64,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    },
    /// A read failure of no known kind. `cause` is the display chain of the
    /// native error. A path below a state directory in it is relative to that
    /// directory.
    #[error("read failed: {cause}")]
    ReadFailed { cause: String },
}

/// The part of the `invalid_state_file` display text that gives its hint.
fn hint_detail(hint: Option<&str>) -> String {
    hint.map_or_else(String::new, |hint| format!("; {hint}"))
}

/// The hint of a state file that does not parse: the two steps that correct
/// the state.
pub const STATE_CORRECTIONS: &str = "get .provenance/state from a branch or a commit where \
    Provenance can read it, or remove all records and settings of the repository: remove \
    .provenance/state and run provenance init";

/// A line of a record file, or the dictionary reference, that does not parse.
///
/// The fields are private, so a refusal gets its file only from [`Self::new`],
/// which never gives an absolute path, and its next step only from
/// [`Self::hint`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct UnparsableStateFile {
    file: String,
    line: u64,
    message: String,
    in_state: bool,
}

impl UnparsableStateFile {
    /// The file at `path` does not parse at `line`. `repository` is the root
    /// of the repository that holds the file, when one holds it. `message` is
    /// the parse message with no position.
    #[provenance_macros::rule("rule_state_parse_refusal_names_the_line")]
    pub fn new(path: &Utf8Path, repository: Option<&Utf8Path>, line: u64, message: String) -> Self {
        Self {
            file: shown_file(path, repository),
            line,
            message,
            in_state: below_state(path).is_some(),
        }
    }

    /// The file as a refusal gives it.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The line that does not parse, from 1.
    pub const fn line(&self) -> u64 {
        self.line
    }

    /// The parse message, with no position.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// For a file below a `.provenance/state` directory, the two steps that
    /// correct a state that Provenance cannot read. Each is true for a line in
    /// a format that this build does not read and for a line that a person
    /// changed. A file outside the state gets no hint.
    #[provenance_macros::rule("rule_state_parse_refusal_names_the_corrections")]
    pub const fn hint(&self) -> Option<&'static str> {
        if self.in_state {
            Some(STATE_CORRECTIONS)
        } else {
            None
        }
    }

    /// The `invalid_state_file` read failure of the file, with its next step.
    pub fn failure(&self) -> ReadFailure {
        ReadFailure::InvalidStateFile {
            file: self.file.clone(),
            line: self.line,
            message: self.message.clone(),
            hint: self.hint().map(str::to_owned),
        }
    }

    /// The first state file that does not parse in the cause chain of `error`.
    pub fn find<'a>(error: &'a (dyn std::error::Error + 'static)) -> Option<&'a Self> {
        std::iter::successors(Some(error), |cause| cause.source())
            .find_map(|cause| cause.downcast_ref::<Self>())
    }
}

impl std::fmt::Display for UnparsableStateFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.failure(), formatter)
    }
}

/// The path of a file as a refusal gives it, with `/` between its parts: the
/// path below the `.provenance/state` directory that holds the file, else the
/// path below `repository`, else the file name.
pub fn shown_file(path: &Utf8Path, repository: Option<&Utf8Path>) -> String {
    if let Some(below) = below_state(path) {
        return below;
    }
    repository
        .and_then(|repository| path.strip_prefix(repository).ok())
        .filter(|below| {
            !below.as_str().is_empty()
                && below
                    .components()
                    .all(|part| matches!(part, camino::Utf8Component::Normal(_)))
        })
        .map(|below| below.iter().collect::<Vec<_>>().join("/"))
        .or_else(|| path.file_name().map(str::to_owned))
        .unwrap_or_default()
}

/// The path below the nearest `.provenance/state` directory above `path`, with
/// `/` between its parts, when such a directory is above it.
fn below_state(path: &Utf8Path) -> Option<String> {
    let parts: Vec<&str> = path.iter().collect();
    parts
        .windows(2)
        .rposition(|pair| pair == [".provenance", "state"])
        .map(|at| parts[at + 2..].join("/"))
}

impl ReadFailure {
    /// The response status of this failure. Each kind has one status.
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::ResourceNotFound => 404,
            Self::ReadFailed { .. } | Self::InvalidStateFile { .. } => 500,
            Self::FileAccessDenied => 403,
            Self::FileUnavailable | Self::GitUnavailable | Self::CacheCloseTimedOut { .. } => 503,
            _ => 409,
        }
    }
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
