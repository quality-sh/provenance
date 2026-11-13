//! An implementation aid with no Rule binding. Each schema of the generated contract accepts a
//! value, and each closed object in it has a field. The models of the clients come from these
//! schemas, so a contract that passes gives no model that has no value.

use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

/// The keywords whose value is a map of schemas.
const SCHEMA_MAPS: [&str; 4] = [
    "properties",
    "patternProperties",
    "$defs",
    "dependentSchemas",
];
/// The keywords whose value is a list of schemas.
const SCHEMA_LISTS: [&str; 4] = ["anyOf", "oneOf", "allOf", "prefixItems"];
/// The keywords whose value is one schema. The guard examines an object value of each. A
/// `false` value closes an object or an array, or it is a condition on a value, so the guard
/// does not examine it as a schema of a value.
const SCHEMAS: [&str; 9] = [
    "items",
    "additionalProperties",
    "unevaluatedProperties",
    "unevaluatedItems",
    "contains",
    "propertyNames",
    "if",
    "then",
    "else",
];

/// The members of the keyword `key` of `schema`, when its value is an object.
fn member_map<'a>(schema: &'a Map<String, Value>, key: &str) -> Option<&'a Map<String, Value>> {
    schema.get(key).and_then(Value::as_object)
}

/// The members of the keyword `key` of `schema`, when its value is an array.
fn member_list<'a>(schema: &'a Map<String, Value>, key: &str) -> Option<&'a Vec<Value>> {
    schema.get(key).and_then(Value::as_array)
}

/// True when the keywords of `schema` accept no value: an `enum` or a choice with no member,
/// or a `not` that refuses each value.
fn accepts_no_value(schema: &Map<String, Value>) -> bool {
    let no_member = |key: &str| member_list(schema, key).is_some_and(Vec::is_empty);
    let refuses_each_value = match schema.get("not") {
        Some(Value::Bool(refuses)) => *refuses,
        Some(Value::Object(not)) => not.is_empty(),
        _ => false,
    };
    no_member("enum") || no_member("anyOf") || no_member("oneOf") || refuses_each_value
}

/// The keywords that can accept a property that the object does not name: `additionalProperties`
/// and the keywords that apply a schema to the same object. `unevaluatedProperties` refuses
/// only a property that none of them accepts.
const EVALUATED_BY: [&str; 10] = [
    "additionalProperties",
    "allOf",
    "anyOf",
    "oneOf",
    "if",
    "then",
    "else",
    "dependentSchemas",
    "$ref",
    "$dynamicRef",
];

/// True when `schema` is an object that accepts no property. Its `type` is `object` or a list
/// with `object`. It accepts a maximum of zero properties, or it refuses each property name,
/// or it names no property and refuses each property that it does not name.
fn closed_without_field(schema: &Map<String, Value>) -> bool {
    let no_member = |key: &str| member_map(schema, key).is_none_or(Map::is_empty);
    let refuses = |key: &str| schema.get(key) == Some(&Value::Bool(false));
    let object = match schema.get("type") {
        Some(Value::String(kind)) => kind == "object",
        Some(Value::Array(kinds)) => kinds.contains(&Value::from("object")),
        _ => false,
    };
    let unevaluated = refuses("unevaluatedProperties")
        && !EVALUATED_BY.iter().any(|key| schema.contains_key(*key));
    let closed = refuses("additionalProperties") || unevaluated;
    let names_no_property = no_member("properties") && no_member("patternProperties");
    let accepts_zero = schema.get("maxProperties").and_then(Value::as_u64) == Some(0);
    object && (accepts_zero || refuses("propertyNames") || (names_no_property && closed))
}

/// Adds each schema in `schema` that accepts no value, and each closed object that has no
/// field.
fn faults(schema: &Value, at: &str, found: &mut Vec<String>) {
    let Some(object) = schema.as_object() else {
        if schema == &Value::Bool(false) {
            found.push(format!("{at}: no value satisfies this schema"));
        }
        return;
    };
    if accepts_no_value(object) {
        found.push(format!("{at}: no value satisfies this schema"));
    }
    if closed_without_field(object) {
        found.push(format!("{at}: closed object with no field"));
    }
    for key in SCHEMA_MAPS {
        for (name, child) in member_map(object, key).into_iter().flatten() {
            faults(child, &format!("{at}/{key}/{name}"), found);
        }
    }
    for key in SCHEMA_LISTS {
        for (index, child) in member_list(object, key).into_iter().flatten().enumerate() {
            faults(child, &format!("{at}/{key}/{index}"), found);
        }
    }
    for key in SCHEMAS {
        if let Some(child) = object.get(key).filter(|child| child.is_object()) {
            faults(child, &format!("{at}/{key}"), found);
        }
    }
}

