//! Client test host with isolated repositories; stdin EOF stops the listener.
use provenance_core::{protocol::GetQuery, ScopeId};
use provenance_store::operations::{queries, read_policy::ReadPolicy};
use provenance_transport::{
    fixture::{records::Repository, FixtureAccess, Target},
    StatementHost,
};
use serde_json::json;
use std::io::Write;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let first = Repository::new("The first shared graph is selected.");
    first.all_kinds();
    let evidence_base = first.evidence();
    first.add_scope("other", "The other shared scope is selected.");
    queries::get(
        Some(first.dir.path().to_str().unwrap().into()),
        &ScopeId::new("default")?,
        ReadPolicy::default(),
        GetQuery {
            protocol_version: None,
            node_type: provenance_core::NodeType::Rule,
            id: "rule_shared".into(),
        },
    )
    .await?;
    first.edit("default", "The saved first shared graph is selected.");
    let second = Repository::new("The second shared graph is selected.");
    second.add_scope("other", "The second other scope is selected.");
    let stale = Repository::new("The old shared graph is selected.");
    let empty = Repository::new("The unmaterialized shared graph is selected.");
    queries::get(
        Some(stale.dir.path().to_str().unwrap().into()),
        &ScopeId::new("default")?,
        ReadPolicy::default(),
        GetQuery {
            protocol_version: None,
            node_type: provenance_core::NodeType::Rule,
            id: "rule_shared".into(),
        },
    )
    .await?;
    stale.edit("default", "The edited shared graph is selected.");
    let targets = [
        ("first", &first),
        ("second", &second),
        ("stale", &stale),
        ("unmaterialized", &empty),
        ("denied", &second),
    ]
    .into_iter()
    .map(|(id, repo)| Target {
        id: id.into(),
        root: repo.dir.path().to_path_buf(),
    })
    .collect();
    let grants = ["first", "second", "stale", "unmaterialized"]
        .into_iter()
        .flat_map(|target| {
            ["default", "other", "missing"].map(move |scope| (target.to_owned(), scope.to_owned()))
        })
        .collect();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let authority = listener.local_addr()?.to_string();
    let host = StatementHost::with_fixture_access(FixtureAccess::new(
        targets,
        grants,
        "fixture-secret",
        &authority,
    )?);
    println!(
        "{}",
        json!({"evidence":{"rule_id":"rule_shared","file":"code.rs","base":evidence_base,"target":"first","no_git_target":"second"},"url":format!("http://{authority}"),"bearer":"fixture-secret","targets":{"first":"first","second":"second"},"nodes":{"domain":"domain_shared","boundary":"boundary_shared","requirement":"req_shared","rule":"rule_shared","source":"source_shared","resolution":"resolution_shared","topic":"topic_shared","question":"question_shared"},"shared_rule":"rule_shared","expected":{"first":"The saved first shared graph is selected.","second":"The second shared graph is selected.","other":"The other shared scope is selected."},"stale_target":"stale","unmaterialized_target":"unmaterialized","denied_target":"denied"})
    );
    std::io::stdout().flush()?;
    let closing = host.clone();
    axum::serve(listener, host.router())
        .with_graceful_shutdown(async move {
            let mut buffer = [0; 256];
            while matches!(tokio::io::stdin().read(&mut buffer).await, Ok(n) if n > 0) {}
            closing.shutdown().await;
        })
        .await?;
    host.shutdown().await;
    Ok(())
}
