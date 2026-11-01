//! Warns when a statement write or check ran without ASD-STE100 Rule 1.1.
use camino::Utf8Path;
use serde_json::{json, Value};

use super::refusal::HostRefusal;

/// Requirement and Rule writes carry their statement in this request field.
pub(super) fn has_statement(data: &Value) -> bool {
    data.get("statement").is_some_and(Value::is_string)
}

/// Prints the warning when the dictionary reference has no loadable index.
pub(super) fn print_if_rule_one_one_did_not_run(repo: &Utf8Path) {
    if let Some(warning) = rule_one_one_warning(repo) {
        eprintln!("warning: {warning}");
    }
}

/// Adds the warning to a refused statement write. A JSON refusal carries it in
/// `meta.warnings`, so standard error stays one JSON object. Other output
/// prints it as a `warning:` line.
#[provenance_macros::rule("rule_ste_dictionary_index_missing_warning")]
pub(super) fn add_to_refusal(error: anyhow::Error, repo: &Utf8Path, json: bool) -> anyhow::Error {
    let Some(warning) = rule_one_one_warning(repo) else {
        return error;
    };
    if !json {
        eprintln!("warning: {warning}");
        return error;
    }
    let mut error = error;
    if let Some(refusal) = error.downcast_mut::<HostRefusal>() {
        if let Some(meta) = refusal
            .failure
            .as_object_mut()
            .map(|failure| failure.entry("meta").or_insert_with(|| json!({})))
            .and_then(Value::as_object_mut)
        {
            meta.insert("warnings".into(), json!([warning]));
        }
    }
    error
}

fn rule_one_one_warning(repo: &Utf8Path) -> Option<String> {
    let root = dunce::canonicalize(repo)
        .ok()
        .and_then(|root| camino::Utf8PathBuf::from_path_buf(root).ok())
        .unwrap_or_else(|| repo.to_owned());
    let layout = provenance_store::layout::ProvenanceLayout::new(root);
    provenance_store::dictionary_reference::missing_index_warning(&layout)
}
