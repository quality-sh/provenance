//! Derived facts about current Rule implementation and verification evidence.

use std::collections::{BTreeMap, BTreeSet};

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{
    ImplementationBinding, Rule, RuleStatus, VerificationBinding, VerificationMethod,
};
use provenance_macros::rule;

use crate::{source_sites, FileScan, SourceSiteRole};

/// Whether the supplied scans can establish evidence absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleEvidenceCompleteness {
    Complete,
    Incomplete,
}

/// The configured severity for governed Rule binding findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingFindingSeverity {
    Warning,
    Error,
}

/// The source of a current binding to an inactive Rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InactiveBindingOrigin {
    Scanned,
    TypedImplementation,
    TypedVerification,
}

/// The role of a current binding to an inactive Rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InactiveBindingRole {
    Implementation,
    Verification,
}

/// One current binding that cites a deprecated or archived Rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InactiveCurrentBinding {
    pub rule_id: String,
    pub status: RuleStatus,
    pub origin: InactiveBindingOrigin,
    pub role: InactiveBindingRole,
    pub file_path: Utf8PathBuf,
    pub line: Option<usize>,
    pub verification_method: Option<VerificationMethod>,
}

/// Rule evidence facts from scanned and typed bindings.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuleEvidenceFacts {
    pub unimplemented: Vec<String>,
    pub unverified: Vec<String>,
    pub inactive_current: Vec<InactiveCurrentBinding>,
}

impl RuleEvidenceFacts {
    /// The number of findings governed by the Rule binding policy.
    pub const fn governed_finding_count(&self) -> usize {
        self.unverified.len() + self.inactive_current.len()
    }
}

/// Derive all current Rule evidence facts through one interface.
///
/// Scanned and typed bindings form one evidence set. Incomplete evidence
/// withholds absence facts but still reports current bindings to inactive
/// Rules.
#[rule("rule_active_rule_requires_verification")]
#[rule("rule_active_rule_reports_missing_implementation")]
#[rule("rule_inactive_rules_have_no_current_bindings")]
pub fn derive_rule_evidence_facts(
    rules: &[Rule],
    scans: &[FileScan],
    implementations: &[ImplementationBinding],
    verifications: &[VerificationBinding],
    completeness: RuleEvidenceCompleteness,
) -> RuleEvidenceFacts {
    let inactive = inactive_rules(rules);
    let inactive_current =
        inactive_current_bindings(&inactive, scans, implementations, verifications);
    if completeness == RuleEvidenceCompleteness::Incomplete {
        return RuleEvidenceFacts {
            inactive_current,
            ..RuleEvidenceFacts::default()
        };
    }

    let implemented = implemented_rule_ids(scans, implementations);
    let verified = verified_rule_ids(scans, verifications);
    let active = rules
        .iter()
        .filter(|rule| rule.status == RuleStatus::Active);
    let unimplemented = active
        .clone()
        .filter(|rule| !implemented.contains(rule.id.as_str()))
        .map(|rule| rule.id.as_str().to_string())
        .collect();
    let unverified = active
        .filter(|rule| !verified.contains(rule.id.as_str()))
        .map(|rule| rule.id.as_str().to_string())
        .collect();

    RuleEvidenceFacts {
        unimplemented,
        unverified,
        inactive_current,
    }
}

/// Whether the configured policy fails for the governed finding count.
#[rule("rule_binding_finding_uses_configured_severity")]
pub const fn binding_findings_fail(severity: BindingFindingSeverity, governed: usize) -> bool {
    matches!(severity, BindingFindingSeverity::Error) && governed > 0
}

/// Whether one scan path covers the repository root.
pub fn scan_covers_repository(repo: &Utf8Path, path: &Utf8Path) -> bool {
    same_file::is_same_file(repo, path).unwrap_or(false)
}

fn implemented_rule_ids(
    scans: &[FileScan],
    bindings: &[ImplementationBinding],
) -> BTreeSet<String> {
    source_sites(scans)
        .filter(|site| site.role() == SourceSiteRole::Implementation)
        .map(|site| site.rule_id().to_string())
        .chain(
            bindings
                .iter()
                .map(|binding| binding.rule_id.as_str().to_string()),
        )
        .collect()
}

fn verified_rule_ids(scans: &[FileScan], bindings: &[VerificationBinding]) -> BTreeSet<String> {
    scans
        .iter()
        .flat_map(|scan| {
            scan.bindings
                .iter()
                .filter(|binding| binding.verification.is_some())
                .map(|binding| binding.rule_id.clone())
                .chain(
                    scan.annotations
                        .iter()
                        .filter(|location| location.annotation.verification.is_some())
                        .map(|location| location.annotation.rule.clone()),
                )
        })
        .chain(
            bindings
                .iter()
                .map(|binding| binding.rule_id.as_str().to_string()),
        )
        .collect()
}

fn inactive_rules(rules: &[Rule]) -> BTreeMap<&str, RuleStatus> {
    rules
        .iter()
        .filter_map(|rule| match rule.status {
            RuleStatus::Deprecated | RuleStatus::Archived => {
                Some((rule.id.as_str(), rule.status.clone()))
            }
            _ => None,
        })
        .collect()
}

fn inactive_current_bindings(
    inactive: &BTreeMap<&str, RuleStatus>,
    scans: &[FileScan],
    implementations: &[ImplementationBinding],
    verifications: &[VerificationBinding],
) -> Vec<InactiveCurrentBinding> {
    let mut current = Vec::new();
    for scan in scans {
        for location in &scan.annotations {
            if let Some(status) = inactive.get(location.annotation.rule.as_str()) {
                current.push(InactiveCurrentBinding {
                    rule_id: location.annotation.rule.clone(),
                    status: status.clone(),
                    origin: InactiveBindingOrigin::Scanned,
                    role: InactiveBindingRole::Implementation,
                    file_path: location.file_path.clone(),
                    line: Some(location.line),
                    verification_method: None,
                });
            }
        }
        for binding in &scan.bindings {
            if let Some(status) = inactive.get(binding.rule_id.as_str()) {
                current.push(InactiveCurrentBinding {
                    rule_id: binding.rule_id.clone(),
                    status: status.clone(),
                    origin: InactiveBindingOrigin::Scanned,
                    role: if binding.verification.is_some() {
                        InactiveBindingRole::Verification
                    } else {
                        InactiveBindingRole::Implementation
                    },
                    file_path: binding.file_path.clone(),
                    line: Some(binding.line),
                    verification_method: binding.verification,
                });
            }
        }
    }
    current.extend(implementations.iter().filter_map(|binding| {
        let status = inactive.get(binding.rule_id.as_str())?;
        Some(InactiveCurrentBinding {
            rule_id: binding.rule_id.as_str().to_string(),
            status: status.clone(),
            origin: InactiveBindingOrigin::TypedImplementation,
            role: InactiveBindingRole::Implementation,
            file_path: binding.file.clone(),
            line: None,
            verification_method: None,
        })
    }));
    current.extend(verifications.iter().filter_map(|binding| {
        let status = inactive.get(binding.rule_id.as_str())?;
        Some(InactiveCurrentBinding {
            rule_id: binding.rule_id.as_str().to_string(),
            status: status.clone(),
            origin: InactiveBindingOrigin::TypedVerification,
            role: InactiveBindingRole::Verification,
            file_path: binding.file.clone(),
            line: None,
            verification_method: Some(binding.method),
        })
    }));
    current
}
