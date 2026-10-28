use provenance_macros::rule;
use provenance_ste100::Report;
use serde::Serialize;
use std::fmt;

#[derive(Debug)]
pub struct StatementWriteError {
    pub(crate) report: Report,
}

#[derive(Serialize)]
struct StatementWriteDiagnostic<'a> {
    field: &'static str,
    #[serde(flatten)]
    report: &'a Report,
}

impl fmt::Display for StatementWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let diagnostic = StatementWriteDiagnostic {
            field: "statement",
            report: &self.report,
        };
        let json = serde_json::to_string(&diagnostic).map_err(|_| fmt::Error)?;
        formatter.write_str(&json)
    }
}

impl std::error::Error for StatementWriteError {}

/// Prevents direct statement writes that have deterministic ASD-STE100 violations.
#[rule("rule_ste_direct_statement_write_gate")]
pub(super) fn ensure_statement_is_writable(
    layout: &crate::layout::ProvenanceLayout,
    statement: &str,
) -> Result<(), StatementWriteError> {
    let report = crate::statement_analysis::check_project_statement(layout, statement);
    if report.findings.is_empty() {
        return Ok(());
    }

    Err(StatementWriteError { report })
}
