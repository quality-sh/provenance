//! The server instructions follow the tool list of the host. These tests have no Rule
//! binding: no Rule states which tool names the instructions give.
#![cfg(feature = "test-fixture")]

use crate::support::{fixture_host, mcp::Session, records::Repository};
use provenance_store::operations::catalog::{self, Operation as _};
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};

/// The first steps that a server with the full tool list adds to the shared guidance.
const FIRST_STEPS: &str = "\n\nFor a focused start, call search with {\"text\":\"<text>\"}, \
then get with {\"target\":\"<id>\",\"view\":\"children\",\"depth\":<N>}.";

/// The tool names and the instructions of one served host.
async fn served(host: StatementHost) -> (Vec<String>, String) {
    let session = Session::start(host).await;
    let instructions = session
        .client
        .peer_info()
        .and_then(|info| info.instructions.clone())
        .expect("server instructions");
    let names = session
        .client
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect();
    session.close().await;
    (names, instructions)
}

/// A host that writes the repository, without the operations that `denied` names.
fn host_without(repository: &Repository, denied: &[&str]) -> StatementHost {
    let access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repository.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
    .allow_writes();
    StatementHost::with_fixture_access(
        denied
            .iter()
            .fold(access, |access, operation| access.deny_operation(operation)),
    )
}

/// The tool names of a host that lists every tool.
async fn every_tool() -> Vec<String> {
    let repository = Repository::new("The shared graph is readable.");
    served(fixture_host::full(&repository)).await.0
}

/// Each tool name of the full list that `instructions` gives as a word, and that
/// `listed` does not have.
fn unlisted_names(every: &[String], listed: &[String], instructions: &str) -> Vec<String> {
    let words: Vec<&str> = instructions
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .collect();
    every
        .iter()
        .filter(|name| !listed.contains(name) && words.contains(&name.as_str()))
        .cloned()
        .collect()
}

#[tokio::test]
async fn a_host_with_every_tool_gives_the_shared_guidance_and_first_steps() {
    let repository = Repository::new("The shared graph is readable.");
    let (names, instructions) = served(fixture_host::full(&repository)).await;
    assert_eq!(
        names,
        [
            "get",
            "search",
            "api",
            "check",
            "trace",
            "change",
            "discussions"
        ]
    );
    assert_eq!(
        instructions,
        provenance_porcelain::guidance::guidance().guidance + FIRST_STEPS
    );
    assert_eq!(instructions, provenance_mcp::instructions());
}

#[tokio::test]
async fn a_host_with_api_and_change_names_no_other_tool() {
    let every = every_tool().await;
    let repository = Repository::new("The shared graph is readable.");
    let host = host_without(
        &repository,
        &[
            catalog::ResolveRecord::NAME,
            catalog::Search::NAME,
            catalog::ListDiscussions::NAME,
            catalog::GetDiscussionConversation::NAME,
        ],
    );
    let (names, instructions) = served(host).await;
    assert_eq!(names, ["api", "change"]);
    assert_eq!(
        unlisted_names(&every, &names, &instructions),
        Vec::<String>::new(),
        "{instructions}"
    );
    let shared = provenance_porcelain::guidance::guidance().guidance;
    let (domain, _) = shared.split_once("## Actions").unwrap();
    let lines: Vec<&str> = [
        "api", "change", "create", "update", "answer", "claim", "release", "submit", "discuss",
        "reply",
    ]
    .iter()
    .map(|name| {
        shared
            .lines()
            .find(|line| line.starts_with(&format!("- {name}: ")))
            .unwrap()
    })
    .collect();
    let expected = format!(
        "{domain}## Actions\n\n\
Create names an explicit record type; actions on an existing record infer its type.\n\
Available actions depend on the interface and its access. \
Each interface lists the inputs and actions it supports.\n\n{}",
        lines.join("\n")
    );
    assert_eq!(instructions, expected);
}

#[tokio::test]
async fn a_host_without_search_or_get_gives_no_first_steps() {
    let every = every_tool().await;
    let repository = Repository::new("The shared graph is readable.");
    let host = host_without(
        &repository,
        &[catalog::ResolveRecord::NAME, catalog::Search::NAME],
    );
    let (names, instructions) = served(host).await;
    for first_step in ["search", "get"] {
        assert!(!names.iter().any(|name| name == first_step), "{names:?}");
    }
    assert!(names.iter().any(|name| name == "change"), "{names:?}");
    assert!(
        !instructions.contains("For a focused start"),
        "{instructions}"
    );
    assert!(!instructions.contains("Use search"), "{instructions}");
    assert_eq!(
        unlisted_names(&every, &names, &instructions),
        Vec::<String>::new(),
        "{instructions}"
    );
}

#[tokio::test]
async fn a_host_with_no_tool_gives_only_the_domain_guidance() {
    let (names, instructions) = served(StatementHost::default()).await;
    assert_eq!(names, Vec::<String>::new());
    let shared = provenance_porcelain::guidance::guidance().guidance;
    let (domain, _) = shared.split_once("## Actions").unwrap();
    assert_eq!(
        instructions,
        format!(
            "{domain}## Actions\n\n\
Create names an explicit record type; actions on an existing record infer its type.\n\
Available actions depend on the interface and its access. \
Each interface lists the inputs and actions it supports."
        )
    );
}
