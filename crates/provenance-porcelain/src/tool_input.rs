//! The input of an MCP tool that takes an action argument.
//!
//! The input schema is one flat object: the action enum and the fields of all
//! actions. The server then checks the fields of a call against its action, and a
//! refusal names the field to fix.

use provenance_core::protocol::failure::InvalidInputReason;
use serde_json::{json, Map, Value};

/// The fields of one action.
struct ActionFields {
    action: &'static str,
    properties: Map<String, Value>,
    required: Vec<String>,
}

/// The actions of one tool and the fields of each action.
pub struct ToolInput {
    actions: Vec<ActionFields>,
    definitions: Map<String, Value>,
    default: Option<&'static str>,
}

/// A call whose fields do not agree with its action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldRefusal {
    pub field: String,
    pub reason: InvalidInputReason,
    pub message: String,
}

impl FieldRefusal {
    /// The error object of the refusal.
    pub fn detail(&self) -> Value {
        json!({
            "kind": "invalid_input",
            "field": self.field,
            "reason": self.reason,
            "message": self.message,
        })
    }
}

impl ToolInput {
    /// A tool without actions. A call without an action argument selects `default`.
    pub fn new(default: Option<&'static str>) -> Self {
        Self {
            actions: Vec::new(),
            definitions: Map::new(),
            default,
        }
    }

    /// Add one action with the fields of an object schema. The definitions of the
    /// schema join the shared definitions; one name always names one schema.
    #[must_use]
    pub fn with_action(mut self, action: &'static str, mut schema: Value) -> Self {
        if let Some(Value::Object(definitions)) = schema
            .as_object_mut()
            .and_then(|object| object.remove("$defs"))
        {
            for (name, definition) in definitions {
                let held = self
                    .definitions
                    .entry(name.clone())
                    .or_insert_with(|| definition.clone());
                assert_eq!(
                    *held, definition,
                    "two schemas share the definition name {name}"
                );
            }
        }
        let properties = schema["properties"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        let required = schema["required"]
            .as_array()
            .map(|fields| {
                fields
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        self.actions.push(ActionFields {
            action,
            properties,
            required,
        });
        self
    }

    /// The action names, in order.
    pub fn actions(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.actions.iter().map(|fields| fields.action)
    }

    /// The flat input schema. Each field description names the actions that take it.
    #[provenance_macros::rule("rule_mcp_tool_inputs_are_flat_objects")]
    pub fn schema(&self) -> Value {
        let mut properties = Map::new();
        properties.insert(
            "action".into(),
            json!({"type": "string", "enum": self.actions().collect::<Vec<_>>()}),
        );
        if let Some(default) = self.default {
            properties["action"]["default"] = json!(default);
        }
        let mut users = Map::new();
        for fields in &self.actions {
            for (name, schema) in &fields.properties {
                match properties.get_mut(name) {
                    Some(held) => *held = merged(held, schema),
                    None => {
                        properties.insert(name.clone(), schema.clone());
                    }
                }
                users
                    .entry(name.clone())
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .expect("users are an array")
                    .push(json!(fields.action));
            }
        }
        for (name, actions) in users {
            let actions = actions
                .as_array()
                .expect("users are an array")
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>();
            let note = format!("Actions: {}.", actions.join(", "));
            let property = properties[&name]
                .as_object_mut()
                .expect("a property schema is an object");
            let description = match property.get("description").and_then(Value::as_str) {
                Some(text) => format!("{text} {note}"),
                None => note,
            };
            property.insert("description".into(), json!(description));
        }
        let required = if self.default.is_some() {
            json!([])
        } else {
            json!(["action"])
        };
        let mut schema = json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        });
        if !self.definitions.is_empty() {
            schema["$defs"] = Value::Object(self.definitions.clone());
        }
        schema
    }

    /// Select the action of one call and remove the action argument. The call is
    /// refused when the action is missing or unknown, when a field does not belong to
    /// the action, or when the action needs a field that the call does not give.
    #[provenance_macros::rule("rule_mcp_refusal_names_field")]
    pub fn select(&self, arguments: &mut Map<String, Value>) -> Result<&'static str, FieldRefusal> {
        let names = self.actions().collect::<Vec<_>>().join(", ");
        let action = match (arguments.remove("action"), self.default) {
            (None, Some(default)) => default,
            (None, None) => {
                return Err(FieldRefusal {
                    field: "action".into(),
                    reason: InvalidInputReason::Required,
                    message: format!("give an action: {names}"),
                })
            }
            (Some(Value::String(word)), _) => match self.actions().find(|action| *action == word) {
                Some(action) => action,
                None => {
                    return Err(FieldRefusal {
                        field: "action".into(),
                        reason: InvalidInputReason::InvalidValue,
                        message: format!("unknown action {word}; the actions are: {names}"),
                    })
                }
            },
            (Some(_), _) => {
                return Err(FieldRefusal {
                    field: "action".into(),
                    reason: InvalidInputReason::InvalidValue,
                    message: format!("the action is one of: {names}"),
                })
            }
        };
        let fields = self
            .actions
            .iter()
            .find(|fields| fields.action == action)
            .expect("the selected action has fields");
        if let Some(name) = arguments
            .keys()
            .find(|name| !fields.properties.contains_key(*name))
        {
            let accepted = fields
                .properties
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>();
            return Err(FieldRefusal {
                field: name.clone(),
                reason: InvalidInputReason::UnknownField,
                message: format!(
                    "the {action} action does not take {name}; it takes: {}",
                    accepted.join(", ")
                ),
            });
        }
        if let Some(name) = fields
            .required
            .iter()
            .find(|name| arguments.get(*name).is_none_or(Value::is_null))
        {
            return Err(FieldRefusal {
                field: name.clone(),
                reason: InvalidInputReason::Required,
                message: format!("the {action} action needs {name}"),
            });
        }
        Ok(action)
    }
}

