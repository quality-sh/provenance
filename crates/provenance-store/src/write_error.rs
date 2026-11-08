//! Source failures retain native diagnostics separately from the safe wire form.
use crate::state_store::readers::StateLineInvalid;
use crate::state_store::{
    ReconciledResource, StatementWriteError, TypedSpecDiagnostic, TypedSpecWriteError,
};
use provenance_core::protocol::failure::InvalidInputReason;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WriteFailure {
    // `InvalidInput` and `AccessDenied` are the common failure kinds of the
    // same name, raised by the store after decoding. Their wire shapes must
    // stay identical to the common ones, so they carry no doc comment: the
    // operation generator merges equal variants and refuses unequal twins.
    InvalidInput {
        field: Option<String>,
        reason: InvalidInputReason,
    },
    AccessDenied,
    AlreadyExists,
    InvalidCommitPin,
    ScopeMismatch,
    EmptyMessageBody,
    StatementInvalid {
        report: provenance_ste100::Report,
    },
    InvalidDeclaration,
    InvalidUpdate {
        message: String,
    },
    ApprovalEvidenceRequired {
        message: String,
    },
    ResolutionNotApproved {
        resolution_id: String,
        message: String,
    },
    RequirementEditConflict {
        current_etag: String,
    },
    RecordEditConflict {
        record_kind: provenance_core::NodeType,
        current_etag: String,
    },
    ReviewSubmissionConflict {
        current_submission: Option<provenance_core::StableId>,
        current_revision: provenance_core::StableId,
    },
    RecordOwnershipConflict {
        owner: Option<String>,
        message: String,
    },
    OwnershipConflict {
        conflicts: Vec<ReconciledResource>,
    },
    EnrolledRecordDeletionConflict {
        record_kind: provenance_core::NodeType,
        record_id: provenance_core::StableId,
    },
    ClaimConflict {
        holder: String,
    },
    MissingReference,
    ResourceNotFound,
    DiscussionVersionConflict,
    DiscussionMembershipMismatch,
    DiscussionClosed,
    DiscussionResolved,
    DiscussionIntentChanged,
    StatementRejected {
        diagnostics: Vec<TypedSpecDiagnostic>,
    },
    InvalidVerificationTarget,
    InvalidCompletion,
    AlreadyComplete,
    FileAccessDenied,
    FileUnavailable,
    RecordTooLarge,
    // `InvalidStateFile` has the same wire shape as the read failure of the
    // same name, so it carries no doc comment either.
    InvalidStateFile {
        file: String,
        line: u64,
        message: String,
    },
    /// A write failure of no known kind. `cause` is the display chain of the
    /// native error. A path below a state directory in it is relative to that
    /// directory.
    WriteFailed {
        cause: String,
    },
}

impl WriteFailure {
    /// Refuses an update and gives the top-level text of `source` as the
    /// reason. Nested causes stay in the native diagnostic.
    #[provenance_macros::rule("rule_invalid_update_refusal_gives_reason")]
    pub(crate) fn invalid_update(source: impl Into<anyhow::Error>) -> anyhow::Error {
        let source = source.into();
        let message = source.to_string();
        SourceFailure::wrap(Self::InvalidUpdate { message }, source)
    }

    pub(crate) fn invalid_field(field: &str) -> Self {
        Self::InvalidInput {
            field: Some(field.to_owned()),
            reason: InvalidInputReason::InvalidValue,
        }
    }

    #[provenance_macros::rule("rule_ownership_refusal_names_owner")]
    pub(crate) fn record_ownership_conflict(owner: Option<&str>) -> Self {
        let message = owner.map_or_else(
            || "This is a manual record; declared-by claims ownership. CLI: omit --declared-by; MCP: omit declared_by.".to_owned(),
            |owner| format!("Record owner: {owner}; declared-by claims ownership. Set CLI --declared-by or MCP declared_by to {owner} when you act for them, or ask the owner to make the change."),
        );
        Self::RecordOwnershipConflict {
            owner: owner.map(str::to_owned),
            message,
        }
    }
}

