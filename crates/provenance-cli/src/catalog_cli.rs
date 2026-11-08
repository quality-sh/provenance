//! CLI dispatch of record actions over the registered resource catalog.
use crate::invocation::{grammar, GlobalContext};
use clap::{parser::ValueSource, ArgMatches, Command};
use provenance_contract::TargetAction;
use provenance_porcelain::action::Action;
use provenance_transport::porcelain::ReplacementError;

pub mod fields;
mod help;
mod input;
mod input_guard;
mod plain_object;
pub mod refusal;
mod statement_warning;

pub use help::required_discussion_flags;

use refusal::HostRefusal;

pub fn target_command() -> anyhow::Result<Command> {
    let definitions = TargetAction::RECORD
        .into_iter()
        .flat_map(provenance_contract::target_definitions)
        .map(|(_, definition)| definition)
        .collect::<Vec<_>>();
    let command = fields::augment_with_overrides(
        grammar::target_command(),
        definitions,
        &COMMON
            .iter()
            .copied()
            .chain([
                "type",
                "view",
                "depth",
                "kind",
                "limit",
                "review_link",
                "stdin",
                "replace_parent",
                "with_parent",
            ])
            .collect::<Vec<_>>(),
        &[],
    )?;
    crate::registry_cli::augment_schemas(
        command,
        provenance_porcelain::action::Action::DISCUSSION
            .map(provenance_porcelain::discussion::input_schema),
        &["target", "declared_by"],
    )
}

pub fn print_target_help(
    command: Command,
    action: Option<grammar::TargetVerb>,
    kind: Option<provenance_core::NodeType>,
) -> anyhow::Result<()> {
    help::target(command, action, kind)
}

pub async fn dispatch_target(
    context: GlobalContext,
    format: Option<provenance_cli::porcelain::OutputFormat>,
    target: String,
    action: Action,
    kind: Option<provenance_core::NodeType>,
    matches: ArgMatches,
) -> anyhow::Result<()> {
    if action == Action::Create {
        provenance_core::ensure_record_id_assignable(&target)
            .unwrap_or_else(|error| usage_error(error));
    }
    let repo = context.repo.clone();
    let host = provenance_cli::porcelain::local_host(&repo)?;
    let route = host
        .target_route(action, &target, kind)
        .await
        .map_err(anyhow::Error::new)?;
    if route.kind == provenance_core::NodeType::Question {
        warn_if_skills_missing(context.repo.repo.as_str(), context.quiet)?;
    }
    let stdin = matches.get_flag("stdin");
    let (data, query, headers) = input::parse(
        route.definition,
        &matches,
        stdin,
        None,
        &["type", "stdin", "replace_parent", "with_parent"],
    )
    .unwrap_or_else(|error| usage_error(error));
    if !query.is_empty() {
        usage_error(anyhow::anyhow!(
            "target actions do not accept query options"
        ));
    }
    let writes_statement = statement_warning::has_statement(&data);
    let replacement = matches.get_one::<String>("replace_parent");
    let guidance = |failure: &provenance_core::protocol::failure::ErasedFailure| {
        failure_with_guidance(failure, &route.path, Some(&target), &context.repo)
    };
    let result = if let Some(old) = replacement {
        let parents = provenance_transport::porcelain::ParentReplacement {
            replace_parent: old,
            with_parent: matches
                .get_one::<String>("with_parent")
                .expect("paired parent options"),
        };
        match host.replace_parent(&route, data, headers, parents).await {
            Ok(replaced) => {
                if !replaced.changed {
                    eprintln!(
                        "{} replaced {} already; nothing changed",
                        parents.with_parent, parents.replace_parent
                    );
                }
                Ok(replaced.record)
            }
            Err(ReplacementError::Failure(failure)) => Err(guidance(&failure)),
            Err(error) => Err(anyhow::Error::new(error)),
        }
    } else {
        host.invoke_target(&route, data, headers)
            .await
            .map_err(|failure| guidance(&failure))
    };
    let json = format == Some(provenance_cli::porcelain::OutputFormat::Json);
    let mut value = match result {
        Ok(value) => value,
        Err(error) if writes_statement => {
            return Err(statement_warning::add_to_refusal(error, &repo.repo, json));
        }
        Err(error) => return Err(error),
    };
    if writes_statement {
        statement_warning::print_if_rule_one_one_did_not_run(&repo.repo);
    }
    let affected = route.definition.registration.review_link.map(|kind| {
        crate::review_link::AffectedReviewRecord {
            kind,
            id: target.clone(),
        }
    });
    crate::review_link::annotate_write(&repo, affected, &mut value).await;
    if json {
        crate::output::print_json(&value)?;
    } else {
        println!(
            "{}",
            provenance_porcelain::action::render_readable(action, &target, route.kind, &value)
        );
    }
    Ok(())
}

