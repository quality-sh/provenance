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
    /// The document read did not catch up the document. `message` is the
    /// cause as text, without its location. A state line that does not parse
    /// also gives its location and its hint, as `invalid_state_file` gives
    /// them.
    #[error("catch-up failed; restart the document read")]
    DocumentCatchUpFailed {
        #[serde(flatten, deserialize_with = "file_and_line")]
        location: Option<FileLine>,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
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
    #[error("{file} line {line}: {message}{}", hint_detail(hint.as_deref()))]
    InvalidStateFile {
        file: String,
        line: u64,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    },
    #[error(
        "a different process has the state file {file} open and does not let other processes \
         replace it: close the file in that process, and then send the call again"
    )]
    StateFileInUse { file: String },
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

/// The hint of a file below `.provenance/state` that does not parse and that
/// has no correction of its own: the two procedures that give a state that
/// reads, each with what it removes.
pub const STATE_CORRECTIONS: &str = "To correct this fault, use one of these two procedures. \
    Procedure 1: get .provenance/state from a commit where Provenance can read it. \
    This can remove records and changes that are not in that commit. \
    Procedure 2: remove .provenance/state and then do the command provenance init. \
    This removes all records, the scopes, the disposition actors and the technical names of \
    the dictionary.";

/// The location of a state line that does not parse.
///
/// A failure gives `file` and `line` together, or neither.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FileLine {
    /// The file that holds the line, as `invalid_state_file` gives it. It
    /// comes with `line`.
    pub file: String,
    /// The line that does not parse, from 1. It comes with `file`.
    pub line: u64,
}

/// Reads the location of a failure from its `file` and `line` keys. A body
/// with one of the two keys is refused, so that no key is lost.
fn file_and_line<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<FileLine>, D::Error> {
    #[derive(Deserialize)]
    struct Keys {
        file: Option<String>,
        line: Option<u64>,
    }
    let keys = Keys::deserialize(deserializer)?;
    match (keys.file, keys.line) {
        (Some(file), Some(line)) => Ok(Some(FileLine { file, line })),
        (None, None) => Ok(None),
        _ => Err(serde::de::Error::custom(
            "a failure gives file and line together, or neither",
        )),
    }
}

/// A line of a file of the state, such as a record file, the manifest or the
/// dictionary reference, that does not parse.
///
/// The fields are private, so a refusal gets its file only from [`Self::new`],
/// which never gives an absolute path, and its next step only from
/// [`Self::hint`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct UnparsableStateFile {
    file: String,
    line: u64,
    message: String,
    correction: Option<String>,
    in_state: bool,
}

impl UnparsableStateFile {
    /// The file at `path` does not parse at `line`. `repository` is the root
    /// of the repository that holds the file, when one holds it. `message` is
    /// the parse message with no position, and `correction` is the step that
    /// corrects the file, when the reader knows one.
    #[provenance_macros::rule("rule_state_parse_refusal_names_the_line")]
    pub fn new(
        path: &Utf8Path,
        repository: Option<&Utf8Path>,
        line: u64,
        message: String,
        correction: Option<String>,
    ) -> Self {
        Self {
            file: shown_file(path, repository),
            line,
            message,
            correction,
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

    /// The next step of each refusal for this line: the correction of the
    /// reader when it knows one, else [`STATE_CORRECTIONS`] for a file below a
    /// `.provenance/state` directory. A file outside the state with no
    /// correction of its own gets no hint.
    #[provenance_macros::rule("rule_state_parse_refusal_names_the_corrections")]
    pub fn hint(&self) -> Option<&str> {
        self.correction
            .as_deref()
            .or_else(|| self.in_state.then_some(STATE_CORRECTIONS))
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

    /// The `document_catch_up_failed` read failure of this line.
    ///
    /// It is the refusal of a document read whose catch-up stopped at this
    /// line, with the same hint as [`Self::failure`].
    pub fn catch_up_failure(&self) -> ReadFailure {
        ReadFailure::DocumentCatchUpFailed {
            location: Some(FileLine {
                file: self.file.clone(),
                line: self.line,
            }),
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
            Self::FileUnavailable
            | Self::GitUnavailable
            | Self::CacheCloseTimedOut { .. }
            | Self::StateFileInUse { .. } => 503,
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