/// Every write request names the actor who makes it. A blank actor is a
/// client fault, named as the decoder names body fields.
#[provenance_macros::rule("rule_write_actor_required")]
pub(crate) fn ensure_actor(actor: &str) -> anyhow::Result<()> {
    if actor.trim().is_empty() {
        return Err(SourceFailure::wrap(
            WriteFailure::invalid_field("request.actor"),
            anyhow::anyhow!("actor must not be blank"),
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub struct SourceFailure {
    pub failure: WriteFailure,
    pub source: anyhow::Error,
}
impl std::fmt::Display for SourceFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.source, f)
    }
}
impl std::error::Error for SourceFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.source()
    }
}
impl SourceFailure {
    pub fn wrap(failure: WriteFailure, source: impl Into<anyhow::Error>) -> anyhow::Error {
        let source = source.into();
        if source.downcast_ref::<Self>().is_some()
            || source.downcast_ref::<PublicationStarted>().is_some()
        {
            return source;
        }
        Self { failure, source }.into()
    }
}

/// This marker takes precedence over any nested validation failure.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct PublicationStarted(pub anyhow::Error);
pub(crate) fn publication_started(error: anyhow::Error) -> anyhow::Error {
    if error.downcast_ref::<PublicationStarted>().is_some() {
        error
    } else {
        PublicationStarted(error).into()
    }
}

