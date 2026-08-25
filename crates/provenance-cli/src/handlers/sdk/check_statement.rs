use std::io::Read as _;

use provenance_core::protocol::CheckStatementRequest;
use provenance_macros::rule;

use crate::output::{self, OutputFormat};

/// Parses the fixed request shape for one unfinished statement.
#[rule("rule_ste_sdk_statement_request_schema")]
fn read_request() -> anyhow::Result<CheckStatementRequest> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    anyhow::ensure!(
        !input.trim().is_empty(),
        "expected a JSON document on stdin"
    );
    serde_json::from_str(&input).map_err(Into::into)
}

/// Returns the authoritative descriptive-text report without translating it.
#[rule("rule_ste_sdk_statement_report")]
fn report(request: &CheckStatementRequest) -> provenance_ste100::Report {
    provenance_ste100::check_descriptive(&request.statement)
}

/// Runs the statement preflight without repository discovery or state access.
#[rule("rule_ste_sdk_statement_repository_independence")]
pub(super) fn handle(format: OutputFormat) -> anyhow::Result<()> {
    let request = read_request()?;
    output::print(format, &report(&request))
}
