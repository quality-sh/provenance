use clap::{Arg, ArgAction, Command};
use provenance_store::operations::catalog::{Definition, Parameter};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub(super) struct Field {
    pub name: String,
    pub source: Source,
}

pub(super) enum Source {
    Parameter(Parameter),
    Body {
        wire_field: String,
        value_schema: Value,
        wrap_array: bool,
    },
}

pub(super) fn declared(definition: &Definition) -> anyhow::Result<Vec<Field>> {
    let mut fields = Vec::new();
    let mut names = BTreeSet::new();
    for parameter in definition.parameters() {
        if parameter.location == "path" {
            continue;
        }
        let name = cli_name(parameter.name);
        insert(
            &mut fields,
            &mut names,
            Field {
                name,
                source: Source::Parameter(parameter),
            },
        )?;
    }
    if let Some(request) = definition.request_schema() {
        if let Some(properties) = request
            .pointer("/properties/data/properties")
            .and_then(Value::as_object)
        {
            for (wire_field, schema) in properties {
                insert(
                    &mut fields,
                    &mut names,
                    Field {
                        name: wire_field.replace('_', "-"),
                        source: Source::Body {
                            wire_field: wire_field.clone(),
                            value_schema: schema.clone(),
                            wrap_array: false,
                        },
                    },
                )?;
            }
        }
        for alias in &definition.registration.request.argument_aliases {
            let schema = request
                .pointer("/properties/data/properties")
                .and_then(|properties| properties.get(alias.field))
                .ok_or_else(|| {
                    anyhow::anyhow!("catalog alias {} has no body field", alias.argument)
                })?;
            let value_schema = if alias.wrap_array {
                schema.get("items").ok_or_else(|| {
                    anyhow::anyhow!("catalog alias {} does not name an array", alias.argument)
                })?
            } else {
                schema
            };
            insert(
                &mut fields,
                &mut names,
                Field {
                    name: alias.argument.replace('_', "-"),
                    source: Source::Body {
                        wire_field: alias.field.to_owned(),
                        value_schema: value_schema.clone(),
                        wrap_array: alias.wrap_array,
                    },
                },
            )?;
        }
    }
    Ok(fields)
}

fn insert(
    fields: &mut Vec<Field>,
    names: &mut BTreeSet<String>,
    field: Field,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        names.insert(field.name.clone()),
        "catalog flag --{} has more than one binding",
        field.name
    );
    fields.push(field);
    Ok(())
}

pub(super) fn augment(
    command: Command,
    definitions: impl IntoIterator<Item = &'static Definition>,
    static_flags: &[&str],
) -> anyhow::Result<Command> {
    augment_with_overrides(command, definitions, static_flags, &[])
}

pub(super) fn augment_with_overrides(
    mut command: Command,
    definitions: impl IntoIterator<Item = &'static Definition>,
    static_flags: &[&str],
    overrides: &[&str],
) -> anyhow::Result<Command> {
    let definitions = definitions.into_iter().collect::<Vec<_>>();
    let mut names = static_flags
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    names.insert("help".into());
    let mut body_flags = BTreeSet::new();
    let mut json_flags = BTreeSet::new();
    for definition in &definitions {
        let mut wire_fields = BTreeSet::new();
        for field in declared(definition)? {
            let Source::Body { wire_field, .. } = field.source else {
                continue;
            };
            anyhow::ensure!(
                !names.contains(&field.name),
                "catalog field --{} collides with a command option",
                field.name
            );
            body_flags.insert(field.name);
            if wire_fields.insert(wire_field.clone()) {
                json_flags.insert(json_flag(&wire_field));
            }
        }
    }
    let mut registered = BTreeSet::new();
    for definition in &definitions {
        for field in declared(definition)? {
            if overrides.contains(&field.name.as_str()) {
                continue;
            }
            anyhow::ensure!(
                !names.contains(&field.name),
                "catalog field --{} collides with a command option",
                field.name
            );
            if registered.insert(field.name.clone()) {
                command = command.arg(flag(&field.name, body_flags.contains(&field.name)));
            }
        }
    }
    for json in json_flags {
        anyhow::ensure!(
            !names.contains(&json),
            "catalog field --{json} collides with a command option"
        );
        if registered.insert(json.clone()) {
            command = command.arg(flag(&json, true));
        }
    }
    Ok(command)
}

