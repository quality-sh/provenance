//! Findings the Rule binding lifecycle policy governs: current bindings to
//! deprecated or archived Rules.
//!
//! A marker the scan read and a typed binding the graph stores are both
//! current evidence, so both produce a finding. A retired historical binding
//! stays readable in the graph without counting as current, and a deprecated
//! or archived Rule with no current binding produces no finding at all.

use std::collections::BTreeMap;

use provenance_macros::rule;

/// Which rules the lifecycle findings name, and why: every rule whose status
/// or retirement withdrew it from current coverage. A retired rule keeps the
/// separate retired-record check, so the lifecycle findings take only the
/// deprecated and archived statuses.
fn inactive_rules(rules: &[provenance_core::Rule]) -> BTreeMap<&str, &'static str> {
    rules
        .iter()
        .filter(|rule| !rule.retired)
        .filter_map(|rule| match rule.status {
            provenance_core::RuleStatus::Deprecated => Some((rule.id.as_str(), "deprecated")),
            provenance_core::RuleStatus::Archived => Some((rule.id.as_str(), "archived")),
            _ => None,
        })
        .collect()
}

/// Current implementation or verification bindings to deprecated or archived
/// Rules, from scanned markers and typed graph bindings alike.
#[rule("rule_inactive_rules_have_no_current_bindings")]
pub(super) fn inactive_rule_binding_warnings(
    rules: &[provenance_core::Rule],
    scans: &[provenance_scanner::FileScan],
    typed_implementations: &[provenance_core::ImplementationBinding],
    typed_verifications: &[provenance_core::VerificationBinding],
) -> Vec<provenance_core::coverage::ValidationWarning> {
    let inactive = inactive_rules(rules);
    let mut findings = marker_findings(&inactive, scans);
    findings.extend(typed_findings(
        &inactive,
        typed_implementations,
        typed_verifications,
    ));
    findings
}

fn marker_findings(
    inactive: &BTreeMap<&str, &'static str>,
    scans: &[provenance_scanner::FileScan],
) -> Vec<provenance_core::coverage::ValidationWarning> {
    scans
        .iter()
        .flat_map(|scan| {
            scan.annotations
                .iter()
                .filter_map(|location| {
                    inactive
                        .get(location.annotation.rule.as_str())
                        .map(|status| {
                            binding_finding(
                                &location.annotation.rule,
                                status,
                                "marker",
                                Some(location.file_path.clone()),
                                Some(location.line),
                            )
                        })
                })
                .chain(scan.bindings.iter().filter_map(|binding| {
                    inactive.get(binding.rule_id.as_str()).map(|status| {
                        binding_finding(
                            &binding.rule_id,
                            status,
                            "marker",
                            Some(binding.file_path.clone()),
                            Some(binding.line),
                        )
                    })
                }))
        })
        .collect()
}

fn typed_findings(
    inactive: &BTreeMap<&str, &'static str>,
    typed_implementations: &[provenance_core::ImplementationBinding],
    typed_verifications: &[provenance_core::VerificationBinding],
) -> Vec<provenance_core::coverage::ValidationWarning> {
    // A retired historical binding stays readable without counting as
    // current, so it never becomes a finding here.
    let implementations = typed_implementations
        .iter()
        .filter(|binding| !binding.retired)
        .filter_map(|binding| {
            let status = inactive.get(binding.rule_id.as_str())?;
            Some(binding_finding(
                binding.rule_id.as_str(),
                status,
                "typed implementation binding",
                Some(binding.file.clone()),
                None,
            ))
        });
    let verifications = typed_verifications
        .iter()
        .filter(|binding| !binding.retired)
        .filter_map(|binding| {
            let status = inactive.get(binding.rule_id.as_str())?;
            Some(binding_finding(
                binding.rule_id.as_str(),
                status,
                "typed verification binding",
                Some(binding.file.clone()),
                None,
            ))
        });
    implementations.chain(verifications).collect()
}

fn binding_finding(
    rule_id: &str,
    status: &str,
    kind: &str,
    file_path: Option<camino::Utf8PathBuf>,
    line: Option<usize>,
) -> provenance_core::coverage::ValidationWarning {
    provenance_core::coverage::ValidationWarning {
        rule_id: rule_id.to_string(),
        file_path,
        line,
        message: format!("{kind} cites rule `{rule_id}` with status `{status}`"),
        binding_finding: true,
    }
}
