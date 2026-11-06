//! The arguments of one registered operation, in one list for each consumer.
//!
//! An argument has one `snake_case` name. Each consumer shows it under that name,
//! spelled as its own standard asks. A path parameter is not an argument: the target
//! of the action fills it.

use super::{Definition, Parameter};
use serde_json::Value;
use std::collections::BTreeSet;

/// One argument of an operation.
#[derive(Clone)]
pub struct Argument {
    /// The argument name, in `snake_case`.
    pub name: String,
    pub place: ArgumentPlace,
}

/// Where the value of an argument goes in the request.
#[derive(Clone)]
pub enum ArgumentPlace {
    /// A query or header parameter.
    Parameter(Parameter),
    /// A field of the request data.
    Body {
        /// The data field that the value fills.
        field: String,
        /// The schema of the value. With `wrap_array`, the schema of one item.
        schema: Value,
        /// The value is one item of the array field.
        wrap_array: bool,
    },
}

impl Argument {
    /// The data field of a body argument.
    pub fn body_field(&self) -> Option<&str> {
        match &self.place {
            ArgumentPlace::Body { field, .. } => Some(field),
            ArgumentPlace::Parameter(_) => None,
        }
    }
}

/// The argument name of a parameter.
pub fn parameter_argument(parameter: &Parameter) -> String {
    parameter.name.to_ascii_lowercase().replace('-', "_")
}

impl Definition {
    /// The arguments in order: the query and header parameters, the data fields, and
    /// the data field aliases. A data field that a request header fills (`If-Match` to
    /// `expected_etag`) has only the header argument.
    ///
    /// # Errors
    ///
    /// Two arguments have one name, or an alias names no data field.
    pub fn arguments(&self) -> Result<Vec<Argument>, String> {
        let mut arguments = Vec::new();
        let mut names = BTreeSet::new();
        let mut insert = |argument: Argument| {
            if names.insert(argument.name.clone()) {
                arguments.push(argument);
                Ok(())
            } else {
                Err(format!(
                    "argument {} has more than one binding",
                    argument.name
                ))
            }
        };
        for parameter in self.parameters() {
            if parameter.location == "path" {
                continue;
            }
            insert(Argument {
                name: parameter_argument(&parameter),
                place: ArgumentPlace::Parameter(parameter),
            })?;
        }
        let header_bound = self
            .registration
            .controls
            .headers
            .iter()
            .map(|binding| binding.field)
            .collect::<BTreeSet<_>>();
        let Some(request) = self.request_schema() else {
            return Ok(arguments);
        };
        let properties = request
            .pointer("/properties/data/properties")
            .and_then(Value::as_object);
        for (field, schema) in properties.into_iter().flatten() {
            if header_bound.contains(field.as_str()) {
                continue;
            }
            insert(Argument {
                name: field.clone(),
                place: ArgumentPlace::Body {
                    field: field.clone(),
                    schema: schema.clone(),
                    wrap_array: false,
                },
            })?;
        }
        for alias in &self.registration.request.argument_aliases {
            let schema = properties
                .and_then(|properties| properties.get(alias.field))
                .ok_or_else(|| format!("alias {} has no data field", alias.argument))?;
            let schema = if alias.wrap_array {
                schema
                    .get("items")
                    .ok_or_else(|| format!("alias {} does not name an array", alias.argument))?
            } else {
                schema
            };
            insert(Argument {
                name: alias.argument.to_owned(),
                place: ArgumentPlace::Body {
                    field: alias.field.to_owned(),
                    schema: schema.clone(),
                    wrap_array: alias.wrap_array,
                },
            })?;
        }
        Ok(arguments)
    }
}
