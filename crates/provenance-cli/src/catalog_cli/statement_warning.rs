//! Warns when a statement write or check ran without ASD-STE100 Rule 1.1.
use camino::Utf8Path;
use serde_json::Value;

/// Requirement and Rule writes carry their statement in this request field.
pub(super) fn has_statement(data: &Value) -> bool {
    data.get("statement").is_some_and(Value::is_string)
}

/// Prints the warning when the dictionary reference has no loadable index.
pub(super) fn print_if_rule_one_one_did_not_run(repo: &Utf8Path) {
    let root = dunce::canonicalize(repo)
        .ok()
        .and_then(|root| camino::Utf8PathBuf::from_path_buf(root).ok())
        .unwrap_or_else(|| repo.to_owned());
    let layout = provenance_store::layout::ProvenanceLayout::new(root);
    if let Some(warning) = provenance_store::dictionary_reference::missing_index_warning(&layout) {
        eprintln!("warning: {warning}");
    }
}
