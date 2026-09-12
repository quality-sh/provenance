//! Findings the Rule binding lifecycle policy governs: current bindings to
//! deprecated or archived Rules, and current typed bindings to retired Rules.
//!
//! A marker the scan read and a typed binding the graph stores are both
//! current evidence, so both produce a finding. A retired historical binding
//! stays readable in the graph without counting as current, and a Rule with
//! no current binding produces no finding at all.

use provenance_macros::rule;

/// The withdrawal word for a Rule current coverage withdrew, or None when
/// the Rule still stands.
const fn withdrawal_word(rule: &provenance_core::Rule) -> Option<&'static str> {
    if rule.retired {
        return Some("retired");
    }
    match rule.status {
        provenance_core::RuleStatus::Deprecated => Some("deprecated"),
        provenance_core::RuleStatus::Archived => Some("archived"),
        _ => None,
    }
}

/// Current implementation or verification bindings to deprecated or archived
/// Rules, from scanned markers and typed graph bindings alike, plus current
/// typed bindings to retired Rules.
#[rule("rule_inactive_rules_have_no_current_bindings")]
pub(super) fn inactive_rule_binding_warnings(
    rules: &[provenance_core::Rule],
    scans: &[provenance_scanner::FileScan],
    typed_implementations: &[provenance_core::ImplementationBinding],
    typed_verifications: &[provenance_core::VerificationBinding],
) -> Vec<provenance_core::coverage::ValidationWarning> {
    let mut findings = marker_findings(rules, scans);
    findings.extend(typed_findings(
        rules,
        typed_implementations,
        typed_verifications,
    ));
    findings
}

fn marker_findings(
    rules: &[provenance_core::Rule],
    scans: &[provenance_scanner::FileScan],
) -> Vec<provenance_core::coverage::ValidationWarning> {
    // A marker citing a retired Rule keeps its separate retired-record
    // check, so the lifecycle marker findings take only the deprecated and
    // archived statuses: retired Rules are skipped here, and their current
    // typed bindings join the findings instead.
    let marker_withdrawal = |rule_id: &str| -> Option<&'static str> {
        let rule = rules.iter().find(|rule| rule.id.as_str() == rule_id)?;
        if rule.retired {
            return None;
        }
        withdrawal_word(rule)
    };
    scans
        .iter()
        .flat_map(|scan| {
            scan.annotations
                .iter()
                .filter_map(|location| {
                    let word = marker_withdrawal(&location.annotation.rule)?;
                    Some(binding_finding(
                        &location.annotation.rule,
                        word,
                        "marker",
                        Some(location.file_path.clone()),
                        Some(location.line),
                    ))
                })
                .chain(scan.bindings.iter().filter_map(|binding| {
                    let word = marker_withdrawal(&binding.rule_id)?;
                    Some(binding_finding(
                        &binding.rule_id,
                        word,
                        "marker",
                        Some(binding.file_path.clone()),
                        Some(binding.line),
                    ))
                }))
        })
        .collect()
}

fn typed_findings(
    rules: &[provenance_core::Rule],
    typed_implementations: &[provenance_core::ImplementationBinding],
    typed_verifications: &[provenance_core::VerificationBinding],
) -> Vec<provenance_core::coverage::ValidationWarning> {
    // A retired historical binding stays readable without counting as
    // current, so it never becomes a finding here.
    let implementations = typed_implementations
        .iter()
        .filter(|binding| !binding.retired)
        .filter_map(|binding| {
            let rule = rules
                .iter()
                .find(|rule| rule.id.as_str() == binding.rule_id.as_str())?;
            let word = withdrawal_word(rule)?;
            Some(binding_finding(
                binding.rule_id.as_str(),
                word,
                "typed implementation binding",
                Some(binding.file.clone()),
                None,
            ))
        });
    let verifications = typed_verifications
        .iter()
        .filter(|binding| !binding.retired)
        .filter_map(|binding| {
            let rule = rules
                .iter()
                .find(|rule| rule.id.as_str() == binding.rule_id.as_str())?;
            let word = withdrawal_word(rule)?;
            Some(binding_finding(
                binding.rule_id.as_str(),
                word,
                "typed verification binding",
                Some(binding.file.clone()),
                None,
            ))
        });
    implementations.chain(verifications).collect()
}

fn binding_finding(
    rule_id: &str,
    word: &'static str,
    kind: &str,
    file_path: Option<camino::Utf8PathBuf>,
    line: Option<usize>,
) -> provenance_core::coverage::ValidationWarning {
    provenance_core::coverage::ValidationWarning {
        rule_id: rule_id.to_string(),
        file_path,
        line,
        message: format!("{kind} cites rule `{rule_id}` with status `{word}`"),
        binding_finding: true,
    }
}
