/// Omits terminal records by default and includes them only on request.
#[provenance_macros::rule("rule_review_defaults_exclude_terminal_records")]
#[provenance_macros::rule("rule_cli_terminal_records_opt_in")]
pub const fn exclude_terminal(include_terminal: bool) -> bool {
    !include_terminal
}
