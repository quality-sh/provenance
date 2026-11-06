//! Reads the two command surfaces from outside: the help of the spawned binary and the
//! MCP `tools/list` of a served host.

use provenance_cli::cli_form;
use provenance_mcp::Client;
use provenance_porcelain::check::{Category, CategoryRun, PortFuture, Refusal};
use provenance_porcelain::registry;
use provenance_transport::fixture::{FixtureAccess, Target};
use provenance_transport::StatementHost;
use serde_json::Value;
use std::collections::BTreeMap;
use std::process::Command;
use std::sync::Arc;

/// The help text of `provenance <words...> --help`.
pub fn help(words: &[&str]) -> String {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(words)
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success(), "{words:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// The words that select one CLI word command.
pub fn words(command: registry::Command) -> Option<Vec<&'static str>> {
    cli_form::declaration(command).words()
}

/// A check port that finds nothing, so a served host lists the check tool.
struct EmptyCheckPort;

impl provenance_porcelain::check::CheckPort for EmptyCheckPort {
    fn run<'a>(&'a self, category: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async move {
            Ok(match category {
                Category::Graph => CategoryRun::Graph {
                    findings: Vec::new(),
                    refusal: Refusal::None,
                },
                Category::Statements => CategoryRun::Statements {
                    findings: Vec::new(),
                    context: None,
                    refusal: Refusal::None,
                },
                Category::Bindings => CategoryRun::Bindings {
                    findings: Vec::new(),
                    context: provenance_porcelain::check::BindingContext {
                        policy: provenance_porcelain::check::BindingPolicy::Warning,
                    },
                    refusal: Refusal::None,
                },
                Category::Gaps => CategoryRun::Gaps {
                    findings: Vec::new(),
                    context: provenance_porcelain::check::GapsContext { scopes: Vec::new() },
                },
            })
        })
    }
}

/// The listed tools, by name, of a full-access host over `repo` with a check port.
pub async fn tools(repo: &std::path::Path) -> BTreeMap<String, Value> {
    let access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repo.to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
    .allow_writes();
    let host = StatementHost::with_fixture_access(access).with_check_port(Arc::new(EmptyCheckPort));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = Client::connect(client_io).await.unwrap();
    let tools = client
        .tools()
        .await
        .into_iter()
        .map(|tool| (tool["name"].as_str().unwrap().to_owned(), tool))
        .collect();
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
    tools
}
