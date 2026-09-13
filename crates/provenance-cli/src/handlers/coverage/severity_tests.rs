//! The configured severity decides whether Rule binding findings fail the
//! command. Other warnings stay report-only under this policy.

use super::binding_finding_refusal;
use provenance_core::coverage::ValidationWarning;
use provenance_store::settings::BindingFindingsSeverity;

fn plain_warning() -> ValidationWarning {
    ValidationWarning {
        rule_id: String::new(),
        file_path: Some("src/lib.rs".into()),
        line: Some(2),
        message: "unknown field".to_string(),
        binding_finding: false,
    }
}

fn binding_finding() -> ValidationWarning {
    ValidationWarning {
        rule_id: "rule_unverified".to_string(),
        file_path: None,
        line: None,
        message: "active rule `rule_unverified` has no verification".to_string(),
        binding_finding: true,
    }
}

#[test]
fn error_policy_refuses_when_a_binding_finding_is_present() {
    let refusal = binding_finding_refusal(
        BindingFindingsSeverity::Error,
        &[plain_warning(), binding_finding()],
    );

    let refusal = refusal.expect("error policy must refuse");
    assert!(refusal.contains('1'), "{refusal}");
    assert!(refusal.contains("error"), "{refusal}");
}

#[test]
fn warning_policy_reports_findings_without_a_refusal() {
    let refusal = binding_finding_refusal(BindingFindingsSeverity::Warning, &[binding_finding()]);

    assert!(refusal.is_none());
}

#[test]
fn error_policy_leaves_warnings_the_policy_does_not_govern_alone() {
    let refusal = binding_finding_refusal(BindingFindingsSeverity::Error, &[plain_warning()]);

    assert!(refusal.is_none());
}

#[test]
fn the_refusal_counts_only_the_findings_the_policy_governs() {
    let refusal = binding_finding_refusal(
        BindingFindingsSeverity::Error,
        &[plain_warning(), binding_finding(), binding_finding()],
    );

    let refusal = refusal.expect("must refuse");
    assert!(refusal.contains('2'), "{refusal}");
}