fn failure_with_guidance(
    failure: &provenance_core::protocol::failure::ErasedFailure,
    path: &str,
    target: Option<&str>,
    context: &provenance_transport::repository::RepoContext,
) -> anyhow::Error {
    actionable_failure(
        failure,
        target,
        path.trim_matches('/').split('/').next(),
        context,
    )
}

/// The `--scope` and `--repo` words that make a hinted command run against
/// the caller's repository and scope. A default repository path stays implicit.
fn context_options(context: Option<&provenance_transport::repository::RepoContext>) -> String {
    let Some(context) = context else {
        return String::new();
    };
    let repo = if context.repo.as_str() == "." {
        String::new()
    } else {
        format!(" --repo {}", context.repo)
    };
    format!(" --scope {}{repo}", context.scope)
}

/// Names the command that reads the review after a review-workflow refusal.
#[provenance_macros::rule("rule_review_refusal_names_read_command")]
fn review_read_command(
    id: &str,
    context: Option<&provenance_transport::repository::RepoContext>,
) -> String {
    format!("provenance {id} --view review{}", context_options(context))
}

fn actionable_failure(
    failure: &provenance_core::protocol::failure::ErasedFailure,
    record_id: Option<&str>,
    collection: Option<&str>,
    context: &provenance_transport::repository::RepoContext,
) -> anyhow::Error {
    let value = serde_json::to_value(failure).unwrap_or_default();
    let error = &value["error"];
    let kind = error["kind"].as_str();
    if kind == Some("review_submission_conflict") {
        if let (Some(id), Some(submission), Some(collection)) =
            (record_id, error["current_submission"].as_str(), collection)
        {
            let hint = format!(
                "record {id} already has pending submission {submission}; \
                 run `{}` to see it; to withdraw it, run \
                 `printf '%s' '{{\"actor\":\"<actor>\"}}' | provenance api {collection}/{id}/submissions/{submission}/withdraw --method post --input -{}`. \
                 For an owned record, also put \"declared_by\":\"<owner>\" in the body",
                review_read_command(id, Some(context)),
                context_options(Some(context))
            );
            return HostRefusal {
                failure: value,
                hint: Some(hint),
                text_shows_failure: false,
                cause: None,
            }
            .into();
        }
    }
    let hint = match (record_id, kind) {
        (
            Some(id),
            Some(
                "review_submission_conflict" | "requirement_edit_conflict" | "record_edit_conflict",
            ),
        ) => Some(format!("run `{}`", review_read_command(id, Some(context)))),
        _ => None,
    };
    HostRefusal {
        hint,
        ..HostRefusal::of(failure)
    }
    .into()
}

const COMMON: &[&str] = &[
    "repo",
    "scope",
    "format",
    "quiet",
    "target",
    "action",
    "command",
    "discussion_id",
];

pub fn ensure_only_fields(matches: &ArgMatches, allowed: &[&str]) {
    for id in matches.ids() {
        let name = id.as_str();
        if matches.value_source(name) != Some(ValueSource::CommandLine) {
            continue;
        }
        let normalized = name.replace('_', "-");
        if !COMMON.contains(&name)
            && !allowed.contains(&name)
            && !allowed.contains(&normalized.as_str())
        {
            usage_error(anyhow::anyhow!(
                "unsupported option --{}",
                name.replace('_', "-")
            ));
        }
    }
}

pub fn usage_error(error: impl std::fmt::Display) -> ! {
    clap::Error::raw(clap::error::ErrorKind::InvalidValue, error.to_string()).exit()
}

/// Exits with the CLI text of an action that cannot be selected. The CLI names the
/// record type field by its `--type` option.
pub fn action_usage_error(error: provenance_porcelain::action::ActionError) -> ! {
    match error {
        provenance_porcelain::action::ActionError::KindSelection => usage_error(
            "create requires --type and existing-record actions infer it without --type",
        ),
        error => usage_error(error),
    }
}

fn warn_if_skills_missing(repo: &str, quiet: bool) -> anyhow::Result<()> {
    if quiet {
        return Ok(());
    }
    let status = provenance_transport::skills::install_status(
        crate::skills::BUNDLED,
        std::path::Path::new(repo),
    )?;
    if !status.installed {
        eprintln!(
            "hint: provenance skills are not installed; run `{}` from the repo root",
            status.install_command
        );
    }
    Ok(())
}
