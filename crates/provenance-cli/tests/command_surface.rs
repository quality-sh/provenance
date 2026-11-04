//! The spawned binary's help and the MCP `tools/list` of a served host show the commands
//! that the registry records, each with its declared flags or schema properties.

mod support {
    pub mod review_paths;
    pub mod surface;
}

use provenance_macros::verifies;
use provenance_porcelain::registry::{CliForm, Command, Input, Interface, Presence};
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
/// A target action shows it in the help of the target-first form.
#[tokio::test]
#[verifies("rule_command_description_same_on_cli_and_mcp", examples)]
async fn porcelain_commands_show_one_description() {
    let (repo, path) = support::review_paths::init();
    let tools = tools(repo.path()).await;
    for command in Command::ALL {
        if !command.is_present(Interface::Mcp) || !command.is_present(Interface::Cli) {
            continue;
        }
        let declaration = command.declaration();
        let tool = &tools[declaration.name];
        assert_eq!(tool["description"], declaration.description, "{command:?}");
        let target_form = ["req_surface", declaration.name, "--repo", path.as_str()];
        let forms: Vec<&[&str]> = match declaration.cli_form {
            CliForm::Words(words) => vec![words],
            CliForm::WordsAndTarget(words) => vec![words, &target_form],
            CliForm::Target => vec![&target_form],
        };
        for words in forms {
            let help = help(words);
            assert!(
                help.contains(declaration.description),
                "provenance {} --help does not show the MCP description:\n{help}",
                words.join(" ")
            );
        }
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

/// Each of `commands` is a CLI-only registry command, and its help shows each declared flag.
fn assert_cli_only_registry_commands(commands: &[&[&str]]) {
    for words in commands {
        let command = Command::find(Interface::Cli, words)
            .unwrap_or_else(|| panic!("provenance {} is not a registry command", words.join(" ")));
        assert!(
            !command.is_present(Interface::Mcp),
            "{command:?} is CLI-only"
        );
        let help = help(words);
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

/// The repository commands come from the registry, and their help shows each declared flag.
#[test]
#[verifies("rule_porcelain_interface_shows_recorded_commands", examples)]
fn repository_commands_are_registry_commands() {
    assert_cli_only_registry_commands(&[
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
    ]);
}

/// The graph read commands come from the registry, and their help shows each declared flag.
#[test]
#[verifies("rule_porcelain_interface_shows_recorded_commands", examples)]
fn graph_read_commands_are_registry_commands() {
    assert_cli_only_registry_commands(&[
        &["graph-reference", "issue"],
        &["graph-reference", "show"],
        &["graph-reference", "verify"],
        &["graph-reference", "exact-export"],
        &["swarm-backtrace", "land"],
    ]);
}

/// The workspace commands come from the registry, and their help shows each declared flag.
#[test]
#[verifies("rule_porcelain_interface_shows_recorded_commands", examples)]
fn workspace_commands_are_registry_commands() {
    assert_cli_only_registry_commands(&[
        &["docs", "check"],
        &["docs", "serve"],
        &["skills", "list"],
        &["skills", "show"],
        &["skills", "install"],
        &["report", "render"],
        &["report", "build"],
        &["review"],
        &["mcp"],
    ]);
}

/// The command names that `provenance <words...> --help` lists, without `help`.
fn listed_commands(words: &[&str]) -> Vec<String> {
    help(words)
        .replace("provenance.exe", "provenance")
        .lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .filter(|name| name != "help")
        .collect()
}

/// Each command that the CLI help lists is a registry command that the CLI shows, or a
/// group of such commands.
#[test]
#[verifies("rule_porcelain_registry_defines_commands", exhaustion)]
fn every_cli_command_is_a_registry_command() {
    let mut groups = vec![Vec::<String>::new()];
    while let Some(group) = groups.pop() {
        let group_words = group.iter().map(String::as_str).collect::<Vec<_>>();
        for name in listed_commands(&group_words) {
            let mut path = group.clone();
            path.push(name);
            let words = path.iter().map(String::as_str).collect::<Vec<_>>();
            if Command::find(Interface::Cli, &words).is_some() {
                continue;
            }
            assert!(
                !listed_commands(&words).is_empty(),
                "provenance {} is not a registry command",
                words.join(" ")
            );
            groups.push(path);
        }
    }
}

/// Each tool that a served host lists is a registry command that MCP shows.
#[tokio::test]
#[verifies("rule_porcelain_registry_defines_commands", exhaustion)]
async fn every_mcp_tool_is_a_registry_command() {
    let (repo, _) = support::review_paths::init();
    for name in tools(repo.path()).await.keys() {
        assert!(
            Command::find(Interface::Mcp, &[name.as_str()]).is_some(),
            "MCP tool {name} is not a registry command"
        );
    }
}
