//! The full CLI parser surface, including hidden commands, target actions and the
//! children of registry commands, holds only the commands that the registry declares
//! and the three server commands.

use clap::Command;
use provenance_cli::servers::{Server, SERVERS};
use provenance_cli::{cli_form, exposed};
use provenance_macros::verifies;
use provenance_porcelain::registry;

/// The paths below `root` that are neither a CLI registry command, a server command,
/// nor a group of them.
fn undeclared_commands(root: &Command) -> Vec<String> {
    let groups = exposed::EXPOSED
        .iter()
        .filter_map(|command| cli_form::declaration(*command).words())
        .chain(
            SERVERS
                .iter()
                .filter_map(|server| server.declaration().words()),
        )
        .collect::<Vec<_>>();
    let mut undeclared = Vec::new();
    let mut pending = vec![(Vec::<&str>::new(), root)];
    while let Some((path, command)) = pending.pop() {
        for child in command.get_subcommands() {
            let mut words = path.clone();
            words.push(child.get_name());
            let declared = exposed::find(&words).is_some() || Server::find(&words).is_some();
            let group = groups
                .iter()
                .any(|selected| selected.len() > words.len() && selected.starts_with(&words));
            if !declared && !group {
                undeclared.push(words.join(" "));
            }
            pending.push((words, child));
        }
    }
    undeclared
}

/// The ACTION values of `target` that are not CLI registry target actions.
fn undeclared_actions(target: &Command) -> Vec<String> {
    let action = target
        .get_arguments()
        .find(|argument| argument.get_id() == "action")
        .expect("the target parser has an ACTION");
    action
        .get_possible_values()
        .iter()
        .map(|value| value.get_name().to_owned())
        .filter(|name| {
            registry::Command::find_target_action(name)
                .is_none_or(|command| !exposed::shows(command))
        })
        .collect()
}

/// Each command of the root parser, hidden or shown and at any depth, and each target
/// action is a registry command that the CLI shows, or a group of such commands.
#[test]
#[verifies("rule_porcelain_registry_defines_commands", exhaustion)]
fn every_parser_command_is_a_registry_command() {
    assert_eq!(
        undeclared_commands(&super::root_command()),
        Vec::<String>::new()
    );
    assert_eq!(
        undeclared_actions(&crate::catalog_cli::target_command().unwrap()),
        Vec::<String>::new()
    );
}

/// The parser commands outside the registry are exactly the three server commands.
#[test]
#[verifies("rule_porcelain_registry_defines_commands", exhaustion)]
fn the_parser_commands_outside_the_registry_are_the_servers() {
    let root = super::root_command();
    let mut outside = Vec::new();
    let mut pending = vec![(Vec::<&str>::new(), &root)];
    while let Some((path, command)) = pending.pop() {
        for child in command.get_subcommands() {
            let mut words = path.clone();
            words.push(child.get_name());
            if let Some(server) = Server::find(&words) {
                outside.push(server);
            }
            pending.push((words, child));
        }
    }
    outside.sort_by_key(|server| server.declaration().name);
    assert_eq!(outside, [Server::DocsServe, Server::Mcp, Server::Review]);
    for server in SERVERS {
        assert_eq!(registry::Command::named(server.declaration().name), None);
    }
}

/// The surface check rejects an undeclared hidden command, an undeclared child of a
/// registry command, and an undeclared target action.
#[test]
#[verifies("rule_porcelain_registry_defines_commands", examples)]
fn surface_check_rejects_undeclared_commands() {
    let root = Command::new("provenance")
        .subcommand(Command::new("__undeclared").hide(true))
        .subcommand(Command::new("__cargo-init").hide(true))
        .subcommand(Command::new("review").subcommand(Command::new("extra")))
        .subcommand(Command::new("schema").subcommand(Command::new("show")));
    let mut undeclared = undeclared_commands(&root);
    undeclared.sort();
    assert_eq!(undeclared, ["__undeclared", "review extra"]);

    let target = Command::new("provenance")
        .arg(clap::Arg::new("action").value_parser(["get", "create", "extra"]));
    assert_eq!(undeclared_actions(&target), ["extra"]);
}
