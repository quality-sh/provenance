//! Parses `provenance search`, and refuses search text that is not in `--text`.

use super::grammar::SearchArgs;
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Arg, Command};
use provenance_porcelain::registry;

/// Parses the search command. When clap refuses a word that is not a flag, or a word
/// after `--`, the refusal says that the search text goes in `--text`. Each other
/// refusal is the refusal of `root_word_matches`.
pub(super) fn parse(arguments: &[String]) -> SearchArgs {
    let command = crate::registry_cli::root_word_command(registry::Command::Search);
    let parsed = crate::registry_cli::root_word_matches(registry::Command::Search, arguments);
    let error = match parsed {
        Ok(matches) => return SearchArgs::from_matches(&matches),
        Err(error) => error,
    };
    let Some(word) = unexpected_word(&error, arguments) else {
        error.exit()
    };
    refuse(&word, &command, arguments)
}

/// The word of an unexpected argument that clap reads as a value: a word that does not
/// start with a hyphen, or a word after `--`.
fn unexpected_word(error: &clap::Error, arguments: &[String]) -> Option<String> {
    if error.kind() != ErrorKind::UnknownArgument {
        return None;
    }
    let Some(ContextValue::String(word)) = error.get(ContextKind::InvalidArg) else {
        return None;
    };
    (!word.starts_with('-') || escaped(word, arguments)).then(|| word.clone())
}

/// Whether the first copy of the word in the arguments comes after the first `--`.
fn escaped(word: &str, arguments: &[String]) -> bool {
    let mut rest = arguments.iter().skip(1);
    let before = rest
        .by_ref()
        .take_while(|argument| *argument != "--")
        .any(|argument| argument == word);
    !before && rest.any(|argument| argument == word)
}

fn refuse(word: &str, command: &Command, arguments: &[String]) -> ! {
    let tip = corrected(command, arguments)
        .map(|line| format!(": {line}"))
        .unwrap_or_default();
    clap::Error::raw(
        ErrorKind::UnknownArgument,
        format!(
            "unexpected argument '{word}' found\n\n  \
             tip: give the search text with --text{tip}\n\n\
             For more information, try 'provenance search --help'.\n"
        ),
    )
    .with_cmd(command)
    .exit()
}

/// The command with the search text in `--text`. The text joins the words that are not
/// flags and the value of `--text`, in the order of the arguments, with one space. The
/// other flags follow it in their order, without `--help` and `-h`. There is no command
/// when the text is empty, or when a flag or its value can read differently to clap.
///
/// This walk is not `grammar::positional_word`. That walk finds a command word, and it
/// takes the word after each flag that is not a switch, as its callers expect. This walk
/// must read each flag value as clap reads it, or give no command, because a wrong
/// command searches for a text that the caller did not type.
fn corrected(command: &Command, arguments: &[String]) -> Option<String> {
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
        if argument == "--help" || argument == "-h" {
            continue;
        }
        let long = argument.strip_prefix("--")?;
        let (name, inline) = long
            .split_once('=')
            .map_or((long, None), |(name, value)| (name, Some(value)));
        let declaration = command
            .get_arguments()
            .find(|arg| arg.get_long() == Some(name))?;
        let value = match inline {
            None if declaration.get_action().takes_values() => {
                Some(value_after(declaration, &mut rest)?.as_str())
            }
            _ => None,
        };
        if name == "text" {
            text.extend(inline.or(value));
            continue;
        }
        flags.push(shell_word(argument));
        flags.extend(value.map(shell_word));
    }
    let text = text.join(" ");
    if text.trim().is_empty() {
        return None;
    }
    let mut line = format!("provenance search --text {}", quoted_text(&text));
    for flag in flags {
        line.push(' ');
        line.push_str(&flag);
    }
    Some(line)
}

/// The word after a flag, when clap reads it as the value of the flag.
fn value_after<'a>(
    declaration: &Arg,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Option<&'a String> {
    rest.next()
        .filter(|value| !value.starts_with('-') || declaration.is_allow_hyphen_values_set())
}

/// The value as one word of a POSIX shell, in single quotes only when the shell reads a
/// character in it.
fn shell_word(value: &str) -> String {
    let plain = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c));
    if plain {
        value.to_owned()
    } else {
        quoted_text(value)
    }
}

/// The value in single quotes for a POSIX shell. Each single quote in it becomes `'\''`.
fn quoted_text(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}
