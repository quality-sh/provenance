//! Prints and opens the reviewer-facing links that the review library builds.

use anyhow::Context as _;
use provenance_cli::repo_context::RepoContext;
use provenance_macros::rule;
use provenance_porcelain::get::{GetInput, View, NO_REVIEW_HOST};
pub use provenance_review::AffectedReviewRecord;
use provenance_review::{PageResult, ReviewLocation};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
struct LinkOutput {
    review_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opened: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
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
    let location = provenance_review::review_location(
        &context.place(),
        &AffectedReviewRecord {
            kind: record.record.node_type(),
            id: record_id.to_owned(),
        },
    )
    .await?
    .ok_or_else(|| anyhow::anyhow!("record {record_id} is not in a Requirement review document"))?;
    let output = match location.plain_link() {
        Some(plain) if no_open => browser_output(&location, plain, no_open),
        // A connected page shows the record, so its drafts and position stay.
        Some(plain) => match location.show_in_connected_page().await {
            PageResult::Shown => LinkOutput {
                review_url: Some(plain.into()),
                opened: Some(true),
                message: None,
            },
            PageResult::Waiting => kept_output(
                plain.into(),
                "a connected review page got the link, so no browser opens; open the printed URL if the page does not show it",
            ),
            PageResult::Replaced => kept_output(
                plain.into(),
                "a newer review link replaced this link in the open review page, so no browser opens",
            ),
            PageResult::NoPage => browser_output(&location, plain, no_open),
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
    let Some(location) = provenance_review::review_location(&context.place(), affected).await?
    else {
        return Ok(());
    };
    match location.plain_link() {
        Some(plain) => value["data"]["review_url"] = Value::String(plain.into()),
        None => value["data"]["review_message"] = Value::String(start_message(context)),
    }
    Ok(())
}

/// Opens the record in a new browser page, signed in when the host issues a launch code.
fn browser_output(location: &ReviewLocation, plain: url::Url, no_open: bool) -> LinkOutput {
    let launch = location
        .launch_link()
        .and_then(|launch| launch.context(NO_REVIEW_HOST));
    match launch {
        Ok(launch) => opened_output(launch, plain.into(), no_open),
        Err(error) => signed_out_output(plain.into(), &error),
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

fn start_message(context: &RepoContext) -> String {
    format!(
        "No review host is running. Start it with `provenance review --repo {} --repository-id local --scope {} --detach`.",
        context.repo, context.scope
    )
}
