//! Builds reviewer-facing URLs from verified review hosts and canonical document reads.

use provenance_cli::repo_context::RepoContext;
use provenance_core::{NodeType, StableId};
use provenance_porcelain::get::{GetInput, View};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone)]
pub struct AffectedReviewRecord {
    pub kind: NodeType,
    pub id: String,
}

#[derive(Serialize)]
struct LinkOutput {
    review_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostIdentity {
    repository_id: String,
    scope: String,
    instance_nonce: String,
}

pub async fn print(
    context: &RepoContext,
    record_id: &str,
    format: Option<provenance_cli::porcelain::OutputFormat>,
) -> anyhow::Result<()> {
    let host = context.local_host()?;
    let record = host
        .porcelain()
        .get()
        .get(GetInput::new(record_id, View::Record))
        .await?;
    let output = link_output(
        context,
        &AffectedReviewRecord {
            kind: record.record.node_type(),
            id: record_id.to_owned(),
        },
    )
    .await?
    .ok_or_else(|| anyhow::anyhow!("record {record_id} is not in a Requirement review document"))?;
    if format == Some(provenance_cli::porcelain::OutputFormat::Json) {
        crate::output::print_json(&output)
    } else {
        println!(
            "{}",
            output
                .review_url
                .as_deref()
                .or(output.message.as_deref())
                .unwrap()
        );
        Ok(())
    }
}

pub async fn annotate_write(
    context: &RepoContext,
    affected: Option<AffectedReviewRecord>,
    value: &mut Value,
) {
    let Some(affected) = affected else {
        return;
    };
    if let Err(error) = try_annotate_write(context, &affected, value).await {
        eprintln!("warning: the write succeeded, but its review link is unavailable: {error}");
    }
}

async fn try_annotate_write(
    context: &RepoContext,
    affected: &AffectedReviewRecord,
    value: &mut Value,
) -> anyhow::Result<()> {
    let Some(output) = link_output(context, affected).await? else {
        return Ok(());
    };
    if let Some(url) = output.review_url {
        value["data"]["review_url"] = Value::String(url);
    } else if let Some(message) = output.message {
        value["data"]["review_message"] = Value::String(message);
    }
    Ok(())
}

async fn link_output(
    context: &RepoContext,
    record: &AffectedReviewRecord,
) -> anyhow::Result<Option<LinkOutput>> {
    let roots = provenance_store::operations::queries::containing_review_documents(
        Some(context.repo.clone()),
        &context.scope_id()?,
        provenance_store::operations::read_policy::ReadPolicy::default(),
        record.kind,
        &StableId::new(&record.id)?,
    )
    .await?
    .result;
    let [root] = roots.as_slice() else {
        if roots.is_empty() {
            return Ok(None);
        }
        anyhow::bail!(
            "record {} appears in multiple Requirement review documents: {}",
            record.id,
            roots
                .iter()
                .map(StableId::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        );
    };
    let Some(runtime) = running_host(context).await else {
        return Ok(Some(LinkOutput {
            review_url: None,
            message: Some(start_message(context)),
        }));
    };
    let focus = (root.as_str() != record.id).then_some(record.id.as_str());
    Ok(Some(LinkOutput {
        review_url: Some(build_url(&runtime.endpoint, root.as_str(), focus)?),
        message: None,
    }))
}

async fn running_host(context: &RepoContext) -> Option<crate::review_runtime::RunningHost> {
    let hosts = crate::review_runtime::read(context.repo.as_std_path(), &context.scope)?;
    if hosts.iter().any(|host| host.scope != context.scope) {
        return None;
    }
    for host in hosts.into_iter().rev() {
        if verified_host(host.clone()).await {
            return Some(host);
        }
    }
    None
}

async fn verified_host(runtime: crate::review_runtime::RunningHost) -> bool {
    let expected = runtime.clone();
    tokio::task::spawn_blocking(move || {
        let Ok(mut url) = crate::review_runtime::validate_endpoint(&runtime.endpoint) else {
            return false;
        };
        url.set_path("/review-host-identity");
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_millis(200))
            .timeout_read(Duration::from_millis(200))
            .timeout_write(Duration::from_millis(200))
            .build();
        let Ok(response) = agent.get(url.as_str()).call() else {
            return false;
        };
        let Ok(text) = response.into_string() else {
            return false;
        };
        let Ok(identity) = serde_json::from_str::<HostIdentity>(&text) else {
            return false;
        };
        identity.repository_id == expected.repository_id
            && identity.scope == expected.scope
            && identity.instance_nonce == expected.instance_nonce
    })
    .await
    .unwrap_or(false)
}

fn start_message(context: &RepoContext) -> String {
    format!(
        "No review host is running. Start it with `provenance review --repo {} --repository-id local --scope {}`.",
        context.repo, context.scope
    )
}

fn build_url(endpoint: &str, root: &str, focus: Option<&str>) -> anyhow::Result<String> {
    let mut url = crate::review_runtime::validate_endpoint(endpoint)?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("root", root);
        if let Some(focus) = focus {
            query.append_pair("focus", focus);
        }
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::build_url;

    #[test]
    fn link_builder_encodes_record_ids_and_never_adds_a_credential() {
        let url = build_url("http://127.0.0.1:1234/", "req root", Some("rule/focus")).unwrap();
        assert_eq!(
            url,
            "http://127.0.0.1:1234/?root=req+root&focus=rule%2Ffocus"
        );
        assert!(!url.contains("bearer"));
    }
}
