use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use provenance_store::publication::with_publication_guard;
use provenance_transport::porcelain::ScopeExport;
use provenance_transport::repository::RepoContext;
use provenance_transport::snapshot::Store;

/// The export reads its records from one view of the canonical state. The
/// review and validation checks read the live store in the same section.
fn export_context(context: &RepoContext) -> anyhow::Result<ScopeExport> {
    let repository = context.open()?;
    let scope_id = repository.scope().clone();
    with_publication_guard(&repository, |guard| {
        let held = Store::new(guard.repository());
        let snapshot =
            provenance_transport::snapshot::snapshot(&guard.repository().view()?, &scope_id)?;
        held.ensure_review_portable(&scope_id)?;
        held.validate_ideation_scope(&scope_id)?;
        held.validate_graph_scope(&scope_id)?;
        Ok(ScopeExport::from_snapshot(
            scope_id.as_str().to_owned(),
            snapshot,
        ))
    })
}

pub(super) fn render_export(
    format: OutputFormat,
    exported: &ScopeExport,
) -> anyhow::Result<String> {
    match format {
        OutputFormat::Json => Ok(format!("{}\n", serde_json::to_string_pretty(exported)?)),
        OutputFormat::Jsonl => render_jsonl(exported),
        OutputFormat::Markdown => Ok(render_markdown(exported)),
        OutputFormat::Toon => Ok(render_toon(exported)),
        OutputFormat::Table => Ok(render_table(exported)),
    }
}

/// One line for each record of each collection. Collections and record keys
/// come out in alphabetical order.
fn render_jsonl(exported: &ScopeExport) -> anyhow::Result<String> {
    let value = serde_json::to_value(exported)?;
    let records = value
        .as_object()
        .into_iter()
        .flat_map(serde_json::Map::values)
        .filter_map(serde_json::Value::as_array)
        .flatten();
    let mut out = String::new();
    for record in records {
        out.push_str(&serde_json::to_string(record)?);
        out.push('\n');
    }
    Ok(out)
}

fn render_markdown(exported: &ScopeExport) -> String {
    format!(
        concat!(
            "# Provenance Export\n\n- Scope: {}\n- Sources: {}\n- Domains: {}\n",
            "- Requirements: {}\n- Boundaries: {}\n- Topics: {}\n- Questions: {}\n",
            "- Resolutions: {}\n- Rules: {}\n- Proposals: {}\n"
        ),
        exported.scope,
        exported.sources.len(),
        exported.domains.len(),
        exported.requirements.len(),
        exported.boundaries.len(),
        exported.topics.len(),
        exported.questions.len(),
        exported.resolutions.len(),
        exported.rules.len(),
        exported.proposal_cards.len()
    )
}

fn render_toon(exported: &ScopeExport) -> String {
    format!(
        concat!(
            "scope: {}\nsources: {}\ndomains: {}\nrequirements: {}\nboundaries: {}\n",
            "topics: {}\nquestions: {}\nresolutions: {}\nrules: {}\nproposals: {}\n"
        ),
        exported.scope,
        exported.sources.len(),
        exported.domains.len(),
        exported.requirements.len(),
        exported.boundaries.len(),
        exported.topics.len(),
        exported.questions.len(),
        exported.resolutions.len(),
        exported.rules.len(),
        exported.proposal_cards.len()
    )
}

fn render_table(exported: &ScopeExport) -> String {
    format!(
        concat!(
            "scope\tsources\tdomains\trequirements\tboundaries\ttopics\tquestions\t",
            "resolutions\trules\tproposals\n{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n"
        ),
        exported.scope,
        exported.sources.len(),
        exported.domains.len(),
        exported.requirements.len(),
        exported.boundaries.len(),
        exported.topics.len(),
        exported.questions.len(),
        exported.resolutions.len(),
        exported.rules.len(),
        exported.proposal_cards.len()
    )
}

pub(super) fn handle(
    context: &RepoContext,
    format: OutputFormat,
    output: Option<Utf8PathBuf>,
) -> anyhow::Result<()> {
    let exported = export_context(context)?;
    let rendered = render_export(format, &exported)?;
    if let Some(output_path) = output {
        std::fs::write(output_path, rendered)?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
