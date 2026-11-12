//! The parser of a root word command, such as `provenance search`, and its refusals.

use super::{args, with_global_flags};
use clap::builder::{PossibleValuesParser, StyledStr};
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Arg, ArgMatches, Command};
use provenance_cli::cli_options;
use provenance_porcelain::registry;
use std::fmt::Write as _;

/// The root parser of one word command that reads the shared repository options, as
/// `provenance search` and `provenance api` read them. The help does not show the
/// command word as an argument, because the usage line starts with that word.
pub fn root_word_command(command: registry::Command) -> Command {
    let declaration = command.declaration();
    let Some(words) = cli_options::words(&declaration) else {
        panic!("{} is not a word command", declaration.name);
    };
    let word = Arg::new("command")
        .required(true)
        .hide(true)
        .value_name("COMMAND")
        .value_parser(PossibleValuesParser::new(words.iter().copied()));
    with_global_flags(Command::new("provenance"))
        .about(declaration.description)
        .arg(word)
        .args(args(&declaration))
}

/// Parses one root word command. The usage line in the help and in a refusal starts with
/// the command word, and a refusal points to the help of the command, for example
/// `provenance search --help`. Each other part of a refusal is the clap text.
pub fn root_word_matches(
    command: registry::Command,
    arguments: &[String],
) -> Result<ArgMatches, clap::Error> {
    let name = command.declaration().name;
    let parser = root_word_command(command);
    let usage = usage(&parser, name, arguments);
    parser
        .override_usage(usage)
        .try_get_matches_from(arguments)
        .map_err(|error| point_to_help(error, command, name))
}

/// The usage of the command, in the form that clap gives to a subcommand: the executable
/// name as clap reads it from the first argument, the command word, `[OPTIONS]` and each
/// other positional argument.
fn usage(parser: &Command, name: &str, arguments: &[String]) -> StyledStr {
    let styles = parser.get_styles();
    let (literal, placeholder) = (styles.get_literal(), styles.get_placeholder());
    let executable = arguments
        .first()
        .and_then(|first| std::path::Path::new(first).file_name())
        .and_then(|file| file.to_str())
        .unwrap_or("provenance");
    let mut usage = StyledStr::new();
    let _ = write!(
        usage,
        "{literal}{executable} {name}{literal:#} {placeholder}[OPTIONS]{placeholder:#}"
    );
    for arg in parser
        .get_positionals()
        .filter(|arg| arg.get_id() != "command" && !arg.is_hide_set())
    {
        let value = arg
            .get_value_names()
            .and_then(|names| names.first())
            .map_or_else(
                || arg.get_id().as_str().to_uppercase(),
                |name| name.as_str().to_owned(),
            );
        let value = if arg.is_required_set() {
            format!("<{value}>")
        } else {
            format!("[{value}]")
        };
        let _ = write!(usage, " {placeholder}{value}{placeholder:#}");
    }
    usage
}

/// The refusal with a help pointer that names the command, for example
/// `provenance search --help`. Clap writes the usage context last, and then its own
/// pointer. Thus this function puts the pointer of the command at the end of the usage
/// context. The error takes a parser without a help flag, so clap writes no pointer.
/// The text of the pointer is a copy of the clap sentence `For more information, try
/// '--help'.`, and `refusal_without_a_table_flag_stays_as_clap_gives_it` in
/// `tests/cli_refusal_next_step.rs` fails when clap changes that sentence.
fn point_to_help(error: clap::Error, command: registry::Command, name: &str) -> clap::Error {
    if matches!(
        error.kind(),
        ErrorKind::DisplayHelp
            | ErrorKind::DisplayVersion
            | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    ) {
        return error;
    }
    let without_help = root_word_command(command).disable_help_flag(true);
    let literal = without_help.get_styles().get_literal();
    let mut tail = StyledStr::new();
    if let Some(ContextValue::StyledStr(usage)) = error.get(ContextKind::Usage) {
        let _ = write!(tail, "{}\n\n", usage.ansi());
    }
    let _ = write!(
        tail,
        "For more information, try '{literal}provenance {name} --help{literal:#}'."
    );
    let mut error = error.with_cmd(&without_help);
    error.insert(ContextKind::Usage, ContextValue::StyledStr(tail));
    error
}
