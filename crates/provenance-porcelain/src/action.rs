//! Target-first action selection and readable results shared by consumers.

use crate::get::{GetPort, ReadError};
use provenance_core::protocol::RecordResolution;
use provenance_core::{NodeType, StableId, ThreadParent};
use provenance_macros::rule;
use serde_json::Value;
use std::fmt::Display;

pub use provenance_core::TargetAction as Action;

/// Describe one action for tool discovery.
pub const fn description(action: Action) -> &'static str {
    match action {
        Action::Create => "Create the target ID as an explicit record type.",
        Action::Update => "Update the existing target while preserving omitted fields.",
        Action::Answer => "Answer the target Question.",
        Action::Claim => "Claim the target Topic.",
        Action::Release => "Release the target Topic claim.",
        Action::Submit => "Submit the target Requirement for review.",
        Action::Discussions => "List addressed Discussions under the target record.",
        Action::Discussion => "Read the addressed Discussion.",
        Action::Discuss => "Start a Discussion under the target record.",
        Action::Reply => "Reply to the addressed Discussion.",
    }
}

/// An action target with a kind selected by its declaration or stored identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    pub action: Action,
    pub target: String,
    pub kind: NodeType,
}

/// A target-first action that cannot be selected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionError {
    InvalidOptions,
    KindSelection,
    NotFound,
    AmbiguousIdentity,
    AccessDenied,
    Operation(String),
}

impl Display for ActionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions => formatter.write_str("unsupported action options"),
            Self::KindSelection => formatter.write_str(
                "create requires --type and existing-record actions infer it without --type",
            ),
            Self::NotFound => formatter.write_str("record does not exist"),
            Self::AmbiguousIdentity => formatter.write_str("record ID is not unique"),
            Self::AccessDenied => formatter.write_str("access denied"),
            Self::Operation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ActionError {}

/// Check action options before a CLI opens a repository or a host resolves an ID.
pub fn validate_target(
    action: Action,
    target: &str,
    create_kind: Option<NodeType>,
) -> Result<(), ActionError> {
    if target.is_empty() {
        return Err(ActionError::InvalidOptions);
    }
    if (action == Action::Create) != create_kind.is_some() {
        return Err(ActionError::KindSelection);
    }
    Ok(())
}

impl<P: GetPort> crate::Porcelain<P> {
    /// Resolve an existing target or use the explicit kind of a new target.
    #[rule("rule_porcelain_create_names_new_record")]
    #[rule("rule_porcelain_existing_action_infers_kind")]
    pub async fn select_target(
        &self,
        action: Action,
        target: &str,
        create_kind: Option<NodeType>,
    ) -> Result<Target, ActionError> {
        validate_target(action, target, create_kind)?;
        let kind = if let Some(kind) = create_kind {
            kind
        } else {
            self.resolve_record_kind(target).await?
        };
        Ok(Target {
            action,
            target: target.to_owned(),
            kind,
        })
    }

    /// Resolve a graph parent for an action whose target is a graph record.
    pub async fn select_parent(&self, target: &str) -> Result<ThreadParent, ActionError> {
        let node_id = StableId::new(target).map_err(|_| ActionError::InvalidOptions)?;
        Ok(ThreadParent {
            node_type: self.resolve_record_kind(target).await?,
            node_id,
        })
    }

    async fn resolve_record_kind(&self, target: &str) -> Result<NodeType, ActionError> {
        match self
            .port
            .resolve(target)
            .await
            .map_err(map_read_error)?
            .result
        {
            RecordResolution::Found(record) => Ok(record.node_type()),
            RecordResolution::Missing => Err(ActionError::NotFound),
            RecordResolution::Ambiguous => Err(ActionError::AmbiguousIdentity),
        }
    }
}

fn map_read_error(error: ReadError) -> ActionError {
    match error {
        ReadError::InvalidOptions => ActionError::InvalidOptions,
        ReadError::NotFound => ActionError::NotFound,
        ReadError::AmbiguousIdentity => ActionError::AmbiguousIdentity,
        ReadError::Operation(message) => ActionError::Operation(message),
    }
}

/// Show the result of one registered action without changing its structured value.
pub fn render_readable(action: Action, target: &str, kind: NodeType, value: &Value) -> String {
    format!(
        "{} {} {}\n\n{}",
        action.as_str(),
        kind.as_str(),
        target,
        serde_json::to_string_pretty(value.get("data").unwrap_or(value))
            .expect("registered output is JSON")
    )
}