/// The `--<field>-json` flag that carries one whole JSON body value. One
/// spelling per body wire field, so alias flags gain no JSON variant.
pub(super) fn json_flag(wire_field: &str) -> String {
    format!("{}-json", cli_name(wire_field))
}

fn flag(name: &str, repeated: bool) -> Arg {
    let argument = Arg::new(name.to_owned())
        .long(name.to_owned())
        .num_args(1)
        .allow_hyphen_values(true);
    if repeated {
        argument.action(ArgAction::Append)
    } else {
        argument.action(ArgAction::Set)
    }
}

/// The request schema of one body wire field.
pub(super) fn wire_field_schema<'a>(request: &'a Value, wire_field: &str) -> Option<&'a Value> {
    request
        .pointer("/properties/data/properties")?
        .as_object()?
        .get(wire_field)
}

pub(super) fn cli_name(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "-")
}

fn schema_properties(schema: &Value) -> anyhow::Result<&Map<String, Value>> {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("declared input has no properties"))
}

/// Register flags from the same typed contracts that declare the MCP inputs.
pub fn augment_schemas(
    mut command: Command,
    schemas: impl IntoIterator<Item = Value>,
    bound: &[&str],
) -> anyhow::Result<Command> {
    let mut registered = command
        .get_arguments()
        .filter_map(|argument| argument.get_long().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    for schema in schemas {
        for field in schema_properties(&schema)?.keys() {
            if bound.contains(&field.as_str()) {
                continue;
            }
            let name = cli_name(field);
            if registered.insert(name.clone()) {
                command = command.arg(
                    Arg::new(name.clone())
                        .long(name)
                        .num_args(1)
                        .allow_hyphen_values(true)
                        .action(ArgAction::Set),
                );
            }
        }
    }
    Ok(command)
}

/// Bind supplied CLI flags using the typed input schema for one keyword.
pub fn schema_input(
    schema: &Value,
    matches: &clap::ArgMatches,
    bound: &[&str],
    usize_flags: &[&str],
) -> anyhow::Result<Map<String, Value>> {
    let properties = schema_properties(schema)?;
    let allowed = properties
        .keys()
        .filter(|name| !bound.contains(&name.as_str()))
        .map(|name| cli_name(name))
        .collect::<Vec<_>>();
    let allowed = allowed.iter().map(String::as_str).collect::<Vec<_>>();
    super::ensure_only_fields(matches, &allowed);
    let mut input = Map::new();
    for (field, declaration) in properties {
        if bound.contains(&field.as_str()) {
            continue;
        }
        let name = cli_name(field);
        let raw = if usize_flags.contains(&field.as_str()) {
            matches
                .try_get_one::<usize>(&name)
                .map(|value| value.map(usize::to_string))
        } else {
            matches
                .try_get_one::<String>(&name)
                .map(Option::<&String>::cloned)
        };
        let raw = raw.map_err(|_| anyhow::anyhow!("incompatible CLI storage for --{name}"))?;
        let parsed = raw
            .as_deref()
            .map(|raw| {
                provenance_store::operations::catalog::parse_schema_value_in(
                    schema,
                    declaration,
                    raw,
                )
                .map_err(|_| anyhow::anyhow!("invalid value for --{name}"))
            })
            .transpose()?;
        if let Some(parsed) = parsed {
            input.insert(field.clone(), parsed);
        }
    }
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn json_flag_declarations_follow_wire_fields_not_aliases() {
        let definitions = provenance_store::operations::catalog::definitions();
        let rules = definitions
            .iter()
            .filter(|definition| definition.path.split('/').nth(1) == Some("rules"))
            .collect::<Vec<_>>();
        assert!(rules.iter().any(|definition| {
            definition
                .registration
                .request
                .argument_aliases
                .iter()
                .any(|alias| alias.wrap_array)
        }));
        let mut command = augment(Command::new("test"), rules, &[]).unwrap();
        let declared = command
            .get_arguments()
            .filter_map(|argument| argument.get_long().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        assert!(declared.contains("requirement-ids-json"));
        assert!(!declared.contains("requirement-id-json"));
        let help = command.render_help().to_string();
        assert!(help.contains("--requirement-ids-json"));
        assert!(!help.contains("--requirement-id-json"));
        let raw = r#"["req_shared"]"#;
        let parsed = command
            .try_get_matches_from(["test", "--requirement-ids-json", raw])
            .unwrap();
        let supplied = parsed
            .get_many::<String>("requirement-ids-json")
            .map(|values| values.cloned().collect::<Vec<_>>());
        assert_eq!(supplied, Some(vec![raw.to_owned()]));
    }

    #[test]
    fn every_registered_json_spelling_is_a_canonical_wire_flag() {
        let definitions = provenance_store::operations::catalog::definitions();
        let mut collections = BTreeMap::<&str, Vec<&Definition>>::new();
        for definition in definitions {
            let collection = definition.path.split('/').nth(1).expect("route collection");
            collections.entry(collection).or_default().push(definition);
        }
        for (collection, definitions) in collections {
            let mut wire_fields = BTreeSet::new();
            for definition in &definitions {
                for field in declared(definition).unwrap() {
                    let Source::Body { wire_field, .. } = field.source else {
                        continue;
                    };
                    wire_fields.insert(json_flag(&wire_field));
                }
            }
            let command = augment(Command::new("test"), definitions, &[]).unwrap();
            for argument in command.get_arguments() {
                let Some(long) = argument.get_long() else {
                    continue;
                };
                if long.ends_with("-json") {
                    assert!(
                        wire_fields.contains(long),
                        "{collection} registers --{long} which no body wire field declares"
                    );
                }
            }
        }
    }

    #[test]
    fn selected_schema_validates_shared_flag_in_either_declaration_order() {
        let first = json!({"properties": {"status": {"type": "string", "enum": ["alpha"]}}});
        let second = json!({"properties": {"status": {"type": "string", "enum": ["beta"]}}});
        for schemas in [
            [first.clone(), second.clone()],
            [second.clone(), first.clone()],
        ] {
            let command = augment_schemas(Command::new("test"), schemas, &[]).unwrap();
            for (schema, accepted, rejected) in
                [(&first, "alpha", "beta"), (&second, "beta", "alpha")]
            {
                let matches = command
                    .clone()
                    .try_get_matches_from(["test", "--status", accepted])
                    .unwrap();
                assert_eq!(
                    schema_input(schema, &matches, &[], &[]).unwrap()["status"],
                    accepted
                );
                let matches = command
                    .clone()
                    .try_get_matches_from(["test", "--status", rejected])
                    .unwrap();
                assert!(schema_input(schema, &matches, &[], &[]).is_err());
            }
        }
    }

    #[test]
    fn incompatible_clap_storage_is_an_error() {
        let schema = json!({"properties": {"value": {"type": "string"}}});
        let command = Command::new("test").arg(
            Arg::new("value")
                .long("value")
                .value_parser(clap::value_parser!(bool)),
        );
        let matches = command
            .try_get_matches_from(["test", "--value", "true"])
            .unwrap();
        assert!(schema_input(&schema, &matches, &[], &[]).is_err());
    }

    #[test]
    fn undeclared_numeric_storage_is_an_error() {
        let schema = json!({"properties": {"page": {"type": "integer"}}});
        let command = Command::new("test").arg(
            Arg::new("page")
                .long("page")
                .value_parser(clap::value_parser!(usize)),
        );
        let matches = command
            .try_get_matches_from(["test", "--page", "2"])
            .unwrap();
        assert!(schema_input(&schema, &matches, &[], &[]).is_err());
        assert_eq!(
            schema_input(&schema, &matches, &[], &["page"]).unwrap()["page"],
            2
        );
    }
}
