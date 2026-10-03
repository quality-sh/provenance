#[cfg(test)]
mod tests {
    #[test]
    fn terminal_visibility_defaults_to_current_work_and_accepts_an_opt_in() {
        assert!(super::exclude_terminal(false));
        assert!(!super::exclude_terminal(true));
    }
}
