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
    opened: Option<bool>,
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
    no_open: bool,
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
        Some(host) if no_open => browser_output(host, &target, no_open),
        // A connected page shows the record, so its drafts and position stay.
        Some(host) => match connected_page(host, &target).await {
            PageResult::Shown => LinkOutput {
                review_url: Some(plain_link(host, &target)),
                opened: Some(true),
                message: None,
            },
            PageResult::Waiting => kept_output(
                plain_link(host, &target),
                "a connected review page got the link, so no browser opens; open the printed URL if the page does not show it",
            ),
            PageResult::Replaced => kept_output(
                plain_link(host, &target),
                "a newer review link replaced this link in the open review page, so no browser opens",
            ),
            PageResult::NoPage => browser_output(host, &target, no_open),
        },
        None => LinkOutput {
            review_url: None,
            opened: None,
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
        Some(host) => value["data"]["review_url"] = Value::String(plain_link(host, &target)),
        None => value["data"]["review_message"] = Value::String(start_message(context)),
    }
    Ok(())
}

/// Opens the record in a new browser page, signed in when the host issues a launch code.
fn browser_output(host: &DiscoveredLocalHost, target: &ReviewTarget, no_open: bool) -> LinkOutput {
    match launch_link(host, target) {
        Ok(launch) => opened_output(launch, plain_link(host, target), no_open),
        Err(error) => signed_out_output(plain_link(host, target), &error),
    }
}

/// Prints the plain review link when a connected page has the link but does not show it.
fn kept_output(plain: String, warning: &str) -> LinkOutput {
    eprintln!("warning: {warning}");
    LinkOutput {
        review_url: Some(plain),
        opened: Some(false),
        message: None,
    }
}

/// Opens the launch link, or puts it in the output for the caller when the CLI cannot open it.
#[rule("rule_review_link_printed_when_not_opened")]
fn opened_output(launch: url::Url, plain: String, no_open: bool) -> LinkOutput {
    match crate::browser::open_or_print(&launch, no_open) {
        crate::browser::Opening::Opened => LinkOutput {
            review_url: Some(plain),
            opened: Some(true),
            message: None,
        },
        crate::browser::Opening::Printed(_) => LinkOutput {
            review_url: Some(launch.into()),
            opened: Some(false),
            message: None,
        },
    }
}

/// Prints the plain review link when the host cannot sign the page in through this CLI.
fn signed_out_output(plain: String, error: &anyhow::Error) -> LinkOutput {
    eprintln!(
        "warning: the review link cannot sign the page in, so open it and sign in: {error:#}"
    );
    LinkOutput {
        review_url: Some(plain),
        opened: Some(false),
        message: None,
    }
}

fn plain_link(host: &DiscoveredLocalHost, target: &ReviewTarget) -> String {
    build_url(
        host.endpoint(),
        target.root.as_str(),
        target.focus.as_deref(),
    )
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
    let Some(root) = roots.first() else {
        return Ok(None);
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
        "No review host is running. Start it with `provenance review --repo {} --repository-id local --scope {} --detach`.",
        context.repo, context.scope
    )
}

/// Adds a single-use launch code that signs the review page in when the link opens.
#[rule("rule_review_link_opens_signed_in")]
fn launch_link(host: &DiscoveredLocalHost, target: &ReviewTarget) -> anyhow::Result<url::Url> {
    let mut link = url::Url::parse(&plain_link(host, target))?;
    link.set_fragment(Some(&format!("launch={}", issue_launch_code(host)?)));
    Ok(link)
}

/// Gets a single-use launch code from the host, with the key that only its user can read.
pub fn issue_launch_code(host: &DiscoveredLocalHost) -> anyhow::Result<String> {
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
    Ok(code.to_owned())
}

/// What a connected page did with a review link, as the host reports it.
#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum PageResult {
    Shown,
    Waiting,
    Replaced,
    NoPage,
}

// The host ends its wait for the page after five seconds; this limit covers a host that does
// not answer.
const NAVIGATION_TIMEOUT: Duration = Duration::from_secs(15);

/// Asks the host to show the record in a connected page. Only a link that no page received
/// uses the browser.
async fn connected_page(host: &DiscoveredLocalHost, target: &ReviewTarget) -> PageResult {
    let Ok(key) = crate::review::launch::LaunchKey::read(host.instance_nonce()) else {
        return PageResult::NoPage;
    };
    let mut url = host.endpoint().clone();
    url.set_path("/review-navigation");
    let body = serde_json::json!({
        "launchKey": key,
        "rootId": target.root.as_str(),
        "focusId": target.focus,
    });
    tokio::task::spawn_blocking(move || {
        ureq::AgentBuilder::new()
            .redirects(0)
            .timeout(NAVIGATION_TIMEOUT)
            .build()
            .post(url.as_str())
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .ok()
            .and_then(|response| response.into_string().ok())
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|reply| serde_json::from_value(reply["result"].clone()).ok())
            .unwrap_or(PageResult::NoPage)
    })
    .await
    .unwrap_or(PageResult::NoPage)
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
