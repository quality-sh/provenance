use provenance_store::operations::catalog;

/// Omits terminal records when the selected read accepts the terminal filter.
#[provenance_macros::rule("rule_review_defaults_exclude_terminal_records")]
pub fn default_exclude_terminal(parameters: &[catalog::Parameter]) -> Option<bool> {
    let supports_filter = parameters
        .iter()
        .any(|parameter| parameter.location == "query" && parameter.name == "exclude_terminal");
    supports_filter.then_some(true)
}

/// Includes terminal records only when the user requests them.
#[provenance_macros::rule("rule_cli_terminal_records_opt_in")]
pub const fn exclude_terminal(include_terminal: bool) -> bool {
    !include_terminal
}
