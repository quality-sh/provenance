use super::grammar::{DiscussionsArgs, TargetArgs};
use provenance_cli::porcelain;
use provenance_core::StableId;
use provenance_porcelain::{
    action::Action,
    discussion::{self, DiscussionOutcome},
};
use serde_json::{json, Value};

pub async fn dispatch_root(
    args: DiscussionsArgs,
    matches: &clap::ArgMatches,
) -> anyhow::Result<()> {
    let (action, target) = if let Some(id) = args.discussion_id {
        anyhow::ensure!(
            args.action.as_deref() == Some("get"),
            "an addressed Discussion requires get"
        );
        (Action::Discussion, Some(StableId::new(id)?))
    } else {
        anyhow::ensure!(args.action.is_none(), "get requires a Discussion ID");
        (Action::Discussions, None)
    };
    let schema = discussion::input_schema(action);
    let mut input = crate::catalog_cli::fields::schema_input(
        &schema,
        matches,
        &["parent", "discussion_id"],
        &[],
    )
    .unwrap_or_else(|error| crate::catalog_cli::usage_error(error));
    if let Some(target) = target {
        input.insert("discussion_id".into(), json!(target));
    }
    let host = porcelain::local_host(&args.common.repo, &args.common.scope)?;
    let service = provenance_porcelain::Porcelain::new(
        provenance_transport::porcelain::HostDiscussionPort::new(host),
    );
    let outcome = service
        .execute_discussion(action, Value::Object(input))
        .await?;
    print(&outcome, args.common.format())
}

pub async fn dispatch_target(
    args: TargetArgs,
    action: Action,
    matches: &clap::ArgMatches,
) -> anyhow::Result<()> {
    let schema = discussion::input_schema(action);
    let mut input = crate::catalog_cli::fields::schema_input(
        &schema,
        matches,
        &["parent", "discussion_id"],
        &["limit"],
    )
    .unwrap_or_else(|error| crate::catalog_cli::usage_error(error));
    let target = StableId::new(args.target)?;
    let host = porcelain::local_host(&args.common.repo, &args.common.scope)?;
    let service = provenance_porcelain::Porcelain::new(
        provenance_transport::porcelain::HostDiscussionPort::new(host.clone()),
    );
    let target_field = discussion::target_field(action);
    let identity = if target_field == "parent" {
        let resolver = provenance_porcelain::Porcelain::new(
            provenance_transport::porcelain::HostGetPort::new(host),
        );
        serde_json::to_value(resolver.select_parent(target.as_str()).await?)?
    } else {
        json!(target)
    };
    input.insert(target_field.into(), identity);
    for (field, value) in [("actor", "cli"), ("role", "user")] {
        if schema["properties"].get(field).is_some() {
            input.entry(field).or_insert_with(|| json!(value));
        }
    }
    if schema["properties"].get("request_id").is_some() {
        input
            .entry("request_id")
            .or_insert_with(|| json!(uuid::Uuid::new_v4().to_string()));
    }
    let outcome = service
        .execute_discussion(action, Value::Object(input))
        .await?;
    print(&outcome, args.common.format())
}

fn print(
    outcome: &DiscussionOutcome,
    format: Option<porcelain::OutputFormat>,
) -> anyhow::Result<()> {
    if format == Some(porcelain::OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(outcome)?);
    } else {
        println!(
            "{}",
            provenance_porcelain::discussion::render_readable(outcome)
        );
    }
    Ok(())
}
