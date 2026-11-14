//! The shape of a success that a port gives: a check of a JSON value against the success schema
//! of a read operation, closed so that an object has no member that the schema does not name.
//!
//! The check reads only the keywords that the schema generator writes for the success types of
//! the read operations, and only the patterns in [`PATTERNS`]. [`Shape::new`] refuses a schema
//! with any other keyword or pattern, so a change of the schemas shows when the checks are made
//! and not as refused answers.
//!
//! Closed: at the root, at each member and at each item, an object may have only the members
//! that the schema names there. A branch of `allOf`, `anyOf` or `oneOf` and the definition that
//! `$ref` names add the members that they name, when the value matches them. So a branch can
//! state only a part of an object, as the `anyOf` of a record that ties its archive commit to its
//! status does. `additionalProperties` names every other member, and a schema that states no
//! member, as the schema of any JSON value, accepts any member.

use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// The test of a string that matches one pattern.
type Matches = fn(&str) -> bool;

/// Each pattern that a success schema holds, and the test of a string that matches it.
const PATTERNS: [(&str, Matches); 3] = [
    ("^[a-z0-9_-]+$", |text| {
        !text.is_empty()
            && text
                .bytes()
                .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
    }),
    ("^([0-9a-f]{40}|[0-9a-f]{64})$", |text| {
        matches!(text.len(), 40 | 64)
            && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    }),
    ("\\S", |text| text.chars().any(|c| !c.is_whitespace())),
];

/// The keywords that the check tests.
const TESTED: [&str; 18] = [
    "$ref",
    "type",
    "enum",
    "const",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "allOf",
    "anyOf",
    "oneOf",
    "minimum",
    "maximum",
    "minItems",
    "maxItems",
    "minLength",
    "maxLength",
    "pattern",
];

/// The keywords that only describe a value. `format` is one: as the schema dialect gives it,
/// it asserts nothing.
const DESCRIBING: [&str; 10] = [
    "$schema",
    "$defs",
    "title",
    "description",
    "default",
    "examples",
    "format",
    "deprecated",
    "readOnly",
    "writeOnly",
];

/// The success schema of one operation, checked to hold only the keywords that the check reads.
pub struct Shape {
    root: Value,
}

impl Shape {
    /// The shape of `root`, or the path and the keyword or pattern that the check cannot read.
    pub fn new(root: Value) -> Result<Self, String> {
        readable(&root, "#")?;
        Ok(Self { root })
    }

    /// Whether `value` matches the schema, with no member that the schema does not name.
    pub fn accepts(&self, value: &Value) -> bool {
        self.closed(&self.root, value)
    }

    fn closed(&self, schema: &Value, value: &Value) -> bool {
        self.names(schema, value).is_some_and(|names| {
            value
                .as_object()
                .is_none_or(|members| free(schema) || names.cover(members))
        })
    }

    /// The members that `schema` names for `value`, when `value` matches it.
    fn names<'s>(&'s self, schema: &'s Value, value: &Value) -> Option<Names<'s>> {
        let schema = match schema {
            Value::Object(schema) => schema,
            Value::Bool(true) => return Some(Names::default()),
            _ => return None,
        };
        let mut names = Names::default();
        for (keyword, rule) in schema {
            let holds = match keyword.as_str() {
                "$ref" => {
                    let target = self.root.pointer(rule.as_str()?.strip_prefix('#')?)?;
                    names.join(self.names(target, value)?);
                    true
                }
                "properties" => {
                    let members = rule.as_object()?;
                    names.named.extend(members.keys().map(String::as_str));
                    value.as_object().is_none_or(|object| {
                        members.iter().all(|(name, member)| {
                            object
                                .get(name)
                                .is_none_or(|given| self.closed(member, given))
                        })
                    })
                }
                "additionalProperties" => {
                    names.every = true;
                    let stated = schema.get("properties").and_then(Value::as_object);
                    value.as_object().is_none_or(|object| {
                        object
                            .iter()
                            .filter(|(name, _)| {
                                stated.is_none_or(|stated| !stated.contains_key(*name))
                            })
                            .all(|(_, given)| self.closed(rule, given))
                    })
                }
                "allOf" => {
                    for branch in rule.as_array()? {
                        names.join(self.names(branch, value)?);
                    }
                    true
                }
                "anyOf" | "oneOf" => {
                    let passing: Vec<_> = rule
                        .as_array()?
                        .iter()
                        .filter_map(|branch| self.names(branch, value))
                        .collect();
                    let matched = if keyword == "oneOf" {
                        passing.len() == 1
                    } else {
                        !passing.is_empty()
                    };
                    for branch in passing {
                        names.join(branch);
                    }
                    matched
                }
                "items" => value
                    .as_array()
                    .is_none_or(|items| items.iter().all(|item| self.closed(rule, item))),
                _ => rule_holds(keyword, rule, value)?,
            };
            if !holds {
                return None;
            }
        }
        Some(names)
    }
}