fn faults_of(schema: &Value) -> Vec<String> {
    let mut found = Vec::new();
    faults(schema, "schema", &mut found);
    found
}

#[test]
fn each_contract_schema_accepts_a_value_and_each_closed_object_has_a_field() {
    let (openapi, mcp) = provenance_codegen::documents();
    let mut found = Vec::new();
    for (name, schema) in openapi["components"]["schemas"].as_object().unwrap() {
        faults(schema, name, &mut found);
    }
    for (path, route) in openapi["paths"].as_object().unwrap() {
        for (method, operation) in route.as_object().unwrap() {
            let parameters = operation["parameters"].as_array();
            for parameter in parameters.into_iter().flatten() {
                let at = format!("{method} {path} parameter {}", parameter["name"]);
                faults(&parameter["schema"], &at, &mut found);
            }
        }
    }
    for tool in mcp["tools"].as_array().unwrap() {
        for key in ["inputSchema", "outputSchema"] {
            faults(&tool[key], &format!("{} {key}", tool["name"]), &mut found);
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}

/// The applicator keywords and the unevaluated keywords of JSON Schema 2020-12 apply a schema
/// to a value, and `$defs` holds the schemas that a reference names. The guard walks each
/// one except `not`, because a schema with no value below `not` is not a fault.
#[test]
fn the_guard_walks_each_keyword_that_applies_a_schema() {
    let mut walked = BTreeSet::from(SCHEMA_MAPS);
    walked.extend(SCHEMA_LISTS);
    walked.extend(SCHEMAS);
    walked.insert("not");
    let keywords = [
        "$defs",
        "additionalProperties",
        "allOf",
        "anyOf",
        "contains",
        "dependentSchemas",
        "else",
        "if",
        "items",
        "not",
        "oneOf",
        "patternProperties",
        "prefixItems",
        "properties",
        "propertyNames",
        "then",
        "unevaluatedItems",
        "unevaluatedProperties",
    ];
    assert_eq!(walked, BTreeSet::from(keywords));
}

/// The guard finds each form of a schema with no value at the root and below each keyword
/// that it walks.
#[test]
fn the_guard_finds_each_schema_form_that_has_no_value() {
    let forms = [
        json!(false),
        json!({"enum":[]}),
        json!({"anyOf":[]}),
        json!({"oneOf":[]}),
        json!({"not":{}}),
        json!({"not":true}),
        json!({"type":"object","additionalProperties":false}),
        json!({"type":"object","additionalProperties":false,"properties":{}}),
        json!({"type":["object","null"],"additionalProperties":false}),
        json!({"type":"object","propertyNames":false}),
        json!({"type":"object","unevaluatedProperties":false}),
        json!({"type":"object","maxProperties":0}),
    ];
    for form in forms {
        let mut holders = vec![form.clone()];
        holders.extend(SCHEMA_MAPS.map(|key| json!({key:{"member":form}})));
        holders.extend(SCHEMA_LISTS.map(|key| json!({key:[{"type":"null"},form]})));
        if form.is_object() {
            holders.extend(SCHEMAS.map(|key| json!({key:form})));
        }
        for holder in holders {
            assert_eq!(faults_of(&holder).len(), 1, "{holder}");
        }
    }
}

/// The guard accepts a schema that has a value: an open object, a closed object with a
/// field, a closed map, an object and an array that a `false` keyword closes, a `not` that
/// refuses some values, a closed value that is not an object, an object with a pattern for
/// its property names, and a condition whose result is `false`. It also accepts an object
/// that `unevaluatedProperties` closes when a different keyword accepts a property, and an
/// object with a maximum of one property.
#[test]
fn the_guard_accepts_each_schema_form_that_has_a_value() {
    let named = json!({"properties":{"field":{}}});
    for schema in [
        json!(true),
        json!({}),
        json!({"type":"object"}),
        json!({"type":"object","additionalProperties":false,"properties":{"field":{}}}),
        json!({"type":["object","null"],"additionalProperties":false,"properties":{"field":{}}}),
        json!({"type":"object","additionalProperties":false,"patternProperties":{"^x":{}}}),
        json!({"type":"object","properties":{"field":{}},"unevaluatedProperties":false}),
        json!({"type":"array","prefixItems":[{"type":"string"}],"items":false}),
        json!({"not":{"type":"null"}}),
        json!({"enum":[null]}),
        json!({"type":["string","null"],"additionalProperties":false}),
        json!({"type":"object","propertyNames":{"pattern":"^x"}}),
        json!({"if":{"required":["field"]},"then":false}),
        json!({"type":"object","allOf":[named],"unevaluatedProperties":false}),
        json!({"type":"object","additionalProperties":{},"unevaluatedProperties":false}),
        json!({"type":"object","maxProperties":1}),
    ] {
        assert_eq!(faults_of(&schema), Vec::<String>::new(), "{schema}");
    }
}
