//! CLI-owned bindings for shared Porcelain capabilities.

use crate::repo_context::RepoContext;
use provenance_porcelain::check::{Category, CheckInput};
use provenance_porcelain::get::{GetInput, GetOutcome};

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
pub fn check_input_from_selectors(graph: bool, statements: bool, bindings: bool) -> CheckInput {
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
    CheckInput::new(categories)
}

/// Run one fully parsed target-first Porcelain get command.
pub async fn dispatch_get(
    repo: &str,
    scope: &str,
    format: Option<OutputFormat>,
    input: GetInput,
) -> anyhow::Result<()> {
    let host = RepoContext::new(repo, scope).local_host()?;
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

/// The `edit` state of the record, read from its member route so that a
/// record read shows the etag that the next guarded update needs.
async fn edit_state(
    host: &provenance_transport::StatementHost,
    outcome: &GetOutcome,
) -> Option<serde_json::Value> {
    use provenance_store::operations::catalog::{target_definitions, TargetAction};
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
        let mut text = render_get_readable(outcome)?;
        if let Some(etag) = edit
            .and_then(|edit| edit.get("etag"))
            .and_then(|etag| etag.as_str())
        {
            text.push_str(&format!("\n\netag: {etag}"));
        }
        Ok(text)
    }
}
