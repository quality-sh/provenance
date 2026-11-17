//! The actor that a write records.

use provenance_core::actor_input::ActorInput;
use provenance_core::protocol::failure::{InvalidInputReason, OperationFailure};
use provenance_macros::rule;

/// The actor that a write records when the request gives no actor.
pub const AUTHORING_ACTOR: &str = "authoring";

/// A Requirement creation, a Requirement update, a Discussion start, or a reply that gives
/// no actor records the actor `authoring`; a write that gives an actor records that actor.
#[rule("rule_write_without_actor_records_authoring")]
pub fn recorded_actor(given: ActorInput) -> String {
    given.0.unwrap_or_else(|| AUTHORING_ACTOR.to_owned())
}

/// The author of a review submission that records `actor`: none when the write
/// that made it gave no actor.
pub fn author(actor: Option<&str>) -> Option<&str> {
    actor.filter(|actor| *actor != AUTHORING_ACTOR)
}

/// The refusal of a write that must name its actor and gives none: `required` on
/// `request.actor`, as the decoder names a missing body field.
pub fn missing_actor() -> OperationFailure {
    OperationFailure::InvalidInput {
        field: Some("request.actor".to_owned()),
        reason: InvalidInputReason::Required,
    }
}

/// The actor of a write that must name its actor, or the refusal of [`missing_actor`] when
/// the request gives none.
pub fn named_actor(given: ActorInput) -> Result<String, OperationFailure> {
    given.0.ok_or_else(missing_actor)
}
