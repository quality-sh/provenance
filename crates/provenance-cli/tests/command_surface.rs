//! The spawned binary's help and the MCP `tools/list` of a served host show the commands
//! that each consumer records, each with its declared flags or schema properties. The
//! three server commands are the only CLI commands outside the registry.

mod support {
    pub mod review_paths;
    pub mod surface;
}

use provenance_cli::cli_options::flag_name;
use provenance_cli::exposed as cli;
use provenance_cli::servers::{Server, SERVERS};
use provenance_macros::verifies;
use provenance_mcp::exposed::{self as mcp, Place};
use provenance_porcelain::registry::{Command, Declaration, Field, Grammar, Input};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
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
    for command in mcp::tools().filter(|command| cli::shows(*command)) {
        let declaration = command.declaration();
        let tool = &tools[mcp::name(command)];
        assert_eq!(tool["description"], declaration.description, "{command:?}");
        let target_form = ["req_surface", declaration.name, "--repo", path.as_str()];
        let words = declaration.words().collect::<Vec<_>>();
        let forms: Vec<&[&str]> = match declaration.grammar {
            Grammar::Name => vec![&words],
            Grammar::NameAndTarget => vec![&words, &target_form],
            Grammar::Target => vec![&target_form],
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

/// The place where MCP shows `command`, when MCP records it.
fn mcp_place(command: Command) -> Option<Place> {
    mcp::EXPOSED
        .iter()
        .find(|(recorded, _)| *recorded == command)
        .map(|(_, place)| *place)
}

/// The values of one enum property of a listed tool.
fn values<'a>(tools: &'a BTreeMap<String, Value>, tool: &str, property: &str) -> Vec<&'a str> {
    let property = &tools[tool]["inputSchema"]["properties"][property];
    property["enum"]
        .as_array()
        .or_else(|| property["items"]["enum"].as_array())
        .map(|values| values.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Whether MCP shows `name` as a tool or as a value of an action or categories argument.
fn mcp_shows(tools: &BTreeMap<String, Value>, name: &str) -> bool {
    tools.contains_key(name)
        || tools.keys().any(|tool| {
            values(tools, tool, "action").contains(&name)
                || values(tools, tool, "categories").contains(&name)
        })
}

/// Each command that a consumer records appears there with its declared flags or schema
/// properties, and each other registry command does not appear there.
#[tokio::test]
#[verifies("rule_porcelain_interface_shows_recorded_commands", exhaustion)]
#[verifies("rule_mcp_shared_command_in_one_tool", exhaustion)]
async fn registry_commands_appear_on_both_interfaces() {
    let (repo, path) = support::review_paths::init();
    let tools = tools(repo.path()).await;
    let root = listed_commands(&[]);
    for command in Command::ALL {
        let declaration = command.declaration();
        if cli::shows(*command) {
            let help = words(*command).map_or_else(
                || help(&["req_surface", declaration.name, "--repo", &path]),
                |words| help(&words),
            );
            for field in declaration
                .input
                .fields()
                .iter()
                .filter(|field| !field.internal)
            {
                let shown = shown(&declaration, field);
                assert!(
                    help.contains(&shown),
                    "{command:?} help lacks {shown}:\n{help}"
                );
            }
        } else {
            let first = words(*command).expect("a CLI target action is on the CLI")[0];
            assert!(!root.iter().any(|name| name == first), "{command:?}");
        }
        // MCP spells the command name in snake_case: the same words, joined by `_`.
        let name = mcp::spelling(declaration.name);
        match mcp_place(*command) {
            Some(Place::Tool) => assert!(tools.contains_key(&name), "tools/list lacks {name}"),
            Some(Place::Action { tool }) => assert!(
                values(&tools, tool, "action").contains(&name.as_str()),
                "{command:?} is not an action of {tool}"
            ),
            Some(Place::Category { tool }) => assert!(
                values(&tools, tool, "categories").contains(&name.as_str()),
                "{command:?} is not a category of {tool}"
            ),
            None => assert!(!mcp_shows(&tools, &name), "{command:?}"),
        }
    }
}

/// The MCP input fields that name the target record of a command.
const TARGET_FIELDS: &[&str] = &["target", "parent", "discussion_id"];

/// Each command that the CLI and MCP both show has its name on both, and takes a target
/// record on MCP exactly when it takes one on the CLI.
#[tokio::test]
#[verifies("rule_shared_command_same_name_and_grammar", exhaustion)]
async fn shared_commands_have_one_name_and_target_form() {
    let (repo, path) = support::review_paths::init();
    let tools = tools(repo.path()).await;
    for (command, place) in mcp::EXPOSED
        .iter()
        .filter(|(command, _)| cli::shows(*command))
    {
        let declaration = command.declaration();
        if declaration.grammar.takes_name() {
            let words = declaration.words().collect::<Vec<_>>();
            assert_eq!(cli::find(&words), Some(*command), "{command:?}");
        } else {
            help(&["req_surface", declaration.name, "--repo", &path]);
        }
        let fields = match place {
            Place::Action { tool } => {
                support::surface::action_properties(&tools[*tool], mcp::name(*command))
            }
            Place::Tool => properties(&tools[mcp::name(*command)]["inputSchema"]),
            Place::Category { tool } => properties(&tools[*tool]["inputSchema"]),
        };
        let mcp_target = TARGET_FIELDS.iter().any(|field| fields.contains(*field));
        assert_eq!(
            mcp_target,
            declaration.grammar.takes_target(),
            "{command:?} target form"
        );
    }
}

/// For each porcelain command with a typed input, the CLI flags and the MCP properties
/// are the fields of that one input. The CLI shows each field as `--kebab-case`; MCP
/// shows the same field under its `snake_case` name, as a property of the listed tool or
/// of the schema that MCP derives for a command it does not list. A field that writes is
/// a property of the change action that writes it, not of the read tool.
#[tokio::test]
#[verifies("rule_shared_command_same_name_and_grammar", exhaustion)]
async fn cli_flags_and_mcp_properties_come_from_one_field_list() {
    let (repo, _) = support::review_paths::init();
    let tools = tools(repo.path()).await;
    let change = properties(&tools["change"]["inputSchema"]);
    let notify_listed = values(&tools, "change", "action").contains(&"notify");
    let mut compared = 0;
    for command in Command::ALL {
        let declaration = command.declaration();
        let Input::Fields(fields) = declaration.input else {
            continue;
        };
        if fields.schema.is_none() {
            continue;
        }
        let mut mcp = if mcp_place(*command) == Some(Place::Tool) {
            properties(&tools[mcp::name(*command)]["inputSchema"])
        } else {
            properties(&provenance_mcp::derived::input_schema(*command))
        };
        assert_eq!(
            mcp.remove("target"),
            declaration.grammar.takes_target(),
            "{command:?} target"
        );
        for field in fields.fields.iter().filter(|field| field.writes) {
            if notify_listed {
                assert!(
                    change.contains(field.name),
                    "{command:?}: change lacks {}",
                    field.name
                );
            }
            assert!(
                mcp.insert(field.name.to_owned()),
                "{command:?}: {} reads",
                field.name
            );
        }
        let cli = fields.fields.iter().map(flag_name).collect::<BTreeSet<_>>();
        let mcp = mcp
            .iter()
            .map(|property| property.replace('_', "-"))
            .collect::<BTreeSet<_>>();
        assert_eq!(cli, mcp, "{command:?}");
        compared += 1;
    }
    assert!(compared > 30, "only {compared} commands have a typed input");
}

/// Each record action and each Discussion action takes the same fields on the CLI and on
/// MCP. The CLI shows a field as `--kebab-case`, and MCP as the `snake_case` property of
/// the tool of the action. `--stdin` and each `--<field>-json` flag are the CLI way to
/// give a whole JSON value; MCP arguments are JSON already.
#[tokio::test]
#[verifies("rule_shared_command_same_name_and_grammar", exhaustion)]
async fn record_and_discussion_actions_take_the_same_fields() {
    use provenance_porcelain::action::Action;
    let (repo, _) = support::review_paths::init();
    let tools = tools(repo.path()).await;
    let cli_options = ["repo", "scope", "format", "quiet", "help", "stdin"];
    let mut compared = 0;
    for action in Action::RECORD.into_iter().chain(Action::DISCUSSION) {
        let word = action.as_str();
        let tool = if matches!(action, Action::Discussions | Action::Discussion) {
            "discussions"
        } else {
            "change"
        };
        let target = if matches!(action, Action::Discussion | Action::Reply) {
            "discussion_x"
        } else {
            "record_x"
        };
        let helps = if action == Action::Create {
            provenance_contract::target_definitions(action)
                .map(|(kind, _)| help(&[target, word, "--type", kind.as_str()]))
                .collect::<Vec<_>>()
        } else {
            vec![help(&[target, word])]
        };
        let mut cli = helps
            .iter()
            .flat_map(|text| support::surface::flags(text))
            .filter(|flag| !cli_options.contains(&flag.as_str()))
            .map(|flag| flag.trim_end_matches("-json").replace('-', "_"))
            .collect::<BTreeSet<_>>();
        cli.insert("target".into());
        let mcp = support::surface::action_properties(&tools[tool], word);
        assert_eq!(cli, mcp, "{word}");
        compared += 1;
    }
    assert_eq!(compared, 10);
}

/// Each repository command that MCP shows as a change action takes the same fields on
/// the CLI and on MCP. The CLI shows each field as `--kebab-case` and takes its own
/// repository and output options; MCP binds the repository and scope when it starts.
#[tokio::test]
#[verifies("rule_shared_command_same_name_and_grammar", exhaustion)]
async fn repository_change_actions_take_the_same_fields() {
    let (repo, _) = support::review_paths::init();
    let tools = tools(repo.path()).await;
    let cli_options = ["repo", "scope", "format", "quiet", "help"];
    let mut compared = 0;
    for (command, place) in mcp::EXPOSED {
        let in_change = *place == (Place::Action { tool: "change" });
        if !in_change || !matches!(command, Command::Repository(_) | Command::Graph(_)) {
            continue;
        }
        let words = words(*command).expect("a word command");
        // The action value is the CLI command path in snake_case: the same words.
        assert_eq!(
            mcp::name(*command),
            words.join("_").replace('-', "_"),
            "{command:?} action value"
        );
        let help = help(&words);
        let cli = support::surface::flags(&help)
            .into_iter()
            .filter(|flag| !cli_options.contains(&flag.as_str()))
            .map(|flag| flag.replace('-', "_"))
            .collect::<BTreeSet<_>>();
        let mcp = support::surface::action_properties(&tools["change"], mcp::name(*command));
        assert_eq!(cli, mcp, "{command:?}");
        compared += 1;
    }
    assert_eq!(compared, 4);
}

/// How the help shows one field: a target field as a positional argument, and each
/// other field as a flag.
fn shown(declaration: &Declaration, field: &Field) -> String {
    if declaration.input.target().contains(&field.name) {
        let name = field.name.to_uppercase();
        if field.required {
            format!("<{name}>")
        } else {
            format!("[{name}]")
        }
    } else {
        format!("--{}", flag_name(field))
    }
}

/// Each list holds each command at most once, and each MCP place names a listed tool.
#[test]
#[verifies("rule_porcelain_interface_records_each_command", exhaustion)]
fn each_consumer_records_each_command_once() {
    for command in Command::ALL {
        assert!(
            cli::EXPOSED.iter().filter(|held| *held == command).count() <= 1,
            "{command:?}"
        );
        assert!(
            mcp::EXPOSED
                .iter()
                .filter(|(held, _)| held == command)
                .count()
                <= 1,
            "{command:?}"
        );
    }
    let tools = mcp::tools()
        .map(|command| command.declaration().name)
        .collect::<Vec<_>>();
    for (command, place) in mcp::EXPOSED {
        if let Place::Action { tool } | Place::Category { tool } = place {
            assert!(tools.contains(tool), "{command:?} names {tool}");
        }
    }
}

/// Each of `commands` is a CLI-only registry command, and its help shows each declared flag.
fn assert_cli_only_registry_commands(commands: &[&[&str]]) {
    for words in commands {
        let command = cli::find(words)
            .unwrap_or_else(|| panic!("provenance {} is not a registry command", words.join(" ")));
        assert_eq!(mcp_place(command), None, "{command:?} is CLI-only");
        let help = help(words);
        let declaration = command.declaration();
        for field in declaration
            .input
            .fields()
            .iter()
            .filter(|field| !field.internal)
        {
            if !declaration.input.target().contains(&field.name) {
                let shown = format!("--{}", flag_name(field));
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
        &["export"],
        &["merge-jsonl"],
        &["validate"],
        &["schema", "show"],
        &["dictionary", "status"],
    ]);
}

/// The workspace commands come from the registry, and their help shows each declared flag.
#[test]
#[verifies("rule_porcelain_interface_shows_recorded_commands", examples)]
fn workspace_commands_are_registry_commands() {
    assert_cli_only_registry_commands(&[
        &["skills", "list"],
        &["skills", "show"],
        &["skills", "install"],
        &["report", "render"],
        &["report", "build"],
    ]);
}

/// The two server commands are CLI commands outside the registry. MCP does not show
/// them, and their help shows each declared flag.
#[tokio::test]
#[verifies("rule_porcelain_registry_defines_commands", examples)]
async fn server_commands_are_outside_the_registry() {
    let (repo, _) = support::review_paths::init();
    let tools = tools(repo.path()).await;
    for words in [&["review"][..], &["mcp"]] {
        let server = Server::find(words).unwrap_or_else(|| panic!("{words:?} is not a server"));
        let declaration = server.declaration();
        assert_eq!(cli::find(words), None, "{words:?}");
        assert_eq!(Command::named(declaration.name), None, "{words:?}");
        assert!(
            !mcp_shows(&tools, &mcp::spelling(declaration.name)),
            "{words:?}"
        );
        let help = help(words);
        for field in declaration.input.fields() {
            let shown = shown(&declaration, field);
            assert!(
                help.contains(&shown),
                "{words:?} help lacks {shown}:\n{help}"
            );
        }
    }
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

/// Each command that the CLI help lists is a registry command that the CLI shows, one
/// of the three server commands, or a group of such commands. Each server command is
/// listed.
#[test]
#[verifies("rule_porcelain_registry_defines_commands", exhaustion)]
fn every_cli_command_is_a_registry_command() {
    let mut servers = Vec::new();
    let mut groups = vec![Vec::<String>::new()];
    while let Some(group) = groups.pop() {
        let group_words = group.iter().map(String::as_str).collect::<Vec<_>>();
        for name in listed_commands(&group_words) {
            let mut path = group.clone();
            path.push(name);
            let words = path.iter().map(String::as_str).collect::<Vec<_>>();
            if cli::find(&words).is_some() {
                continue;
            }
            if let Some(server) = Server::find(&words) {
                servers.push(server);
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
    servers.sort_by_key(|server| server.declaration().name);
    let mut expected = SERVERS.to_vec();
    expected.sort_by_key(|server| server.declaration().name);
    assert_eq!(servers, expected);
}

/// Each tool that a served host lists is a registry command that MCP records as a tool.
#[tokio::test]
#[verifies("rule_porcelain_registry_defines_commands", exhaustion)]
async fn every_mcp_tool_is_a_registry_command() {
    let (repo, _) = support::review_paths::init();
    for name in tools(repo.path()).await.keys() {
        assert!(
            Command::named(name).is_some_and(|command| mcp_place(command) == Some(Place::Tool)),
            "MCP tool {name} is not a registry command"
        );
    }
}
