use crate::output::OutputFormat;
use crate::store::{ScopeSnapshot, Store};
use camino::Utf8PathBuf;
use provenance_core::ScopeId;
use serde::Serialize;

#[derive(Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeExport {
    pub scope: String,
    pub sources: Vec<provenance_core::Source>,
    #[serde(default)]
    pub domains: Vec<provenance_core::Domain>,
    pub requirements: Vec<provenance_core::Requirement>,
    #[serde(default)]
    pub boundaries: Vec<provenance_core::Boundary>,
    #[serde(default)]
    pub topics: Vec<provenance_core::Topic>,
    #[serde(default)]
    pub questions: Vec<provenance_core::Question>,
    pub resolutions: Vec<provenance_core::Resolution>,
    pub rules: Vec<provenance_core::Rule>,
    #[serde(default)]
    pub verification_bindings: Vec<provenance_core::VerificationBinding>,
    #[serde(default)]
    pub implementation_bindings: Vec<provenance_core::ImplementationBinding>,
    pub threads: Vec<provenance_core::Thread>,
    pub messages: Vec<provenance_core::Message>,
    #[serde(default)]
    pub contributions: Vec<provenance_core::Contribution>,
    #[serde(default)]
    pub synthesis_packets: Vec<provenance_core::SynthesisPacket>,
    #[serde(default)]
    pub proposal_cards: Vec<provenance_core::ProposalCard>,
    #[serde(default)]
    pub assertion_records: Vec<provenance_core::AssertionRecord>,
    #[serde(default, alias = "promotion_decisions")]
    pub dispositions: Vec<provenance_core::DispositionRecord>,
}

impl ScopeExport {
    fn from_snapshot(scope: String, snapshot: ScopeSnapshot) -> Self {
        Self {
            scope,
            sources: snapshot.sources,
            domains: snapshot.domains,
            requirements: snapshot.requirements,
            boundaries: snapshot.boundaries,
            topics: snapshot.topics,
            questions: snapshot.questions,
            resolutions: snapshot.resolutions,
            rules: snapshot.rules,
            verification_bindings: snapshot.verification_bindings,
            implementation_bindings: snapshot.implementation_bindings,
            threads: snapshot.threads,
            messages: snapshot.messages,
            contributions: snapshot.contributions,
            synthesis_packets: snapshot.synthesis_packets,
            proposal_cards: snapshot.proposal_cards,
            assertion_records: snapshot.assertion_records,
            dispositions: snapshot.dispositions,
        }
    }
}

pub fn export_scope(repo: Utf8PathBuf, scope: String) -> anyhow::Result<ScopeExport> {
    let scope_id = ScopeId::new(scope.clone())?;
    let store = Store::open_required(repo)?;
    store.with_repository_publication(|| {
        store.ensure_review_portable(&scope_id)?;
        store.validate_ideation_scope(&scope_id)?;
        store.validate_graph_scope(&scope_id)?;
        Ok(ScopeExport::from_snapshot(
            scope,
            store.snapshot(&scope_id)?,
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

/// One line for each record of each collection, in field order.
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
        "# Provenance Export\n\n- Scope: {}\n- Sources: {}\n- Domains: {}\n- Requirements: {}\n- Boundaries: {}\n- Topics: {}\n- Questions: {}\n- Resolutions: {}\n- Rules: {}\n- Proposals: {}\n",
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
        "scope: {}\nsources: {}\ndomains: {}\nrequirements: {}\nboundaries: {}\ntopics: {}\nquestions: {}\nresolutions: {}\nrules: {}\nproposals: {}\n",
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
        "scope\tsources\tdomains\trequirements\tboundaries\ttopics\tquestions\tresolutions\trules\tproposals\n{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
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
    repo: Utf8PathBuf,
    scope: String,
    format: OutputFormat,
    output: Option<Utf8PathBuf>,
) -> anyhow::Result<()> {
    let exported = export_scope(repo, scope)?;
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
