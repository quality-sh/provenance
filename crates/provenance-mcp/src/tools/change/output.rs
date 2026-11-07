//! The output schema of the change tool: one `oneOf` branch per result type. Each
//! result names its action, and a record result also names the record kind, so a
//! result matches exactly one branch even where two result types have the same fields.

use super::NOTIFY;
use provenance_transport::porcelain::Action;
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{json, Map, Value};

/// The output schema of the change tool for the actions that the host can run.
pub fn schema(host: &StatementHost, actions: &[&'static str]) -> Value {
    let mut branches = Vec::new();
    for (schema, actions, kinds) in record_results(host) {
        branches.push(discriminated(schema, &actions, Some(&kinds)));
    }
    let discussion = [Action::Discuss.as_str(), Action::Reply.as_str()]
        .into_iter()
        .filter(|action| actions.contains(action))
        .collect::<Vec<_>>();
    if !discussion.is_empty() {
        let schema = provenance_porcelain::discussion::output_schema();
        branches.push(discriminated(schema, &discussion, None));
    }
    if actions.contains(&NOTIFY) {
        let schema = provenance_porcelain::get::notify_output_schema();
        branches.push(discriminated(schema, &[NOTIFY], None));
    }
    for command in super::repository_commands() {
        let name = crate::exposed::name(command);
        if actions.contains(&name) {
            let schema = super::repository_output_schema(command);
            branches.push(discriminated(schema, &[name], None));
        }
    }
    one_of(branches)
}

/// Add the action, and the record kind of a record result, to a successful result.
/// A text block that copies the structured result copies the new result.
pub fn name_action(result: &mut CallToolResult, action: &str) {
    let Some(Value::Object(value)) = result.structured_content.as_mut() else {
        return;
    };
    let copy = Value::Object(value.clone());
    value.insert("action".into(), json!(action));
    let text = Value::Object(value.clone()).to_string();
    for block in &mut result.content {
        let copies = block
            .as_text()
            .and_then(|content| serde_json::from_str::<Value>(&content.text).ok())
            .is_some_and(|value| value == copy);
        if copies {
            *block = ContentBlock::text(text.clone());
        }
    }
}

type Words = Vec<&'static str>;

/// One record result type: its schema, its actions, and its record kinds.
type RecordResult = (Value, Words, Words);

/// The record result types: each result schema, with the actions and the record kinds
/// that give it. The actions of one entry give the schema on each of its kinds, so no
/// pair of an action and a kind is in two entries.
fn record_results(host: &StatementHost) -> Vec<RecordResult> {
    let mut by_schema: Vec<(Value, Vec<(&'static str, Words)>)> = Vec::new();
    for action in Action::RECORD {
        for (kind, definition) in host.porcelain().executable_target_definitions(action) {
            let schema = definition.mcp_output_schema();
            if !by_schema.iter().any(|(held, _)| *held == schema) {
                by_schema.push((schema.clone(), Vec::new()));
            }
            let kinds = &mut by_schema
                .iter_mut()
                .find(|(held, _)| *held == schema)
                .expect("the schema is held")
                .1;
            match kinds.iter_mut().find(|(held, _)| *held == kind.as_str()) {
                Some((_, actions)) => actions.push(action.as_str()),
                None => kinds.push((kind.as_str(), vec![action.as_str()])),
            }
        }
    }
    let mut results = Vec::new();
    for (schema, kinds) in by_schema {
        let mut groups: Vec<(Words, Words)> = Vec::new();
        for (kind, actions) in kinds {
            match groups.iter_mut().find(|(held, _)| *held == actions) {
                Some((_, held)) => held.push(kind),
                None => groups.push((actions, vec![kind])),
            }
        }
        for (actions, kinds) in groups {
            results.push((schema.clone(), actions, kinds));
        }
    }
    results
}

/// The result schema with its `action` and record kind `type` as required properties.
fn discriminated(mut schema: Value, actions: &[&str], kinds: Option<&[&str]>) -> Value {
    let words = |words: &[&str]| match words {
        [word] => json!({"type": "string", "const": word}),
        words => json!({"type": "string", "enum": words}),
    };
    let object = schema
        .as_object_mut()
        .expect("a result schema is an object");
    let mut fields = vec![("action", words(actions))];
    if let Some(kinds) = kinds {
        fields.push(("type", words(kinds)));
    }
    for (name, field) in fields {
        object
            .entry("properties")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("properties are an object")
            .insert(name.into(), field);
        object
            .entry("required")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("required is an array")
            .push(json!(name));
    }
    schema
}

/// A `oneOf` of the branches, with each definition held once at the root. A definition
/// that two branches hold the same way has one name; two different definitions with one
/// name get the index of their branch. A property schema with fields that two or more
/// branches repeat is one definition too.
fn one_of(mut branches: Vec<Value>) -> Value {
    let mut definitions = Map::new();
    for (index, branch) in branches.iter_mut().enumerate() {
        hold_definitions(branch, &format!("Output{index}"), &mut definitions);
    }
    hold_repeated_properties(&mut branches, &mut definitions);
    let mut schema = json!({ "oneOf": branches });
    if !definitions.is_empty() {
        schema["$defs"] = Value::Object(definitions);
    }
    schema
}

/// Moves the definitions of one branch to `held`. Each definition keeps its name unless
/// `held` has a different definition of that name; then it gets the branch prefix. A
/// renamed definition changes the references in the others, so the names are chosen
/// again until no name changes.
fn hold_definitions(branch: &mut Value, prefix: &str, held: &mut Map<String, Value>) {
    let Some(object) = branch.as_object_mut() else {
        return;
    };
    object.remove("$schema");
    let Some(Value::Object(definitions)) = object.remove("$defs") else {
        return;
    };
    let mut names = definitions
        .keys()
        .map(|name| (name.clone(), json!(name)))
        .collect::<Map<String, Value>>();
    let renamed = loop {
        let renamed = definitions
            .iter()
            .map(|(name, schema)| {
                let mut schema = schema.clone();
                rewrite_references(&mut schema, &names);
                (names[name].as_str().expect("a name").to_owned(), schema)
            })
            .collect::<Vec<_>>();
        let clashes = definitions
            .keys()
            .zip(&renamed)
            .filter(|(name, (new, schema))| {
                *name == new && held.get(new).is_some_and(|other| other != schema)
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        if clashes.is_empty() {
            break renamed;
        }
        for name in clashes {
            names.insert(name.clone(), json!(format!("{prefix}{name}")));
        }
    };
    held.extend(renamed);
    rewrite_references(branch, &names);
}

/// Each object property schema that two or more branches hold the same way becomes one
/// definition, named by its title or by the property name.
fn hold_repeated_properties(branches: &mut [Value], held: &mut Map<String, Value>) {
    let mut seen: Vec<(String, Value, usize)> = Vec::new();
    for branch in branches.iter() {
        for (name, schema) in branch["properties"].as_object().into_iter().flatten() {
            if schema.get("properties").is_none() {
                continue;
            }
            match seen
                .iter_mut()
                .find(|(held_name, held_schema, _)| held_name == name && held_schema == schema)
            {
                Some((_, _, count)) => *count += 1,
                None => seen.push((name.clone(), schema.clone(), 1)),
            }
        }
    }
    for (name, schema, count) in seen {
        if count < 2 {
            continue;
        }
        let title = schema["title"].as_str().map_or_else(
            || {
                let mut letters = name.chars();
                letters
                    .next()
                    .map(|first| first.to_uppercase().chain(letters).collect::<String>())
                    .unwrap_or_default()
            },
            str::to_owned,
        );
        if held.get(&title).is_some_and(|other| *other != schema) {
            continue;
        }
        for branch in branches.iter_mut() {
            let property = branch
                .get_mut("properties")
                .and_then(|properties| properties.get_mut(&name));
            if let Some(property) = property {
                if *property == schema {
                    *property = json!({ "$ref": format!("#/$defs/{title}") });
                }
            }
        }
        held.insert(title, schema);
    }
}

/// Points each reference to a definition in `names` at the new name of that definition.
fn rewrite_references(value: &mut Value, names: &Map<String, Value>) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                if let Some(new) = reference
                    .strip_prefix("#/$defs/")
                    .and_then(|name| names.get(name))
                    .and_then(Value::as_str)
                {
                    *reference = format!("#/$defs/{new}");
                }
            }
            for child in object.values_mut() {
                rewrite_references(child, names);
            }
        }
        Value::Array(values) => {
            for child in values {
                rewrite_references(child, names);
            }
        }
        _ => {}
    }
}
