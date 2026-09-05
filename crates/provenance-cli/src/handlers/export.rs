use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use provenance_core::ScopeId;
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
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

pub fn export_scope(repo: Utf8PathBuf, scope: String) -> anyhow::Result<ScopeExport> {
    let scope_id = ScopeId::new(scope.clone())?;
    let store = StateStore::new(ProvenanceLayout::new(repo));
    store.with_repository_publication(|| {
        store.validate_ideation_scope(&scope_id)?;
        store.validate_graph_scope(&scope_id)?;
        Ok(ScopeExport {
            scope,
            sources: store.list_sources(&scope_id)?,
            domains: store.list_domains(&scope_id)?,
            requirements: store.list_requirements(&scope_id)?,
            boundaries: store.list_boundaries(&scope_id)?,
            topics: store.list_topics(&scope_id)?,
            questions: store.list_questions(&scope_id)?,
            resolutions: store.list_resolutions(&scope_id)?,
            rules: store.list_rules(&scope_id)?,
            verification_bindings: store.list_verification_bindings(&scope_id)?,
            implementation_bindings: store.list_implementation_bindings(&scope_id)?,
            threads: store.list_threads(&scope_id)?,
            messages: store.list_messages(&scope_id)?,
            contributions: store.list_contributions(&scope_id)?,
            synthesis_packets: store.list_synthesis_packets(&scope_id)?,
            proposal_cards: store.list_proposal_definitions(&scope_id)?,
            assertion_records: store.list_assertion_records(&scope_id)?,
            dispositions: store.list_dispositions(&scope_id)?,
        })
    })
}

pub(super) fn render_export(
    format: OutputFormat,
    exported: &ScopeExport,
) -> anyhow::Result<String> {
    match format {
        OutputFormat::Json => Ok(format!("{}\n", serde_json::to_string_pretty(exported)?)),
        OutputFormat::Jsonl => {
            let mut out = String::new();
            for value in serde_json::to_value(exported)?.as_object().unwrap().values() {
                if let Some(records) = value.as_array() {
                    for record in records {
                        out.push_str(&serde_json::to_string(record)?);
                        out.push('\n');
                    }
                }
            }
            Ok(out)
        }
        OutputFormat::Markdown => Ok(format!(
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
        )),
        OutputFormat::Toon => Ok(format!(
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
        )),
        OutputFormat::Table => Ok(format!(
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
        )),
    }
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
