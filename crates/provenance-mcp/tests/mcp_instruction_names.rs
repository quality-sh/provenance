//! The server instructions and the change tool description name each tool and each action
//! that the server lists, with the name that the server lists, and no other command.
#![cfg(feature = "test-fixture")]

use crate::mcp_instructions::{host_without, operations_of, served_tools};
use crate::support::{fixture_host, records::Repository};
use provenance_porcelain::action::Action;
use provenance_store::operations::catalog::{self, Operation as _};
use provenance_transport::repository::RepoContext;
use rmcp::model::Tool;

/// Whether `text` names `name`: a space before it, and a space, a comma, or a period
/// after it.
fn names(text: &str, name: &str) -> bool {
    [" ", ",", "."]
        .iter()
        .any(|after| text.contains(&format!(" {name}{after}")))
}

/// The names of the tools that the server lists and of the action values of each tool.
fn listed_names(tools: &[Tool]) -> Vec<String> {
    let mut names = Vec::new();
    for tool in tools {
        names.push(tool.name.to_string());
        let properties = tool
            .input_schema
            .get("properties")
            .cloned()
            .unwrap_or_default();
        for value in properties["action"]["enum"]
            .as_array()
            .into_iter()
            .flatten()
        {
            names.extend(value.as_str().map(str::to_owned));
        }
    }
    names
}

/// Whether `name` is the MCP name of a command. An action value that is not, as the
/// notify write, has no line: the change line names it.
fn is_command(name: &str) -> bool {
    provenance_mcp::exposed::EXPOSED
        .iter()
        .any(|(command, _)| provenance_mcp::exposed::name(*command) == name)
}

/// The names of the command lines of the instructions.
fn line_names(instructions: &str) -> Vec<&str> {
    let (_, actions) = instructions.split_once("\n## Actions\n").unwrap();
    actions
        .lines()
        .filter_map(|line| line.strip_prefix("- ")?.split_once(": "))
        .map(|(name, _)| name)
        .collect()
}

/// The instructions have a line for each listed tool and action value that is a command,
/// and each line names a listed tool or action value. The change tool description names
/// each of its action values, is the change line of the instructions, and names no other
/// spelling of a command that MCP shows.
fn assert_names_listed(tools: &[Tool], instructions: &str) {
    let listed = listed_names(tools);
    let lines = line_names(instructions);
    for name in &listed {
        assert!(
            !is_command(name) || lines.contains(&name.as_str()),
            "the instructions do not name {name}, which the server lists:\n{instructions}"
        );
    }
    for name in &lines {
        assert!(
            listed.iter().any(|listed| listed == name),
            "the instructions name {name}, which the server does not list:\n{instructions}"
        );
    }
    let change = tools.iter().find(|tool| tool.name == "change").unwrap();
    let description = change.description.as_deref().unwrap();
    for action in change.input_schema["properties"]["action"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|action| action.as_str())
    {
        assert!(
            names(description, action),
            "{action} is not named: {description}"
        );
    }
    assert!(
        instructions.contains(&format!("\n- change: {description}")),
        "the change line is not the change tool description:\n{instructions}"
    );
    for (command, _) in provenance_mcp::exposed::EXPOSED {
        let declared = command.declaration().name;
        if declared != provenance_mcp::exposed::name(*command) {
            for text in [description, instructions] {
                assert!(
                    !text.contains(declared),
                    "MCP does not accept {declared}, which the text names: {text}"
                );
            }
        }
    }
}

#[tokio::test]
#[provenance_macros::verifies("rule_guidance_names_accepted_commands", examples)]
async fn a_host_with_every_write_names_each_tool_and_action_that_it_lists() {
    let repository = Repository::new("The shared graph is readable.");
    let root = repository.dir.path().to_str().unwrap().to_owned();
    let host = fixture_host::full(&repository).with_repository(RepoContext::new(root, "default"));
    let (tools, instructions) = served_tools(host).await;
    let listed = listed_names(&tools);
    assert!(
        listed.iter().any(|name| name == "dictionary_import"),
        "{listed:?}"
    );
    assert_names_listed(&tools, &instructions);
}

/// The host has no repository writes and no claim or release.
#[tokio::test]
#[provenance_macros::verifies("rule_guidance_names_accepted_commands", examples)]
async fn a_host_with_fewer_writes_names_each_tool_and_action_that_it_lists() {
    let repository = Repository::new("The shared graph is readable.");
    let denied = operations_of(&[Action::Claim, Action::Release]);
    let (tools, instructions) = served_tools(host_without(&repository, &denied)).await;
    let listed = listed_names(&tools);
    assert!(listed.iter().any(|name| name == "update"), "{listed:?}");
    for name in ["claim", "release", "materialize"] {
        assert!(!listed.iter().any(|listed| listed == name), "{listed:?}");
    }
    assert_names_listed(&tools, &instructions);
}

/// The description of the discussions tool is the description of a read that the host
/// grants: a host that grants the read of one Discussion and not the list gets the
/// description of that read, which the instructions also give.
#[tokio::test]
#[provenance_macros::verifies("rule_guidance_names_accepted_commands", examples)]
async fn the_discussions_tool_describes_only_the_reads_that_the_host_grants() {
    let repository = Repository::new("The shared graph is readable.");
    let hosts = [
        fixture_host::full(&repository),
        host_without(&repository, &[catalog::ListDiscussions::NAME]),
        host_without(&repository, &[catalog::GetDiscussionConversation::NAME]),
    ];
    for host in hosts {
        let (tools, instructions) = served_tools(host).await;
        let tool = tools
            .iter()
            .find(|tool| tool.name == "discussions")
            .unwrap();
        let description = tool.description.as_deref().unwrap();
        assert!(
            instructions
                .lines()
                .any(|line| line.starts_with("- ") && line.ends_with(&format!(": {description}"))),
            "the discussions tool description is not the line of a shown command: \
             {description}\n{instructions}"
        );
    }
}
