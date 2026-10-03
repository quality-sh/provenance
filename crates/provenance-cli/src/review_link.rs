//! Builds reviewer-facing URLs from verified review hosts and canonical document reads.

use provenance_cli::repo_context::RepoContext;
use provenance_core::{NodeType, StableId};
use provenance_macros::rule;
use provenance_porcelain::get::{GetInput, View};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
struct LaunchCode {
    code: String,
}
pub async fn print(
    context: &RepoContext,
    record_id: &str,
    format: Option<provenance_cli::porcelain::OutputFormat>,
    no_open: bool,
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
    let url = output.review_url.clone();
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
    }?;
    if let Some(url) = url {
        crate::browser_open::open(&url, no_open);
    }
    Ok(())
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

/// Builds the direct record link that an agent gives to a person for review.
#[rule("rule_agent_review_request_includes_link")]
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
    let Some(host) =
        provenance_transport::local_host::discover(context.repo.as_std_path(), &context.scope)?
    else {
        return Ok(Some(LinkOutput {
            review_url: None,
            message: Some(start_message(context)),
        }));
    };
    let focus = (root.as_str() != record.id).then_some(record.id.as_str());
    let code = mint_code(&host).await?;
    Ok(Some(LinkOutput {
        review_url: Some(build_url(host.endpoint(), root.as_str(), focus, &code)),
        message: None,
    }))
}

async fn mint_code(
    host: &provenance_transport::local_host::DiscoveredLocalHost,
) -> anyhow::Result<String> {
    let endpoint = host.endpoint().clone();
    let identity = host.identity().clone();
    tokio::task::spawn_blocking(move || {
        let mut url = endpoint;
        url.set_path("/review-launch");
        let response = ureq::post(url.as_str())
            .set("Content-Type", "application/json")
            .send_string(&serde_json::to_string(&identity)?)?;
        Ok::<_, anyhow::Error>(serde_json::from_reader::<_, LaunchCode>(response.into_reader())?.code)
    })
    .await?
}
fn start_message(context: &RepoContext) -> String {
    format!(
        "No review host is running. Start it with `provenance review --repo {} --repository-id local --scope {}`.",
        context.repo, context.scope
    )
}

/// Puts the review document and optional focused record into the review URL.
#[rule("rule_review_link_needs_no_record_id")]
fn build_url(endpoint: &url::Url, root: &str, focus: Option<&str>, code: &str) -> String {
    let mut url = endpoint.clone();
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("root", root);
        if let Some(focus) = focus {
            query.append_pair("focus", focus);
        }
        query.append_pair("code", code);
    }
    url.into()
}

#[cfg(test)]
mod tests {
    use super::build_url;
    use provenance_macros::verifies;

    #[test]
    #[verifies("rule_agent_review_request_includes_link", examples)]
    fn link_builder_encodes_record_ids() {
        let endpoint = url::Url::parse("http://127.0.0.1:1234/").unwrap();
        let url = build_url(&endpoint, "req root", Some("rule/focus"), "one-use-code");
        assert_eq!(
            url,
            "http://127.0.0.1:1234/?root=req+root&focus=rule%2Ffocus&code=one-use-code"
        );
    }
}
