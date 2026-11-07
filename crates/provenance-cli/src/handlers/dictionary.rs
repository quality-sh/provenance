use crate::output;
use camino::Utf8Path;
use provenance_ste100::StandardIssue;
use provenance_store::{
    dictionary_reference::{index_directory, resolve_project_dictionary, DictionaryResolution},
    layout::ProvenanceLayout,
};
use serde::Serialize;

/// One status line for CI logs: the referenced identity, whether the index is
/// present, and the analyzer the statement checks use.
#[derive(Serialize)]
struct DictionaryStatusSummary {
    referenced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    issue: Option<StandardIssue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extractor_version: Option<String>,
    index_directory: Option<String>,
    index_present: bool,
    analyzer: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    load_problem: Option<String>,
}

pub(super) fn status(repo: &Utf8Path) -> anyhow::Result<()> {
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
    let summary = DictionaryStatusSummary {
        referenced,
        issue: identity.as_ref().map(|identity| identity.issue),
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
        },
        load_problem,
    };
    output::print_json(&summary)
}
