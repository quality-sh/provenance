use crate::{
    cli::dictionary::DictionaryCommand,
    output::{self, OutputFormat},
};
use camino::Utf8Path;
use provenance_ste100::{DictionaryImportIdentity, DictionaryStatus};
use provenance_store::layout::ProvenanceLayout;
use serde::Serialize;

/// The import result. The summary holds no dictionary entry content.
#[derive(Serialize)]
struct DictionaryImportSummary<'a> {
    #[serde(flatten)]
    identity: &'a DictionaryImportIdentity,
    approved_rows: usize,
    unapproved_rows: usize,
}

pub(super) fn handle(
    command: DictionaryCommand,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
    match command {
        DictionaryCommand::Import { pdf, repo, format } => import(&pdf, &repo, actor_claim, format),
    }
}

fn import(
    pdf: &Utf8Path,
    repo: &Utf8Path,
    actor_claim: Option<&provenance_core::RbacClaim>,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let bytes = std::fs::read(pdf)
        .map_err(|error| anyhow::anyhow!("read the dictionary PDF at {pdf}: {error}"))?;
    let import = provenance_ste100::import_dictionary(&bytes)
        .map_err(|error| anyhow::anyhow!("import the dictionary: {error}"))?;
    let layout = ProvenanceLayout::new(repo.to_owned());
    provenance_store::dictionary_reference::set_project_dictionary(&layout, actor_claim, &import)?;

    let approved_rows = import
        .entries
        .iter()
        .filter(|entry| entry.status == DictionaryStatus::Approved)
        .count();
    let summary = DictionaryImportSummary {
        identity: &import.identity,
        approved_rows,
        unapproved_rows: import.entries.len() - approved_rows,
    };
    output::print(format, &summary)
}
