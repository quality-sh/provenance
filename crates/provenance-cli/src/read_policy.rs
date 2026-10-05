use provenance_store::operations::catalog;

/// Omits terminal records from review reads by default. The `stale` and
/// `resolve-symbol` queries keep terminal Rules because their code bindings
/// remain necessary binding and symbol-resolution information.
#[provenance_macros::rule("rule_review_defaults_exclude_terminal_records")]
pub fn default_exclude_terminal(
    query_action: Option<&str>,
    route_parameters: &[catalog::Parameter],
) -> Option<bool> {
    let supports_filter = route_parameters
        .iter()
        .any(|parameter| parameter.location == "query" && parameter.name == "exclude_terminal");
    if !supports_filter || matches!(query_action, Some("stale" | "resolve-symbol")) {
        return None;
    }
    Some(true)
}

/// Includes terminal records only when the user requests them.
#[provenance_macros::rule("rule_cli_terminal_records_opt_in")]
pub const fn exclude_terminal(include_terminal: bool) -> bool {
    !include_terminal
}

#[cfg(test)]
mod tests {
    use provenance_store::operations::catalog;

    use super::default_exclude_terminal;

    fn route_parameters() -> Vec<catalog::Parameter> {
        let definition = catalog::definitions()
            .iter()
            .find(|definition| definition.path == "/rules")
            .unwrap();
        definition.parameters().to_vec()
    }

    #[test]
    fn review_reads_default_to_excluding_terminal_records() {
        let parameters = route_parameters();

        assert_eq!(default_exclude_terminal(None, &parameters), Some(true));
        assert_eq!(
            default_exclude_terminal(Some("search"), &parameters),
            Some(true)
        );
    }

    #[test]
    fn code_binding_queries_do_not_get_a_terminal_default() {
        let parameters = route_parameters();

        assert_eq!(default_exclude_terminal(Some("stale"), &parameters), None);
        assert_eq!(
            default_exclude_terminal(Some("resolve-symbol"), &parameters),
            None
        );
    }

    #[test]
    fn routes_without_a_terminal_parameter_do_not_get_a_default() {
        assert_eq!(default_exclude_terminal(None, &[]), None);
    }
}
