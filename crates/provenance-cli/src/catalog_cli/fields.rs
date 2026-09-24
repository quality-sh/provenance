use clap::{builder::PossibleValuesParser, Arg, ArgAction, Command};
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
    let mut names = static_flags
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    names.insert("help".into());
    let mut registered = BTreeSet::new();
    for definition in definitions {
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
                command = command.arg(
                    Arg::new(field.name.clone())
                        .long(field.name)
                        .num_args(1)
                        .allow_hyphen_values(true)
                        .action(ArgAction::Set),
                );
            }
        }
    }
    Ok(command)
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
pub(crate) fn augment_schemas(
    mut command: Command,
    schemas: impl IntoIterator<Item = Value>,
    bound: &[&str],
) -> anyhow::Result<Command> {
    let mut registered = command
        .get_arguments()
        .filter_map(|argument| argument.get_long().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    for schema in schemas {
        for (field, declaration) in schema_properties(&schema)? {
            if bound.contains(&field.as_str()) {
                continue;
            }
            let name = cli_name(field);
            if registered.insert(name.clone()) {
                let mut argument = Arg::new(name.clone())
                    .long(name)
                    .num_args(1)
                    .allow_hyphen_values(true)
                    .action(ArgAction::Set);
                if let Some(values) = enum_values(&schema, declaration) {
                    argument = argument.value_parser(PossibleValuesParser::new(values));
                }
                command = command.arg(argument);
            }
        }
    }
    Ok(command)
}

fn enum_values(root: &Value, declaration: &Value) -> Option<Vec<String>> {
    if let Some(name) = declaration
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix("#/$defs/"))
    {
        return root
            .get("$defs")
            .and_then(|defs| defs.get(name))
            .and_then(|schema| enum_values(root, schema));
    }
    declaration
        .get("enum")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
}

/// Bind supplied CLI flags using the typed input schema for one keyword.
pub(crate) fn schema_input(
    schema: &Value,
    matches: &clap::ArgMatches,
    bound: &[&str],
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
        let parsed = if name == "limit" {
            matches
                .try_get_one::<usize>(&name)
                .ok()
                .flatten()
                .copied()
                .map(Value::from)
        } else {
            matches
                .try_get_one::<String>(&name)
                .ok()
                .flatten()
                .map(|raw| {
                    provenance_store::operations::catalog::parse_schema_value_in(
                        schema, declaration, raw,
                    )
                    .map_err(|_| anyhow::anyhow!("invalid value for --{name}"))
                })
                .transpose()?
        };
        if let Some(parsed) = parsed {
            input.insert(field.clone(), parsed);
        }
    }
    Ok(input)
}
