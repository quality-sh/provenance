//! Markers that cite a Rule the graph has retired.
//!
//! Unlike an unknown id, the record exists and its status is the reason the
//! marker cannot establish current coverage.

use std::collections::BTreeSet;

/// Markers that cite retired rules cannot establish current coverage. Unlike
/// an unknown id, the graph record exists and its retirement explains why the
/// marker is stale. This check stays separate from the lifecycle binding
/// findings, which the repository configuration can turn into errors.
pub(super) fn stale_rule_warnings(
    rules: &[provenance_core::Rule],
    scans: &[provenance_scanner::FileScan],
) -> Vec<provenance_core::coverage::ValidationWarning> {
    let stale = rules
        .iter()
        .filter(|rule| rule.retired)
        .map(|rule| rule.id.as_str().to_string())
        .collect::<BTreeSet<_>>();

    scans
        .iter()
        .flat_map(|scan| {
            scan.annotations
                .iter()
                .filter(|location| stale.contains(&location.annotation.rule))
                .map(|location| {
                    stale_marker_warning(
                        &location.annotation.rule,
                        location.file_path.clone(),
                        location.line,
                    )
                })
                .chain(
                    scan.bindings
                        .iter()
                        .filter(|binding| stale.contains(&binding.rule_id))
                        .map(|binding| {
                            stale_marker_warning(
                                &binding.rule_id,
                                binding.file_path.clone(),
                                binding.line,
                            )
                        }),
                )
        })
        .collect()
}

fn stale_marker_warning(
    rule_id: &str,
    file_path: camino::Utf8PathBuf,
    line: usize,
) -> provenance_core::coverage::ValidationWarning {
    provenance_core::coverage::ValidationWarning {
        rule_id: rule_id.to_string(),
        file_path: Some(file_path),
        line: Some(line),
        message: format!("marker cites rule `{rule_id}` with status `retired`"),
        binding_finding: false,
    }
}
