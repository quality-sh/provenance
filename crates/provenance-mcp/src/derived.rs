//! The MCP form of a porcelain command input.
//!
//! It is the schema of the typed input, with the documentation of each field as the
//! description of its property. A field name is already `snake_case`, so each property
//! has the field name.
//!
//! A read tool only reads, so a field that writes is not a property of the read tool.
//! The change tool has it as the properties of the action that writes.

use provenance_porcelain::registry::{Command, Field, Input};
use serde_json::{Map, Value};

fn fields(command: Command) -> (&'static [Field], Value) {
    let Input::Fields(fields) = command.declaration().input else {
        panic!("{command:?} has no typed input");
    };
    let schema = fields
        .schema
        .unwrap_or_else(|| panic!("{command:?} has no typed input"));
    (fields.fields, schema())
}

fn properties(schema: &mut Value) -> &mut Map<String, Value> {
    schema["properties"]
        .as_object_mut()
        .expect("a typed input has properties")
}

/// Gives each property of a field the documentation of that field.
fn describe(schema: &mut Value, fields: &[Field]) {
    let properties = properties(schema);
    for field in fields {
        let Some(property) = properties
            .get_mut(field.name)
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        match field.doc {
            Some(doc) => property.insert("description".into(), doc.into()),
            None => property.remove("description"),
        };
    }
}

/// The input schema of the read tool or action of `command`: each field that does not
/// write.
#[provenance_macros::rule("rule_shared_command_same_name_and_grammar")]
pub fn input_schema(command: Command) -> Value {
    let (fields, mut schema) = fields(command);
    describe(&mut schema, fields);
    for field in fields.iter().filter(|field| field.writes) {
        properties(&mut schema).remove(field.name);
        if let Some(required) = schema["required"].as_array_mut() {
            required.retain(|name| name != field.name);
        }
    }
    schema
}

/// The input schema of the change action that writes the fields of `command` that
/// write: the target and those fields, as `schema` declares them.
pub fn writes_schema(command: Command, mut schema: Value) -> Value {
    let (fields, _) = fields(command);
    describe(&mut schema, fields);
    schema
}
