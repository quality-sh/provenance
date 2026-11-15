//! The actor that a write records.

use crate::write_error::WriteFailure;
use provenance_core::protocol::failure::InvalidInputReason;
use provenance_macros::rule;

/// The actor that a write records when the request gives no actor.
pub(crate) const AUTHORING_ACTOR: &str = "authoring";

/// A record write or a Discussion message that gives no actor records the actor
/// `authoring`; a write that gives an actor records that actor.
#[rule("rule_write_without_actor_records_authoring")]
pub(crate) fn recorded_actor(given: Option<String>) -> String {
    given.unwrap_or_else(|| AUTHORING_ACTOR.to_owned())
}

/// The actor of a write that names its actor, or a `required` refusal on `request.actor`
/// when the request gives none.
pub(crate) fn named_actor(given: Option<String>) -> anyhow::Result<String> {
    let required = WriteFailure::InvalidInput {
        field: Some("request.actor".to_owned()),
        reason: InvalidInputReason::Required,
    };
    given.ok_or_else(|| crate::refusal::refuse!((required), "the write needs an actor"))
}
