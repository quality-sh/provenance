use crate::state_store::UpdateRequirementInput;
use provenance_core::{SourceReference, StableId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One singleton reference edit: the record id sets it, JSON `null` clears it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum SingleEdit {
    Set(StableId),
    Clear,
}

impl<'de> Deserialize<'de> for SingleEdit {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Null => Ok(Self::Clear),
            value => StableId::deserialize(value)
                .map(Self::Set)
                .map_err(serde::de::Error::custom),
        }
    }
}

/// Reads one singleton field so JSON `null` reaches [`SingleEdit::Clear`]
/// instead of disappearing into an outer `Option`.
fn single_edit_field<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<SingleEdit>, D::Error> {
    SingleEdit::deserialize(deserializer).map(Some)
}

/// One list edit: an array is the complete final set; an object names a
/// partial delta of entries to add and entries to remove.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum ListEdit {
    Set(Vec<StableId>),
    Delta {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        add: Vec<StableId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        remove: Vec<StableId>,
    },
}

impl<'de> Deserialize<'de> for ListEdit {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Array(entries) => {
                Ok(Self::Set(parse(Value::Array(entries)).map_err(serde::de::Error::custom)?))
            }
            Value::Object(fields) => {
                let (add, remove) =
                    split_delta(fields, "relation").map_err(serde::de::Error::custom)?;
                Ok(Self::Delta {
                    add: parse(add).map_err(serde::de::Error::custom)?,
                    remove: parse(remove).map_err(serde::de::Error::custom)?,
                })
            }
            _ => Err(serde::de::Error::custom(
                "a relation list takes an array or an add/remove delta",
            )),
        }
    }
}

/// One citation edit: an array is the complete final set of citations; an
/// object names a partial delta that adds citations and removes every clause
/// of the sources it names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum CitesEdit {
    Set(Vec<SourceReference>),
    Delta {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        add: Vec<SourceReference>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        remove: Vec<StableId>,
    },
}

impl<'de> Deserialize<'de> for CitesEdit {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Array(entries) => {
                Ok(Self::Set(parse(Value::Array(entries)).map_err(serde::de::Error::custom)?))
            }
            Value::Object(fields) => {
                let (add, remove) =
                    split_delta(fields, "citation").map_err(serde::de::Error::custom)?;
                Ok(Self::Delta {
                    add: parse(add).map_err(serde::de::Error::custom)?,
                    remove: parse(remove).map_err(serde::de::Error::custom)?,
                })
            }
            _ => Err(serde::de::Error::custom(
                "cites takes an array or an add/remove delta",
            )),
        }
    }
}

/// Splits one delta object into its `add` and `remove` arrays and refuses any
/// other field, so a misspelled delta name never silently no-ops.
fn split_delta(
    mut fields: serde_json::Map<String, Value>,
    what: &str,
) -> Result<(Value, Value), anyhow::Error> {
    for name in fields.keys() {
        anyhow::ensure!(
            name == "add" || name == "remove",
            "unknown {what} delta field: {name}"
        );
    }
    let none = Value::Array(Vec::new());
    let add = fields.remove("add").unwrap_or_else(|| none.clone());
    let remove = fields.remove("remove").unwrap_or(none);
    Ok((add, remove))
}

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> anyhow::Result<T> {
    Ok(serde_json::from_value(value)?)
}

/// Relationship edits on one Requirement. Each field is absent (unchanged),
/// a complete final set, or a partial delta. The Store expands the edit
/// against the record's current state under the publication lock, and every
/// validation runs on the resulting final resource.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementRelations {
    #[serde(
        default,
        deserialize_with = "single_edit_field",
        skip_serializing_if = "Option::is_none"
    )]
    pub refines: Option<SingleEdit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<ListEdit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<ListEdit>,
    #[serde(
        default,
        deserialize_with = "single_edit_field",
        skip_serializing_if = "Option::is_none"
    )]
    pub spawned_by: Option<SingleEdit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cites: Option<CitesEdit>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveRequirement {
    pub request_id: StableId,
    pub actor: String,
    pub expected_etag: String,
    pub update: UpdateRequirementInput,
    pub relationships: Option<RequirementRelations>,
}
