//! The flag that one positional value most probably means, for each root command that
//! takes no positional value but has a flag for that value.

use clap::builder::StyledStr;
use clap::error::{ContextKind, ContextValue, ErrorKind};

/// For each command, by its words: the value that a caller gives in the place of a flag,
/// and the name of that flag.
const FLAGS: &[(&[&str], &str, &str)] = &[
    (&["check"], "the statement", "statement"),
    (&["import"], "the scope export file", "input"),
    (&["export"], "the scope export file", "output"),
    (&["init"], "the repository", "path"),
    (&["validate"], "the artifact file", "input"),
    (&["dictionary", "import"], "the PDF file", "pdf"),
    (&["prime"], "the format", "format"),
    (&["mcp"], "the repository", "repo"),
    (&["review"], "the repository", "repo"),
];

/// A clap refusal of a root command. When clap refuses a value that does not start with a
/// hyphen for a command of the table, the refusal has one more tip line that names the
/// flag. Each other refusal stays as clap gives it.
pub(super) fn with_tip(mut error: clap::Error, arguments: &[String]) -> clap::Error {
    if error.kind() != ErrorKind::UnknownArgument {
        return error;
    }
    let Some(ContextValue::String(word)) = error.get(ContextKind::InvalidArg) else {
        return error;
    };
    if word.starts_with('-') {
        return error;
    }
    let Some((thing, flag)) = flag(arguments) else {
        return error;
    };
    let mut tips = match error.get(ContextKind::Suggested) {
        Some(ContextValue::StyledStrs(tips)) => tips.clone(),
        _ => Vec::new(),
    };
    tips.push(StyledStr::from(format!("give {thing} with --{flag}")));
    error.insert(ContextKind::Suggested, ContextValue::StyledStrs(tips));
    error
}

/// The value and the flag of the table for the command that the arguments select. The
/// parse ignores errors, so it selects the command that clap refused a value for. When
/// the arguments already give the flag, there is no tip, because the refused value is
/// one value too many.
fn flag(arguments: &[String]) -> Option<(&'static str, &'static str)> {
    let matches = super::root_command()
        .ignore_errors(true)
        .try_get_matches_from(arguments)
        .ok()?;
    let (words, _) = crate::registry_cli::selected(&matches);
    let (_, thing, flag) = FLAGS
        .iter()
        .find(|(command, _, _)| *command == words.as_slice())?;
    let short = declared(&words, flag).and_then(|arg| arg.get_short());
    let given = arguments
        .iter()
        .skip(1)
        .take_while(|argument| *argument != "--")
        .any(|argument| names_flag(argument, flag, short));
    (!given).then_some((*thing, *flag))
}

/// The argument of the command that has the long name `flag`.
fn declared(words: &[&str], flag: &str) -> Option<clap::Arg> {
    let mut command = super::root_command();
    command.build();
    for word in words {
        command = command.find_subcommand(word)?.clone();
    }
    let arg = command
        .get_arguments()
        .find(|arg| arg.get_long() == Some(flag))?;
    Some(arg.clone())
}

/// True when one argument gives the flag by its long name or by its short name, with or
/// without an attached value.
fn names_flag(argument: &str, flag: &str, short: Option<char>) -> bool {
    if let Some(long) = argument.strip_prefix("--") {
        return match long.strip_prefix(flag) {
            Some(rest) => rest.is_empty() || rest.starts_with('='),
            None => false,
        };
    }
    let Some(shorts) = argument.strip_prefix('-') else {
        return false;
    };
    short.is_some_and(|short| shorts.starts_with(short))
}

#[cfg(test)]
mod tests {
    /// Each flag of the table is a flag of its command.
    #[test]
    fn each_flag_is_declared_by_its_command() {
        for (words, _, flag) in super::FLAGS {
            let declared = super::declared(words, flag);
            assert!(declared.is_some(), "{words:?} --{flag}");
        }
    }
}
