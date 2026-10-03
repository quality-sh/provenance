/// Omits terminal records unless a person explicitly includes them.
#[provenance_macros::rule("rule_review_defaults_exclude_terminal_records")]
pub const fn exclude_terminal(include_terminal: bool) -> bool {
    !include_terminal
}

#[cfg(test)]
mod tests {
    #[test]
    fn terminal_visibility_defaults_to_current_work_and_accepts_an_opt_in() {
        assert!(super::exclude_terminal(false));
        assert!(!super::exclude_terminal(true));
    }
}
