//! Builds reviewer-facing URLs from verified review hosts and canonical document reads.

use anyhow::Context as _;
use provenance_cli::repo_context::RepoContext;
use provenance_core::{NodeType, StableId};
use provenance_macros::rule;
use provenance_porcelain::get::{GetInput, View};
use provenance_transport::local_host::{DiscoveredLocalHost, LAUNCH_CODE_ROUTE};
use serde::Serialize;
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

/// The review document of a record and the verified host that serves it, if one runs.
struct ReviewTarget {
    host: Option<DiscoveredLocalHost>,
    root: StableId,
    focus: Option<String>,
}

#[rule("rule_agent_review_request_includes_link")]
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
    let target = review_target(
        context,
        &AffectedReviewRecord {
            kind: record.record.node_type(),
            id: record_id.to_owned(),
        },
    )
    .await?
    .ok_or_else(|| anyhow::anyhow!("record {record_id} is not in a Requirement review document"))?;
    let output = match &target.host {
        Some(host) => LinkOutput {
            review_url: Some(launch_link(host, &target)?),
            message: None,
        },
        None => LinkOutput {
            review_url: None,
            message: Some(start_message(context)),
        },
    };
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

#[rule("rule_cli_record_review_action_returns_url")]
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
    let Some(target) = review_target(context, affected).await? else {
        return Ok(());
    };
    match &target.host {
        Some(host) => {
            value["data"]["review_url"] = Value::String(build_url(
                host.endpoint(),
                target.root.as_str(),
                target.focus.as_deref(),
            ));
        }
        None => value["data"]["review_message"] = Value::String(start_message(context)),
    }
    Ok(())
}

/// Finds the review document that holds the record and the host that can show it.
async fn review_target(
    context: &RepoContext,
    record: &AffectedReviewRecord,
) -> anyhow::Result<Option<ReviewTarget>> {
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
    let host =
        provenance_transport::local_host::discover(context.repo.as_std_path(), &context.scope)?;
    Ok(Some(ReviewTarget {
        host,
        root: root.clone(),
        focus: (root.as_str() != record.id).then(|| record.id.clone()),
    }))
}

fn start_message(context: &RepoContext) -> String {
    format!(
        "No review host is running. Start it with `provenance review --repo {} --repository-id local --scope {}`.",
        context.repo, context.scope
    )
}

/// Adds a single-use launch code that signs the review page in when the link opens.
#[rule("rule_review_link_opens_signed_in")]
fn launch_link(host: &DiscoveredLocalHost, target: &ReviewTarget) -> anyhow::Result<String> {
    let key = crate::review::launch::LaunchKey::read(host.instance_nonce())?;
    let mut url = host.endpoint().clone();
    url.set_path(LAUNCH_CODE_ROUTE);
    let response = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(5))
        .build()
        .post(url.as_str())
        .set("Content-Type", "application/json")
        .send_string(&serde_json::json!({ "launchKey": key }).to_string())
        .context("the review host did not issue a launch code")?;
    let body: Value = serde_json::from_str(&response.into_string()?)?;
    let code = body["code"]
        .as_str()
        .filter(|code| code.len() == 64 && code.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .context("the review host sent a launch code that is not valid")?;
    let mut link = url::Url::parse(&build_url(
        host.endpoint(),
        target.root.as_str(),
        target.focus.as_deref(),
    ))?;
    link.set_fragment(Some(&format!("launch={code}")));
    Ok(link.into())
}

/// Puts the review document and optional focused record into the review URL.
#[rule("rule_review_link_needs_no_record_id")]
fn build_url(endpoint: &url::Url, root: &str, focus: Option<&str>) -> String {
    let mut url = endpoint.clone();
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("root", root);
        if let Some(focus) = focus {
            query.append_pair("focus", focus);
        }
    }
    url.into()
}
