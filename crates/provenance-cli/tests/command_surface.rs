//! The spawned binary's help and the MCP `tools/list` of a served host show the commands
//! that the registry records, each with its declared flags or schema properties.

mod support {
    pub mod review_paths;
    pub mod surface;
}

use provenance_macros::verifies;
use provenance_porcelain::registry::{Command, Input, Interface, Presence};
use serde_json::Value;
use std::collections::BTreeSet;
use support::surface::{help, tools, words};

fn properties(schema: &Value) -> BTreeSet<String> {
    schema["properties"]
        .as_object()
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default()
}

/// The CLI help and the MCP tool of each command on both interfaces show one description.
#[tokio::test]
#[verifies("rule_command_description_same_on_cli_and_mcp", examples)]
async fn porcelain_commands_show_one_description() {
    let (repo, _) = support::review_paths::init();
    let tools = tools(repo.path()).await;
    for command in Command::ALL {
        let (Some(words), true) = (words(*command), command.is_present(Interface::Mcp)) else {
            continue;
        };
        let declaration = command.declaration();
        let tool = &tools[declaration.name];
        assert_eq!(tool["description"], declaration.description, "{command:?}");
        let help = help(words);
        assert!(
            help.contains(declaration.description),
            "provenance {} --help does not show the MCP description:\n{help}",
            words.join(" ")
        );
    }
}

/// Each command that the registry records on an interface appears there with its
/// declared flags or schema properties, and an absent command does not appear.
#[tokio::test]
#[verifies("rule_porcelain_interface_shows_recorded_commands", exhaustion)]
async fn registry_commands_appear_on_both_interfaces() {
    let (repo, path) = support::review_paths::init();
    let tools = tools(repo.path()).await;
    for command in Command::ALL {
        let declaration = command.declaration();
        match command.presence(Interface::Cli) {
            Presence::Present => {
                let help = words(*command).map_or_else(
                    || help(&["req_surface", declaration.name, "--repo", &path]),
                    help,
                );
                for flag in declaration.flags.iter().filter(|flag| !flag.hidden) {
                    let shown = if flag.kind == provenance_porcelain::registry::FlagKind::Positional
                    {
                        let name = flag.field_id().to_uppercase();
                        if flag.required {
                            format!("<{name}>")
                        } else {
                            format!("[{name}]")
                        }
                    } else {
                        format!("--{}", flag.name)
                    };
                    assert!(
                        help.contains(&shown),
                        "{command:?} help lacks {shown}:\n{help}"
                    );
                }
            }
            Presence::Absent { reason } => assert!(!reason.is_empty(), "{command:?}"),
        }
        match command.presence(Interface::Mcp) {
            Presence::Present => {
                let tool = tools
                    .get(declaration.name)
                    .unwrap_or_else(|| panic!("tools/list lacks {}", declaration.name));
                let expected = match declaration.mcp_input {
                    Input::Typed(schema) => Some(properties(&schema())),
                    Input::Discussion(action) => Some(properties(
                        &provenance_porcelain::discussion::input_schema(action),
                    )),
                    Input::RecordFields(_) | Input::None => None,
                };
                if let Some(expected) = expected {
                    assert_eq!(properties(&tool["inputSchema"]), expected, "{command:?}");
                }
            }
            Presence::Absent { reason } => {
                assert!(!reason.is_empty(), "{command:?}");
                assert!(!tools.contains_key(declaration.name), "{command:?}");
            }
        }
    }
}

/// The registry command that `provenance <words...>` selects, and its help.
fn registry_command(words: &[&str]) -> (Command, String) {
    let command = Command::find(Interface::Cli, words)
        .unwrap_or_else(|| panic!("provenance {} is not a registry command", words.join(" ")));
    (command, help(words))
}

/// The repository commands come from the registry, and their help shows each declared flag.
#[test]
#[verifies("rule_porcelain_interface_shows_recorded_commands", examples)]
fn repository_commands_are_registry_commands() {
    let commands: &[&[&str]] = &[
        &["init"],
        &["__cargo-init"],
        &["materialize"],
        &["export"],
        &["import"],
        &["merge-jsonl"],
        &["validate"],
        &["schema", "show"],
        &["dictionary", "import"],
        &["dictionary", "status"],
    ];
    for words in commands {
        let (command, help) = registry_command(words);
        assert!(
            !command.is_present(Interface::Mcp),
            "{command:?} is CLI-only"
        );
        for flag in command
            .declaration()
            .flags
            .iter()
            .filter(|flag| !flag.hidden)
        {
            if flag.kind != provenance_porcelain::registry::FlagKind::Positional {
                let shown = format!("--{}", flag.name);
                assert!(
                    help.contains(&shown),
                    "{words:?} help lacks {shown}:\n{help}"
                );
            }
        }
    }
}
