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

/// Runs the statement preflight without repository discovery or state access.
#[rule("rule_ste_sdk_statement_repository_independence")]
pub(super) async fn handle(format: OutputFormat) -> anyhow::Result<()> {
    let request = read_request()?;
    let report = provenance_store::operations::catalog::invoke_typed::<
        provenance_store::operations::catalog::CheckStatement,
    >(
        provenance_store::operations::catalog::PreparedContext::data_free(),
        request,
    )
    .await?;
    output::print(format, &report)
}