/// Whether `value` holds the rule of one keyword that has no subschema. `None` for a rule that
/// is not of its keyword's form.
fn rule_holds(keyword: &str, rule: &Value, value: &Value) -> Option<bool> {
    let length = |value: &Value| match value {
        Value::String(text) => u64::try_from(text.chars().count()).ok(),
        Value::Array(items) => u64::try_from(items.len()).ok(),
        _ => None,
    };
    Some(match keyword {
        "type" => match rule {
            Value::Array(types) => types.iter().any(|kind| is(kind, value)),
            kind => is(kind, value),
        },
        "enum" => rule.as_array()?.contains(value),
        "const" => rule == value,
        "required" => value.as_object().is_none_or(|object| {
            rule.as_array()
                .into_iter()
                .flatten()
                .all(|name| name.as_str().is_some_and(|name| object.contains_key(name)))
        }),
        "minimum" => {
            let least = rule.as_f64()?;
            value.as_f64().is_none_or(|number| number >= least)
        }
        "maximum" => {
            let most = rule.as_f64()?;
            value.as_f64().is_none_or(|number| number <= most)
        }
        "minItems" | "minLength" => {
            let least = rule.as_u64()?;
            length(value).is_none_or(|size| size >= least)
        }
        "maxItems" | "maxLength" => {
            let most = rule.as_u64()?;
            length(value).is_none_or(|size| size <= most)
        }
        "pattern" => value.as_str().is_none_or(|text| {
            PATTERNS
                .iter()
                .any(|(pattern, test)| rule.as_str() == Some(pattern) && test(text))
        }),
        _ => true,
    })
}

/// Whether `schema` states no member of an object, as the schema of any JSON value does: then
/// an object may have any member.
fn free(schema: &Value) -> bool {
    match schema {
        Value::Object(schema) => [
            "properties",
            "additionalProperties",
            "$ref",
            "allOf",
            "anyOf",
            "oneOf",
        ]
        .iter()
        .all(|keyword| !schema.contains_key(*keyword)),
        other => other == &Value::Bool(true),
    }
}

/// Whether `value` is of the JSON type `kind`.
fn is(kind: &Value, value: &Value) -> bool {
    match kind.as_str() {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("boolean") => value.is_boolean(),
        Some("null") => value.is_null(),
        Some("number") => value.is_number(),
        Some("integer") => value.is_i64() || value.is_u64(),
        _ => false,
    }
}

/// The members that a schema names for one object.
#[derive(Default)]
struct Names<'s> {
    named: BTreeSet<&'s str>,
    /// `additionalProperties` names each other member.
    every: bool,
}

impl Names<'_> {
    fn join(&mut self, other: Self) {
        self.named.extend(other.named);
        self.every |= other.every;
    }

    fn cover(&self, members: &Map<String, Value>) -> bool {
        self.every
            || members
                .keys()
                .all(|name| self.named.contains(name.as_str()))
    }
}

/// Refuses a keyword that the check does not read, or a pattern that it does not know, in
/// `schema` or below it. `at` is the path of `schema`.
fn readable(schema: &Value, at: &str) -> Result<(), String> {
    let Value::Object(schema) = schema else {
        return Ok(());
    };
    for (keyword, rule) in schema {
        let path = format!("{at}/{keyword}");
        let known = TESTED.contains(&keyword.as_str())
            || DESCRIBING.contains(&keyword.as_str())
            || keyword.starts_with("x-");
        if !known {
            return Err(format!("{path}: the check does not read this keyword"));
        }
        if keyword == "pattern"
            && !PATTERNS
                .iter()
                .any(|(pattern, _)| rule.as_str() == Some(pattern))
        {
            return Err(format!(
                "{path}: the check does not know the pattern {rule}"
            ));
        }
        match (keyword.as_str(), rule) {
            ("properties" | "$defs", Value::Object(members)) => {
                for (name, member) in members {
                    readable(member, &format!("{path}/{name}"))?;
                }
            }
            ("allOf" | "anyOf" | "oneOf", Value::Array(branches)) => {
                for (index, branch) in branches.iter().enumerate() {
                    readable(branch, &format!("{path}/{index}"))?;
                }
            }
            ("items" | "additionalProperties", rule) => readable(rule, &path)?,
            _ => {}
        }
    }
    Ok(())
}
