//! Renders the clap parser of each CLI command from its declaration.

use clap::CommandFactory as _;
use clap::{
    builder::{PossibleValuesParser, ValueParser},
    Arg, ArgAction, ArgMatches, Command,
};
use provenance_cli::cli_options::{self, flag_name, GLOBAL_OPTIONS, GROUP_DESCRIPTIONS};
use provenance_porcelain::registry::{
    self, Binding, Declaration, Field, FieldValue, Input, Relation,
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// The CLI switch that reads all the fields of a record or Discussion action as one JSON
/// object from standard input.
const STDIN: Field = Field::switch("stdin");

/// The clap arguments of the declared fields, with their relations. A target field is a
/// positional argument and each other field is a `--kebab-case` flag. A record or
/// Discussion action also takes `--stdin` after its fields, and a command with CLI options
/// takes them after its fields.
pub fn args(declaration: &Declaration) -> Vec<Arg> {
    let input = &declaration.input;
    let stdin = matches!(input, Input::RecordFields { .. } | Input::Discussion(_));
    let (options, relations) = cli_options::command_options(declaration);
    input
        .fields()
        .iter()
        .map(|field| {
            let positional = input.target().contains(&field.name);
            relate(arg(field, positional), field.name, input.relations())
        })
        .chain(
            options
                .iter()
                .map(|option| relate(arg(option, false), option.name, relations)),
        )
        .chain(stdin.then(|| arg(&STDIN, false)))
        .collect()
}

fn arg(field: &Field, positional: bool) -> Arg {
    let mut arg = Arg::new(field.name);
    arg = match (positional, field.value, field.repeated) {
        (true, ..) => arg.action(ArgAction::Set),
        (false, FieldValue::Switch, _) => arg.long(flag_name(field)).action(ArgAction::SetTrue),
        (false, _, false) => arg.long(flag_name(field)).action(ArgAction::Set),
        (false, _, true) => arg.long(flag_name(field)).action(ArgAction::Append),
    };
    if positional || field.value != FieldValue::Switch {
        arg = arg
            .value_parser(value_parser(field))
            .value_name(field.name.to_uppercase());
    }
    match (cli_options::field_help(field), field.value) {
        // The CLI reads an object field from a file or from standard input.
        (Some(doc), FieldValue::Object) => {
            arg = arg.help(format!(
                "{doc} Give a JSON file, or \"-\" for standard input."
            ));
        }
        (Some(doc), _) => arg = arg.help(doc),
        (None, _) => {}
    }
    if let Some(default) = field.default {
        arg = arg.default_value(default);
    }
    arg.allow_hyphen_values(field.leading_hyphen)
        .hide(field.internal)
        .required(field.required)
}

/// The CLI options of a bound command: each repository option whose name is not a
/// field of the command.
fn bound_options(declaration: &Declaration) -> Vec<Arg> {
    let scoped = match declaration.bound {
        Binding::None => return Vec::new(),
        Binding::Repository => false,
        Binding::Scope => true,
    };
    GLOBAL_OPTIONS
        .iter()
        .filter(|option| scoped || option.name != "scope")
        .filter(|option| {
            !declaration
                .input
                .fields()
                .iter()
                .any(|field| field.name == option.name)
        })
        .map(|option| arg(option, false))
        .collect()
}

/// The repository and output options that a bound word command reads.
pub struct BoundContext {
    pub repo: String,
    pub scope: String,
    /// The caller asked for the JSON answer.
    pub json: bool,
}

/// Reads the repository and output options of a bound word command.
pub fn bound_context(matches: &ArgMatches) -> BoundContext {
    // A command bound to a repository only has no scope option.
    let option = |id: &str| matches.try_get_one::<String>(id).ok().flatten().cloned();
    BoundContext {
        repo: option("repo").unwrap_or_else(|| ".".to_owned()),
        scope: option("scope").unwrap_or_else(|| "default".to_owned()),
        json: option("format").is_some(),
    }
}

/// The command with the shared repository options that the porcelain parsers take.
pub fn with_global_flags(command: Command) -> Command {
    command.args(
        GLOBAL_OPTIONS
            .iter()
            .map(|option| arg(option, false).global(true)),
    )
}

fn value_parser(field: &Field) -> ValueParser {
    if !field.values.is_empty() {
        return PossibleValuesParser::new(field.values).into();
    }
    match field.value {
        FieldValue::Count => clap::value_parser!(usize).into(),
        FieldValue::RecordKind => ValueParser::new(parse_kind),
        FieldValue::ApiMethod => ValueParser::new(parse_api_method),
        FieldValue::CatalogLimit => ValueParser::new(parse_catalog_limit),
        FieldValue::Port => clap::value_parser!(u16).into(),
        FieldValue::Switch | FieldValue::Text | FieldValue::Pairs(_) | FieldValue::Object => {
            ValueParser::string()
        }
    }
}

fn relate(mut arg: Arg, name: &str, relations: &[Relation]) -> Arg {
    for relation in relations {
        match *relation {
            Relation::Requires(field, needed) if field == name => arg = arg.requires(needed),
            Relation::Conflicts(field, other) if field == name => {
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

/// The root parser: the built-in commands and the word commands that it lists. `docs`
/// follows the graph commands, and the other server commands follow the workspace
/// commands.
pub fn root_command() -> clap::Command {
    let shown = |command: &registry::Command| provenance_cli::exposed::shows(*command);
    let listed = |filter: fn(&registry::Command) -> bool| {
        registry::Command::ALL
            .iter()
            .filter(move |command| shown(command) && filter(command))
            .map(|command| command.declaration())
    };
    let servers = |filter: fn(&provenance_cli::servers::Server) -> bool| {
        provenance_cli::servers::SERVERS
            .iter()
            .filter(move |server| filter(server))
            .map(|server| server.declaration())
    };
    [
        registry::Command::Search,
        registry::Command::Check,
        registry::Command::Prime,
    ]
    .iter()
    .map(|command| command.declaration())
    .chain(listed(|command| {
        matches!(
            command,
            registry::Command::Repository(_) | registry::Command::Graph(_)
        )
    }))
    .chain(servers(is_docs))
    .chain(listed(|command| {
        matches!(command, registry::Command::Workspace(_))
    }))
    .chain(servers(|server| !is_docs(server)))
    .chain(listed(|command| {
        matches!(command, registry::Command::Dogfood(_))
    }))
    .fold(crate::cli::Cli::command(), add_subcommand)
}

fn is_docs(server: &provenance_cli::servers::Server) -> bool {
    *server == provenance_cli::servers::Server::DocsServe
}

/// Adds one word command to `root` under its words, as `provenance <words...>` shows it.
/// Each word before the last is a group that needs one of its commands.
pub fn add_subcommand(root: Command, declaration: Declaration) -> Command {
    let Some(words) = cli_options::words(&declaration) else {
        panic!("{} is not a word command", declaration.name);
    };
    let (word, groups) = words.split_last().expect("a word command has a word");
    // The root already gives some options to each command after it.
    let options = bound_options(&declaration)
        .into_iter()
        .filter(|option| {
            root.get_arguments()
                .all(|arg| arg.get_id() != option.get_id())
        })
        .collect::<Vec<_>>();
    let mut leaf = Command::new(*word)
        .hide(declaration.internal)
        .args(options)
        .args(args(&declaration));
    if !declaration.description.is_empty() {
        leaf = leaf.about(declaration.description);
    }
    add_under(root, groups, groups, leaf)
}

/// Adds `leaf` under the `rest` of the group words, creating each missing group.
fn add_under(
    parent: Command,
    groups: &[&'static str],
    rest: &[&'static str],
    leaf: Command,
) -> Command {
    let Some((group, rest)) = rest.split_first() else {
        return parent.subcommand(leaf);
    };
    let path = groups[..groups.len() - rest.len()].join(" ");
    let parent = if parent.find_subcommand(group).is_some() {
        parent
    } else {
        parent.subcommand(
            Command::new(*group)
                .about(
                    GROUP_DESCRIPTIONS
                        .iter()
                        .find(|(words, _)| *words == path)
                        .map(|(_, description)| *description),
                )
                .subcommand_required(true)
                .arg_required_else_help(true),
        )
    };
    parent.mut_subcommand(*group, |child| add_under(child, groups, rest, leaf))
}

/// The words of the selected subcommands and the matches of the last one.
pub fn selected(matches: &ArgMatches) -> (Vec<&str>, &ArgMatches) {
    let mut words = Vec::new();
    let mut leaf = matches;
    while let Some((word, next)) = leaf.subcommand() {
        words.push(word);
        leaf = next;
    }
    (words, leaf)
}

/// The root parser of one word command that reads the shared repository options, as
/// `provenance search` and `provenance api` read them.
pub fn root_word_command(command: provenance_porcelain::registry::Command) -> Command {
    let declaration = command.declaration();
    let Some(words) = cli_options::words(&declaration) else {
        panic!("{} is not a word command", declaration.name);
    };
    let word = Arg::new("command")
        .required(true)
        .value_name("COMMAND")
        .value_parser(PossibleValuesParser::new(words.iter().copied()));
    with_global_flags(Command::new("provenance"))
        .about(declaration.description)
        .arg(word)
        .args(args(&declaration))
}

/// Reads a typed input from the declared fields of one command.
pub fn input<T: serde::de::DeserializeOwned>(
    declaration: &Declaration,
    matches: &ArgMatches,
) -> anyhow::Result<T> {
    let mut object = Map::new();
    for field in declaration.input.fields() {
        let id = field.name;
        let value = match (field.value, field.repeated) {
            (FieldValue::Switch, _) => Some(Value::Bool(matches.get_flag(id))),
            (_, true) => Some(Value::Array(
                matches
                    .get_raw(id)
                    .into_iter()
                    .flatten()
                    .map(|value| Value::String(value.to_string_lossy().into_owned()))
                    .collect(),
            )),
            (FieldValue::Count | FieldValue::CatalogLimit, false) => {
                matches.get_one::<usize>(id).map(|value| (*value).into())
            }
            (FieldValue::Port, false) => matches.get_one::<u16>(id).map(|value| (*value).into()),
            (FieldValue::Pairs(separator), _) => Some(pairs(matches, id, separator)?),
            (FieldValue::Object, false) => matches
                .get_one::<String>(id)
                .map(|source| json_object(source))
                .transpose()?,
            _ => matches
                .get_raw(id)
                .and_then(|mut values| values.next())
                .map(|value| Value::String(value.to_string_lossy().into_owned())),
        };
        if let Some(value) = value {
            object.insert(id.to_owned(), value);
        }
    }
    serde_json::from_value(Value::Object(object))
        .map_err(|error| anyhow::anyhow!("unsupported {} options: {error}", declaration.name))
}

/// The `NAME<separator>VALUE` values of a pairs field, as one JSON object.
fn pairs(matches: &ArgMatches, id: &str, separator: char) -> anyhow::Result<Value> {
    let mut object = Map::new();
    for entry in matches.get_many::<String>(id).into_iter().flatten() {
        let Some((name, value)) = entry.split_once(separator) else {
            anyhow::bail!("--{} uses NAME{separator}VALUE", id.replace('_', "-"));
        };
        object.insert(
            name.trim().to_owned(),
            Value::String(value.trim_start().to_owned()),
        );
    }
    Ok(Value::Object(object))
}

/// The JSON object of an object field: a file, or standard input with `-`.
fn json_object(source: &str) -> anyhow::Result<Value> {
    let text = if source == "-" {
        std::io::read_to_string(std::io::stdin())?
    } else {
        std::fs::read_to_string(source)?
    };
    let value: Value = serde_json::from_str(&text)?;
    anyhow::ensure!(value.is_object(), "{source} needs one JSON object");
    Ok(value)
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
