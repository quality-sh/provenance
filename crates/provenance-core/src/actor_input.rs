//! The optional actor input of a write that makes or changes a Requirement, or that starts or
//! replies to a Discussion.

use serde::{Deserialize, Deserializer};

/// The description of the optional actor input, in each request schema that has it.
pub const DESCRIPTION: &str = "The actor is optional for a write that makes or changes a \
     Requirement, and for a Discussion start or reply. A Requirement write or a Discussion \
     start with no actor records the actor `authoring`.";

/// Reads a given actor. A given actor is a string, so a JSON `null` actor is invalid input
/// as on a write that needs its actor. A request with no actor member gets `None` from
/// `#[serde(default)]`; `skip_serializing_if = "Option::is_none"` on the field keeps that
/// default out of the request schema, which allows no `null`.
pub fn given<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}
