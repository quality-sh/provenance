use super::grammar::{DiscussionsArgs, TargetArgs};
use provenance_cli::porcelain;
use provenance_core::StableId;
use provenance_porcelain::{
    action::Action,
    discussion::{self, DiscussionOutcome},
};
use serde_json::{json, Value};

/// Lists the Discussions of a scope. One Discussion is read only with
/// `provenance <discussion-id> discussion`.
#[provenance_macros::rule("rule_cli_one_discussion_read_form")]
pub async fn dispatch_root(
    args: DiscussionsArgs,
    matches: &clap::ArgMatches,
) -> anyhow::Result<()> {
    if let Some(id) = args.discussion_id {
        anyhow::bail!(
            "`provenance discussions <discussion-id> get` was removed. Use \
             `provenance {id} discussion`"
        );
    }
    anyhow::ensure!(args.action.is_none(), "get requires a Discussion ID");
    let action = Action::Discussions;
    let schema = discussion::input_schema(action);
    let input = crate::catalog_cli::fields::schema_input(&schema, matches, &["target"], &[])
        .unwrap_or_else(|error| crate::catalog_cli::usage_error(error));
    let host = provenance_cli::porcelain::local_host(&args.common.context().repo)?;
    let request = host
        .porcelain()
        .get()
        .discussion_request(action, input)
        .await?;
    let outcome = host
        .porcelain()
        .discussion()
        .execute_discussion(action, request)
        .await?;
    print(&outcome, args.common.format())
}

pub async fn dispatch_target(
    args: TargetArgs,
    action: Action,
    matches: &clap::ArgMatches,
) -> anyhow::Result<()> {
    let schema = discussion::input_schema(action);
    let mut input =
        crate::catalog_cli::fields::schema_input(&schema, matches, &["target"], &["limit"])
            .unwrap_or_else(|error| crate::catalog_cli::usage_error(error));
    let target = StableId::new(args.target)?;
    input.insert("target".into(), json!(target));
    for (field, value) in [("actor", "cli"), ("role", "user")] {
        if schema["properties"].get(field).is_some() {
            input.entry(field).or_insert_with(|| json!(value));
        }
    }
    let host = provenance_cli::porcelain::local_host(&args.common.context().repo)?;
    let request = host
        .porcelain()
        .get()
        .discussion_request(action, input.clone())
        .await?;
    ensure_required_flags(&schema, action, &input);
    let outcome = host
        .porcelain()
        .discussion()
        .execute_discussion(action, request)
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

/// A Discussion action that lacks a required option is a usage error, so
/// the shell sees exit code 2 and the flag name, not a host refusal.
fn ensure_required_flags(schema: &Value, action: Action, input: &serde_json::Map<String, Value>) {
    let missing = crate::catalog_cli::required_discussion_flags(schema, action)
        .into_iter()
        .filter(|flag| !input.contains_key(&flag.replace('-', "_")))
        .collect::<Vec<_>>();
    if let Some(first) = missing.first() {
        crate::catalog_cli::usage_error(anyhow::anyhow!(
            "--{first} is required for {}",
            action.as_str()
        ));
    }
}
