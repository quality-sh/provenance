#[cfg(test)]
mod tests {
    use super::build_url;

    #[test]
    fn link_builder_encodes_record_ids_and_never_adds_a_credential() {
        let url = build_url(
            "http://127.0.0.1:1234/",
            "req root",
            Some("rule/focus"),
        )
        .unwrap();
        assert_eq!(
            url,
            "http://127.0.0.1:1234/?root=req+root&focus=rule%2Ffocus"
        );
        assert!(!url.contains("bearer"));
    }
}
