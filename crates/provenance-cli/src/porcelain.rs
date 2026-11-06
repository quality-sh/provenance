//! CLI-owned bindings for shared Porcelain capabilities.
use provenance_macros::rule;

use camino::Utf8PathBuf;
use provenance_porcelain::check::{Category, CategoryReport, CheckInput, Status};
use provenance_porcelain::get::{GetInput, GetOutcome};
use provenance_transport::check::RepositoryCheckPort;
use provenance_transport::repository::RepoContext;
use provenance_transport::StatementHost;

mod api;
mod search;
pub use api::dispatch_api;
pub use provenance_porcelain::check::render_readable as render_check;
pub use provenance_porcelain::get::render_readable as render_get_readable;
pub use search::dispatch_search;

/// An explicit CLI output format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    /// Emit JSON result data.
    Json,
}
/// Translate the live CLI selector fields into one semantic check request.
pub fn check_input_from_selectors(
    graph: bool,
    statements: bool,
    bindings: bool,
    statement: Option<String>,
) -> CheckInput {
    let mut categories = Vec::new();
    if graph {
        categories.push(Category::Graph);
    }
    if statements {
        categories.push(Category::Statements);
    }
    if bindings {
        categories.push(Category::Bindings);
    }
    statement
        .into_iter()
        .fold(CheckInput::new(categories), CheckInput::with_statement)
}

/// The host of one repository and scope, with the notify and review link adapters of
/// the review host.
pub fn local_host(context: &RepoContext) -> anyhow::Result<StatementHost> {
    Ok(context
        .host()?
        .with_notify_port(std::sync::Arc::new(provenance_review::HostNotifyPort::new(
            place(context),
        )))
        .with_review_link_port(std::sync::Arc::new(
            provenance_review::HostReviewLinkPort::new(place(context)),
        )))
}

/// The repository root and scope as the review library names them.
pub fn place(context: &RepoContext) -> provenance_review::RepositoryScope {
    provenance_review::RepositoryScope::new(context.repo.clone(), context.scope.clone())
}

/// Runs the check command with the repository check adapter, prints the report, and
/// fails when a category refuses.
#[rule("rule_ste_strict_committed_statement_gate")]
pub async fn dispatch_check(repo: &str, json: bool, input: CheckInput) -> anyhow::Result<()> {
    let repo = Utf8PathBuf::from(repo);
    if let Some(scope) = &input.scope {
        // An undeclared scope refuses the whole check before any category runs.
        provenance_transport::check::ensure_declared_scope(&repo, scope)?;
    }
    if let Err(reason) = input.validate() {
        anyhow::bail!("unsupported check options: {reason}");
    }
    let check = input;
    let port = RepositoryCheckPort::new(repo, false, None);
    let report = provenance_porcelain::Porcelain::new(port)
        .check(check)
        .await;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", render_check(&report));
    }
    if report.categories.iter().any(CategoryReport::refuses) {
        let details = report
            .categories
            .iter()
            .filter(|category| category.status != Status::Passed)
            .flat_map(|category| {
                category
                    .findings
                    .iter()
                    .map(|finding| finding.message.as_str())
                    .chain(category.unavailable_reason.as_deref())
            })
            .collect::<Vec<_>>()
            .join("; ");
        anyhow::bail!(details);
    }
    Ok(())
}

/// Run one fully parsed target-first Porcelain get command.
pub async fn dispatch_get(
    repo: &str,
    scope: &str,
    format: Option<OutputFormat>,
    input: GetInput,
) -> anyhow::Result<()> {
    let host = local_host(&RepoContext::new(repo, scope))?;
    let service = host.porcelain().get();
    let outcome = service.get(input).await?;
    let edit = if outcome.view() == provenance_porcelain::get::View::Record {
        edit_state(&host, &outcome).await
    } else {
        None
    };
    println!("{}", render_get(&outcome, edit.as_ref(), format)?);
    Ok(())
}

/// The `edit` state of the record, read from its member route so that a JSON
/// record read shows the etag that the next guarded update needs. The text
/// form stays identical to the shared readable rendering.
async fn edit_state(
    host: &provenance_transport::StatementHost,
    outcome: &GetOutcome,
) -> Option<serde_json::Value> {
    use provenance_contract::{target_definitions, TargetAction};
    let kind = outcome.record.node_type();
    let (_, definition) =
        target_definitions(TargetAction::Update).find(|(registered, _)| *registered == kind)?;
    let path = definition
        .path
        .replace("{id}", outcome.record.id().as_str());
    let value = host
        .invoke_resource(
            axum::http::Method::GET,
            &path,
            serde_json::json!({}),
            std::collections::BTreeMap::new(),
            axum::http::HeaderMap::new(),
        )
        .await
        .ok()?;
    value.get("data")?.get("edit").cloned()
}

/// Renders the selected record as readable text or structured JSON.
#[rule("rule_porcelain_cli_readable_json")]
fn render_get(
    outcome: &GetOutcome,
    edit: Option<&serde_json::Value>,
    format: Option<OutputFormat>,
) -> serde_json::Result<String> {
    if format == Some(OutputFormat::Json) {
        let mut value = serde_json::to_value(outcome)?;
        if let (Some(object), Some(edit)) = (value.as_object_mut(), edit) {
            object.insert("edit".into(), edit.clone());
        }
        serde_json::to_string_pretty(&value)
    } else {
        render_get_readable(outcome)
    }
}
