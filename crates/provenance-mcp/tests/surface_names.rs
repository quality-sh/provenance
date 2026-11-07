//! Characterization of the MCP surface, without a Rule binding.
//!
//! This test pins the input schema of each MCP tool, so a change to a property name shows
//! here first. A changed name changes its pin in the same commit. The CLI help pins are in
//! `provenance-cli/tests/surface_names.rs`.
#![cfg(feature = "test-fixture")]

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/check_port.rs"]
    pub mod check_port;
}

use provenance_transport::{
    fixture::{FixtureAccess, Target},
    repository::RepoContext,
    StatementHost,
};
use rmcp::ServiceExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use support::check_port::FixtureCheckPort;

fn pin_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/surface_names")
        .join(name)
}

/// A full-access host over `repo` with a check port and the repository files, so the
/// host lists each tool and each action.
fn host(repo: &Path) -> StatementHost {
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
    StatementHost::with_fixture_access(access)
        .with_check_port(Arc::new(FixtureCheckPort))
        .with_repository(RepoContext::new(
            repo.to_str().unwrap().to_owned(),
            "default",
        ))
}

#[tokio::test]
async fn mcp_input_schema_of_each_tool_is_pinned() {
    let repo = tempfile::tempdir().unwrap();
    let host = host(repo.path());
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();

    let mut changed = Vec::new();
    for tool in tools {
        let name = format!("mcp_{}.json", tool.name);
        let mut text = serde_json::to_string_pretty(&tool.input_schema).unwrap();
        text.push('\n');
        let expected = std::fs::read_to_string(pin_path(&name)).ok();
        if expected.as_deref() != Some(text.as_str()) {
            // A missing or different pin prints the actual text between marker lines, so a
            // reviewer can read the change.
            println!("=== PIN {name}\n{text}=== END PIN {name}");
            changed.push(name);
        }
    }
    assert!(changed.is_empty(), "changed pins: {changed:?}");
}
