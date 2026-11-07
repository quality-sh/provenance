//! The dictionary adapters: import the ASD-STE100 dictionary from one local PDF file, and
//! report the dictionary reference of a repository.

use super::RepositoryPort;
use camino::Utf8Path;
use provenance_porcelain::dictionary::{
    DictionaryImportInput, DictionaryImportOutput, DictionaryImportPort, DictionaryStatusInput,
    DictionaryStatusOutput, DictionaryStatusPort,
};
use provenance_porcelain::WorkFuture;
use provenance_ste100::DictionaryStatus;
use provenance_store::dictionary_reference::{
    index_directory, resolve_project_dictionary, DictionaryResolution,
};
use provenance_store::layout::ProvenanceLayout;

impl DictionaryImportPort for RepositoryPort {
    type Failure = anyhow::Error;

    fn dictionary_import(
        &self,
        input: DictionaryImportInput,
    ) -> WorkFuture<'_, DictionaryImportOutput, anyhow::Error> {
        Box::pin(async move { import(Utf8Path::new(&input.pdf), &self.context.repo) })
    }
}

fn import(pdf: &Utf8Path, repo: &Utf8Path) -> anyhow::Result<DictionaryImportOutput> {
    let bytes = std::fs::read(pdf)
        .map_err(|error| anyhow::anyhow!("read the dictionary PDF at {pdf}: {error}"))?;
    let import = provenance_ste100::import_dictionary(&bytes)
        .map_err(|error| anyhow::anyhow!("import the dictionary: {error}"))?;
    let layout = ProvenanceLayout::new(repo.to_owned());
    provenance_store::dictionary_reference::set_project_dictionary(&layout, &import)?;

    let approved_rows = import
        .entries
        .iter()
        .filter(|entry| entry.status == DictionaryStatus::Approved)
        .count();
    let identity = import.identity;
    Ok(DictionaryImportOutput {
        issue: identity.issue.into(),
        source_sha256: identity.source_sha256,
        data_sha256: identity.data_sha256,
        extractor_version: identity.extractor_version,
        approved_rows,
        unapproved_rows: import.entries.len() - approved_rows,
    })
}

impl DictionaryStatusPort for RepositoryPort {
    type Failure = anyhow::Error;

    fn dictionary_status(
        &self,
        _input: DictionaryStatusInput,
    ) -> WorkFuture<'_, DictionaryStatusOutput, anyhow::Error> {
        Box::pin(async move { Ok(status(&self.context.repo)) })
    }
}

fn status(repo: &Utf8Path) -> DictionaryStatusOutput {
    let layout = ProvenanceLayout::new(repo.to_owned());
    let (referenced, identity, index_present, load_problem) =
        match resolve_project_dictionary(&layout) {
            DictionaryResolution::Loaded(dictionary) => {
                (true, Some(dictionary.import.identity), true, None)
            }
            DictionaryResolution::NoReference => (false, None, false, None),
            DictionaryResolution::Unavailable {
                identity, reason, ..
            } => (true, identity, false, Some(reason)),
        };
    DictionaryStatusOutput {
        referenced,
        issue: identity.as_ref().map(|identity| identity.issue.into()),
        source_sha256: identity
            .as_ref()
            .map(|identity| identity.source_sha256.clone()),
        data_sha256: identity
            .as_ref()
            .map(|identity| identity.data_sha256.clone()),
        extractor_version: identity
            .as_ref()
            .map(|identity| identity.extractor_version.clone()),
        index_directory: index_directory().map(|directory| directory.display().to_string()),
        index_present,
        analyzer: if index_present {
            "project_dictionary"
        } else {
            "built_in_rules"
        }
        .to_owned(),
        load_problem,
    }
}
