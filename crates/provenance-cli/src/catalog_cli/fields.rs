use clap::{Arg, ArgAction, Command};
use provenance_store::operations::catalog::{Definition, Parameter};
use serde_json::Value;
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
