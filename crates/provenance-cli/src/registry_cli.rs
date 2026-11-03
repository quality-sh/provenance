//! Renders the clap parser of each registry command from its declaration.

use clap::{
    builder::{PossibleValuesParser, ValueParser},
    Arg, ArgAction, ArgMatches, Command,
};
use provenance_porcelain::registry::{
    self, CliForm, Declaration, Flag, FlagKind, FlagValue, Relation,
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// The clap arguments of the declared flags, with their relations.
pub fn args(declaration: &Declaration) -> Vec<Arg> {
    declaration
        .flags
        .iter()
        .map(|flag| relate(arg(flag), &flag.field_id(), declaration.relations))
        .collect()
}

fn arg(flag: &Flag) -> Arg {
    let id = flag.field_id();
    let mut arg = Arg::new(id.clone());
    arg = match flag.kind {
        FlagKind::Switch => arg.long(flag.name).action(ArgAction::SetTrue),
        FlagKind::Value => arg.long(flag.name).action(ArgAction::Set),
        FlagKind::Repeated => arg.long(flag.name).action(ArgAction::Append),
        FlagKind::Positional => arg.action(ArgAction::Set),
    };
    if flag.kind != FlagKind::Switch {
        arg = arg
            .value_parser(value_parser(flag))
            .value_name(id.to_uppercase());
    }
    if let Some(help) = flag.help {
        arg = arg.help(help);
    }
    if let Some(default) = flag.default {
        arg = arg.default_value(default);
    }
    arg.allow_hyphen_values(flag.hyphen_values)
        .hide(flag.hidden)
}

fn value_parser(flag: &Flag) -> ValueParser {
    if !flag.values.is_empty() {
        return PossibleValuesParser::new(flag.values).into();
    }
    match flag.value {
        FlagValue::Text => ValueParser::string(),
        FlagValue::Count => clap::value_parser!(usize).into(),
        FlagValue::RecordKind => ValueParser::new(parse_kind),
        FlagValue::ApiMethod => ValueParser::new(parse_api_method),
        FlagValue::CatalogLimit => ValueParser::new(parse_catalog_limit),
    }
}

fn relate(mut arg: Arg, name: &str, relations: &[Relation]) -> Arg {
    for relation in relations {
        match *relation {
            Relation::Requires(flag, needed) if flag == name => arg = arg.requires(needed),
            Relation::Conflicts(flag, other) if flag == name => {
                arg = arg.conflicts_with(other);
            }
            Relation::Requires(..) | Relation::Conflicts(..) => {}
        }
    }
    arg
}

fn parse_kind(value: &str) -> Result<provenance_core::NodeType, String> {
    provenance_core::NodeType::parse(value).map_err(|_| "unsupported record type".to_owned())
}

fn parse_api_method(value: &str) -> Result<provenance_porcelain::api::ApiMethod, String> {
    provenance_porcelain::api::ApiMethod::parse(value)
        .ok_or_else(|| "unsupported method".to_owned())
}

fn parse_catalog_limit(value: &str) -> Result<usize, String> {
    let limit = value
        .parse::<usize>()
        .map_err(|_| "catalog limit must be a number from 1 through 200".to_owned())?;
    if (1..=200).contains(&limit) {
        Ok(limit)
    } else {
        Err("catalog limit must be a number from 1 through 200".to_owned())
    }
}

/// The subcommand of one word command, as `provenance <word>` shows it.
pub fn subcommand(command: registry::Command) -> Command {
    let declaration = command.declaration();
    let CliForm::Words(words) = declaration.cli_form else {
        panic!("{} is not a word command", declaration.name);
    };
    Command::new(*words.last().expect("a word command has a word"))
        .about(declaration.description)
        .hide(declaration.hidden)
        .args(args(&declaration))
}

/// The root parser of one word command that reads the shared repository options, as
/// `provenance search` and `provenance api` read them.
pub fn root_word_command(command: registry::Command) -> Command {
    let declaration = command.declaration();
    let CliForm::Words(words) = declaration.cli_form else {
        panic!("{} is not a word command", declaration.name);
    };
    let word = Arg::new("command")
        .required(true)
        .value_name("COMMAND")
        .value_parser(PossibleValuesParser::new(words.iter().copied()));
    let common = <crate::invocation::grammar::Common as clap::Args>::augment_args(Command::new(
        "provenance",
    ));
    common
        .about(declaration.description)
        .arg(word)
        .args(args(&declaration))
}

/// Reads a typed input from the declared flags of one command.
pub fn input<T: serde::de::DeserializeOwned>(
    declaration: &Declaration,
    matches: &ArgMatches,
) -> anyhow::Result<T> {
    let mut object = Map::new();
    for flag in declaration.flags {
        let id = flag.field_id();
        let value = match (flag.kind, flag.value) {
            (FlagKind::Switch, _) => Some(Value::Bool(matches.get_flag(&id))),
            (FlagKind::Repeated, _) => Some(Value::Array(
                matches
                    .get_raw(&id)
                    .into_iter()
                    .flatten()
                    .map(|value| Value::String(value.to_string_lossy().into_owned()))
                    .collect(),
            )),
            (_, FlagValue::Count | FlagValue::CatalogLimit) => {
                matches.get_one::<usize>(&id).map(|value| (*value).into())
            }
            _ => matches
                .get_raw(&id)
                .and_then(|mut values| values.next())
                .map(|value| Value::String(value.to_string_lossy().into_owned())),
        };
        if let Some(value) = value {
            object.insert(id, value);
        }
    }
    serde_json::from_value(Value::Object(object))
        .map_err(|error| anyhow::anyhow!("unsupported {} options: {error}", declaration.name))
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
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow::anyhow!("declared input has no properties"))?;
        for field in properties.keys() {
            if bound.contains(&field.as_str()) {
                continue;
            }
            let name = provenance_contract::cli_name(field);
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
