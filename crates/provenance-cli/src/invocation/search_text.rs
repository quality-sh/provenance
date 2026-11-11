//! Parses `provenance search`, and refuses search text that is not in `--text`.

use super::grammar::SearchArgs;
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::Command;
use provenance_porcelain::registry;

/// Parses the search command. When clap refuses a word that is not a flag, the refusal
/// gives the same command with the search text in `--text`. Each other refusal of clap
/// stays as clap gives it.
pub(super) fn parse(arguments: &[String]) -> SearchArgs {
    let command = crate::registry_cli::root_word_command(registry::Command::Search);
    let error = match command.clone().try_get_matches_from(arguments) {
        Ok(matches) => return SearchArgs::from_matches(&matches),
        Err(error) => error,
    };
    let Some(word) = unexpected_word(&error) else {
        error.exit()
    };
    refuse(&word, &command, arguments)
}

/// The word of an unexpected argument that does not start with a hyphen.
fn unexpected_word(error: &clap::Error) -> Option<String> {
    if error.kind() != ErrorKind::UnknownArgument {
        return None;
    }
    match error.get(ContextKind::InvalidArg) {
        Some(ContextValue::String(word)) if !word.starts_with('-') => Some(word.clone()),
        _ => None,
    }
}

fn refuse(word: &str, command: &Command, arguments: &[String]) -> ! {
    clap::Error::raw(
        ErrorKind::UnknownArgument,
        format!(
            "unexpected argument '{word}' found\n\n  \
             tip: give the search text with --text: {}\n\n\
             For more information, try 'provenance search --help'.\n",
            corrected(command, arguments)
        ),
    )
    .exit()
}

/// The command with the search text in `--text`. The text joins the words that are not
/// flags and the value of `--text`, in the order of the arguments, with one space. The
/// other flags follow it in their order.
fn corrected(command: &Command, arguments: &[String]) -> String {
    let mut text = Vec::new();
    let mut flags = Vec::new();
    let mut command_word = false;
    let mut rest = arguments.iter().skip(1);
    while let Some(argument) = rest.next() {
        if argument == "--" {
            text.extend(
                rest.by_ref()
                    .skip(usize::from(!command_word))
                    .map(String::as_str),
            );
            break;
        }
        if !argument.starts_with('-') || argument == "-" {
            if command_word {
                text.push(argument.as_str());
            }
            command_word = true;
            continue;
        }
        let long = argument.strip_prefix("--").unwrap_or_default();
        let (name, inline) = long
            .split_once('=')
            .map_or((long, None), |(name, value)| (name, Some(value)));
        let takes_value = command
            .get_arguments()
            .any(|arg| arg.get_long() == Some(name) && arg.get_action().takes_values());
        let value = match (takes_value, inline) {
            (true, None) => rest.next(),
            _ => None,
        };
        if name == "text" {
            text.extend(inline.or_else(|| value.map(String::as_str)));
            continue;
        }
        flags.push(quoted(argument, false));
        flags.extend(value.map(|value| quoted(value, false)));
    }
    let mut line = format!(
        "provenance search --text {}",
        quoted(&text.join(" "), true)
    );
    for flag in flags {
        line.push(' ');
        line.push_str(&flag);
    }
    line
}

/// The value as one word of a POSIX shell. A value with a character that the shell
/// reads gets single quotes, and each single quote in it becomes `'\''`.
fn quoted(value: &str, always: bool) -> String {
    let plain = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c));
    if plain && !always {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', r"'\''"))
    }
}