/// One schema for a field that two actions declare. Schemas that differ only in a
/// null choice or a description are one schema: it allows null when one of them does,
/// and it keeps a description. Other differences become `anyOf`.
fn merged(held: &Value, other: &Value) -> Value {
    if shape(held) == shape(other) {
        let mut kept = if allows_null(other) && !allows_null(held) {
            other.clone()
        } else {
            held.clone()
        };
        let description = held.get("description").or_else(|| other.get("description"));
        if let (Some(object), Some(description)) = (kept.as_object_mut(), description) {
            object
                .entry("description")
                .or_insert_with(|| description.clone());
        }
        return kept;
    }
    let mut branches = match held.get("anyOf") {
        Some(Value::Array(branches))
            if held.as_object().is_some_and(|object| object.len() == 1) =>
        {
            branches.clone()
        }
        _ => vec![held.clone()],
    };
    if !branches.contains(other) {
        branches.push(other.clone());
    }
    json!({"anyOf": branches})
}

/// The schema without its null choice and its description.
fn shape(schema: &Value) -> Value {
    let mut shape = without_null(schema);
    if let Some(object) = shape.as_object_mut() {
        object.remove("description");
    }
    shape
}

fn allows_null(schema: &Value) -> bool {
    without_null(schema) != *schema
}

/// The schema without its null choice and without a null default.
fn without_null(schema: &Value) -> Value {
    let mut schema = schema.clone();
    let Some(object) = schema.as_object_mut() else {
        return schema;
    };
    if object.get("default") == Some(&Value::Null) {
        object.remove("default");
    }
    if let Some(Value::Array(types)) = object.get_mut("type") {
        types.retain(|kind| kind != "null");
        if types.len() == 1 {
            let only = types[0].clone();
            object.insert("type".into(), only);
        }
    }
    if let Some(Value::Array(branches)) = object.get("anyOf") {
        let kept = branches
            .iter()
            .filter(|branch| **branch != json!({"type": "null"}))
            .cloned()
            .collect::<Vec<_>>();
        if kept.len() == 1 {
            object.remove("anyOf");
            if let Value::Object(only) = &kept[0] {
                for (key, value) in only {
                    object.insert(key.clone(), value.clone());
                }
            }
        }
    }
    schema
}

#[cfg(test)]
mod tests;