/// The error of a write set that did not land. A failure after landing
/// began is a [`PublicationStarted`]; any other failure landed no key.
pub(crate) fn publish_failure(failure: crate::storage::PublishFailure) -> anyhow::Error {
    match failure {
        crate::storage::PublishFailure::Started(error) => publication_started(error),
        crate::storage::PublishFailure::Refused(error) => error,
        moved @ crate::storage::PublishFailure::BaseMoved { .. } => moved.into(),
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct WriteError(pub anyhow::Error);
impl From<anyhow::Error> for WriteError {
    fn from(error: anyhow::Error) -> Self {
        Self(error)
    }
}
impl From<provenance_core::protocol::failure::OperationFailure> for WriteError {
    fn from(error: provenance_core::protocol::failure::OperationFailure) -> Self {
        Self(error.into())
    }
}
impl WriteError {
    pub fn safe(&self) -> WriteFailure {
        if self.0.downcast_ref::<PublicationStarted>().is_some() {
            return self.write_failed();
        }
        if let Some(error) = self.0.downcast_ref::<SourceFailure>() {
            return error.failure.clone();
        }
        if self
            .0
            .downcast_ref::<provenance_core::DispositionActorRefusal>()
            .is_some()
        {
            return WriteFailure::AccessDenied;
        }
        if self
            .0
            .downcast_ref::<provenance_core::ApprovalEvidenceRequired>()
            .is_some()
        {
            return WriteFailure::ApprovalEvidenceRequired {
                message: "An approval requires a prior assertion or a human canonical_artifact."
                    .into(),
            };
        }
        if let Some(error) = self.0.downcast_ref::<StatementWriteError>() {
            return WriteFailure::StatementInvalid {
                report: error.report.clone(),
            };
        }
        if let Some(error) = self.0.downcast_ref::<TypedSpecWriteError>() {
            return WriteFailure::StatementRejected {
                diagnostics: error.diagnostics.clone(),
            };
        }
        if let Some(error) = self
            .0
            .downcast_ref::<crate::operations::files::FileAccessRefusal>()
        {
            return match error {
                crate::operations::files::FileAccessRefusal::Denied => {
                    WriteFailure::FileAccessDenied
                }
                _ => WriteFailure::FileUnavailable,
            };
        }
        StateLineInvalid::find(&self.0).map_or_else(
            || self.write_failed(),
            |error| WriteFailure::InvalidStateFile {
                file: error.file(),
                line: error.line,
                message: error.message.clone(),
            },
        )
    }
    fn write_failed(&self) -> WriteFailure {
        WriteFailure::WriteFailed {
            cause: failure_cause(&self.0),
        }
    }
    /// The native cause chain of a failure of no known kind, with absolute
    /// paths, for the host log. The wire answer gives the cause with each
    /// state path relative to the state directory.
    pub fn unclassified_cause(&self) -> Option<String> {
        matches!(self.safe(), WriteFailure::WriteFailed { .. }).then(|| format!("{:#}", self.0))
    }
    pub fn status(&self) -> u16 {
        match self.safe() {
            WriteFailure::WriteFailed { .. } | WriteFailure::InvalidStateFile { .. } => 500,
            WriteFailure::FileAccessDenied | WriteFailure::AccessDenied => 403,
            WriteFailure::FileUnavailable => 503,
            WriteFailure::ResourceNotFound => 404,
            WriteFailure::RecordOwnershipConflict { .. }
            | WriteFailure::AlreadyExists
            | WriteFailure::OwnershipConflict { .. }
            | WriteFailure::EnrolledRecordDeletionConflict { .. }
            | WriteFailure::RequirementEditConflict { .. }
            | WriteFailure::RecordEditConflict { .. }
            | WriteFailure::ReviewSubmissionConflict { .. }
            | WriteFailure::DiscussionVersionConflict
            | WriteFailure::DiscussionClosed
            | WriteFailure::DiscussionResolved
            | WriteFailure::DiscussionIntentChanged
            | WriteFailure::ClaimConflict { .. }
            | WriteFailure::AlreadyComplete => 409,
            _ => 400,
        }
    }
}
/// The display chain of `error` for a refusal of no known kind. Each path
/// below a state directory is given relative to that directory, with `/`
/// between its parts, so the refusal holds no absolute state path. Other
/// text stays as the native error gives it.
pub(crate) fn failure_cause(error: &anyhow::Error) -> String {
    let text = format!("{error:#}");
    let mut cause = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some((at, marker)) = state_marker(rest) {
        let before = &rest[..at];
        cause.push_str(&before[..path_start(before)]);
        rest = &rest[at + marker..];
        let end = rest.find(ends_path).unwrap_or(rest.len());
        cause.push_str(&rest[..end].replace('\\', "/"));
        rest = &rest[end..];
    }
    cause.push_str(rest);
    cause
}

/// The first `.provenance/state/` directory in `text`, with either separator,
/// as its byte offset and length.
fn state_marker(text: &str) -> Option<(usize, usize)> {
    const MARKERS: [&str; 4] = [
        ".provenance/state/",
        ".provenance\\state\\",
        ".provenance/state\\",
        ".provenance\\state/",
    ];
    MARKERS
        .iter()
        .filter_map(|marker| text.find(marker).map(|at| (at, marker.len())))
        .min()
}

/// Where the path that ends `before` starts. An absolute path can hold a
/// space, so the last root (`/`, `\\` or a drive letter) after a space wins.
/// The search does not go back past `: ` or another separator. With no root,
/// the path starts after the last space.
fn path_start(before: &str) -> usize {
    let window = before
        .char_indices()
        .rev()
        .find(|&(_, character)| character != ' ' && ends_path(character))
        .map_or(0, |(at, character)| at + character.len_utf8())
        .max(before.rfind(": ").map_or(0, |at| at + 2));
    let bytes = before.as_bytes();
    let is_root = |index: usize| {
        let after_space = index == window || bytes[index - 1] == b' ';
        let root = match bytes[index] {
            b'/' | b'\\' => true,
            letter => {
                letter.is_ascii_alphabetic()
                    && bytes.get(index + 1) == Some(&b':')
                    && matches!(bytes.get(index + 2), Some(b'/' | b'\\'))
            }
        };
        after_space && root
    };
    (window..bytes.len())
        .rev()
        .find(|&index| is_root(index))
        .or_else(|| before[window..].rfind(' ').map(|at| window + at + 1))
        .unwrap_or(window)
}

const fn ends_path(character: char) -> bool {
    character.is_whitespace() || matches!(character, '"' | '\'' | '`' | '(' | ')' | ',' | ';')
}

impl Serialize for WriteError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.safe().serialize(serializer)
    }
}
#[cfg(feature = "schema")]
impl schemars::JsonSchema for WriteError {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "WriteFailure".into()
    }
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        WriteFailure::json_schema(generator)
    }
}

macro_rules! ensure {
    (InvalidUpdate, $condition:expr, $($message:tt)*) => {
        if !$condition { return Err($crate::write_error::WriteFailure::invalid_update(
            anyhow::anyhow!($($message)*))); }
    };
    ($kind:ident, $condition:expr, $($message:tt)*) => {
        if !$condition { return Err($crate::write_error::SourceFailure::wrap(
            $crate::write_error::WriteFailure::$kind, anyhow::anyhow!($($message)*))); }
    };
}
pub(crate) use ensure;
