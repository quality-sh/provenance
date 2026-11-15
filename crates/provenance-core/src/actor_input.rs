//! The optional actor input of a Requirement creation or update, and of a Discussion start or
//! reply.

use serde::{Deserialize, Deserializer, Serializer};

/// The description of the optional actor input, in each request schema that has it.
pub const DESCRIPTION: &str = "The actor is optional for a Requirement creation or update, and \
     for a Discussion start or reply. A Requirement creation or update, or a Discussion start, \
     with no actor records the actor `authoring`.";

/// The optional actor of a write request: the given actor, or `None` when the request has no
/// actor member.
///
/// A field of this type takes `#[serde(default)]`. A JSON `null` actor is invalid input, as on
/// a write that needs its actor. The type has no `Serialize`, so the request schema gives the
/// field no `default`; a request type that serializes gives the field `skip_serializing_if =
/// "ActorInput::is_absent"` and `serialize_with = "ActorInput::serialize_given"`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActorInput(pub Option<String>);

impl ActorInput {
    /// True when the request gives no actor.
    pub fn is_absent(&self) -> bool {
        self.0.is_none()
    }

    /// Writes the given actor as a string, for a request type that serializes.
    pub fn serialize_given<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Some(actor) => serializer.serialize_str(actor),
            None => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for ActorInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|actor| Self(Some(actor)))
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for ActorInput {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ActorInput".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"description": DESCRIPTION, "type": "string"})
    }
}
