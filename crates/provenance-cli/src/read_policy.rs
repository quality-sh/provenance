/// Omits terminal records unless a person explicitly includes them.
#[provenance_macros::rule("rule_cli_terminal_records_opt_in")]
pub const fn exclude_terminal(include_terminal: bool) -> bool {
    !include_terminal
}
