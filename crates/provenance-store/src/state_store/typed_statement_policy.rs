use super::{
    ReconcileState, ReconciledResource, TypedResourceKind, TypedSpecDiagnostic, TypedSpecResult,
};
use provenance_core::{Requirement, Rule};
use provenance_macros::rule;
use serde::Serialize;
use std::fmt;

/// A typed-spec apply failure carrying the same diagnostics returned by plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedSpecWriteError {
    pub diagnostics: Vec<TypedSpecDiagnostic>,
}

#[derive(Serialize)]
struct TypedSpecWriteFailure<'a> {
    error: &'static str,
    diagnostics: &'a [TypedSpecDiagnostic],
}

impl fmt::Display for TypedSpecWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let failure = TypedSpecWriteFailure {
            error: "asd_ste100_violations",
            diagnostics: &self.diagnostics,
        };
        formatter.write_str(&serde_json::to_string(&failure).map_err(|_| fmt::Error)?)
    }
}

impl std::error::Error for TypedSpecWriteError {}

/// Selects changed typed statements and attaches Rule 8.1 findings to their declarations.
#[rule("rule_ste_typed_analysis_selection")]
#[rule("rule_ste_typed_diagnostic_parity")]
pub(super) fn analyze_typed_statements(
    resources: &[ReconciledResource],
    requirements: &[Requirement],
    rules: &[Rule],
    dictionary: Option<&provenance_ste100::DictionaryImport>,
) -> Vec<TypedSpecDiagnostic> {
    resources
        .iter()
        .filter(|resource| needs_analysis(resource))
        .flat_map(|resource| {
            let statement = statement_for(resource, requirements, rules);
            let report = crate::statement_analysis::check_statement(statement, dictionary);
            report
                .findings
                .into_iter()
                .map(move |finding| TypedSpecDiagnostic {
                    address: resource.address.clone(),
                    resource_kind: resource.kind,
                    field: "statement".to_owned(),
                    standard: report.standard,
                    issue: report.issue,
                    rule: finding.rule,
                    disposition: finding.kind,
                    span: finding.span,
                    message: finding.message,
                })
        })
        .collect()
}

/// Rejects typed apply before publication when its shared analysis found violations.
#[rule("rule_ste_typed_apply_atomic_rejection")]
pub(super) fn ensure_typed_spec_is_writable(
    result: &TypedSpecResult,
) -> Result<(), TypedSpecWriteError> {
    if result.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(TypedSpecWriteError {
            diagnostics: result.diagnostics.clone(),
        })
    }
}

fn needs_analysis(resource: &ReconciledResource) -> bool {
    matches!(
        resource.kind,
        TypedResourceKind::Requirement | TypedResourceKind::Rule
    ) && (resource.state == ReconcileState::Created
        || (resource.state == ReconcileState::Updated
            && resource
                .changes
                .iter()
                .any(|change| change.field == "statement")))
}

fn statement_for<'a>(
    resource: &ReconciledResource,
    requirements: &'a [Requirement],
    rules: &'a [Rule],
) -> &'a str {
    match resource.kind {
        TypedResourceKind::Requirement => requirements
            .iter()
            .find(|record| record.id == resource.id)
            .map(|record| record.statement.as_str()),
        TypedResourceKind::Rule => rules
            .iter()
            .find(|record| record.id == resource.id)
            .map(|record| record.statement.as_str()),
        TypedResourceKind::Source => None,
    }
    .expect("each reconciled statement resource has a candidate record")
}
